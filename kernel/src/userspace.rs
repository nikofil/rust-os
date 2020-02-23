pub unsafe fn userspace_prog_1() {
    asm!("\
        mov rbx, 0xf0000000
        prog1start:
        push 0x595ca11a // keep the syscall number in the stack
        mov rbp, 0 // distinct values for each register
        mov rax, 1
        mov rcx, 3
        mov rdx, 4
        mov rdi, 6
        mov r8, 7
        mov r9, 8
        mov r10, 9
        mov r11, 10
        mov r12, 11
        mov r13, 12
        mov r14, 13
        mov r15, 14
        mov rax, 0x0
        prog1loop:
        inc rax
        cmp rax, 0x4000000
        jnz prog1loop // loop for some milliseconds
        mov rdi, rsp // first syscall arg is rsp
        pop rax // pop syscall number from the stack
        inc rbx // increase loop counter
        mov rsi, rbx // second syscall arg is the loop counter
        syscall // perform the syscall!
        jmp prog1start // do it all over
    ":::: "volatile", "intel");
}

pub unsafe fn userspace_prog_2() {
    asm!("\
        mov rbx, 0
        prog2start:
        push 0x595ca11b // keep the syscall number in the stack
        mov rbp, 100 // distinct values for each register
        mov rax, 101
        mov rcx, 103
        mov rdx, 104
        mov rdi, 106
        mov r8, 107
        mov r9, 108
        mov r10, 109
        mov r11, 110
        mov r12, 111
        mov r13, 112
        mov r14, 113
        mov r15, 114
        mov rax, 0x0
        prog2loop:
        inc rax
        cmp rax, 0x4000000
        jnz prog2loop // loop for some milliseconds
        mov rdi, rsp // first syscall arg is rsp
        pop rax // pop syscall number from the stack
        inc rbx // increase loop counter
        mov rsi, rbx // second syscall arg is the loop counter
        syscall // perform the syscall!
        jmp prog2start // do it all over
    ":::: "volatile", "intel");
}
