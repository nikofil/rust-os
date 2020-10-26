use alloc::boxed::Box;
use core::fmt::Display;

const VIRT_OFFSET: u64 = 0xC0000000;
pub const FRAME_SIZE: u64 = 0x1000;
type EmptyFrame = [u8; FRAME_SIZE as usize];

#[repr(C)]
#[derive(Copy, Clone)]
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
        if ((self.0 & (bit as u64)) != 0) != v {
            self.0 ^= bit as u64;
        }
    }

    pub fn set_phys_addr(&mut self, addr: PhysAddr) {
        let val = self.0 & ((1 << 9) - 1);
        self.0 = addr.addr() | val;
    }

    pub fn phys_addr(&self) -> PhysAddr {
        PhysAddr::new(self.0 & (((1 << 40) - 1) * FRAME_SIZE))
    }

    pub unsafe fn next_pt(&self) -> &'static mut PageTable {
        self.phys_addr().to_virt().unwrap().to_ref::<PageTable>()
    }
}

impl Display for PTEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.get_bit(BIT_PRESENT) {
            let res = write!(f, "{}", self.phys_addr());
            if self.get_bit(BIT_WRITABLE) {
                write!(f, " writable").unwrap();
            }
            if self.get_bit(BIT_USER) {
                write!(f, " user").unwrap();
            }
            if self.get_bit(BIT_WRITE_THROUGH) {
                write!(f, " write_through").unwrap();
            }
            if self.get_bit(BIT_NO_CACHE) {
                write!(f, " no_cache").unwrap();
            }
            if self.get_bit(BIT_ACCESSED) {
                write!(f, " accessed").unwrap();
            }
            if self.get_bit(BIT_DIRTY) {
                write!(f, " dirty").unwrap();
            }
            if self.get_bit(BIT_HUGE) {
                write!(f, " huge").unwrap();
            }
            if self.get_bit(BIT_GLOBAL) {
                write!(f, " global").unwrap();
            }
            res
        } else {
            write!(f, "<not present>")
        }
    }
}

pub unsafe fn get_page_table() -> &'static mut PageTable {
    let mut p4: u64;
    asm!("mov rax, cr3", out("rax") p4);
    &mut *((p4 + VIRT_OFFSET) as *mut PageTable)
}

impl PageTable {
    pub unsafe fn new() -> Box<PageTable> {
        let mut pt = Box::new(PageTable {
            entries: [PTEntry(0); 512],
        }); // allocate the master PT struct
        pt.entries[0].set_phys_addr(Self::alloc_page()); // allocate page for the first child PT
        pt.entries[0].set_bit(BIT_PRESENT, true);
        pt.entries[0].set_bit(BIT_WRITABLE, true);
        pt.entries[0].set_bit(BIT_USER, true); // entry is present, writable and accessible by user
        let mut pt0 = pt.entries[0].next_pt(); // get the child PT we just allocated
        let cur_pt0 = get_page_table().entries[0].next_pt();
        pt0.entries[3] = cur_pt0.entries[3].clone(); // copy over the entries 3, 4, 5, 6 from the equivalent
        pt0.entries[4] = cur_pt0.entries[4].clone(); // child PT that is currently in use
        pt0.entries[5] = cur_pt0.entries[5].clone(); // these correspond to the addresses our kernel uses
        pt0.entries[6] = cur_pt0.entries[6].clone(); // plus some more, so that the entire physical memory is mapped
        pt
    }

    pub unsafe fn phys_addr(&self) -> PhysAddr {
        let virt = VirtAddr::new(self as *const _ as u64);
        virt.to_phys().unwrap().0
    }

    pub unsafe fn enable(&self) {
        let phys_addr = self.phys_addr().addr();
        asm!("mov cr3, rax", in("rax") phys_addr);
    }

    unsafe fn alloc_page() -> PhysAddr {
        let frame: Box<EmptyFrame> = Box::new([0; FRAME_SIZE as usize]);
        VirtAddr::new(Box::into_raw(frame) as u64)
            .to_phys()
            .unwrap()
            .0
    }

    pub fn get_entry(&mut self, i: usize) -> &mut PTEntry {
        &mut self.entries[i]
    }
    pub unsafe fn map_virt_to_phys(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        create_options: u16,
    ) -> &'static PTEntry {
        let create_huge = (create_options & BIT_HUGE) != 0;
        let p4_off = (virt.addr() >> 39) & 0b1_1111_1111;
        let pte = self.get_entry(p4_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            let new_frame = Self::alloc_page();
            pte.set_phys_addr(new_frame);
            pte.set_bit(BIT_PRESENT, true);
        }
        if (create_options & BIT_WRITABLE) != 0 {
            pte.set_bit(BIT_WRITABLE, true);
        }
        if (create_options & BIT_USER) != 0 {
            pte.set_bit(BIT_USER, true);
        }
        let p3_off = (virt.addr() >> 30) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p3_off as usize);
        if !pte.get_bit(BIT_PRESENT) || pte.get_bit(BIT_HUGE) {
            let new_frame = Self::alloc_page();
            pte.set_phys_addr(new_frame);
            pte.set_bit(BIT_PRESENT, true);
        }
        if (create_options & BIT_WRITABLE) != 0 {
            pte.set_bit(BIT_WRITABLE, true);
        }
        if (create_options & BIT_USER) != 0 {
            pte.set_bit(BIT_USER, true);
        }
        let p2_off = (virt.addr() >> 21) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p2_off as usize);
        if !pte.get_bit(BIT_PRESENT) || pte.get_bit(BIT_HUGE) {
            if create_huge {
                pte.set_phys_addr(phys);
                pte.set_opts(create_options);
                return pte;
            } else {
                let new_frame = Self::alloc_page();
                pte.set_phys_addr(new_frame);
                pte.set_bit(BIT_PRESENT, true);
            }
        }
        if (create_options & BIT_WRITABLE) != 0 {
            pte.set_bit(BIT_WRITABLE, true);
        }
        if (create_options & BIT_USER) != 0 {
            pte.set_bit(BIT_USER, true);
        }
        let p1_off = (virt.addr() / FRAME_SIZE) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p1_off as usize);
        pte.set_phys_addr(phys);
        pte.set_opts(create_options);
        return pte;
    }
}

#[derive(Copy, Clone, Debug)]
pub struct PhysAddr(u64);
#[derive(Copy, Clone, Debug)]
pub struct VirtAddr(u64);

impl VirtAddr {
    pub fn new(addr: u64) -> Self {
        VirtAddr(addr)
    }

    pub fn offset(&self, offset: u64) -> VirtAddr {
        VirtAddr::new(self.0 + offset)
    }

    pub unsafe fn to_ref<T>(&self) -> &'static mut T {
        &mut *(self.0 as *mut T)
    }

    pub unsafe fn to_phys(&self) -> Option<(PhysAddr, &'static PTEntry)> {
        let p4_off = (self.0 >> 39) & 0b1_1111_1111;
        let pte = get_page_table().get_entry(p4_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None;
        }
        let p3_off = (self.0 >> 30) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p3_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None;
        } else if pte.get_bit(BIT_HUGE) {
            let page_off = self.0 & 0x3fffffff; // 1 GiB huge page
            return Some((pte.phys_addr().offset(page_off), &*pte));
        }
        let p2_off = (self.0 >> 21) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p2_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None;
        } else if pte.get_bit(BIT_HUGE) {
            let page_off = self.0 & 0x1fffff; // 2 MiB huge page
            return Some((pte.phys_addr().offset(page_off), &*pte));
        }
        let p1_off = (self.0 / FRAME_SIZE) & 0b1_1111_1111;
        let pte = pte.next_pt().get_entry(p1_off as usize);
        if !pte.get_bit(BIT_PRESENT) {
            return None;
        } else {
            let page_off = self.0 & 0xfff; // normal page
            return Some((pte.phys_addr().offset(page_off), &*pte));
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
