use core::fmt::Display;
use crate::frame_alloc::FrameSingleAllocator;

const VIRT_OFFSET: u64 = 0xC0000000;

#[repr(C)]
pub struct PTEntry(u64);

#[repr(C)]
pub struct PageTable {
    entries: [PTEntry; 512],
}

pub const BIT_PRESENT: u16 = 1;
pub const BIT_WRITABLE: u16 = 1 << 1;
pub const BIT_USER: u16 = 1 << 2;
pub const BIT_WRITE_THROUGH: u16 = 1 << 3;
pub const BIT_NO_CACHE: u16 = 1 << 4;
pub const BIT_ACCESSED: u16 = 1 << 5;
pub const BIT_DIRTY: u16 = 1 << 6;
pub const BIT_HUGE: u16 = 1 << 7;
pub const BIT_GLOBAL: u16 = 1 << 8;

impl PTEntry {
    pub fn get_bit(&self, bit: u16) -> bool {
        (self.0 & (bit as u64)) != 0
    }

    pub fn set_opts(&mut self, options: u16) {
        let val = (self.0 >> 9) << 9;
        self.0 = val | options as u64;
    }

    pub fn set_bit(&mut self, bit: u16, v: bool) {
        if ((self.0  & (bit as u64)) != 0) != v {
            self.0 ^= bit as u64;
        }
    }

    pub fn set_phys_addr(&mut self, addr: PhysAddr) {
        let val = self.0 & ((1 << 9) - 1);
        self.0 = addr.addr() | val;
    }

    pub fn phys_addr(&self) -> PhysAddr {
        PhysAddr::new(self.0 & (((1 << 40) - 1) << 12))
    }

    pub unsafe fn next_pt(&self) -> &'static mut PageTable {
        self.phys_addr().to_virt().unwrap().to_ref::<PageTable>()
    }
}

impl Display for PTEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.get_bit(BIT_PRESENT) {
            let res = write!(f, "{}", self.phys_addr());
            if self.get_bit(BIT_WRITABLE) { write!(f, " writable").unwrap(); }
            if self.get_bit(BIT_USER) { write!(f, " user").unwrap(); }
            if self.get_bit(BIT_WRITE_THROUGH) { write!(f, " write_through").unwrap(); }
            if self.get_bit(BIT_NO_CACHE) { write!(f, " no_cache").unwrap(); }
            if self.get_bit(BIT_ACCESSED) { write!(f, " accessed").unwrap(); }
            if self.get_bit(BIT_DIRTY) { write!(f, " dirty").unwrap(); }
            if self.get_bit(BIT_HUGE) { write!(f, " huge").unwrap(); }
            if self.get_bit(BIT_GLOBAL) { write!(f, " global").unwrap(); }
            res
        } else {
            write!(f, "<not present>")
        }
    }
}

pub unsafe fn get_page_table() -> &'static mut PageTable {
    let mut p4: u64;
    asm!("mov %cr3, $0" : "=r"(p4) : "i"(VIRT_OFFSET) ::: "volatile");
    &mut *((p4 + VIRT_OFFSET) as *mut PageTable)
}

impl PageTable {
    pub fn get_entry(&mut self, i: usize) -> &mut PTEntry {
        &mut self.entries[i]
    }
    pub unsafe fn map_virt_to_phys(&mut self, virt: VirtAddr, phys: PhysAddr, create_options: u16,
    allocator: &mut dyn FrameSingleAllocator) -> &'static PTEntry {
        let create_huge = (create_options & BIT_HUGE) != 0;
        let p4_off = (virt.addr() >> 39) & 0b1_1111_1111;
        let pte = self.get_entry(p4_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            let new_frame = allocator.allocate().expect("Could not allocate!");
            pte.set_phys_addr(new_frame);
            pte.set_bit(BIT_PRESENT, true);
            pte.set_bit(BIT_WRITABLE, true);
        }
        let p3_off = (virt.addr() >> 30) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p3_off as usize);
        if !pte.get_bit(BIT_PRESENT) || pte.get_bit(BIT_HUGE) {
            let new_frame = allocator.allocate().expect("Could not allocate!");
            pte.set_phys_addr(new_frame);
            pte.set_bit(BIT_PRESENT, true);
            pte.set_bit(BIT_WRITABLE, true);
        }
        let p2_off = (virt.addr() >> 21) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p2_off as usize);
        if !pte.get_bit(BIT_PRESENT) || pte.get_bit(BIT_HUGE) {
            if create_huge {
                pte.set_phys_addr(phys);
                pte.set_opts(create_options);
                return pte
            } else {
                let new_frame = allocator.allocate().expect("Could not allocate!");
                pte.set_phys_addr(new_frame);
                pte.set_bit(BIT_PRESENT, true);
                pte.set_bit(BIT_WRITABLE, true);
            }
        }
        let p1_off = (virt.addr() >> 12) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p1_off as usize);
        pte.set_phys_addr(phys);
        pte.set_opts(create_options);
        return pte
    }
}

#[derive(Copy, Clone)]
pub struct PhysAddr(u64);
#[derive(Copy, Clone)]
pub struct VirtAddr(u64);

impl VirtAddr {
    pub fn new(addr: u64) -> Self {
        VirtAddr(addr)
    }

    pub unsafe fn to_ref<T>(&self) -> &'static mut T {
        &mut *(self.0 as *mut T)
    }

    pub unsafe fn to_phys(&self) -> Option<(PhysAddr, &'static PTEntry)> {
        let p4_off = (self.0 >> 39) & 0b1_1111_1111;
        let pte = get_page_table().get_entry(p4_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None
        }
        let p3_off = (self.0 >> 30) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p3_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None
        } else if pte.get_bit(BIT_HUGE) {
            let page_off = self.0 & 0x3fffffff; // 1 GiB huge page
            return Some((pte.phys_addr().offset(page_off), &*pte))
        }
        let p2_off = (self.0 >> 21) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p2_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None
        } else if pte.get_bit(BIT_HUGE) {
            let page_off = self.0 & 0x1fffff; // 2 MiB huge page
            return Some((pte.phys_addr().offset(page_off), &*pte))
        }
        let p1_off = (self.0 >> 12) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p1_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None
        } else {
            let page_off = self.0 & 0xfff; // normal page
            return Some((pte.phys_addr().offset(page_off), &*pte))
        }
    }

    pub fn addr(&self) -> u64 {
        self.0
    }
}

impl PhysAddr {
    pub fn new(addr: u64) -> Self {
        PhysAddr(addr)
    }

    pub unsafe fn to_virt(&self) -> Option<VirtAddr> {
        if self.0 < 0x100000000 {
            Some(VirtAddr::new(self.0 + VIRT_OFFSET))
        } else {
            None
        }
    }

    pub fn addr(&self) -> u64 {
        self.0
    }

    pub fn offset(&self, offset: u64) -> PhysAddr {
        PhysAddr::new(self.0 + offset)
    }
}

impl Display for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtAddr <{:x}>", self.0)
    }
}

impl Display for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysAddr <{:x}>", self.0)
    }
}
