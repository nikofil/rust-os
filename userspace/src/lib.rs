#![no_std]
#![feature(str_from_raw_parts)]
use core::arch::asm;
use core::str;
use core::panic::PanicInfo;

#[inline(never)]
pub fn syscall(
    n: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
) -> u64 {
    let mut ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n as u64 => ret,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            in("r10") arg4,
            out("rcx") _, // rcx is used to store old rip
            out("r11") _, // r11 is used to store old rflags
            options(nostack, preserves_flags)
        );
    }
    ret
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

pub fn sleep(c: u64) {
    for _ in 0..c {
        unsafe {
            asm!("nop");
        }
    }
}

pub fn printf(str: &str, a1: u64, a2: u64) -> u64 {
    syscall(0x1337, str.as_ptr() as *const u8 as u64, str.len() as u64, a1, a2)
}

pub fn bytes_to_str(b: &[u8], l: usize) -> &str {
    unsafe {
        str::from_raw_parts(b.as_ptr(), l)
    }
}

pub fn getline(buf: &mut [u8]) -> usize {
    let mut l = 0;
    while l == 0 {
        l = syscall(0x1338, buf.as_ptr() as u64, buf.len() as u64, 0, 0) as usize;
    }
    l
}

pub fn prefix(s: &str, pre: &str) -> bool {
    if s.len() < pre.len() {
        return false;
    }
    for i in 0..pre.len() {
        if pre.bytes().nth(i) != s.bytes().nth(i) {
            return false;
        }
    }
    true
}

pub fn readi(inode: u64, out: &mut [u8]) -> usize {
    syscall(0x8EAD, inode, out.as_ptr() as u64, out.len() as u64, 0) as usize
}

#[unsafe(no_mangle)]
pub fn memset(s: &mut [u8], c: u8) {
    for i in 0..s.len() {
        s[i] = c;
    }
}
