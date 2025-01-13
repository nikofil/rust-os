use alloc::vec::Vec;
use crate::mem::VirtAddr;
use core::convert::TryInto;
use crate::println;

#[derive(Debug)]
struct ProgramHeader {
    physical_offset: usize,
    load_address: VirtAddr,
    phys_size: usize,
}

pub struct Elf {
    data: Vec<u8>,
    entry_point: VirtAddr,
    headers: Vec<ProgramHeader>,
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
            let physical_offset = usize::from_le_bytes(header[8..16].try_into().unwrap());
            let load_address = VirtAddr::new(usize::from_le_bytes(header[16..24].try_into().unwrap()));
            let phys_size = usize::from_le_bytes(header[32..40].try_into().unwrap());
            ProgramHeader { physical_offset, load_address, phys_size }
        }).collect();

        println!("Elf headers: {:x?} EIP: {:x?}", headers, entry_point);

        Self { data, entry_point, headers }
    }
}