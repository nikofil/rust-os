use alloc::vec::Vec;
use alloc::boxed::Box;
use core::pin::Pin;
use crate::{mem::VirtAddr, scheduler::Task, mem::PageTable, mem};
use core::convert::TryInto;
use crate::serial_println;

#[derive(Debug)]
struct ProgramHeader {
    htype: u8,
    physical_offset: usize,
    load_address: VirtAddr,
    phys_size: usize,
}

pub struct Elf {
    data: Pin<Box<[u8]>>,
    entry_point: VirtAddr,
    headers: Vec<ProgramHeader>,
}

impl Into<Task> for Elf {
    fn into(self) -> Task {
        let mut task_pt = unsafe {PageTable::new()};
        let phys_addr = unsafe {VirtAddr::new(self.data.as_ptr() as usize).to_phys().unwrap().0};
        for header in self.headers.iter() {
            if header.htype != 1 {
                continue;
            }
            for page_idx in (0..header.phys_size).step_by(mem::FRAME_SIZE) { // map each page (frame) at a time for this header
                unsafe {
                    let page_virt = header.load_address.offset(page_idx * mem::FRAME_SIZE);
                    let page_phys = phys_addr.offset(header.physical_offset + page_idx * mem::FRAME_SIZE);
                    serial_println!("ELF: Mapping {:x} to {:x}", page_virt.addr(), page_phys.addr());
                    task_pt.map_virt_to_phys(
                        page_virt, // map the nth page corresponding to this header's loadable data
                        page_phys, // locate the physical address of the prog data for this page
                        mem::BIT_PRESENT | mem::BIT_USER); // mapped page is user accessible and present
                }
            }
        }

        let mut stack_space: Pin<Box<[u8]>> = Pin::new(Box::new([0u8; mem::FRAME_SIZE])); // allocate some memory to use for the stack
        unsafe {
            let stack_space_phys = VirtAddr::new(stack_space.as_mut_ptr() as *const u8 as usize)
                .to_phys()
                .unwrap()
                .0;
            // take physical address of stack
            task_pt.map_virt_to_phys(
                mem::VirtAddr::new(0x800000),
                stack_space_phys,
                mem::BIT_PRESENT | mem::BIT_WRITABLE | mem::BIT_USER,
            ); // map the stack memory to 0x800000
        }
        Task::new(
            self.entry_point,
            mem::VirtAddr::new(0x801000),
            task_pt,
            self.data,
            stack_space,
        )
    }
}

impl Elf {
    pub fn new(data: Vec<u8>) -> Self {
        let entry_point = VirtAddr::new(usize::from_le_bytes(data[24..32].try_into().unwrap()));
        let ph_off = usize::from_le_bytes(data[32..40].try_into().unwrap());
        let ph_siz = u16::from_le_bytes(data[54..56].try_into().unwrap()) as usize;
        let ph_cnt = u16::from_le_bytes(data[56..58].try_into().unwrap()) as usize;

        let headers = (0..ph_cnt).map(|i| {
            let header_index = ph_off + i * ph_siz;
            let header = &data[header_index..header_index+ph_siz];
            let htype = header[0] as u8;
            let physical_offset = usize::from_le_bytes(header[8..16].try_into().unwrap());
            let load_address = VirtAddr::new(usize::from_le_bytes(header[16..24].try_into().unwrap()));
            let phys_size = usize::from_le_bytes(header[32..40].try_into().unwrap());
            ProgramHeader { htype, physical_offset, load_address, phys_size }
        }).collect();

        serial_println!("Elf headers: {:x?} EIP: {:x?}", headers, entry_point);

        Self { data: Pin::new(data.into_boxed_slice()), entry_point, headers }
    }
}