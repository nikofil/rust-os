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
        mov rbx, 1
        mov rbp, 2
        mov r12, 3
        mov r13, 4
        mov r14, 5
        mov r15, 6
        xx2:
        mov rax, 0x0
        xx1:
        inc rax
        cmp rax, 0x10000000
        jnz xx1
        syscall
        jmp xx2
    ":::: "volatile", "intel");
}

/*
iretq to outer pops:
rip cs rflags rsp ss

in irq convention the following are saved by the handler as they are caller-saved normally
but the caller doesn't know the irq happened
rax rcx rdx rsi rdi r8 r9 r10 r11

callee-saved should not change anyway - they should be restored by the compiler normally
however for context switch we must also save them
rbx rbp r12 r13 r14 r15
*/
