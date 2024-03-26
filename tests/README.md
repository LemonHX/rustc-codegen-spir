Test `test-shader.rs`:

```
rustc tests/test-shader.rs --crate-name test_shader --crate-type lib -o target/test-shader -Z codegen-backend=target/debug/rustc_codegen_spir.dll -Z unstable-options

rustc ./tests/test-shader.rs -Z codegen-backend="../target/debug/rustc_codegen_spir.dll" -Z unstable-options -C target-feature="+Int8,+Int16,+Int64,+Float64,+ShaderClockKHR,+ext:SPV_KHR_shader_clock" --crate-type lib --target spirv-unknown-vulkan1.1 
```
