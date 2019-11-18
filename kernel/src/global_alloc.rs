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
    // Get our current buddy allocator
    ALLOCATOR_INFO
        .strategy
        .read()
        .as_ref()
        .map(|buddy_manager| {
            // Allocate increasingly large memory areas.
            // The previously created buddy allocator (which uses a single page) will be used to back
            // the first of these areas' internal structures to avoid the area having to use itself.
            // Then the first two areas will be used to support the third, etc.
            // Until we can support 1GiB buddy allocators (the final type) which need a big
            // amount of continuous backing memory (some MiB for the is_split bitmap plus
            // several Vecs for the free lists).
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
    // Find and create a buddy allocator with the memory area requested.
    // We use get_mem_area_with_size first to find the memory area.
    // That function might instead find one (or two) smaller memory areas if the current
    // memory block that we're pulling memory from isn't big enough.
    // In that case add these smaller ones but keep looping until we get a memory block
    // as big as the one requested.
    // If we run out of memory, we simply return false.
    loop {
        match get_mem_area_with_size(frame_alloc, mem_size) {
            // Success! Found a memory area big enough for our purposes.
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
            // Found one or two smaller memory areas instead, insert them and keep looking.
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
            // Ran out of memory! Return false.
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
    // This function tries to find a continuous memory area as big as the one requested by
    // pulling pages from the frame allocator. If it doesn't find an area big enough immediately,
    // it might return one or two smaller ones (so that we don't leave memory unused for no reason
    // if it doesn't fit our purposes).
    if let Some(first_page) = unsafe { frame_alloc.allocate() } {
        let first_addr = first_page.addr();
        let mut last_addr = first_addr + FRAME_SIZE;
        // Keep pulling pages from the frame allocator until we hit the required memory size
        // or until we run out of memory or we get a block that is not after the previous block received.
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
        // If we found a memory area big enough, great! Return it.
        if last_addr - first_addr == mem_size {
            MemAreaRequest::Success((PhysAddr::new(first_addr), PhysAddr::new(last_addr)))
        } else {
            // If we found a smaller memory block, get the largest piece that is a power of 2
            // and also greater than a page size. We can use that to make a smaller buddy allocator.
            if let Some(first_memarea) = get_largest_page_multiple(first_addr, last_addr) {
                // Try to form a second such block with the left-over memory to not waste it.
                let second_memarea = get_largest_page_multiple(first_memarea.1.addr(), last_addr);
                MemAreaRequest::SmallerThanReq(first_memarea, second_memarea)
            } else {
                // This should never happen but let's be safe
                MemAreaRequest::Fail
            }
        }
    } else {
        // Couldn't even pull a single page from the frame allocator :(
        MemAreaRequest::Fail
    }
}

fn get_largest_page_multiple(start: u64, end: u64) -> Option<(PhysAddr, PhysAddr)> {
    // Given a start and end address, try to find the largest memory size that can fit into that
    // area that is also a left shift of a FRAME_SIZE (ie. 4096, 8192, 16384 etc.)
    // We need this because our buddy allocator needs a memory area whose size is a power of 2
    // in order to be able to split it cleanly and efficiently.
    // Also, the smallest size of that memory area will be the FRAME_SIZE.
    let mem_len = end - start;
    if mem_len == 0 {
        None
    } else {
        // double page_mult while it still fits in this mem area
        let mut page_mult = FRAME_SIZE;
        while page_mult <= mem_len {
            page_mult <<= 1;
        }
        // we went over the limit so divide by two
        page_mult >>= 1;
        let start_addr = PhysAddr::new(start);
        Some((start_addr, start_addr.offset(page_mult)))
    }
}
