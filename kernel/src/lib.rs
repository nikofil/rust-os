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

pub mod buddy_alloc;
pub mod frame_alloc;
mod gdt;
pub mod global_alloc;
pub mod interrupts;
pub mod mem;
pub mod port;
pub mod serial_port;
pub mod vga_buffer;
pub mod scheduler;
pub mod syscalls;

use gdt::init_gdt;
use interrupts::setup_idt;
use vga_buffer::cls;

use crate::port::init_pics;
use crate::vga_buffer::set_color;
use crate::vga_buffer::Color;

use alloc::boxed::Box;
use alloc::vec::Vec;
#[cfg(not(feature = "no-panic-handler"))]
use core::panic::PanicInfo;
use multiboot2::BootInformation;

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
    init_pics();
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
        let alloc = frame_alloc::SimpleAllocator::new(&boot_info);
        frame_alloc::BOOTINFO_ALLOCATOR.replace(alloc);
        global_alloc::init_global_alloc(frame_alloc::BOOTINFO_ALLOCATOR.as_mut().unwrap());
        let x = Box::new(Vec::with_capacity(1000000000) as Vec<u8>);
        println!("BOX IS HERE: {:?}", x);
        drop(x);
        println!("BOX IS NOT HERE ANYMORE :<");
    }
    set_color(Color::Green, Color::Black, false);
    unsafe {
        let userspace_fn_in_kernel = mem::VirtAddr::new(scheduler::userspace_func as *const () as u64);
        let userspace_fn_phys = userspace_fn_in_kernel.to_phys().unwrap().0;
        let page_phys_start = (userspace_fn_phys.addr() >> 12) << 12; // zero out page offset to get which page we should map
        let fn_page_offset = userspace_fn_phys.addr() - page_phys_start;
        let userspace_fn_virt_base = 0x400000;
        let userspace_fn_virt = userspace_fn_virt_base + fn_page_offset;
        println!("Mapping {:x} to {:x}", page_phys_start, userspace_fn_virt_base);
        mem::get_page_table().map_virt_to_phys(
            mem::VirtAddr::new(userspace_fn_virt_base ),
            mem::PhysAddr::new(page_phys_start),
            mem::BIT_PRESENT | mem::BIT_USER);
        let stack_space = mem::VirtAddr::new(Vec::with_capacity(0x1000).as_mut_ptr() as *const u8 as u64).to_phys().unwrap().0;
        mem::get_page_table().map_virt_to_phys(
            mem::VirtAddr::new(0x800000),
            stack_space,
            mem::BIT_PRESENT | mem::BIT_WRITABLE | mem::BIT_USER);
        println!("Dropping to usermode");
        scheduler::jmp_to_usermode(userspace_fn_virt, 0x800000 + 0x1000);
        drop(stack_space); // prevent Rust from freeing this early
    }
    loop {
        x86_64::instructions::hlt();
    }
}
