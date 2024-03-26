#![feature(rustc_private)]

extern crate rustc_codegen_ssa;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use core::any::Any;

use rustc_codegen_ssa::{traits::CodegenBackend, CodegenResults, CrateInfo};
use rustc_data_structures::fx::FxIndexMap;
use rustc_metadata::EncodedMetadata;
use rustc_middle::{
    dep_graph::{WorkProduct, WorkProductId},
    ty::TyCtxt,
};
use rustc_session::{config::OutputFilenames, Session};
use rustc_span::ErrorGuaranteed;

pub struct SpirCodegenBackend;

impl CodegenBackend for SpirCodegenBackend {
    fn locale_resource(&self) -> &'static str {
        ""
    }

    fn codegen_crate<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        metadata: EncodedMetadata,
        _need_metadata_module: bool,
    ) -> Box<dyn Any> {
        Box::new(CodegenResults {
            modules: vec![],
            allocator_module: None,
            metadata_module: None,
            metadata,
            crate_info: CrateInfo::new(tcx, "fake_target_cpu".to_string()),
        })
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        _sess: &Session,
        _outputs: &OutputFilenames,
    ) -> (CodegenResults, FxIndexMap<WorkProductId, WorkProduct>) {
        let codegen_results = ongoing_codegen
            .downcast::<CodegenResults>()
            .expect("in join_codegen: ongoing_codegen is not a CodegenResults");
        (*codegen_results, FxIndexMap::default())
    }

    fn link(
        &self,
        sess: &Session,
        codegen_results: CodegenResults,
        outputs: &OutputFilenames,
    ) -> Result<(), ErrorGuaranteed> {
        // Fake linkage; should be replaced to `link_binary` in rustc_codegen_ssa package instead.
        // Ref: https://github.com/rust-lang/rust/blob/8b9e47c136aeee998effdcae356e134b8de65891/compiler/rustc_codegen_llvm/src/lib.rs#L394
        use std::io::Write;

        use rustc_session::{
            config::{CrateType, OutFileName},
            output::out_filename,
        };
        let crate_name = codegen_results.crate_info.local_crate_name;
        for &crate_type in sess.opts.crate_types.iter() {
            if crate_type != CrateType::Rlib {
                sess.dcx().fatal(format!("Crate type is {:?}", crate_type));
            }
            let output_name = out_filename(sess, crate_type, &outputs, crate_name);
            match output_name {
                OutFileName::Real(ref path) => {
                    let mut out_file = ::std::fs::File::create(path).unwrap();
                    write!(
                        out_file,
                        "This has been \"compiled\" successfully using rustc_codegen_spir."
                    )
                    .unwrap();
                }
                OutFileName::Stdout => {
                    let mut stdout = std::io::stdout();
                    write!(
                        stdout,
                        "This has been \"compiled\" successfully using rustc_codegen_spir."
                    )
                    .unwrap();
                }
            }
        }
        Ok(())
    }
}

#[no_mangle]
pub fn __rustc_codegen_backend() -> Box<dyn CodegenBackend> {
    Box::new(SpirCodegenBackend)
}
