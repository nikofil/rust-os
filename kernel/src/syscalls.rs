use core::arch::{asm, naked_asm};
use crate::println;
use alloc::vec::Vec;

// register for address of syscall handler
const MSR_STAR: usize = 0xc0000081;
const MSR_LSTAR: usize = 0xc0000082;
const MSR_FMASK: usize = 0xc0000084;

pub unsafe fn init_syscalls() {
    let handler_addr = handle_syscall_wrapper as *const () as u64;
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
fn sys0(a: u64, b: u64, c: u64, d: u64) -> u64 {
    println!("sys0 {:x} {:x} {:x} {:x}", a, b, c, d);
    123
}

#[inline(never)]
fn sys1(a: u64, b: u64, c: u64, d: u64) -> u64 {
    println!("sys1 {:x} {:x} {:x} {:x}", a, b, c, d);
    456
}


#[inline(never)]
fn sys_hello(a: u64, b: u64, c: u64, d: u64) -> u64 {
    println!("hello world! {:x} {:x} {:x} {:x}", a, b, c, d);
    0
}

#[inline(never)]
fn sys_unhandled() -> u64 {
    panic!("bad syscall number!");
}


// save the registers, handle the syscall and return to usermode
#[naked]
extern "C" fn handle_syscall_wrapper() {
    unsafe {
        naked_asm!("\
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
        mov rcx, r10 // move fourth syscall arg to rcx which is the fourth argument register in sysv64
        mov r8, rax // move syscall number to the 5th argument register
        call {syscall_alloc_stack} // call the handler with the syscall number in r8
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
        syscall_alloc_stack = sym syscall_alloc_stack);
    }
}

// allocate a temp stack and call the syscall handler
unsafe extern "sysv64" fn syscall_alloc_stack(arg0: u64, arg1: u64, arg2: u64, arg3: u64, syscall: u64) -> u64 {
    let syscall_stack: Vec<u8> = Vec::with_capacity(0x10000);
    let stack_ptr = syscall_stack.as_ptr();
    let retval = handle_syscall_with_temp_stack(arg0, arg1, arg2, arg3, syscall, stack_ptr);
    drop(syscall_stack); // we can now drop the syscall temp stack
    return retval;
}

#[inline(never)]
extern "sysv64" fn handle_syscall_with_temp_stack(arg0: u64, arg1: u64, arg2: u64, arg3: u64, syscall: u64, temp_stack: *const u8) -> u64 {
    let old_stack: *const u8;
    unsafe {
        asm!("\
        mov {old_stack}, rsp
        mov rsp, {temp_stack} // move our stack to the newly allocated one
        sti // enable interrupts",
        temp_stack = in(reg) temp_stack, old_stack = out(reg) old_stack);
    }
    let retval: u64 = match syscall {
        0x595ca11a => sys0(arg0, arg1, arg2, arg3),
        0x595ca11b => sys1(arg0, arg1, arg2, arg3),
        0x42 => sys_hello(arg0, arg1, arg2, arg3),
        _ => sys_unhandled(),
    };
    unsafe {
        asm!("\
        cli // disable interrupts while restoring the stack
        mov rsp, {old_stack} // restore the old stack
        ",
        old_stack = in(reg) old_stack);
    }
    retval
}
