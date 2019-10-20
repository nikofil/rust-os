use core::fmt::Display;

const VIRT_OFFSET: u64 = 0xC0000000;

#[repr(C)]
pub struct PTEntry(u64);

#[repr(C)]
pub struct PageTable {
    entries: [PTEntry; 512],
}

pub const BIT_PRESENT: usize = 0;
pub const BIT_WRITABLE: usize = 1;
pub const BIT_USER: usize = 2;
pub const BIT_WRITE_THROUGH: usize = 3;
pub const BIT_NO_CACHE: usize = 4;
pub const BIT_ACCESSED: usize = 5;
pub const BIT_DIRTY: usize = 6;
pub const BIT_HUGE: usize = 7;
pub const BIT_GLOBAL: usize = 8;

impl PTEntry {
    pub fn get_bit(&self, bit: usize) -> bool {
        ((self.0 >> bit) & 1) == 1
    }

    pub fn set_bit(&mut self, bit: usize, v: bool) {
        if ((self.0  & (1 << bit)) != 0) != v {
            self.0 ^= 1 << bit;
        }
    }

    pub fn phys_addr(&self) -> u64 {
        self.0 & (((1 << 40) - 1) << 12)
    }

    pub unsafe fn next_pt(&self) -> &'static mut PageTable {
        &mut *((self.phys_addr() + VIRT_OFFSET) as *mut PageTable)
    }
}

impl Display for PTEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.get_bit(BIT_PRESENT) {
            let res = write!(f, "{:x}", self.phys_addr());
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
}

pub struct PhysAddr(u64);
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
            return Some((PhysAddr::new(pte.phys_addr() + page_off), &*pte))
        }
        let p2_off = (self.0 >> 21) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p2_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None
        } else if pte.get_bit(BIT_HUGE) {
            let page_off = self.0 & 0x1fffff; // 2 MiB huge page
            return Some((PhysAddr::new(pte.phys_addr() + page_off), &*pte))
        }
        let p1_off = (self.0 >> 12) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p1_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None
        } else {
            let page_off = self.0 & 0xfff; // normal page
            return Some((PhysAddr::new(pte.phys_addr() + page_off), &*pte))
        }
    }

    pub fn addr(&self) -> &u64 {
        &self.0
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

    pub fn addr(&self) -> &u64 {
        &self.0
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
