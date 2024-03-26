#![no_std]
// no code inside.

// after compilation, check path target/test-shader. It should include :
// 'This has been "compiled" successfully using rustc_codegen_spir.'

use core::panic::PanicInfo;

pub fn sum(a : i32, b:i32) -> i32 {
    a + b
}

#[panic_handler]
fn panic(_: & PanicInfo) -> ! {
    loop {}
}