
use alloc::vec::Vec;

use crate::{port::Port, println};

const SECTOR_SIZE: usize = 512;

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
    pub fn new_primary_master() -> IDE {
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

pub fn do1() {
    let mut buf = [0u8; 40];

    IDE::new_primary_master().read(0 , &mut buf);
    println!("Hello, fat16! {:x?}", &buf);
    IDE::new_primary_master().read(10, &mut buf);
    println!("Hello, fat16! {:x?}", &buf);
    IDE::new_primary_master().read(0x800, &mut buf);
    println!("Hello, fat16! {:x?}", &buf);
}