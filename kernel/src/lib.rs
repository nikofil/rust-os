#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]

mod vga_buffer;
mod serial_port;
mod interrupts;

use vga_buffer::cls;
use interrupts::setup_idt;

use crate::vga_buffer::set_color;
use crate::vga_buffer::Color;

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
    setup_idt();
    set_color(Color::LightGreen, Color::Black, false);
    println!("Hello world1!");
    // divide_by_zero();
    // x86_64::instructions::interrupts::int3();
    cause_page_fault();
    set_color(Color::Red, Color::Black, false);
    println!("I'M STILL ALIVE!!!");
    loop {}
}

fn divide_by_zero() {
    unsafe {
        asm!("mov rax, 0; mov rdx, 0; div rdx" :::: "volatile", "intel")
    }
}

fn cause_page_fault() {
    unsafe {
        *(0xdeadbeef as *mut u64) = 42;
    };
}
