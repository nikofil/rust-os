#![no_std]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

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
mod userspace;
pub mod vga_buffer;

use core::arch::asm;
use gdt::init_gdt;
use interrupts::setup_idt;
use vga_buffer::cls;

use crate::port::init_pics;
use crate::vga_buffer::set_color;
use crate::vga_buffer::Color;

#[cfg(not(feature = "no-panic-handler"))]
use core::panic::PanicInfo;
use multiboot2::{BootInformationHeader, BootInformation};

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

#[no_mangle]
pub extern "C" fn ua64_mode_start() -> ! {
    let mut multiboot_info_addr: u64;
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
        BootInformation::load(
            mem::PhysAddr::new(multiboot_info_addr)
                .to_virt()
                .unwrap()
                .addr() as *const BootInformationHeader,
        ).unwrap_unchecked()
    };
    start(boot_info);
}

pub fn start(boot_info: BootInformation) -> ! {
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
    let userspace_fn_1_in_kernel =
        mem::VirtAddr::new(userspace::userspace_prog_1 as *const () as u64);
    let userspace_fn_2_in_kernel =
        mem::VirtAddr::new(userspace::userspace_prog_2 as *const () as u64);
    let userspace_fn_hello_in_kernel =
        mem::VirtAddr::new(userspace::userspace_prog_hello as *const () as u64);
    unsafe {
        let sched = &scheduler::SCHEDULER;
        sched.schedule(userspace_fn_1_in_kernel);
        sched.schedule(userspace_fn_2_in_kernel);
        sched.schedule(userspace_fn_hello_in_kernel);
        loop {
            sched.run_next();
        }
    }
}
