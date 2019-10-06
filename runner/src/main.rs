#![no_std]
#![no_main]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[allow(unused_imports)]
use rust_os::{println, serial_println};
use rust_os::vga_buffer::WRITER;
use x86_64::instructions::port::Port;
use core::panic::PanicInfo;
use x86_64::structures::idt::ExceptionStackFrame;

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    #[cfg(test)]
    test_main();
    loop {}
}

#[test_case]
fn test_vga_out() {
    serial_println!("Testing: VGA output... ");
    for i in 0..200 {
        println!("output {}", i);
    }
    let line = WRITER.lock().get_line(1);
    assert_eq!(&line[0..10], "output 199".as_bytes());
    serial_println!("Ok");
    for _ in 0..2000000 {}
}
