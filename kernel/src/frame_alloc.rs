use crate::mem::{PhysAddr, VirtAddr, FRAME_SIZE};
use crate::serial_println;
use core::cmp::max;
use multiboot2::BootInformation;
use multiboot2::MemoryAreaIter;

pub static mut BOOTINFO_ALLOCATOR: Option<SimpleAllocator> = None;

pub trait FrameSingleAllocator: Send {
    unsafe fn allocate(&mut self) -> Option<PhysAddr>;
}

pub struct SimpleAllocator {
    kernel_end_phys: u64, // end address of our kernel sections (don't write before this!)
    mem_areas: MemoryAreaIter, // iter of memory areas
    cur_area: Option<(u64, u64)>, // currently used area's bounds
    next_page: usize,     // next page no. in this area to return
}

unsafe impl core::marker::Send for SimpleAllocator {} // shh it's ok pointers are thread-safe

impl SimpleAllocator {
    pub unsafe fn init(boot_info: &'static BootInformation) {
        let mem_tag = boot_info
            .memory_map_tag()
            .expect("Must have memory map tag");
        let mem_areas = mem_tag.memory_areas();
        let kernel_end = boot_info.end_address() as u64;
        let kernel_end_phys = VirtAddr::new(kernel_end).to_phys().unwrap().0.addr();
        let mut alloc = SimpleAllocator {
            kernel_end_phys,
            mem_areas,
            cur_area: None,
            next_page: 0,
        };
        alloc.next_area();

        BOOTINFO_ALLOCATOR.replace(alloc);
    }

    fn next_area(&mut self) {
        self.next_page = 0;
        if let Some(mem_area) = self.mem_areas.next() {
            // get base addr and length for current area
            let base_addr = mem_area.base_addr;
            let area_len = mem_area.length;
            // start after kernel end
            let mem_start = max(base_addr, self.kernel_end_phys);
            let mem_end = base_addr + area_len;
            // memory start addr aligned with page size
            let start_addr = ((mem_start + FRAME_SIZE - 1) / FRAME_SIZE) * FRAME_SIZE;
            // memory end addr aligned with page size
            let end_addr = (mem_end / FRAME_SIZE) * FRAME_SIZE;
            serial_println!(
                "- FrameAlloc: New area: {:x} to {:x} ({})",
                start_addr,
                end_addr,
                end_addr - start_addr
            );
            self.cur_area = Some((start_addr, end_addr));
        } else {
            self.cur_area = None; // out of mem areas :(
        };
    }
}

impl FrameSingleAllocator for SimpleAllocator {
    unsafe fn allocate(&mut self) -> Option<PhysAddr> {
        // get current area start and end addr if we still have an area left
        let (start_addr, end_addr) = self.cur_area?;
        let frame = PhysAddr::new(start_addr + (self.next_page as u64 * FRAME_SIZE));
        // return a page from this area
        if frame.addr() + (FRAME_SIZE as u64) < end_addr {
            self.next_page += 1;
            Some(frame)
        } else {
            // go to next area and try again
            self.next_area();
            self.allocate()
        }
    }
}
