#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]

extern crate x86_64;
extern crate pc_keyboard;
extern crate multiboot2;

pub mod vga_buffer;
pub mod serial_port;
pub mod interrupts;
pub mod port;
pub mod mem;
pub mod frame_alloc;
mod gdt;

use vga_buffer::cls;
use interrupts::setup_idt;
use gdt::init_gdt;

use crate::vga_buffer::set_color;
use crate::vga_buffer::Color;
use crate::port::init_pics;

#[cfg(not(feature = "no-panic-handler"))]
use core::panic::PanicInfo;
use multiboot2::BootInformation;
use crate::frame_alloc::FrameSingleAllocator;

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
    let boot_info = unsafe{
        multiboot2::load(mem::PhysAddr::new(multiboot_info_addr).to_virt().unwrap().addr() as usize)
    };
    start(boot_info);
}

pub fn start(boot_info: &'static BootInformation) -> ! {
    cls();
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
        println!("addr 0x172d05e00 is: {}", mem::VirtAddr::new(0x172d05e00).to_phys().unwrap().0);
    }
    println!("kernel end at : {:x}", boot_info.end_address());
    unsafe {
        let mut alloc = frame_alloc::SimpleAllocator::new(&boot_info);
        let p = alloc.allocate().unwrap();
        println!("GOT PAGE!!! {}", p);

        let virt = mem::VirtAddr::new(0xB0000000);
        mem::get_page_table().map_virt_to_phys(virt, mem::PhysAddr::new(0xb8000), mem::BIT_WRITABLE | mem::BIT_PRESENT, &mut alloc);
        let cptr: &mut [u8; 100] = virt.to_ref();
        cptr[0] = 'Z' as u8;
        cptr[1] = 15u8;

        let virt = mem::VirtAddr::new(0x60000000);
        mem::get_page_table().map_virt_to_phys(virt, mem::PhysAddr::new(0), mem::BIT_WRITABLE | mem::BIT_PRESENT | mem::BIT_HUGE, &mut alloc);
        let cptr: &mut [u8; 1000000000] = virt.to_ref();
        cptr[0xb8004] = 'Y' as u8;
        cptr[0xb8005] = 15u8;
    }
    set_color(Color::Green, Color::Black, false);
    println!("I'M STILL ALIVE!!!");
    loop {
        x86_64::instructions::hlt();
    }
}
