use crate::frame_alloc::FrameSingleAllocator;
use crate::mem::PhysAddr;
use crate::mem::VirtAddr;
use crate::mem::FRAME_SIZE;
// use crate::serial_println;
use alloc::alloc::{GlobalAlloc, Layout};
use alloc::vec::Vec;
use core::cmp;
use core::fmt::Display;
use core::ptr::null_mut;
use spin::{Mutex, RwLock};

pub struct BuddyAllocatorManager {
    buddy_allocators: RwLock<Vec<Mutex<BuddyAllocator>>>,
}

enum MemAreaRequest {
    Success((PhysAddr, PhysAddr)),
    SmallerThanReq((PhysAddr, PhysAddr), Option<(PhysAddr, PhysAddr)>),
    Fail,
}

impl BuddyAllocatorManager {
    pub fn new() -> BuddyAllocatorManager {
        // Create an empty buddy allocator list. At this point we're still using the dumb page allocator.
        let buddy_allocators = RwLock::new(Vec::with_capacity(32));
        BuddyAllocatorManager { buddy_allocators }
    }

    pub fn add_memory_area(&self, start_addr: PhysAddr, end_addr: PhysAddr, block_size: u16) {
        // Add a new buddy allocator to the list with these specs.
        // As each one has some dynamic internal structures, we try to make it so that none of these
        // has to use itself when allocating these.
        let new_buddy_alloc = Mutex::new(BuddyAllocator::new(start_addr, end_addr, block_size));
        // On creation the buddy allocator constructor might lock the list of buddy allocators
        // due to the fact that it allocates memory for its internal structures (except for the very
        // first buddy allocator which still uses the previous, dumb allocator).
        // Therefore we first create it and then we lock the list in order to push the new
        // buddy allocator to the list.
        self.buddy_allocators.write().push(new_buddy_alloc);
    }

    pub fn add_mem_area_with_size(
        &self,
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
            match Self::get_mem_area_with_size(frame_alloc, mem_size) {
                // Success! Found a memory area big enough for our purposes.
                MemAreaRequest::Success((mem_start, mem_end)) => {
                    // serial_println!(
                    //     "* Adding requested mem area to BuddyAlloc: {} to {} ({})",
                    //     mem_start,
                    //     mem_end,
                    //     mem_end.addr() - mem_start.addr()
                    // );
                    self.add_memory_area(mem_start, mem_end, block_size);
                    return true;
                }
                // Found one or two smaller memory areas instead, insert them and keep looking.
                MemAreaRequest::SmallerThanReq((mem_start, mem_end), second_area) => {
                    self.add_memory_area(mem_start, mem_end, block_size);
                    // serial_println!(
                    //     "* Adding smaller mem area to BuddyAlloc: {} to {} ({})",
                    //     mem_start,
                    //     mem_end,
                    //     mem_end.addr() - mem_start.addr()
                    // );
                    if let Some((mem_start, mem_end)) = second_area {
                        self.add_memory_area(mem_start, mem_end, block_size);
                        // serial_println!(
                        //     "* Adding smaller mem area to BuddyAlloc: {} to {} ({})",
                        //     mem_start,
                        //     mem_end,
                        //     mem_end.addr() - mem_start.addr()
                        // );
                    }
                }
                // Ran out of memory! Return false.
                MemAreaRequest::Fail => {
                    // serial_println!(
                    //     "! Failed to find mem area big enough for BuddyAlloc: {}",
                    //     mem_size
                    // );
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
                if let Some(first_memarea) = Self::get_largest_page_multiple(first_addr, last_addr)
                {
                    // Try to form a second such block with the left-over memory to not waste it.
                    let second_memarea =
                        Self::get_largest_page_multiple(first_memarea.1.addr(), last_addr);
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
}

unsafe impl GlobalAlloc for BuddyAllocatorManager {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Loop through the list of buddy allocators until we can find one that can give us
        // the requested memory.
        let allocation =
            self.buddy_allocators
                .read()
                .iter()
                .enumerate()
                .find_map(|(_i, allocator)| {
                    // for each allocator
                    allocator.try_lock().and_then(|mut allocator| {
                        allocator
                            .alloc(layout.size(), layout.align())
                            .map(|allocation| {
                                // try allocating until one succeeds and return this allocation
                                // serial_println!(
                                //     " - BuddyAllocator #{} allocated {} bytes",
                                //     i,
                                //     layout.size()
                                // );
                                // serial_println!("{}", *allocator);
                                allocation
                            })
                    })
                });
        // Convert physical address to virtual if we got an allocation, otherwise return null.
        allocation
            .and_then(|phys| phys.to_virt())
            .map(|virt| virt.addr() as *mut u8)
            .unwrap_or(null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let virt_addr = VirtAddr::new(ptr as u64);
        if let Some((phys_addr, _)) = virt_addr.to_phys() {
            for (_i, allocator_mtx) in self.buddy_allocators.read().iter().enumerate() {
                // for each allocator
                if let Some(mut allocator) = allocator_mtx.try_lock() {
                    // find the one whose memory range contains this address
                    if allocator.contains(phys_addr) {
                        // deallocate using this allocator!
                        allocator.dealloc(phys_addr, layout.size(), layout.align());
                        // serial_println!(
                        //     " - BuddyAllocator #{} de-allocated {} bytes",
                        //     i,
                        //     layout.size()
                        // );
                        // serial_println!("{}", *allocator);
                        return;
                    }
                }
            }
        }
        // serial_println!(
        //     "! Could not de-allocate virtual address: {} / Memory lost",
        //     virt_addr
        // );
    }
}

struct BuddyAllocator {
    start_addr: PhysAddr,      // the first physical address that this struct manages
    end_addr: PhysAddr,        // one byte after the last physical address that this struct manages
    num_levels: u8,            // the number of non-leaf levels
    block_size: u16,           // the size of blocks on the leaf level
    free_lists: Vec<Vec<u32>>, // the list of free blocks on each level
}

impl BuddyAllocator {
    fn new(start_addr: PhysAddr, end_addr: PhysAddr, block_size: u16) -> BuddyAllocator {
        // number of levels excluding the leaf level
        let mut num_levels: u8 = 0;
        while ((block_size as u64) << num_levels as u64) < end_addr.addr() - start_addr.addr() {
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
        BuddyAllocator {
            start_addr,
            end_addr,
            num_levels,
            block_size,
            free_lists,
        }
    }

    fn contains(&self, addr: PhysAddr) -> bool {
        // whether a given physical address belongs to this allocator
        addr.addr() >= self.start_addr.addr() && addr.addr() < self.end_addr.addr()
    }

    fn max_size(&self) -> usize {
        // max size that can be supported by this buddy allocator
        (self.block_size as usize) << (self.num_levels as usize)
    }

    fn req_size_to_level(&self, size: usize) -> Option<usize> {
        // Find the level of this allocator than can accommodate the required memory size.
        let max_size = self.max_size();
        if size > max_size {
            // can't allocate more than the maximum size for this allocator!
            None
        } else {
            // find the largest block level that can support this size
            let mut next_level = 1;
            while (max_size >> next_level) >= size {
                next_level += 1;
            }
            // ...but not larger than the max level!
            let req_level = cmp::min(next_level - 1, self.num_levels as usize);
            Some(req_level)
        }
    }

    fn alloc(&mut self, size: usize, alignment: usize) -> Option<PhysAddr> {
        // We should always be aligned due to how the buddy allocator works
        // (everything will be aligned to block_size bytes).
        // If we need in some case that we are aligned to a greater size,
        // allocate a memory block of (alignment) bytes.
        let size = cmp::max(size, alignment);
        // find which level of this allocator can accommodate this amount of memory (if any)
        self.req_size_to_level(size).and_then(|req_level| {
            // We can accommodate it! Now to check if we actually have / can make a free block
            // or we're too full.
            self.get_free_block(req_level).map(|block| {
                // We got a free block!
                // get_free_block gives us the index of the block in the given level
                // so we need to find the size of each block in that level and multiply by the index
                // to get the offset of the memory that was allocated.
                let offset = block as u64 * (self.max_size() >> req_level as usize) as u64;
                // Add the base address of this buddy allocator's block and return
                PhysAddr::new(self.start_addr.addr() + offset)
            })
        })
    }

    fn dealloc(&mut self, addr: PhysAddr, size: usize, alignment: usize) {
        // As above, find which size was used for this allocation so that we can find the level
        // that gave us this memory block.
        let size = cmp::max(size, alignment);
        // find which level of this allocator was used for this memory request
        if let Some(req_level) = self.req_size_to_level(size) {
            // find size of each block at this level
            let level_block_size = self.max_size() >> req_level;
            // calculate which # block was just freed by using the start address and block size
            let block_num =
                ((addr.addr() - self.start_addr.addr()) as usize / level_block_size) as u32;
            // push freed block to the free list so we can reuse it
            self.free_lists[req_level].push(block_num);
            // try merging buddy blocks now that we might have some to merge
            self.merge_buddies(req_level, block_num);
        }
    }

    fn merge_buddies(&mut self, level: usize, block_num: u32) {
        // toggle last bit to get buddy block
        let buddy_block = block_num ^ 1;
        // if buddy block in free list
        if let Some(buddy_idx) = self.free_lists[level]
            .iter()
            .position(|blk| *blk == buddy_block)
        {
            // remove current block (in last place)
            self.free_lists[level].pop();
            // remove buddy block
            self.free_lists[level].remove(buddy_idx);
            // add free block to free list 1 level above
            self.free_lists[level - 1].push(block_num / 2);
            // repeat the process!
            self.merge_buddies(level - 1, block_num / 2)
        }
    }

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
}

impl Display for BuddyAllocator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut res = writeln!(
            f,
            "  Start: {:x?} / End: {:x?} / Levels: {} / Block size: {} / Max alloc: {}",
            self.start_addr,
            self.end_addr,
            self.num_levels + 1,
            self.block_size,
            (self.block_size as usize) << (self.num_levels as usize),
        );
        res = res.and_then(|_| write!(f, "  Free lists: "));
        for i in 0usize..(self.num_levels as usize + 1) {
            res = res.and_then(|_| write!(f, "{} in L{} / ", self.free_lists[i].len(), i));
        }
        res
    }
}
