use crate::frame_alloc::FrameSingleAllocator;
use crate::mem::{PhysAddr, VirtAddr};
use alloc::alloc::{GlobalAlloc, Layout};
use alloc::vec::Vec;
use core::ptr::null_mut;
use lazy_static::lazy_static;
use spin::Mutex;
use if_chain::if_chain;

struct AllocatorInfo {
    frame_allocator: Mutex<Option<&'static mut dyn FrameSingleAllocator>>,
    free_frames: Mutex<Option<Vec<PhysAddr>>>,
}

pub struct Allocator;

lazy_static! {
    static ref ALLOCATOR_INFO: AllocatorInfo = AllocatorInfo {
        frame_allocator: Mutex::new(None),
        free_frames: Mutex::new(None),
    };
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if_chain! {
            if let Some(ref mut x) = ALLOCATOR_INFO.free_frames.try_lock();
            if let Some(ref mut free) = x.as_mut();
            if let Some(page) = free.pop();
            if let Some(virt) = page.to_virt();
            then {
                crate::println!("Reusing! ^_^ {:x}", page.addr());
                return virt.to_ref();
            }
        }
        if_chain! {
            if let Some(ref mut allocator) = ALLOCATOR_INFO.frame_allocator.lock().as_mut();
            if let Some(page) = allocator.allocate();
            if let Some(virt) = page.to_virt();
            then {
                crate::println!("Allocated! ^_^ {:x}", virt.addr());
                return virt.to_ref();
            }
        }
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        crate::println!("Deallocating: {:x}", ptr as u64);
        if let Some((phys_addr, _)) = VirtAddr::new(ptr as u64).to_phys() {
            if let Some(ref mut free) = ALLOCATOR_INFO.free_frames.lock().as_mut() {
                free.push(phys_addr);
            }
        }
        crate::println!("Deallocated! v_v {:x}", ptr as u64);
    }
}

pub fn init_global_alloc(frame_alloc: &'static mut dyn FrameSingleAllocator) {
    ALLOCATOR_INFO.frame_allocator.lock().replace(frame_alloc);
    ALLOCATOR_INFO
        .free_frames
        .lock()
        .replace(Vec::with_capacity(0));
}
