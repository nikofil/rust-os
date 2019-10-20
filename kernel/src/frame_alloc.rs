use core::cmp::max;
use multiboot2::BootInformation;
use crate::mem::PhysAddr;
use crate::mem;

pub const FRAME_SIZE: usize = 0x1000;

pub trait FrameSingleAllocator {
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
        let kernel_end_phys = mem::VirtAddr::new(kernel_end).to_phys().unwrap().0.addr();
        let mem_start = max(mem_area.base_addr, kernel_end_phys);
        let mem_end = mem_area.base_addr + mem_area.length;
        let start_addr = ((mem_start + FRAME_SIZE as u64 - 1) >> 12) << 12;
        let end_addr = (mem_end >> 12) << 12;
        let frame = PhysAddr::new(start_addr + (self.next_page * FRAME_SIZE) as u64);
        if frame.addr() + (FRAME_SIZE as u64) < end_addr {
            self.next_page += 1;
            Some(frame)
        } else {
            self.next_page = 0;
            self.cur_mem_area += 1;
            self.allocate()
        }
    }
}
