#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]

extern crate x86_64;
extern crate pc_keyboard;

pub mod vga_buffer;
pub mod serial_port;
pub mod interrupts;
pub mod port;
mod gdt;

use vga_buffer::cls;
use interrupts::setup_idt;
use gdt::init_gdt;

use crate::vga_buffer::set_color;
use crate::vga_buffer::Color;
use crate::port::init_pics;

#[cfg(not(feature = "no-panic-handler"))]
use core::panic::PanicInfo;

/// This function is called on panic.
#[cfg(not(feature = "no-panic-handler"))]
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
    start();
}

pub fn start() -> ! {
    cls();
    println!("Hello world1!");
    init_gdt();
    setup_idt();
    init_pics();
    set_color(Color::LightGreen, Color::Black, false);
    set_color(Color::Red, Color::Black, false);
    println!("I'M STILL ALIVE!!!");
    loop {
        x86_64::instructions::hlt();
    }
}
