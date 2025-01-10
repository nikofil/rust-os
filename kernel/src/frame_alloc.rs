use crate::mem::{PhysAddr, VirtAddr, FRAME_SIZE};
use crate::serial_println;
use core::cmp::max;
use core::slice::Iter;
use multiboot2::BootInformation;
use multiboot2::MemoryArea;

pub static mut BOOTINFO_ALLOCATOR: Option<SimpleAllocator> = None;

pub trait FrameSingleAllocator: Send {
    unsafe fn allocate(&mut self) -> Option<PhysAddr>;
}

pub struct SimpleAllocator {
    kernel_end_phys: usize, // end address of our kernel sections (don't write before this!)
    mem_areas: Iter<'static, MemoryArea>, // memory areas from multiboot
    cur_area: Option<(usize, usize)>, // currently used area's bounds
    next_page: Option<usize>,     // physical address of last page returned
}

unsafe impl core::marker::Send for SimpleAllocator {} // shh it's ok pointers are thread-safe

impl SimpleAllocator {
    pub unsafe fn init(boot_info: &'static BootInformation<'static>) {
        let kernel_end = boot_info.end_address();
        let kernel_end_phys = VirtAddr::new(kernel_end).to_phys().unwrap().0.addr();
        let mem_tag = boot_info
            .memory_map_tag()
            .expect("Must have memory map tag");
        let mut alloc = SimpleAllocator {
            kernel_end_phys,
            mem_areas: mem_tag.memory_areas().iter(),
            cur_area: None,
            next_page: None,
        };
        alloc.next_area();

        BOOTINFO_ALLOCATOR.replace(alloc);
    }

    fn next_area(&mut self) -> Option<(usize, usize)> {
        self.cur_area = self.mem_areas.next().map(|mem_area| {
            // get base addr and length for current area
            let base_addr = mem_area.start_address() as usize;
            let area_len = mem_area.size() as usize;
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
            self.next_page = Some(start_addr);
            (start_addr, end_addr)
        });

        self.cur_area
    }
}

impl FrameSingleAllocator for SimpleAllocator {
    unsafe fn allocate(&mut self) -> Option<PhysAddr> {
        // return a page from this area
        let frame = PhysAddr::new(self.next_page?);
        // get current area end addr if we still have an area left
        let (_, end_addr) = self.cur_area?;
        // increment addr to the next page
        *(self.next_page.as_mut()?) += FRAME_SIZE;
        if self.next_page? <= end_addr {
            Some(frame)
        } else {
            // end of the frame is beyond the area limits, go to next area and try again
            self.next_area()?;
            self.allocate()
        }
    }
}
