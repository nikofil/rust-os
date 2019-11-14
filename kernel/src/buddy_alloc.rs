use crate::frame_alloc::FrameSingleAllocator;
use crate::mem::{PhysAddr, FRAME_SIZE};
use crate::{print, println};
use alloc::alloc::{GlobalAlloc, Layout};
use alloc::vec::Vec;
use core::cmp;
use core::ptr::null_mut;
use lazy_static::lazy_static;
use spin::{Mutex, RwLock};

pub struct BuddyAllocatorManager {
    buddy_allocators: RwLock<Vec<Mutex<BuddyAllocator>>>,
}

impl BuddyAllocatorManager {
    pub fn new() -> BuddyAllocatorManager {
        // Create an empty buddy allocator list. At this point we're still using the dumb page allocator.
        let mut buddy_allocators = RwLock::new(Vec::with_capacity(16));
        BuddyAllocatorManager { buddy_allocators }
    }

    pub fn add_memory_area(&self, start_addr: PhysAddr, size: u64, block_size: u16) {
        // Add a new buddy allocator to the list with these specs.
        // As each one has some dynamic internal structures, we try to make it so that none of these
        // has to use itself when allocating these.
        let new_buddy_alloc = Mutex::new(BuddyAllocator::new(
            start_addr,
            start_addr.offset(size),
            block_size,
        ));
        // On creation the buddy allocator constructor might lock the list of buddy allocators
        // due to the fact that it allocates memory for its internal structures (except for the very
        // first buddy allocator which still uses the previous, dumb allocator).
        // Therefore we first create it and then we lock the list in order to push the new
        // buddy allocator to the list.
        self.buddy_allocators.write().push(new_buddy_alloc);
    }

    pub fn print_info(&self) {
        for (i, ba) in self.buddy_allocators.read().iter().enumerate() {
            ba.lock().print_info(i);
        }
    }
}

unsafe impl GlobalAlloc for BuddyAllocatorManager {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Loop through the list of buddy allocators until we can find one that can give us
        // the requested memory.
        let allocation = self.buddy_allocators.read().iter().find_map(|allocator| {
            allocator
                .try_lock()
                .and_then(|mut allocator| allocator.alloc(layout.size(), layout.align()))
        });
        println!("Our dear allocator gave us a {:x?}", allocation);
        // Convert physical address to virtual if we got an allocation, otherwise return null.
        allocation
            .and_then(|phys| phys.to_virt())
            .map(|virt| virt.addr() as *mut u8)
            .unwrap_or(null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // println!("I AM DE-ALLOCATING NAO");
        // TODO
    }
}

struct BuddyAllocator {
    start_addr: PhysAddr,
    end_addr: PhysAddr,
    num_levels: u8,
    block_size: u16,
    free_lists: Vec<Vec<u32>>,
    is_split: Vec<u8>,
}

impl BuddyAllocator {
    fn new(start_addr: PhysAddr, end_addr: PhysAddr, block_size: u16) -> BuddyAllocator {
        // number of levels excluding the leaf level
        let mut num_levels: u8 = 0;
        while ((block_size << num_levels as u16) as u64) < end_addr.addr() - start_addr.addr() {
            num_levels += 1;
        }
        // vector of free lists
        let mut free_lists: Vec<Vec<u32>> = Vec::with_capacity((num_levels + 1) as usize);
        // Initialize each free list with a small capacity (in order to use the current allocator
        // at least for the first few items and not the one that will be in use when we're actually
        // using this as the allocator as this might lead to this allocator using itself and locking)
        for _ in 0..(num_levels + 1) {
            free_lists.push(Vec::with_capacity(4));
        }
        // The top-most block is (the only) free for now!
        free_lists[0].push(0);
        // We need 1<<levels bits to store which blocks are split (so 1<<(levels-3) bytes)
        let is_split = Vec::with_capacity(1 << (num_levels - 3) as usize);
        BuddyAllocator {
            start_addr,
            end_addr,
            num_levels,
            block_size,
            free_lists,
            is_split,
        }
    }

    fn contains(&self, addr: PhysAddr) -> bool {
        // whether a given physical address belongs to this allocator
        addr.addr() >= self.start_addr.addr() && addr.addr() < self.end_addr.addr()
    }

    fn alloc(&mut self, size: usize, alignment: usize) -> Option<PhysAddr> {
        // max size that can be supported by this buddy allocator
        let max_size = (self.block_size as usize) << (self.num_levels as usize);
        // can't allocate more than that!
        if size > max_size {
            None
        } else {
            // find the largest block level that can support this size
            let mut next_level = 1;
            while (max_size >> next_level) >= size {
                next_level += 1;
            }
            // ...but not larger than the max level!
            let req_level = cmp::min(next_level - 1, self.num_levels as usize);
            self.get_free_block(req_level).map(|block| {
                println!(
                    "Allocated a size {} at level {} number {} (max {})",
                    size, req_level, block, max_size
                );
                // The fn above gives us the index of the block in the given level
                // so we need to find the size of each block in that level and multiply by the index
                // to get the offset of the memory that was allocated.
                let offset = block as u64 * ((max_size as u64) >> req_level as u64);
                // Add the base address of this buddy allocator's block and return
                PhysAddr::new(self.start_addr.addr() + offset)
            })
        }
    }

    fn dealloc(&mut self, addr: PhysAddr, size: usize, alignment: usize) {}

    fn get_free_block(&mut self, level: usize) -> Option<u32> {
        // Get a block from the free list at this level or split a block above and
        // return one of the splitted blocks.
        self.free_lists[level]
            .pop()
            .or_else(|| self.split_level(level))
    }

    fn split_level(&mut self, level: usize) -> Option<u32> {
        // We reached the maximum level, we can't split anymore! We can't support this allocation.
        if level == 0 {
            None
        } else {
            self.get_free_block(level - 1).map(|block| {
                // Get a block from 1 level above us and split it.
                // We push the second of the splitted blocks to the current free list
                // and we return the other one as we now have a block for this allocation!
                self.free_lists[level].push(block * 2 + 1);
                block * 2
            })
        }
    }

    fn print_info(&self, i: usize) {
        println!("BA #{}: start {:?} / levels {} / bs {}", i, self.start_addr, self.num_levels, self.block_size);
        for i in 0usize..(self.num_levels as usize + 1) {
            print!("  Level {} has {} free /", i, self.free_lists[i].len());
        }
        println!();
    }
}
