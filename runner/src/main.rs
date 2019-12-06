#![no_std]
#![no_main]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader::bootinfo::MemoryRegionType;
use bootloader::BootInfo;
use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::panic::PanicInfo;
use lazy_static::lazy_static;
use rust_os::buddy_alloc::BuddyAllocatorManager;
use rust_os::frame_alloc;
use rust_os::frame_alloc::FrameSingleAllocator;
use rust_os::global_alloc;
use rust_os::interrupts::setup_idt;
use rust_os::mem;
use rust_os::mem::FRAME_SIZE;
use rust_os::port::init_pics;
use rust_os::vga_buffer::{cls, WRITER};
use rust_os::{println, serial_println};
use spin::Mutex;
use x86_64::instructions::port::Port;

lazy_static! {
    static ref BOOT_INFO: Mutex<Option<&'static BootInfo>> = Mutex::new(None);
}

static mut DUMMY_ALLOCATOR: Option<DummyFrameAllocator> = None;

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
    serial_println!(
        "Last output: {} == 'output 199'",
        core::str::from_utf8(&line[0..10]).unwrap()
    );
    serial_println!("[x] Test passed!");
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
    serial_println!("[x] Test passed!");
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
    serial_println!(
        "Line has dots after some time: {}",
        core::str::from_utf8(&line).unwrap()
    );
    serial_println!("[x] Test passed!");
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
    serial_println!("[x] Test passed!");
}

struct DummyFrameAllocator(u64, u64);

impl frame_alloc::FrameSingleAllocator for DummyFrameAllocator {
    unsafe fn allocate(&mut self) -> Option<mem::PhysAddr> {
        if self.0 < self.1 {
            let phys = mem::PhysAddr::new(self.0 * mem::FRAME_SIZE as u64);
            serial_println!(" - Allocated frame #{} ({})", self.0, phys);
            self.0 += 1;
            Some(phys)
        } else {
            serial_println!(" - Could not allocate frame!");
            None
        }
    }
}

fn get_frame_allocator() -> DummyFrameAllocator {
    let frame_range = (*BOOT_INFO.lock().unwrap().memory_map)
        .iter()
        .rev()
        .find(|region| region.region_type == MemoryRegionType::Usable)
        .unwrap();

    DummyFrameAllocator(
        frame_range.range.start_frame_number,
        frame_range.range.end_frame_number,
    )
}

#[test_case]
fn test_frame_allocation() {
    cls();
    serial_println!("Testing: Frame allocation and mapping new paging table entries...");

    unsafe {
        let allocator = get_frame_allocator();
        DUMMY_ALLOCATOR.replace(allocator);
        // initialize frame allocator
        global_alloc::init_allocator_info(DUMMY_ALLOCATOR.as_mut().unwrap());
    }

    let virt = mem::VirtAddr::new(0xB0000000);
    let phys = mem::PhysAddr::new(0xb8000);
    unsafe {
        serial_println!("Mapping physical frame {} to virtual {}", phys, virt);
        let pte = mem::get_page_table().map_virt_to_phys(
            virt,
            phys,
            mem::BIT_WRITABLE | mem::BIT_PRESENT,
        );
        serial_println!("Mapping written in PT entry at addr {:p}: {}", pte, pte);
        serial_println!("Writing 'X' to virtual {}", virt);
        let cptr: &mut [u8; 100] = virt.to_ref();
        cptr[0] = 'X' as u8;
        cptr[1] = 15u8;
    }
    let line = WRITER.lock().get_line(19);
    assert_eq!(line[0], 'X' as u8);
    serial_println!(
        "VGA buffer: {}",
        core::str::from_utf8(&line[0..10]).unwrap()
    );

    serial_println!("[x] Test passed!");
}

#[test_case]
fn test_global_allocator() {
    cls();
    serial_println!("Creating new buddy allocator manager");
    unsafe {
        let mut allocator = get_frame_allocator();
        let first_page = allocator.allocate().unwrap();
        DUMMY_ALLOCATOR.replace(allocator);
        // initialize frame allocator
        global_alloc::init_allocator_info(DUMMY_ALLOCATOR.as_mut().unwrap());
        // initialize buddy allocator with a single page
        let buddy_alloc_manager = BuddyAllocatorManager::new();
        buddy_alloc_manager.add_memory_area(first_page, first_page.offset(FRAME_SIZE), 16);
        // allocate different block sizes
        let blk_16 = buddy_alloc_manager.alloc(Layout::from_size_align(16, 4).unwrap());
        let blk_32 = buddy_alloc_manager.alloc(Layout::from_size_align(32, 4).unwrap());
        let blk_64 = buddy_alloc_manager.alloc(Layout::from_size_align(64, 4).unwrap());
        let blk_8 = buddy_alloc_manager.alloc(Layout::from_size_align(8, 4).unwrap());
        // test relations of block sizes
        let diff = blk_32 as usize - blk_16 as usize;
        serial_println!(
            "32-block and 16-block must be 32 bytes apart: {:?} - {:?} = {}",
            blk_16,
            blk_32,
            diff
        );
        assert_eq!(diff, 32);
        let diff = blk_64 as usize - blk_32 as usize;
        serial_println!(
            "64-block and 32-block must be 32 bytes apart: {:?} - {:?} = {}",
            blk_64,
            blk_32,
            diff
        );
        assert_eq!(diff, 32);
        let diff = blk_8 as usize - blk_16 as usize;
        serial_println!(
            "8-block and 16-block must be 16 bytes apart: {:?} - {:?} = {}",
            blk_8,
            blk_16,
            diff
        );
        assert_eq!(diff, 16);
        buddy_alloc_manager.dealloc(blk_16, Layout::from_size_align(16, 4).unwrap());
        let blk_8_2 = buddy_alloc_manager.alloc(Layout::from_size_align(8, 8).unwrap());
        serial_println!("After deallocating 16-block new 8-block should be in the same position as old 16-block: {:?} == {:?}", blk_16, blk_8_2);
        assert_eq!(blk_16, blk_8_2);
        buddy_alloc_manager.dealloc(blk_8_2, Layout::from_size_align(8, 8).unwrap());
        buddy_alloc_manager.dealloc(blk_32, Layout::from_size_align(32, 4).unwrap());
        buddy_alloc_manager.dealloc(blk_64, Layout::from_size_align(64, 4).unwrap());
        buddy_alloc_manager.dealloc(blk_8, Layout::from_size_align(8, 4).unwrap());
        // deallocate everything and allocate a 128-byte block
        let blk_128 = buddy_alloc_manager.alloc(Layout::from_size_align(128, 4).unwrap());
        serial_println!(
            "After deallocating everything the blocks should have been merged together and \
             a 128-block should be in the same location as the last 8-block: {:?} == {:?}",
            blk_128,
            blk_8_2
        );
        assert_eq!(blk_128, blk_8_2);
    }
    serial_println!("[x] Test passed!");
}
