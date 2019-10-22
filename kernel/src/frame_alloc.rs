use core::cmp::max;
use multiboot2::BootInformation;
use crate::mem::{PhysAddr, VirtAddr, FRAME_SIZE};

pub static mut BOOTINFO_ALLOCATOR: Option<SimpleAllocator> = None;

pub trait FrameSingleAllocator: Send {
    unsafe fn allocate(&mut self) -> Option<PhysAddr>;
}

pub struct SimpleAllocator<'a> {
    boot_info: &'a BootInformation,
    cur_mem_area: usize,
    next_page: usize,
}

impl SimpleAllocator<'_> {
    pub fn new(boot_info: &BootInformation) -> SimpleAllocator {
        SimpleAllocator {
            boot_info,
            cur_mem_area: 0,
            next_page: 0,
        }
    }
}

impl FrameSingleAllocator for SimpleAllocator<'_> {
    unsafe fn allocate(&mut self) -> Option<PhysAddr> {
        let mem_tag = self.boot_info.memory_map_tag().expect("Must have memory map tag");
        let mem_area = mem_tag.memory_areas().nth(self.cur_mem_area)?;
        let kernel_end = self.boot_info.end_address() as u64;
        let kernel_end_phys = VirtAddr::new(kernel_end).to_phys().unwrap().0.addr();
        let mem_start = max(mem_area.base_addr, kernel_end_phys);
        let mem_end = mem_area.base_addr + mem_area.length;
        let start_addr = ((mem_start + FRAME_SIZE - 1) / FRAME_SIZE) * FRAME_SIZE;
        let end_addr = (mem_end / FRAME_SIZE) * FRAME_SIZE;
        let frame = PhysAddr::new(start_addr + (self.next_page as u64 * FRAME_SIZE));
        if frame.addr() + (FRAME_SIZE as u64) < end_addr {
            self.next_page += 1;
            Some(frame)
        } else {
            self.next_page = 0;
            self.cur_mem_area += 1;
            crate::println!("woah allocating woo");
            self.allocate()
        }
    }
}
