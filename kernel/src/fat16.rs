use core::fmt::{self, Display, Debug};
use core::convert::TryInto;
use alloc::vec::Vec;
use crate::{port::Port, println};
const SECTOR_SIZE: usize = 512;
const DIR_ENTRY_SIZE: usize = 32;

struct SizedString<const N: usize>([u8; N]);

static IDE: IDE = IDE::new_primary_master();

impl<const N: usize> SizedString<N> {
    pub fn new(c: &[u8]) -> Self {
        Self(c.try_into().unwrap())
    }
}

impl<const N: usize> Display for SizedString<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for c in self.0.iter() {
            write!(f, "{}", *c as char)?;
        }
        Ok(())
    }
}

impl<const N: usize> Debug for SizedString<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

pub struct IDE {
    io_port: Port<u16>,
    sel_port: Port<u8>,
    err_io_port: Port<u8>,
    sec_count_port: Port<u8>,
    lba0: Port<u8>,
    lba1: Port<u8>,
    lba2: Port<u8>,
    ctl_port: Port<u8>,
}

impl IDE {
    pub const fn new_primary_master() -> IDE {
        IDE { // port numbers for primary IDE
            io_port: Port::<u16>::new(0x1F0),
            sel_port: Port::<u8>::new(0x1F6),
            err_io_port: Port::<u8>::new(0x1F1),
            sec_count_port: Port::<u8>::new(0x1F2),
            lba0: Port::<u8>::new(0x1F3),
            lba1: Port::<u8>::new(0x1F4),
            lba2: Port::<u8>::new(0x1F5),
            ctl_port: Port::<u8>::new(0x1F7),
        }
    }

    fn is_ready(&self) -> bool {
        let status = self.ctl_port.read();
        if status & 0xA1 != 0 { // BSY or ERR or DF set
            false
        } else {
            status & 8 != 0 // DRQ set
        }
    }

    fn read_sectors(&self, lba: usize, cnt: usize, buf: &mut [u8]) {
        self.sel_port.write(0xE0 | ((lba >> 24) as u8 & 0xF)); // select master IDE
        self.err_io_port.write(0); // wait
        self.sec_count_port.write(cnt as u8); // read cnt sectors
        self.lba0.write(lba as u8); // write logical block address
        self.lba1.write((lba>>8) as u8);
        self.lba2.write((lba>>16) as u8);
        self.ctl_port.write(0x20); // read cmd


        for i in 0..cnt {
            while !self.is_ready() {} //wait for disk to be ready to transfer stuff

            for j in 0..SECTOR_SIZE/2 {
                let b = self.io_port.read(); // read 2 bytes of data
                buf[i*SECTOR_SIZE + j*2] = b as u8;
                buf[i*SECTOR_SIZE + j*2+1] = (b>>8) as u8;
            }

            for _ in 0..4 {
                self.ctl_port.read(); // wait a bit for status to be set
            }
        }

        self.is_ready();
    }

    pub fn read(&self, address: usize, buf: &mut [u8]) {
        let mut v: Vec<u8> = Vec::new();
        v.resize(buf.len() + SECTOR_SIZE*2, 0);

        let first_sector = address / SECTOR_SIZE;
        let read_sectors = v.len() / SECTOR_SIZE;
        let start_address = address % SECTOR_SIZE;

        self.read_sectors(first_sector, read_sectors, &mut v);

        buf.copy_from_slice(&v[start_address..(start_address+buf.len())]);
    }
}

#[derive(Debug)]
struct FAT16 {
    label: SizedString<11>,
    sector_size: u16,
    cluster_sectors: u8,
    reserved_sectors: u16,
    fat_cnt: u8,
    fat_size: u16,
    root_start: u16,
    root_entries: u16,
    data_start: usize,
}

impl FAT16 {
    pub fn new() -> FAT16 {
        let mut buf = [0u8; 512];

        IDE.read(0, &mut buf);

        let sector_size = buf[11] as u16 + ((buf[12] as u16) << 8);
        let fat_cnt = buf[16];
        let reserved_sectors = buf[14] as u16 + ((buf[15] as u16) << 8);
        let fat_size = buf[22] as u16 + ((buf[23] as u16) << 8);
        let root_start = fat_cnt as u16 * fat_size as u16 + reserved_sectors;
        let root_entries = buf[17] as u16 + ((buf[18] as u16) << 8);
        FAT16 {
            label: SizedString::<11>::new(&buf[43..54]),
            sector_size,
            cluster_sectors: buf[13],
            reserved_sectors,
            fat_cnt,
            fat_size,
            root_start,
            root_entries,
            data_start: root_start as usize * sector_size as usize + root_entries as usize * DIR_ENTRY_SIZE,
        }
    }

    pub fn iter(&self) -> DirIter {
        DirIter(self.root_start as usize * self.sector_size as usize)
   }

    fn next_cluster(&self, cluster: u16) -> Option<u16> {
        let mut buf = [0u8; 2];
        let fat_start = self.reserved_sectors as usize * self.sector_size as usize;
        let fat_offset = cluster as usize * 2;
        IDE.read(fat_start + fat_offset, &mut buf);
        let next_cluster = buf[0] as u16 + ((buf[1] as u16) << 8);
        // println!("{:x} {} NEXT IS {}", fat_start, cluster, next_cluster);
        if next_cluster >= 0xFFF8 {
            None
        } else {
            Some(next_cluster)
        }
    }

   pub fn read_data(&self, d: &DirEntry) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut to_read = d.size as usize;
        buf.resize(to_read, 0);
        let mut idx = 0usize;
        let cluster_bytes = self.cluster_sectors as usize * self.sector_size as usize;

        let mut cluster = Some(d.cluster);
        while let Some(cl) = cluster {
            IDE.read((cl-2) as usize * cluster_bytes + self.data_start, &mut buf[idx..idx + cluster_bytes.min(to_read)]);

            if to_read <= cluster_bytes {
                break;
            }
            to_read -= cluster_bytes;
            idx += cluster_bytes;

            cluster = self.next_cluster(cl);
        }

        buf
   }
}

struct DirIter(usize);

impl Iterator for DirIter {
    type Item = DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0u8; DIR_ENTRY_SIZE];
        IDE.read(self.0, &mut buf);
        if buf[0] == 0 {
            None
        } else {
            self.0 += DIR_ENTRY_SIZE;
            Some(DirEntry::new(&buf))
        }
    }
}

#[derive(Debug)]
pub struct DirEntry {
    name: SizedString<11>,
    attr: u8,
    cluster: u16,
    size: u32,
}

impl DirEntry {
    pub fn new(b: &[u8; DIR_ENTRY_SIZE]) -> Self {
        Self {
            name: SizedString::<11>::new(&b[0..11]),
            attr: b[11],
            cluster: b[26] as u16 + ((b[27] as u16) << 8),
            size: u32::from_le_bytes(b[28..32].try_into().unwrap()),
        }
    }
}

pub fn load_main() -> Option<Vec<u8>> {
    let f = FAT16::new();
    println!("FAT16: {:x?}", f);
    for i in f.iter() {
        println!("{:?}", i);
        if i.attr == 32 {
            let v = f.read_data(&i);
            println!("contents {:?}", v.iter().take(20).collect::<Vec<&u8>>());
            if &i.name.0[0..4] == "BOOT".as_bytes() {
                return Some(v);
            }
        }
    }
    None
}