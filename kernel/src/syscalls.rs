use crate::println;
use crate::serial_println;
use alloc::vec::Vec;

// register for address of syscall handler
const MSR_STAR: usize = 0xc0000081;
const MSR_LSTAR: usize = 0xc0000082;
const MSR_FMASK: usize = 0xc0000084;

pub unsafe fn init_syscalls() {
    let handler_addr = handle_syscall as *const () as u64;
    // write handler address to AMD's MSR_LSTAR register
    asm!("\
    xor rdx, rdx
    mov rax, 0x200
    wrmsr" :: "{rcx}"(MSR_FMASK) : "rdx" : "intel", "volatile");
    // write handler address to AMD's MSR_LSTAR register
    asm!("\
    mov rdx, rax
    shr rdx, 32
    wrmsr" :: "{rax}"(handler_addr), "{rcx}"(MSR_LSTAR) : "rdx" : "intel", "volatile");
    // write segments to use on syscall/sysret to AMD'S MSR_STAR register
    asm!("\
    xor rax, rax
    mov rdx, 0x230008 // use seg selectors 8, 16 for syscall and 43, 51 for sysret
    wrmsr" :: "{rcx}"(MSR_STAR) : "rax", "rdx" : "intel", "volatile");
}

fn sys0(a: u64, b: u64, c: u64, d: u64) -> i64 {
    println!("sys0 {:x} {} {} {}", a, b, c, d);
    123
}

fn sys1(a: u64, b: u64, c: u64, d: u64) -> i64 {
    serial_println!("sys1 {:x} {} {} {}", a, b, c, d);
    456
}

#[naked]
fn handle_syscall() {
    unsafe { asm!("\
        push rcx // backup registers for sysretq
        push r11
        push rbp
        push rbx // save callee-saved registers
        push r12
        push r13
        push r14
        push r15
        mov rbp, rsp
        sub rsp, 0x400 // make some room in the stack
        push rax // backup syscall params while we get some stack space
        push rdi
        push rsi
        push rdx
        push r10"
        :::: "intel", "volatile"); }
    let syscall_stack: Vec<u8> = Vec::with_capacity(0x1000);
    let stack_ptr = syscall_stack.as_ptr();
    unsafe { asm!("\
        pop r10 // restore syscall params to their registers
        pop rdx
        pop rsi
        pop rdi
        pop rax
        mov rsp, rbx // move our stack to the newly allocated one
        sti // enable interrupts"
        :: "{rbx}"(stack_ptr) : "rbx" : "intel", "volatile"); }
    let syscall: u64;
    let arg0: u64;
    let arg1: u64;
    let arg2: u64;
    let arg3: u64;
    unsafe { asm!("nop" : "={rax}"(syscall), "={rdi}"(arg0), "={rsi}"(arg1), "={rdx}"(arg2), "={r10}"(arg3) ::: "intel", "volatile"); }
    // println!("syscall {:x} {} {} {} {}", syscall, arg0, arg1, arg2, arg3);
    let retval: i64 = match syscall {
        666 => sys0(arg0, arg1, arg2, arg3),
        999 => sys1(arg0, arg1, arg2, arg3),
        _ => -1,
    };
    unsafe { asm!("mov rbx, $0; cli" :: "r"(retval) :: "intel", "volatile"); } // disable interrupts while restoring the stack
    drop(syscall_stack); // we can now drop the syscall temp stack
    unsafe { asm!("\
        mov rax, rbx // restore syscall return value
        mov rsp, rbp
        pop r15 // restore callee-saved registers
        pop r14
        pop r13
        pop r12
        pop rbx
        pop rbp // restore stack and registers for sysretq
        pop r11
        pop rcx
        sysretq // back to userland"
        :::: "intel", "volatile"); }
}
