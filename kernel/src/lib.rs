#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;
extern crate multiboot2;
extern crate pc_keyboard;
extern crate x86_64;
extern crate if_chain;

pub mod frame_alloc;
mod gdt;
mod global_alloc;
pub mod interrupts;
pub mod mem;
pub mod port;
pub mod serial_port;
pub mod vga_buffer;

use gdt::init_gdt;
use interrupts::setup_idt;
use vga_buffer::cls;

use crate::port::init_pics;
use crate::vga_buffer::set_color;
use crate::vga_buffer::Color;

use alloc::boxed::Box;
#[cfg(not(feature = "no-panic-handler"))]
use core::panic::PanicInfo;
use multiboot2::BootInformation;
use alloc::vec::Vec;

#[global_allocator]
static ALLOCATOR: global_alloc::Allocator = global_alloc::Allocator;

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

/// This function is called on panic.
#[cfg(not(feature = "no-panic-handler"))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[naked]
#[no_mangle]
pub extern "C" fn ua64_mode_start(multiboot_info_addr: u64) -> ! {
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
    let boot_info = unsafe {
        multiboot2::load(
            mem::PhysAddr::new(multiboot_info_addr)
                .to_virt()
                .unwrap()
                .addr() as usize,
        )
    };
    start(boot_info);
}

pub fn start(boot_info: &'static BootInformation) -> ! {
    cls();
    init_gdt();
    setup_idt();
    // init_pics();
    unsafe {
        let alloc = frame_alloc::SimpleAllocator::new(&boot_info);
        frame_alloc::BOOTINFO_ALLOCATOR.replace(alloc);
        global_alloc::init_global_alloc(frame_alloc::BOOTINFO_ALLOCATOR.as_mut().unwrap());
        {
            println!("Before first Vec");
            let mut x: Vec<u8> = Vec::new();
            x.push(123);
            println!("{}", x[0]);
        }
        {
            println!("Before second Vec");
            let mut x: Vec<u8> = Vec::new();
            x.push(123);
            println!("{}", x[0]);
        }
        println!("After second Vec");
    }
    set_color(Color::Green, Color::Black, false);
    loop {
        x86_64::instructions::hlt();
    }
}
