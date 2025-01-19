use core::arch::{asm, naked_asm};
use crate::println;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

// register for address of syscall handler
const MSR_STAR: usize = 0xc0000081;
const MSR_LSTAR: usize = 0xc0000082;
const MSR_FMASK: usize = 0xc0000084;

lazy_static! {
    pub static ref STDIN_BUF: Mutex<Option<Vec<u8>>> = Mutex::new(None);
}

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
fn sys_print(str: u64, strlen: u64, i1: u64, i2: u64) -> u64 {
    let s = unsafe{core::str::from_raw_parts(str as *const u8, strlen as usize)};
    if i1 != 0 && i2 != 0 {
        println!("{} {} {}", s, i1, i2);
    } else if i1 != 0 {
        println!("{} {}", s, i1);
    } else {
        println!("{}", s);
    }
    1
}

#[inline(never)]
fn sys_getline(str: u64, strlen: u64) -> u64 {
    if let Some(mut v) = STDIN_BUF.try_lock().take() {
        if let Some(vv) = v.take() {
            let strptr = str as *mut u8;
            let cplen = vv.len().min(strlen as usize);
            unsafe {strptr.copy_from(vv.as_ptr() as *mut u8, cplen)};
            return cplen as u64;
        }
    }
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
        0x1337 => sys_print(arg0, arg1, arg2, arg3),
        0x1338 => sys_getline(arg0, arg1),
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
