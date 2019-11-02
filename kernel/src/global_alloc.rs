use crate::frame_alloc::FrameSingleAllocator;
use crate::mem::PhysAddr;
use alloc::alloc::{GlobalAlloc, Layout};
use alloc::vec::Vec;
use core::ptr::null_mut;
use lazy_static::lazy_static;
use spin::Mutex;

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
        if let Some(ref mut allocator) = ALLOCATOR_INFO.frame_allocator.lock().as_mut() {
            if let Some(page) = allocator.allocate() {
                if let Some(virt) = page.to_virt() {
                    crate::println!("Allocated! ^_^ {:x}", virt.addr());
                    return virt.to_ref();
                }
            }
        }
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        crate::println!("Deallocated! v_v {:x}", ptr as u64);
        if let Some(ref mut free) = ALLOCATOR_INFO.free_frames.lock().as_mut() {
            free.push(PhysAddr::new(ptr as u64));
        }
        if let Some(ref mut free) = ALLOCATOR_INFO.free_frames.lock().as_mut() {
            for i in free.iter() {
                crate::println!("free: {}", i);
            }
        }
    }
}

pub fn init_global_alloc(frame_alloc: &'static mut dyn FrameSingleAllocator) {
    ALLOCATOR_INFO.frame_allocator.lock().replace(frame_alloc);
    ALLOCATOR_INFO
        .free_frames
        .lock()
        .replace(Vec::with_capacity(0));
}
