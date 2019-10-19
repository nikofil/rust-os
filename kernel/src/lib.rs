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
pub mod mem;
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
    unsafe {
        let pt = mem::get_page_table();
        println!("Page table: {:p}", pt);
        let entry0 = pt.get_entry(0);
        println!("Entry 0: {}", entry0);
        let entry03 = entry0.next_pt().get_entry(3);
        println!("Entry 0-3: {}", entry03);
        let entry032 = entry03.next_pt().get_entry(2);
        println!("Entry 0-3-2: {}", entry032);
        println!("addr 0x172d05e00 is: {:x}", mem::virt_to_phys(0x172d05e00).unwrap().0);
    }
    set_color(Color::Green, Color::Black, false);
    println!("I'M STILL ALIVE!!!");
    loop {
        x86_64::instructions::hlt();
    }
}
