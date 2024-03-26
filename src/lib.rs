#![feature(rustc_private)]
#![feature(assert_matches)]
#![feature(lint_reasons)]
#![feature(lazy_cell)]

extern crate rustc_apfloat;
extern crate rustc_arena;
extern crate rustc_ast;
extern crate rustc_attr;
extern crate rustc_codegen_ssa;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_mir_transform;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;

mod abi;
mod attr;
mod builder;
mod builder_spirv;
mod codegen_cx;
mod custom_decorations;
mod custom_insts;
mod link;
mod linker;
mod rustc_codegen_spirv_types;
mod spirv_type;
mod spirv_type_constraints;
mod symbols;
mod target;
mod target_feature;

use core::any::Any;
use std::{
    fs::{create_dir_all, File}, io::{Cursor, Write}, path::Path, str::FromStr, sync::Arc
};

use builder::Builder;
use codegen_cx::CodegenCx;
use rspirv::binary::Assemble;
use rustc_ast::expand::allocator::AllocatorKind;
use rustc_codegen_ssa::{
    back::{
        lto::{LtoModuleCodegen, SerializedModule, ThinModule},
        write::{
            CodegenContext, FatLtoInput, ModuleConfig, OngoingCodegen, TargetMachineFactoryConfig,
        },
    }, base::maybe_create_entry_wrapper, mono_item::MonoItemExt, traits::{
        CodegenBackend, ExtraBackendMethods, ModuleBufferMethods, ThinBufferMethods,
        WriteBackendMethods,
    }, CodegenResults, CompiledModule, CrateInfo, ModuleCodegen, ModuleKind
};
use rustc_data_structures::fx::FxIndexMap;
use rustc_errors::{DiagCtxt, FatalError};
use rustc_metadata::EncodedMetadata;
use rustc_middle::{
    dep_graph::{WorkProduct, WorkProductId},
    mir::{
        mono::{MonoItem, MonoItemData},
        write_mir_pretty,
    },
    ty::{self, print::with_no_trimmed_paths, Instance, InstanceDef, TyCtxt},
};
use rustc_session::{
    config::{self, OutputFilenames, OutputType},
    Session,
};
use rustc_span::{sym, ErrorGuaranteed, Symbol};
use rustc_target::{json::ToJson, spec::{Target, TargetTriple}};
use target::{SpirvTarget, ALL_VALID_TARGETS};

fn dump_mir(tcx: TyCtxt<'_>, mono_items: &[(MonoItem<'_>, MonoItemData)], path: &Path) {
    create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = File::create(path).unwrap();
    for &(mono_item, _) in mono_items {
        if let MonoItem::Fn(instance) = mono_item {
            if matches!(instance.def, InstanceDef::Item(_)) {
                let mut mir = Cursor::new(Vec::new());
                if write_mir_pretty(tcx, Some(instance.def_id()), &mut mir).is_ok() {
                    file.write(format!("{}\n", String::from_utf8(mir.into_inner()).unwrap()).into_bytes().as_ref()).unwrap();
                }
            }
        }
    }
}

// TODO: Should this store Vec or Module?
pub struct SpirvModuleBuffer(Vec<u32>);

impl ModuleBufferMethods for SpirvModuleBuffer {
    fn data(&self) -> &[u8] {
        spirv_tools::binary::from_binary(&self.0)
    }
}

// TODO: Should this store Vec or Module?
pub struct SpirvThinBuffer(Vec<u32>);

impl ThinBufferMethods for SpirvThinBuffer {
    fn data(&self) -> &[u8] {
        spirv_tools::binary::from_binary(&self.0)
    }
}

#[macro_export]
macro_rules! assert_ty_eq {
    ($codegen_cx:expr, $left:expr, $right:expr) => {
        assert!(
            $left == $right,
            "Expected types to be equal:\n{}\n==\n{}",
            $codegen_cx.debug_type($left),
            $codegen_cx.debug_type($right)
        )
    };
}

fn is_blocklisted_fn<'tcx>(
    tcx: TyCtxt<'tcx>,
    sym: &symbols::Symbols,
    instance: Instance<'tcx>,
) -> bool {
    // TODO: These sometimes have a constant value of an enum variant with a hole
    if let InstanceDef::Item(def_id) = instance.def {
        if let Some(debug_trait_def_id) = tcx.get_diagnostic_item(sym::Debug) {
            // Helper for detecting `<_ as core::fmt::Debug>::fmt` (in impls).
            let is_debug_fmt_method = |def_id| match tcx.opt_associated_item(def_id) {
                Some(assoc) if assoc.ident(tcx).name == sym::fmt => match assoc.container {
                    ty::ImplContainer => {
                        let impl_def_id = assoc.container_id(tcx);
                        tcx.impl_trait_ref(impl_def_id)
                            .map(|tr| tr.skip_binder().def_id)
                            == Some(debug_trait_def_id)
                    }
                    ty::TraitContainer => false,
                },
                _ => false,
            };

            if is_debug_fmt_method(def_id) {
                return true;
            }

            if tcx.opt_item_ident(def_id).map(|i| i.name) == Some(sym.fmt_decimal) {
                if let Some(parent_def_id) = tcx.opt_parent(def_id) {
                    if is_debug_fmt_method(parent_def_id) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

struct DumpModuleOnPanic<'a, 'cx, 'tcx> {
    cx: &'cx CodegenCx<'tcx>,
    path: &'a Path,
}

impl Drop for DumpModuleOnPanic<'_, '_, '_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if self.path.has_root() {
                self.cx.builder.dump_module(self.path);
            } else {
                println!("{}", self.cx.builder.dump_module_str());
            }
        }
    }
}

#[derive(Clone)]
pub struct SpirCodegenBackend;


impl WriteBackendMethods for SpirCodegenBackend {
    type Module = Vec<u32>;
    type ModuleBuffer = SpirvModuleBuffer;
    type TargetMachine = ();
    type TargetMachineError = String;
    type ThinBuffer = SpirvThinBuffer;
    type ThinData = ();

    fn run_link(
        _cgcx: &CodegenContext<Self>,
        _diag_handler: &DiagCtxt,
        _modules: Vec<ModuleCodegen<Self::Module>>,
    ) -> Result<ModuleCodegen<Self::Module>, FatalError> {
        todo!()
    }

    fn run_fat_lto(
        _: &CodegenContext<Self>,
        _: Vec<FatLtoInput<Self>>,
        _: Vec<(SerializedModule<Self::ModuleBuffer>, WorkProduct)>,
    ) -> Result<LtoModuleCodegen<Self>, FatalError> {
        todo!()
    }

    fn run_thin_lto(
        cgcx: &CodegenContext<Self>,
        modules: Vec<(String, Self::ThinBuffer)>,
        cached_modules: Vec<(SerializedModule<Self::ModuleBuffer>, WorkProduct)>,
    ) -> Result<(Vec<LtoModuleCodegen<Self>>, Vec<WorkProduct>), FatalError> {
        link::run_thin(cgcx, modules, cached_modules)
    }

    fn print_pass_timings(&self) {
        println!("TODO: Implement print_pass_timings");
    }

    fn print_statistics(&self) {
        println!("TODO: Implement print_statistics");
    }

    unsafe fn optimize(
        _: &CodegenContext<Self>,
        _: &DiagCtxt,
        _: &ModuleCodegen<Self::Module>,
        _: &ModuleConfig,
    ) -> Result<(), FatalError> {
        // TODO: Implement
        Ok(())
    }

    unsafe fn optimize_thin(
        _cgcx: &CodegenContext<Self>,
        thin_module: ThinModule<Self>,
    ) -> Result<ModuleCodegen<Self::Module>, FatalError> {
        let module = ModuleCodegen {
            module_llvm: spirv_tools::binary::to_binary(thin_module.data())
                .unwrap()
                .to_vec(),
            name: thin_module.name().to_string(),
            kind: ModuleKind::Regular,
        };
        Ok(module)
    }

    fn optimize_fat(
        _: &CodegenContext<Self>,
        _: &mut ModuleCodegen<Self::Module>,
    ) -> Result<(), FatalError> {
        todo!()
    }

    unsafe fn codegen(
        cgcx: &CodegenContext<Self>,
        _diag_handler: &DiagCtxt,
        module: ModuleCodegen<Self::Module>,
        _config: &ModuleConfig,
    ) -> Result<CompiledModule, FatalError> {
        let path = cgcx
            .output_filenames
            .temp_path(OutputType::Object, Some(&module.name));
        // Note: endianness doesn't matter, readers deduce endianness from magic header.
        let spirv_module = spirv_tools::binary::from_binary(&module.module_llvm);
        File::create(&path)
            .unwrap()
            .write_all(spirv_module)
            .unwrap();
        Ok(CompiledModule {
            name: module.name,
            kind: module.kind,
            object: Some(path),
            dwarf_object: None,
            bytecode: None,
        })
    }

    fn prepare_thin(module: ModuleCodegen<Self::Module>) -> (String, Self::ThinBuffer) {
        (module.name, SpirvThinBuffer(module.module_llvm))
    }

    fn serialize_module(module: ModuleCodegen<Self::Module>) -> (String, Self::ModuleBuffer) {
        (module.name, SpirvModuleBuffer(module.module_llvm))
    }
}

impl ExtraBackendMethods for SpirCodegenBackend {
    fn codegen_allocator<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        module_name: &str,
        kind: AllocatorKind,
        alloc_error_handler_kind: AllocatorKind,
    ) -> Self::Module {
        vec![]
    }

    fn compile_codegen_unit(
        &self,
        tcx: TyCtxt<'_>,
        cgu_name: Symbol,
    ) -> (ModuleCodegen<Self::Module>, u64) {
        let _timer = tcx
            .prof
            .verbose_generic_activity_with_arg("codegen_module", cgu_name.to_string());

        // TODO: Do dep_graph stuff
        let cgu = tcx.codegen_unit(cgu_name);

        let cx = CodegenCx::new(tcx, cgu);
        let do_codegen = || {
            let mono_items = cx.codegen_unit.items_in_deterministic_order(cx.tcx);

            if let Some(dir) = &cx.codegen_args.dump_mir {
                dump_mir(tcx, mono_items.as_slice(), &dir.join(cgu_name.to_string()));
            }

            for &(mono_item, mono_item_data) in mono_items.iter() {
                if let MonoItem::Fn(instance) = mono_item {
                    if is_blocklisted_fn(cx.tcx, &cx.sym, instance) {
                        continue;
                    }
                }
                mono_item.predefine::<Builder<'_, '_>>(
                    &cx,
                    mono_item_data.linkage,
                    mono_item_data.visibility,
                );
            }

            // ... and now that we have everything pre-defined, fill out those definitions.
            for &(mono_item, _) in mono_items.iter() {
                if let MonoItem::Fn(instance) = mono_item {
                    if is_blocklisted_fn(cx.tcx, &cx.sym, instance) {
                        continue;
                    }
                }
                mono_item.define::<Builder<'_, '_>>(&cx);
            }

            if let Some(_entry) = maybe_create_entry_wrapper::<Builder<'_, '_>>(&cx) {
                // attributes::sanitize(&cx, SanitizerSet::empty(), entry);
            }
        };
        if let Some(path) = &cx.codegen_args.dump_module_on_panic {
            let module_dumper = DumpModuleOnPanic { cx: &cx, path };
            with_no_trimmed_paths!(do_codegen());
            drop(module_dumper);
        } else {
            with_no_trimmed_paths!(do_codegen());
        }
        let spirv_module = cx.finalize_module().assemble();

        (
            ModuleCodegen {
                name: cgu_name.to_string(),
                module_llvm: spirv_module,
                kind: ModuleKind::Regular,
            },
            0,
        )
    }

    fn target_machine_factory(
        &self,
        _sess: &Session,
        _opt_level: config::OptLevel,
        _target_features: &[String],
    ) -> Arc<(dyn Fn(TargetMachineFactoryConfig) -> Result<(), String> + Send + Sync + 'static)>
    {
        Arc::new(|_| Ok(()))
    }
}

impl CodegenBackend for SpirCodegenBackend {

    fn locale_resource(&self) -> &'static str {
        rustc_errors::DEFAULT_LOCALE_RESOURCE
    }

    fn target_features(&self, sess: &Session, _allow_unstable: bool) -> Vec<Symbol> {
        let cmdline = sess.opts.cg.target_feature.split(',');
        let cfg = sess.target.options.features.split(',');
        cfg.chain(cmdline)
            .filter(|l| l.starts_with('+'))
            .map(|l| &l[1..])
            .filter(|l| !l.is_empty())
            .map(Symbol::intern)
            .collect()
    }

    fn init(&self, sess: &Session) {

    }
//     // #122810
//     fn target_override(&self, opts: &config::Options) -> Option<Target> {
//         match opts.target_triple {
//             TargetTriple::TargetTriple(ref target) => {
//                 println!("providing target {}", target);
//                 Some(target
//                 .parse::<target::SpirvTarget>()
//                 .map(|target| target.rustc_target())
// .unwrap())
//             },
//             TargetTriple::TargetJson { .. } => None,
//         }
//         // panic!("{:#?}", opts.target_triple)
//     }

    fn provide(&self, providers: &mut rustc_middle::util::Providers) {
        // FIXME(eddyb) this is currently only passed back to us, specifically
        // into `target_machine_factory` (which is a noop), but it might make
        // sense to move some of the target feature parsing into here.
        providers.global_backend_features = |_tcx, ()| vec![];

        crate::abi::provide(providers);
        crate::attr::provide(providers);
    }

    fn codegen_crate(
        &self,
        tcx: TyCtxt<'_>,
        metadata: EncodedMetadata,
        need_metadata_module: bool,
    ) -> Box<dyn Any> {
        Box::new(rustc_codegen_ssa::base::codegen_crate(
            Self,
            tcx,
            tcx.sess
                .opts
                .cg
                .target_cpu
                .clone()
                .unwrap_or_else(|| tcx.sess.target.cpu.to_string()),
            metadata,
            need_metadata_module,
        ))
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        sess: &Session,
        _outputs: &OutputFilenames,
    ) -> (CodegenResults, FxIndexMap<WorkProductId, WorkProduct>) {
        let (codegen_results, work_products) = ongoing_codegen
            .downcast::<OngoingCodegen<Self>>()
            .expect("Expected OngoingCodegen, found Box<Any>")
            .join(sess);

        (codegen_results, work_products)
    }

    fn link(
        &self,
        sess: &Session,
        codegen_results: CodegenResults,
        outputs: &OutputFilenames,
    ) -> Result<(), ErrorGuaranteed> {
        let timer = sess.timer("link_crate");
        link::link(
            sess,
            &codegen_results,
            outputs,
            codegen_results.crate_info.local_crate_name.as_str(),
        );
        drop(timer);

        // sess.psess.dcx.compile_status()?;
        Ok(())
    }
}

#[no_mangle]
pub fn __rustc_codegen_backend() -> Box<dyn CodegenBackend> {
    rustc_driver::install_ice_hook(
        "https://github.com/lemonhx/rust-codegen-spir/issues/new",
        |handler| {
            handler.note(concat!(
                "`rust-codegen-spir` version `",
                env!("CARGO_PKG_VERSION"),
                "`"
            ));
        },
    );
    Box::new(SpirCodegenBackend)
}
