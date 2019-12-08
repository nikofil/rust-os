use crate::println;

// register for address of syscall handler
const MSR_STAR: usize = 0xc0000081;
const MSR_LSTAR: usize = 0xc0000082;

pub unsafe fn init_syscalls() {
    let handler_addr = handle_syscall as *const () as u64;
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
    println!("FUCK YEAH SYSCALLS");
    unsafe { asm!("add rsp, 0x400; pop r11; pop rcx; sysretq" :::: "intel"); }
}
