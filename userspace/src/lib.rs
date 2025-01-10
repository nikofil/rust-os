#![no_std]
#![no_main]
use core::arch::asm;
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

#[unsafe(no_mangle)]
extern "C" fn _start() {
    let mut i = 0u64;
    loop {
        sleep(100000000);
        syscall(0x595ca11a, i, i*2, 0, 0);
        i+=1;
    }
}
