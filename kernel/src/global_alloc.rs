use crate::buddy_alloc::BuddyAllocatorManager;
use crate::frame_alloc::FrameSingleAllocator;
use crate::mem::{PhysAddr, VirtAddr, FRAME_SIZE};
use crate::serial_println;
use alloc::alloc::{GlobalAlloc, Layout};
use alloc::vec::Vec;
use core::ptr::null_mut;
use if_chain::if_chain;
use lazy_static::lazy_static;
use spin::{Mutex, RwLock};

struct AllocatorInfo {
    strategy: RwLock<Option<BuddyAllocatorManager>>,
    frame_allocator: Mutex<Option<&'static mut dyn FrameSingleAllocator>>,
    free_frames: Mutex<Option<Vec<PhysAddr>>>,
}

lazy_static! {
    static ref ALLOCATOR_INFO: AllocatorInfo = AllocatorInfo {
        strategy: RwLock::new(None),
        frame_allocator: Mutex::new(None),
        free_frames: Mutex::new(None),
    };
}

pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if_chain! {
            if let Some(ref strategy) = *ALLOCATOR_INFO.strategy.read();
            then {
                return strategy.alloc(layout);
            }
        }
        if_chain! {
            // try locking the free_frames mutex (this locking fails when dealloc needs to allocate
            // more space for its Vec and calls this as it already holds this lock!)
            if let Some(ref mut guard) = ALLOCATOR_INFO.free_frames.try_lock();
            // get as mutable
            if let Some(ref mut free) = guard.as_mut();
            // get last page (if it exists)
            if let Some(page) = free.pop();
            // if a page exists
            if let Some(virt) = page.to_virt();
            // return the page
            then {
                serial_println!("- GlobalAlloc: Reusing {:x}", virt.addr());
                return virt.to_ref();
            }
        }
        if_chain! {
            // lock the frame allocator
            if let Some(ref mut allocator) = ALLOCATOR_INFO.frame_allocator.lock().as_mut();
            // get a physical page from it
            if let Some(page) = allocator.allocate();
            // convert it to virtual (add 0xC0000000)
            if let Some(virt) = page.to_virt();
            // return the page
            then {
                serial_println!("- GlobalAlloc: Allocated {:x} {}", virt.addr(), layout.size());
                return virt.to_ref();
            }
        }
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if_chain! {
            if let Some(ref strategy) = *ALLOCATOR_INFO.strategy.read();
            then {
                return strategy.dealloc(ptr, layout);
            }
        }
        if_chain! {
            // try converting the deallocated virtual page address to the physical address
            if let Some((phys_addr, _)) = VirtAddr::new(ptr as u64).to_phys();
            // lock the free frames list
            if let Some(ref mut free) = ALLOCATOR_INFO.free_frames.lock().as_mut();
            // add the physical address to the free frames list
            then {
                free.push(phys_addr);
            }
        }
        serial_println!("- GlobalAlloc: Deallocated {:x}", ptr as u64);
    }
}

pub fn init_global_alloc(frame_alloc: &'static mut dyn FrameSingleAllocator) {
    let first_page = unsafe { frame_alloc.allocate().unwrap() };
    ALLOCATOR_INFO.frame_allocator.lock().replace(frame_alloc);
    ALLOCATOR_INFO
        .free_frames
        .lock()
        .replace(Vec::with_capacity(0));
    // create our buddy allocator manager (holds a list of buddy allocators for memory regions)
    let manager = BuddyAllocatorManager::new();
    // Create a buddy allocator over a single page which will be provided by our old allocator.
    // This helps us have a single valid page from which our buddy allocator
    // will be able to give blocks away, as otherwise on its first allocation, the buddy allocator
    // would have to call itself in order to create its own internal structures
    // (ie. the free list for each level and the array that holds whether each block is split or not).
    // This way we have one buddy allocator with a single page, which will be used by the second
    // one which will be larger, which will then be used by a larger one until we can map most
    // of the memory. None of these allocators should therefore need to use itself in order to
    // allocate its internal structures which saves us some headaches.
    manager.add_memory_area(first_page, first_page.offset(FRAME_SIZE), 16);
    // Moment of truth! Start using our list of buddy allocators.
    ALLOCATOR_INFO.strategy.write().replace(manager);
    // Create a second, larger buddy allocator in our list which is supported by the first one,
    // as described above.
    let frame_alloc = ALLOCATOR_INFO.frame_allocator.lock().take().unwrap();
    // TODO comments
    ALLOCATOR_INFO
        .strategy
        .read()
        .as_ref()
        .map(|buddy_manager| {
            add_mem_area_with_size(buddy_manager, frame_alloc, FRAME_SIZE * 8, 16);
            add_mem_area_with_size(buddy_manager, frame_alloc, FRAME_SIZE * 64, 16);
            add_mem_area_with_size(buddy_manager, frame_alloc, 1 << 24, 16);
            while add_mem_area_with_size(buddy_manager, frame_alloc, 1 << 30, 16) {}
        });
}

enum MemAreaRequest {
    Success((PhysAddr, PhysAddr)),
    SmallerThanReq((PhysAddr, PhysAddr), Option<(PhysAddr, PhysAddr)>),
    Fail,
}

fn add_mem_area_with_size(
    buddy_alloc: &BuddyAllocatorManager,
    frame_alloc: &mut dyn FrameSingleAllocator,
    mem_size: u64,
    block_size: u16,
) -> bool {
    // TODO comments
    loop {
        match get_mem_area_with_size(frame_alloc, mem_size) {
            MemAreaRequest::Success((mem_start, mem_end)) => {
                serial_println!(
                    "* Adding requested mem area to BuddyAlloc: {} to {} ({})",
                    mem_start,
                    mem_end,
                    mem_end.addr() - mem_start.addr()
                );
                buddy_alloc.add_memory_area(mem_start, mem_end, block_size);
                return true;
            }
            MemAreaRequest::SmallerThanReq((mem_start, mem_end), second_area) => {
                buddy_alloc.add_memory_area(mem_start, mem_end, block_size);
                serial_println!(
                    "* Adding smaller mem area to BuddyAlloc: {} to {} ({})",
                    mem_start,
                    mem_end,
                    mem_end.addr() - mem_start.addr()
                );
                if let Some((mem_start, mem_end)) = second_area {
                    buddy_alloc.add_memory_area(mem_start, mem_end, block_size);
                    serial_println!(
                        "* Adding smaller mem area to BuddyAlloc: {} to {} ({})",
                        mem_start,
                        mem_end,
                        mem_end.addr() - mem_start.addr()
                    );
                }
            }
            MemAreaRequest::Fail => {
                serial_println!(
                    "! Failed to find mem area big enough for BuddyAlloc: {}",
                    mem_size
                );
                return false;
            }
        }
    }
}

fn get_mem_area_with_size(
    frame_alloc: &mut dyn FrameSingleAllocator,
    mem_size: u64,
) -> MemAreaRequest {
    // TODO comments
    if let Some(first_page) = unsafe { frame_alloc.allocate() } {
        let first_addr = first_page.addr();
        let mut last_addr = first_addr + FRAME_SIZE;
        while let Some(next_page) = unsafe { frame_alloc.allocate() } {
            if next_page.addr() == last_addr {
                last_addr += FRAME_SIZE;
            } else {
                break;
            }
            if last_addr - first_addr == mem_size {
                break;
            }
        }
        if last_addr - first_addr == mem_size {
            MemAreaRequest::Success((PhysAddr::new(first_addr), PhysAddr::new(last_addr)))
        } else {
            if let Some(first_memarea) = get_largest_page_multiple(first_addr, last_addr) {
                let second_memarea = get_largest_page_multiple(first_memarea.1.addr(), last_addr);
                MemAreaRequest::SmallerThanReq(first_memarea, second_memarea)
            } else {
                MemAreaRequest::Fail
            }
        }
    } else {
        MemAreaRequest::Fail
    }
}

fn get_largest_page_multiple(start: u64, end: u64) -> Option<(PhysAddr, PhysAddr)> {
    // TODO comments
    let mem_len = end - start;
    if mem_len == 0 {
        None
    } else {
        let mut page_mult = FRAME_SIZE;
        while page_mult <= mem_len {
            page_mult <<= 1;
        }
        page_mult >>= 1;
        let start_addr = PhysAddr::new(start);
        Some((start_addr, start_addr.offset(page_mult)))
    }
}
