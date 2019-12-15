use crate::println;
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

#[naked]
fn handle_syscall() {
    unsafe { asm!("\
        push rcx
        push r11
        push rbp // backup registers for sysretq
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
    println!("syscall {:x} {} {} {} {}", syscall, arg0, arg1, arg2, arg3);
    unsafe { asm!("cli" :::: "intel", "volatile"); } // disable interrupts while restoring the stack
    drop(syscall_stack); // we can now drop the syscall temp stack
    unsafe { asm!("\
        mov rsp, rbp // restore stack and registers for sysretq
        pop rbp
        pop r11
        pop rcx
        sysretq // back to userland"
        :::: "intel", "volatile"); }
}
