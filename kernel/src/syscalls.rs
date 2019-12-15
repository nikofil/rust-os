use crate::println;

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
    unsafe { asm!("push rcx; push r11; sub rsp, 0x400" :::: "intel"); }
    let syscall: u64;
    let arg0: u64;
    let arg1: u64;
    let arg2: u64;
    let arg3: u64;
    unsafe { asm!("nop" : "={rax}"(syscall), "={rdi}"(arg0), "={rsi}"(arg1), "={rdx}"(arg2), "={r10}"(arg3) ::: "intel"); }
    println!("syscall {} {} {} {} {}", syscall, arg0, arg1, arg2, arg3);
    unsafe { asm!("add rsp, 0x400; pop r11; pop rcx; sysretq" :::: "intel"); }
}
