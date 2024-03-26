Test `test-shader.rs`:

```
rustc tests/test-shader.rs --crate-name test_shader --crate-type lib -o target/test-shader -Z codegen-backend=target/debug/rustc_codegen_spir.dll -Z unstable-options
```
