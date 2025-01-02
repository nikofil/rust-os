use crate::println;
use alloc::vec::Vec;

// register for address of syscall handler
const MSR_STAR: usize = 0xc0000081;
const MSR_LSTAR: usize = 0xc0000082;
const MSR_FMASK: usize = 0xc0000084;

pub unsafe fn init_syscalls() {
    let handler_addr = handle_syscall as *const () as u64;
    // clear Interrupt flag on syscall with AMD's MSR_FSTAR register
    asm!("\
    xor rdx, rdx
    mov rax, 0x200
    wrmsr", in("rcx") MSR_FMASK, out("rdx") _);
    // write handler address to AMD's MSR_LSTAR register
    asm!("\
    mov rdx, rax
    shr rdx, 32
    wrmsr", in("rax") handler_addr, in("rcx") MSR_LSTAR, out("rdx") _);
    // write segments to use on syscall/sysret to AMD'S MSR_STAR register
    asm!("\
    xor rax, rax
    mov rdx, 0x230008 // use seg selectors 8, 16 for syscall and 43, 51 for sysret
    wrmsr", in("rcx") MSR_STAR, out("rax") _, out("rdx") _);
}

#[inline(never)]
fn sys0(a: u64, b: u64, c: u64, d: u64) -> i64 {
    println!("sys0 {:x} {:x} {:x} {:x}", a, b, c, d);
    123
}

#[inline(never)]
fn sys1(a: u64, b: u64, c: u64, d: u64) -> i64 {
    println!("sys1 {:x} {:x} {:x} {:x}", a, b, c, d);
    456
}


#[inline(never)]
fn sys_hello(a: u64, b: u64, c: u64, d: u64) -> i64 {
    println!("hello world! {:x} {:x} {:x} {:x}", a, b, c, d);
    0
}

#[inline(never)]
fn sys_unhandled() -> i64 {
    // println!("bad syscall number!");
    panic!("bad syscall number!");
    0xdeadbeef
}


// naked functions are supposed to be a single asm block
// #[naked]
fn handle_syscall() {
    unsafe {
        asm!("\
        push rcx // backup registers for sysretq
        push r11
        push rbp // save callee-saved registers
        push rbx
        push r12
        push r13
        push r14
        push r15
        mov rbp, rsp // save rsp
        sub rsp, 0x400 // make some room in the stack
        push rax // backup syscall params while we get some stack space
        push rdi
        push rsi
        push rdx
        push r10"
        );
    }
    let syscall_stack: Vec<u8> = Vec::with_capacity(0x10000);
    let stack_ptr = syscall_stack.as_ptr();
    unsafe {
        asm!("\
        pop r10 // restore syscall params to their registers
        pop rdx
        pop rsi
        pop rdi
        pop rax
        mov rsp, r9 // move our stack to the newly allocated one
        sti // enable interrupts",
        inout("r9") stack_ptr => _);
    }
    let syscall: u64;
    let arg0: u64;
    let arg1: u64;
    let arg2: u64;
    let arg3: u64;
    unsafe {
        // move the syscall arguments from registers to variables
        asm!("nop",
        out("rax") syscall, out("rdi") arg0, out("rsi") arg1, out("rdx") arg2, out("r10") arg3);
    }
    let retval: i64 = match syscall {
        0x595ca11a => sys0(arg0, arg1, arg2, arg3),
        0x595ca11b => sys1(arg0, arg1, arg2, arg3),
        0x42 => sys_hello(arg0, arg1, arg2, arg3),
        _ => sys_unhandled(),
    };
    unsafe {
        asm!("\
        mov rbx, {} // save return value into rbx so that it's maintained through free
        cli",
        in(reg) retval // disable interrupts while restoring the stack
        );
    }
    drop(syscall_stack); // we can now drop the syscall temp stack
    unsafe {
        asm!("\
        mov rax, rbx // restore syscall return value from rbx to rax
        mov rsp, rbp // restore rsp from rbp
        pop r15 // restore callee-saved registers
        pop r14
        pop r13
        pop r12
        pop rbx
        pop rbp // restore stack and registers for sysretq
        pop r11
        pop rcx
        sysretq // back to userland",
        options(noreturn));
    }
}
