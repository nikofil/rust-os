#![no_std]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![allow(static_mut_refs)]

extern crate alloc;
extern crate multiboot2;
extern crate pc_keyboard;
extern crate x86_64;

pub mod buddy_alloc;
pub mod frame_alloc;
mod gdt;
pub mod global_alloc;
pub mod interrupts;
pub mod mem;
pub mod port;
pub mod scheduler;
pub mod serial_port;
pub mod syscalls;
pub mod vga_buffer;
pub mod fat16;
pub mod elf;

use core::arch::asm;
use gdt::init_gdt;
use interrupts::setup_idt;
use vga_buffer::cls;

use crate::port::init_pics;
use crate::vga_buffer::set_color;
use crate::vga_buffer::Color;
use crate::elf::Elf;

#[cfg(not(feature = "no-panic-handler"))]
use core::panic::PanicInfo;
use multiboot2::{BootInformationHeader, BootInformation};

#[global_allocator]
static ALLOCATOR: global_alloc::Allocator = global_alloc::Allocator;

static mut BOOT_INFO: Option<BootInformation> = None;

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

#[no_mangle]
pub extern "C" fn ua64_mode_start() -> ! {
    let mut multiboot_info_addr: usize;
    unsafe {
        asm!("\
            mov ax, 0
            mov ss, ax
            mov ds, ax
            mov es, ax
            mov fs, ax
            mov gs, ax
        ", out("rdi") multiboot_info_addr);
    }
    let boot_info = unsafe {
        BOOT_INFO = Some(BootInformation::load(
            mem::PhysAddr::new(multiboot_info_addr)
                .to_virt()
                .unwrap()
                .addr() as *const BootInformationHeader,
        ).unwrap_unchecked());
        BOOT_INFO.as_ref().unwrap()
    };
    start(boot_info);
}

pub fn start(boot_info: &'static BootInformation) -> ! {
    cls();
    init_gdt();
    setup_idt();
    unsafe {
        syscalls::init_syscalls();
    }
    unsafe {
        let pt = mem::get_page_table();
        println!("Page table: {:p}", pt);
        let entry0 = pt.get_entry(0);
        println!("Entry 0: {}", entry0);
        let entry03 = entry0.next_pt().get_entry(3);
        println!("Entry 0-3: {}", entry03);
        let entry032 = entry03.next_pt().get_entry(2);
        println!("Entry 0-3-2: {}", entry032);
        println!(
            "addr 0x172d05e00 is: {}",
            mem::VirtAddr::new(0x172d05e00).to_phys().unwrap().0
        );
    }
    println!("Kernel end at: {:x}", boot_info.end_address());
    unsafe {
        frame_alloc::SimpleAllocator::init(boot_info);
        global_alloc::init_global_alloc(frame_alloc::BOOTINFO_ALLOCATOR.as_mut().unwrap());
    }
    set_color(Color::Green, Color::Black, false);
    init_pics();

    let main = fat16::load_main().unwrap(); // load the /BOOT main program from fat16

    let elf = Elf::new(main); // parse the file as an elf to find loadable sections

    let sched = &scheduler::SCHEDULER;
    sched.schedule_task(elf.into()); // transform to a task and schedule it
    loop {} // no need to do anything here as we will be interrupted anyway
}
