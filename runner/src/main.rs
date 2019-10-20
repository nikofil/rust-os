#![no_std]
#![no_main]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[allow(unused_imports)]
use rust_os::{println, serial_println};
use rust_os::vga_buffer::{WRITER, cls};
use rust_os::interrupts::setup_idt;
use rust_os::mem;
use rust_os::frame_alloc;
use x86_64::instructions::port::Port;
use core::panic::PanicInfo;
use rust_os::port::init_pics;
use lazy_static::lazy_static;
use bootloader::BootInfo;
use bootloader::bootinfo::MemoryRegionType;
use spin::Mutex;

lazy_static! {
    static ref BOOT_INFO: Mutex<Option<&'static BootInfo>> = Mutex::new(None);
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    {
        BOOT_INFO.lock().replace(boot_info);
    }
    #[cfg(test)]
    test_main();
    loop {}
}

#[test_case]
fn test_vga_out() {
    serial_println!("Testing: VGA output...");
    for i in 0..200 {
        println!("output {}", i);
    }
    let line = WRITER.lock().get_line(1);
    assert_eq!(&line[0..10], "output 199".as_bytes());
    serial_println!("Last output: {} == 'output 199'", core::str::from_utf8(&line[0..10]).unwrap());
    serial_println!("Ok");
}

#[test_case]
fn test_int3() {
    setup_idt();
    cls();
    serial_println!("Testing: INT3 breakpoint exception is handled...");
    x86_64::instructions::interrupts::int3();
    let mut found_int3 = false;
    for i in (1..20).rev() {
        let line = WRITER.lock().get_line(i);
        if line[0] != 0x20 {
            let line_s = core::str::from_utf8(&line).unwrap();
            if line_s.contains("int3") {
                found_int3 = true;
            }
            serial_println!("Exception handler output: {}", line_s);
        }
    }
    assert!(found_int3);
    serial_println!("Found \"int3\" pattern");
    serial_println!("Ok");
}

#[test_case]
fn test_timer() {
    setup_idt();
    cls();
    serial_println!("Testing: Timer IRQ handler writes dots on screen...");
    let line = WRITER.lock().get_line(0);
    assert!(!line.contains(&('.' as u8)));
    serial_println!("Line starts out with no dots...");
    init_pics();
    for _ in 0..1000000 {}
    let line = WRITER.lock().get_line(0);
    assert!(line.contains(&('.' as u8)));
    serial_println!("Line has dots after some time: {}", core::str::from_utf8(&line).unwrap());
    serial_println!("Ok");
}

#[test_case]
fn test_paging_table() {
    cls();
    serial_println!("Testing: Paging table resolution...");
    let phys = mem::PhysAddr::new(0x1000000);
    let virt = unsafe { phys.to_virt().unwrap() };
    serial_println!("Testing {} phys to virt: {}", phys, virt);
    assert_eq!(virt.addr(), 0xC1000000);
    let (phys, pte) = unsafe { virt.to_phys().unwrap() };
    serial_println!("Testing virt {} back to phys: {}", virt, phys);
    assert_eq!(phys.addr(), 0x1000000);
    serial_println!("Testing page table entry attrs: {}", pte);
    assert!(pte.get_bit(mem::BIT_PRESENT));
    assert!(pte.get_bit(mem::BIT_WRITABLE));
    assert!(pte.get_bit(mem::BIT_HUGE));
    serial_println!("Ok");
}

struct DummyFrameAllocator(u64, u64);

impl frame_alloc::FrameSingleAllocator for DummyFrameAllocator {
    unsafe fn allocate(&mut self) -> Option<mem::PhysAddr> {
        if self.0 < self.1 {
            let phys = mem::PhysAddr::new(self.0 * frame_alloc::FRAME_SIZE as u64);
            serial_println!("Allocated frame #{} ({})", self.0, phys);
            self.0 += 1;
            Some(phys)
        } else {
            serial_println!("Could not allocate frame!");
            None
        }
    }
}

#[test_case]
fn test_frame_allocation() {
    cls();
    serial_println!("Testing: Frame allocation and mapping new paging table entries...");

    let frame_range = (*BOOT_INFO.lock().unwrap().memory_map)
        .iter()
        .rev()
        .find(|region| region.region_type == MemoryRegionType::Usable)
        .unwrap();
    let mut allocator = DummyFrameAllocator(frame_range.range.start_frame_number, frame_range.range.end_frame_number);

    let virt = mem::VirtAddr::new(0xB0000000);
    let phys = mem::PhysAddr::new(0xb8000);
    unsafe {
        serial_println!("Mapping physical frame {} to virtual {}", phys, virt);
        let pte = mem::get_page_table()
            .map_virt_to_phys(virt,
            phys,
            mem::BIT_WRITABLE | mem::BIT_PRESENT,
            &mut allocator);
        serial_println!("Mapping written in PT entry at addr {:p}: {}", pte, pte);
        serial_println!("Writing 'X' to virtual {}", virt);
        let cptr: &mut [u8; 100] = virt.to_ref();
        cptr[0] = 'X' as u8;
        cptr[1] = 15u8;
    }
    let line = WRITER.lock().get_line(19);
    assert_eq!(line[0], 'X' as u8);
    serial_println!("VGA buffer: {}", core::str::from_utf8(&line[0..10]).unwrap());

    serial_println!("Ok");
}
