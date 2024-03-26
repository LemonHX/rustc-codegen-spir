#![no_std]

use core::panic::PanicInfo;

#[cfg_attr(target_arch = "spirv", rust_gpu::spirv(vertex))]
pub fn main(#[cfg_attr(target_arch = "spirv", rust_gpu::spirv(invariant))] var: &mut f32) {
    *var += 1.0;
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
