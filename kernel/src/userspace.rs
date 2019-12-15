pub unsafe fn userspace_prog_1() {
    asm!("\
        xx2:
        mov rbp, 0
        mov rax, 1
        mov rbx, 2
        mov rcx, 3
        mov rdx, 4
        mov rsi, 5
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
        cmp rax, 0x4000000
        jnz xx1
        mov rax, 666
        syscall
        jmp xx2
    ":::: "volatile", "intel");
}

pub unsafe fn userspace_prog_2() {
    asm!("\
        xx4:
        mov rbp, 100
        mov rax, 101
        mov rbx, 102
        mov rcx, 103
        mov rdx, 104
        mov rsi, 105
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
        cmp rax, 0x4000000
        jnz xx3
        mov rax, 999
        syscall
        jmp xx4
    ":::: "volatile", "intel");
}
