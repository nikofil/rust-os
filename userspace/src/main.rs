#![no_std]
#![no_main]
use core::arch::asm;
use core::panic::PanicInfo;

#[inline(never)]
pub unsafe fn syscall(
    n: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
) -> u64 {
    let mut ret: u64;
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
    ret
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
fn main() {
    loop {
        for _ in 0..1000000 {}
        unsafe {
            syscall(0x595ca11a, 1, 2, 3, 4);
        }
    }
}
