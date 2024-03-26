use rspirv::spirv::MemoryModel;
use rustc_target::spec::{Cc, LinkerFlavor, PanicStrategy, Target, TargetMetadata, TargetOptions};
use spirv_tools::TargetEnv;
const ARCH: &str = "spirv";

pub struct SpirvTarget {
    env: TargetEnv,
    vendor: String,
}

impl SpirvTarget {
    pub fn memory_model(&self) -> MemoryModel {
        match self.env {
            TargetEnv::Universal_1_0
            | TargetEnv::Universal_1_1
            | TargetEnv::Universal_1_2
            | TargetEnv::Universal_1_3
            | TargetEnv::Universal_1_4
            | TargetEnv::Universal_1_5 => MemoryModel::Simple,

            TargetEnv::OpenGL_4_0
            | TargetEnv::OpenGL_4_1
            | TargetEnv::OpenGL_4_2
            | TargetEnv::OpenGL_4_3
            | TargetEnv::OpenGL_4_5 => MemoryModel::GLSL450,

            TargetEnv::OpenCL_2_1
            | TargetEnv::OpenCL_2_2
            | TargetEnv::OpenCL_1_2
            | TargetEnv::OpenCLEmbedded_1_2
            | TargetEnv::OpenCL_2_0
            | TargetEnv::OpenCLEmbedded_2_0
            | TargetEnv::OpenCLEmbedded_2_1
            | TargetEnv::OpenCLEmbedded_2_2 => MemoryModel::OpenCL,

            TargetEnv::Vulkan_1_0
            | TargetEnv::Vulkan_1_1
            | TargetEnv::WebGPU_0
            | TargetEnv::Vulkan_1_1_Spirv_1_4
            | TargetEnv::Vulkan_1_2 => MemoryModel::Vulkan,
        }
    }

    pub fn spirv_version(&self) -> (u8, u8) {
        #[allow(clippy::match_same_arms)]
        match self.env {
            TargetEnv::Universal_1_0 => (1, 0),
            TargetEnv::Universal_1_1 => (1, 1),
            TargetEnv::Universal_1_2 => (1, 2),
            TargetEnv::Universal_1_3 => (1, 3),
            TargetEnv::Universal_1_4 => (1, 4),
            TargetEnv::Universal_1_5 => (1, 5),

            TargetEnv::OpenGL_4_0 => (1, 0),
            TargetEnv::OpenGL_4_1 => (1, 0),
            TargetEnv::OpenGL_4_2 => (1, 0),
            TargetEnv::OpenGL_4_3 => (1, 0),
            TargetEnv::OpenGL_4_5 => (1, 0),

            TargetEnv::OpenCL_1_2 => (1, 0),
            TargetEnv::OpenCL_2_0 => (1, 0),
            TargetEnv::OpenCL_2_1 => (1, 0),
            TargetEnv::OpenCL_2_2 => (1, 2),
            TargetEnv::OpenCLEmbedded_1_2 => (1, 0),
            TargetEnv::OpenCLEmbedded_2_0 => (1, 0),
            TargetEnv::OpenCLEmbedded_2_1 => (1, 0),
            TargetEnv::OpenCLEmbedded_2_2 => (1, 2),

            TargetEnv::Vulkan_1_0 => (1, 0),
            TargetEnv::Vulkan_1_1 => (1, 3),
            TargetEnv::WebGPU_0 => (1, 3),
            TargetEnv::Vulkan_1_1_Spirv_1_4 => (1, 4),
            TargetEnv::Vulkan_1_2 => (1, 5),
        }
    }

    fn init_target_opts(&self) -> TargetOptions {
        let mut o = TargetOptions::default();
        o.simd_types_indirect = false;
        o.allows_weak_linkage = false;
        o.crt_static_allows_dylibs = true;
        o.dll_prefix = "".into();
        o.dll_suffix = ".spv.json".into();
        o.dynamic_linking = true;
        o.emit_debug_gdb_scripts = false;
        o.linker_flavor = LinkerFlavor::Unix(Cc::No);
        o.panic_strategy = PanicStrategy::Abort;
        o.os = "unknown".into();
        o.env = self.env.to_string().into();
        o.vendor = self.vendor.clone().into();
        // TODO: Investigate if main_needs_argc_argv is useful (for building exes)
        o.main_needs_argc_argv = false;
        o
    }

    pub fn rustc_target(&self) -> Target {
        Target {
            llvm_target: self.to_string().into(),
            pointer_width: 32,
            data_layout: "e-m:e-p:32:32:32-i64:64-n8:16:32:64".into(),
            arch: ARCH.into(),
            options: self.init_target_opts(),
            metadata: TargetMetadata::default(),
        }
    }
}

impl std::str::FromStr for SpirvTarget {
    type Err = InvalidTarget;

    fn from_str(target: &str) -> Result<Self, Self::Err> {
        let iter = target.split('-').collect::<Vec<&str>>();
        let error = || InvalidTarget(target.into());

        if iter[0] != ARCH {
            return Err(error());
        }

        let vendor = iter[1].to_string();

        let env = TargetEnv::from_str(iter[2]).unwrap();

        let result = Self { env, vendor };
        Ok(result)
    }
}

impl std::fmt::Display for SpirvTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", ARCH, self.vendor, self.env)
    }
}

pub(crate) const ALL_VALID_TARGETS: [&'static str; 24] = [
    "spirv-unknown-spv1.0",
    "spirv-unknown-spv1.1",
    "spirv-unknown-spv1.2",
    "spirv-unknown-spv1.3",
    "spirv-unknown-spv1.4",
    "spirv-unknown-spv1.5",
    "spirv-unknown-vulkan1.0",
    "spirv-unknown-vulkan1.1",
    "spirv-unknown-vulkan1.1spv1.4",
    "spirv-unknown-vulkan1.2",
    "spirv-unknown-webgpu0",
    "spirv-unknown-opengl4.0",
    "spirv-unknown-opengl4.1",
    "spirv-unknown-opengl4.2",
    "spirv-unknown-opengl4.3",
    "spirv-unknown-opengl4.5",
    "spirv-unknown-opencl1.2",
    "spirv-unknown-opencl1.2embedded",
    "spirv-unknown-opencl2.0",
    "spirv-unknown-opencl2.0embedded",
    "spirv-unknown-opencl2.1",
    "spirv-unknown-opencl2.1embedded",
    "spirv-unknown-opencl2.2",
    "spirv-unknown-opencl2.2embedded",
];

#[derive(Debug)]
pub struct InvalidTarget(String);

impl std::error::Error for InvalidTarget {}
impl std::fmt::Display for InvalidTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid target `{}`.\n Valid targets: {:#?}",
            self.0, ALL_VALID_TARGETS
        )
    }
}

#[cfg(test)]
mod register_targets {
    use std::{fs::File, io::Write, path::Path, str::FromStr};

    use rustc_target::json::ToJson;

    use crate::target::SpirvTarget;

    use super::ALL_VALID_TARGETS;

    #[test]
    fn write_target_json() {
        // generate target json file if it is not exist
        let target_path = Path::new("./target");
        ALL_VALID_TARGETS.iter().for_each(|target| {
            let target_json = target_path.join(format!("{}.json", target));
            if let Ok(mut file) = File::create_new(target_json.clone()) {
                file.write_all(
                    SpirvTarget::from_str(target)
                        .unwrap()
                        .rustc_target()
                        .to_json()
                        .to_string()
                        .as_bytes(),
                )
                .unwrap();
                println!("Generating target json file: {:?}", target_json);
            }
        });
    }
}
