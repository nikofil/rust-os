use crate::gdt;

pub unsafe fn jmp_to_usermode(code: u64, stack_end: u64) {
    let (cs_idx, ds_idx) = gdt::set_usermode_segs();
    x86_64::instructions::tlb::flush_all();
    asm!("\
    push rax // stack segment
    push rsi // rsp
    pushfq   // rflags
    push rdx // code segment
    push rdi // ret to virtual addr
    iretq"
    :: "{rdi}"(code), "{rsi}"(stack_end), "{dx}"(cs_idx), "{ax}"(ds_idx) :: "intel", "volatile");
}

pub unsafe fn userspace_func() {
    asm!("\
    syscall
    syscall
    syscall
    syscall
    "::::: "intel", "volatile");
}
