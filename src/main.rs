#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        asm!("\
        mov dword ptr [0xb8000], 0x2f402f40
        hlt
        " : : : : "intel");
    }
    let vga_buffer = 0xb8000 as *mut u8;
    let hello: &[u8] = b"Hello world!";
    for (i, &byte) in hello.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0x0b;
        }
    }
    loop {}
}


