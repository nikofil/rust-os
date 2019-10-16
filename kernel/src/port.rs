use core::marker::PhantomData;
use crate::println;

pub trait InOut {
    unsafe fn port_in(port: u16) -> Self;
    unsafe fn port_out(port: u16, val: Self);
}

impl InOut for u8 {
    unsafe fn port_in(port: u16) -> Self {
        let mut val = 0;
        asm!("in al, dx" : "={al}"(val) : "{dx}"(port) :: "intel");
        return val;
    }

    unsafe fn port_out(port: u16, val: Self) {
        asm!("out dx, al" :: "{al}"(val), "{dx}"(port) :: "intel", "volatile");
    }
}

impl InOut for u16 {
    unsafe fn port_in(port: u16) -> Self {
        let mut val = 0;
        asm!("in ax, dx" : "={ax}"(val) : "{dx}"(port) :: "intel");
        return val;
    }

    unsafe fn port_out(port: u16, val: Self) {
        asm!("out dx, ax" :: "{ax}"(val), "{dx}"(port) :: "intel", "volatile");
    }
}

impl InOut for u32 {
    unsafe fn port_in(port: u16) -> Self {
        let mut val = 0;
        asm!("in eax, dx" : "={eax}"(val) : "{dx}"(port) :: "intel");
        return val;
    }

    unsafe fn port_out(port: u16, val: Self) {
        asm!("out dx, eax" :: "{eax}"(val), "{dx}"(port) :: "intel", "volatile");
    }
}

pub struct Port<T> where T: InOut {
    port: u16,
    pt: PhantomData<T>,
}

impl<T> Port<T> where T: InOut {
    pub fn new(port: u16) -> Port<T> {
        Port {
            port,
            pt: PhantomData,
        }
    }

    pub fn write(&self, val: T) {
        unsafe { T::port_out(self.port, val); }
    }

    pub fn read(&self) -> T {
        unsafe { T::port_in(self.port) }
    }
}

const PIC_MASTER_PORT: u16 = 0x20;
const PIC_SLAVE_PORT: u16 = 0xA0;
const WAIT_PORT: u16 = 0x11;

const ICW1_ICW4: u8 = 0x01; // ICW4 (not) needed
const ICW1_INIT: u8 = 0x10; // Initialization - required!
const ICW4_8086: u8 = 0x01; // 8086/88 (MCS-80/85) mode

const PIC_MASTER_NEW_OFFSET: u8 = 0x20;
const PIC_SLAVE_NEW_OFFSET: u8 = 0x20;

const END_OF_INTERRUPT: u8 = 0x20;

pub fn init_pics() {
    let master_cmd: Port<u8> = Port::new(PIC_MASTER_PORT);
    let master_data: Port<u8> = Port::new(PIC_MASTER_PORT + 1);
    let slave_cmd: Port<u8> = Port::new(PIC_SLAVE_PORT);
    let slave_data: Port<u8> = Port::new(PIC_SLAVE_PORT + 1);
    let wait_port: Port<u8> = Port::new(WAIT_PORT);
    let wait = || { wait_port.write(0) };

    // save interrupt masks
    let a1 = master_data.read();
    let a2 = slave_data.read();

    println!(" - PIC interrupt masks: master {} slave {}", a1, a2);

    // begin initialization
    master_cmd.write(ICW1_INIT+ICW1_ICW4);
    wait();
    slave_cmd.write(ICW1_INIT+ICW1_ICW4);
    wait();

    // set interrupt offsets
    master_data.write(PIC_MASTER_NEW_OFFSET);
    wait();
    slave_data.write(PIC_SLAVE_NEW_OFFSET);
    wait();

    // chain slave PIC to master
    master_data.write(4); // tell master there is a slave PIC at IRQ2
    wait();
    slave_data.write(2); // tell slave it's cascade
    wait();

    // set mode
    master_data.write(ICW4_8086);
    wait();
    slave_data.write(ICW4_8086);
    wait();

    // restore interrupt masks
    master_data.write(a1);
    slave_data.write(a2);

    unsafe {
        asm!("sti" ::::: "intel");
    }
}

pub fn end_of_interrupt() {
     Port::new(PIC_MASTER_PORT).write(END_OF_INTERRUPT);
}