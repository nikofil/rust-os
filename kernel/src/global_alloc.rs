use crate::buddy_alloc::BuddyAllocatorManager;
use crate::frame_alloc::FrameSingleAllocator;
use crate::mem::{PhysAddr, VirtAddr, FRAME_SIZE};
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
                crate::println!("going to other alloc!!");
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
                crate::println!("* Reusing! ^_^ {:x}", virt.addr());
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
                crate::println!("* Allocated! ^_^ {:x}", virt.addr());
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
        crate::println!("* Deallocating: {:x}", ptr as u64);
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
        crate::println!("* Deallocated! v_v {:x}", ptr as u64);
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
    let mut manager = BuddyAllocatorManager::new();
    // Create a buddy allocator over a single page which will be provided by our old allocator.
    // This helps us have a single valid page from which our buddy allocator
    // will be able to give blocks away, as otherwise on its first allocation, the buddy allocator
    // would have to call itself in order to create its own internal structures
    // (ie. the free list for each level and the array that holds whether each block is split or not).
    // This way we have one buddy allocator with a single page, which will be used by the second
    // one which will be larger, which will then be used by a larger one until we can map most
    // of the memory. None of these allocators should therefore need to use itself in order to
    // allocate its internal structures which saves us some headaches.
    manager.add_memory_area(first_page, FRAME_SIZE, 16);
    // Moment of truth! Start using our list of buddy allocators.
    ALLOCATOR_INFO.strategy.write().replace(manager);
    // Create a second, larger buddy allocator in our list which is supported by the first one,
    // as described above.
    let mut frame_alloc = ALLOCATOR_INFO.frame_allocator.lock().take().unwrap();
    let first_page = unsafe { frame_alloc.allocate().unwrap() };
    ALLOCATOR_INFO
        .strategy
        .read()
        .as_ref()
        .map(|x| x.add_memory_area(first_page, FRAME_SIZE * 4, 16));
    ALLOCATOR_INFO
        .strategy
        .read()
        .as_ref()
        .map(|x| x.print_info());
}
