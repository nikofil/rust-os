use core::arch::naked_asm;

#[naked]
pub unsafe extern "C" fn userspace_prog_1() {
    /*
    error if named labels used in inline asm:
    = note: `#[deny(named_asm_labels)]` on by default
    = help: only local labels of the form `<number>:` should be used in inline asm
    https://doc.rust-lang.org/unstable-book/library-features/asm.html#labels
    */
    
    naked_asm!("\
        mov rbx, 0xf0000000
        2: // prog1 start
        push 0x595ca11a // keep the syscall number in the stack
        mov rbp, 0x0 // distinct values for each register
        mov rax, 0x1
        mov rcx, 0x3
        mov rdx, 0x4
        mov rdi, 0x6
        mov r8, 0x7
        mov r9, 0x8
        mov r10, 0x9
        mov r11, 0x10
        mov r12, 0x11
        mov r13, 0x12
        mov r14, 0x13
        mov r15, 0x14
        xor rax, rax
        3: //prog 1 loop
        inc rax
        cmp rax, 0x4000000
        jnz 3b // loop for some milliseconds
        pop rax // pop syscall number from the stack
        inc rbx // increase loop counter
        mov rdi, rsp // first syscall arg is rsp
        mov rsi, rbx // second syscall arg is the loop counter
        syscall // perform the syscall!
        jmp 2b // do it all over
    ");
}

#[naked]
pub unsafe extern "C" fn userspace_prog_2() {
    naked_asm!("\
        mov rbx, 0
        4: // prog2start
        push 0x595ca11b // keep the syscall number in the stack
        mov rbp, 0x100 // distinct values for each register
        mov rax, 0x101
        mov rcx, 0x103
        mov rdx, 0x104
        mov rdi, 0x106
        mov r8, 0x107
        mov r9, 0x108
        mov r10, 0x109
        mov r11, 0x110
        mov r12, 0x111
        mov r13, 0x112
        mov r14, 0x113
        mov r15, 0x114
        xor rax, rax
        5: //prog2loop
        inc rax
        cmp rax, 0x4000000
        jnz 5b // loop for some milliseconds
        pop rax // pop syscall number from the stack
        inc rbx // increase loop counter
        mov rdi, rsp // first syscall arg is rsp
        mov rsi, rbx // second syscall arg is the loop counter
        syscall // perform the syscall!
        jmp 4b // do it all over
    ");
}

#[naked]
pub unsafe extern "C" fn userspace_prog_hello() {
    naked_asm!("\
            42:
            mov rax, 0x42 // syscall number in rax
            mov rdi, rsp // first syscall arg is rsp
            mov rsi, 0 // second syscall arg is some number

            xor rcx,rcx
            43: // make a loop so it doesn go forever?
            inc rcx
            cmp rcx, 0x4000000
            jnz 43b //loop for some milliseconds

            syscall // perform the syscall!
            jmp 42b // 1 for the label 1: , b for before (the one closet before)
        ");
}
