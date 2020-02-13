pub unsafe fn userspace_prog_1() {
    asm!("\
        mov rbx, 0xf0000000
        xx2:
        push 0x595ca11a
        mov rbp, 0
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
        xx1:
        inc rax
        cmp rax, 0x40000
        jnz xx1
        mov rdi, rsp
        pop rax
        inc rbx
        mov rsi, rbx
        syscall
        jmp xx2
    ":::: "volatile", "intel");
}

pub unsafe fn userspace_prog_2() {
    asm!("\
        mov rbx, 0
        xx4:
        push 0x595ca11b
        mov rbp, 100
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
        xx3:
        inc rax
        cmp rax, 0x40000
        jnz xx3
        mov rdi, rsp
        pop rax
        inc rbx
        mov rsi, rbx
        syscall
        jmp xx4
    ":::: "volatile", "intel");
}
