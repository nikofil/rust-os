use crate::mem::{PhysAddr, VirtAddr, FRAME_SIZE};
use crate::serial_println;
use core::cmp::max;
use core::convert::TryInto;
use core::slice::Iter;
use multiboot2::BootInformation;
use multiboot2::MemoryArea;

pub static mut BOOTINFO_ALLOCATOR: Option<SimpleAllocator> = None;

pub trait FrameSingleAllocator: Send {
    unsafe fn allocate(&mut self) -> Option<PhysAddr>;
}

pub struct SimpleAllocator {
    kernel_end_phys: u64, // end address of our kernel sections (don't write before this!)
    cur_area: Option<(u64, u64)>, // currently used area's bounds
    mem_areas: [MemoryArea; 8], // memory areas from multiboot
    idx_next: usize, // current index in memory areas
    next_page: usize,     // next page no. in this area to return
}

unsafe impl core::marker::Send for SimpleAllocator {} // shh it's ok pointers are thread-safe

impl SimpleAllocator {
    pub unsafe fn init(boot_info: BootInformation) {
        let kernel_end = boot_info.end_address() as u64;
        let kernel_end_phys = VirtAddr::new(kernel_end).to_phys().unwrap().0.addr();
        let mem_tag = boot_info
            .memory_map_tag()
            .expect("Must have memory map tag");
        let mut alloc = SimpleAllocator {
            kernel_end_phys,
            cur_area: None,
            mem_areas: mem_tag.memory_areas().try_into().unwrap(),
            idx_next: 0,
            next_page: 0,
        };
        alloc.next_area();

        BOOTINFO_ALLOCATOR.replace(alloc);
    }

    fn next_area(&mut self) -> Option<(u64, u64)> {
        self.next_page = 0;

        if self.idx_next >= self.mem_areas.len() {
            self.cur_area = None;
            return None;
        }
        // TODO fix all this crap

        let mem_area = self.mem_areas[self.idx_next];
            // get base addr and length for current area
            let base_addr = mem_area.start_address();
            let area_len = mem_area.size();
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

        self.idx_next += 1;
        self.cur_area
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
            self.next_area()?;
            self.allocate()
        }
    }
}
