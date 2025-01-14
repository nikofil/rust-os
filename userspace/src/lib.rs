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

fn printf(fmt: &str, a1: u64, a2: u64) -> u64 {
    syscall(0x1337, fmt.as_ptr() as *const u8 as u64, fmt.len() as u64, a1, a2)
}

#[unsafe(no_mangle)]
extern "C" fn _start() {
    let mut i = 1u64;
    let mut l = 123;
    loop {
        sleep(100000000);
        l = printf("hello world", i, l);
        i+=1;
    }
}
