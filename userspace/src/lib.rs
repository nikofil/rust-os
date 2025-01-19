#![no_std]
#![no_main]
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

fn sleep(c: u64) {
    for _ in 0..c {
        unsafe {
            asm!("nop");
        }
    }
}

fn printf(str: &str, a1: u64, a2: u64) -> u64 {
    syscall(0x1337, str.as_ptr() as *const u8 as u64, str.len() as u64, a1, a2)
}

fn printb(b: &[u8], l: usize, a1: u64, a2: u64) -> u64 {
    unsafe {
        let s = str::from_raw_parts(b.as_ptr(), l);
        printf(s, a1, a2)
    }
}

fn getline(buf: &mut [u8]) -> usize {
    syscall(0x1338, buf.as_ptr() as u64, buf.len() as u64, 0, 0) as usize
}

#[unsafe(no_mangle)]
extern "C" fn _start() {
    let mut buf = [0u8; 20];
    let mut i = 1u64;
    let mut l = 0usize;
    loop {
        l = 0;
        sleep(500000000);
        printf("write something. last bytes: ", buf[0] as u64, buf[1] as u64);
        while l == 0 {
            l = getline(&mut buf);
        }
        printf("", 0, 0);
        printf("you said:", 0, 0);
        printb(&buf, l, l as u64, i);
        i+=1;
    }
}
