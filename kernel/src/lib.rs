#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]

pub mod vga_buffer;
pub mod serial_port;
pub mod interrupts;

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
#[allow(const_err)]
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
    setup_idt();
    set_color(Color::Red, Color::Black, false);
    println!("IT'S ALIVE!!!");
    set_color(Color::LightGreen, Color::Black, false);
    println!("Hello world!");
    cause_page_fault();
    loop {}
}

fn divide_by_zero() {
    unsafe {
        asm!("mov dx, 0; div dx" ::: "ax", "dx" : "volatile", "intel")
    }
}

fn cause_page_fault() {
    let x = [1,2,3,4,5,6,7,8,9];
    unsafe{ *(0xdeadbeaf as *mut u64) = x[4] };
}
