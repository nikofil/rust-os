#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]

pub mod vga_buffer;

use vga_buffer::cls;

use core::panic::PanicInfo;
use crate::vga_buffer::set_color;
use crate::vga_buffer::Color;

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[naked]
#[no_mangle]
pub extern "C" fn ua64_mode_start() -> ! {
    unsafe {
        asm!("\
            mov ax, 0
            mov ss, ax
            mov ds, ax
            mov es, ax
            mov fs, ax
            mov gs, ax
        " :::: "intel");
    }
    cls();
    set_color(Color::Red, Color::Black, false);
    println!("IT'S ALIVE!!!");
    set_color(Color::LightGreen, Color::Black, false);
    println!("Hello world!");
    loop {}
}
