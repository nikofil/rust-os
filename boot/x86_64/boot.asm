extern ua64_mode_start

global _start

section .multiboot_header
header_start:
    dd 0xe85250d6                ; multiboot 2 magic number
    dd 0                         ; architecture 0 (protected mode i386)
    dd header_end - header_start ; header length
    dd 0x100000000 - (0xe85250d6 + 0 + (header_end - header_start)) ; checksum
    ; end tag
    dw 0 ; type
    dw 0 ; flags
    dd 8 ; size
header_end:

bits 32

_start:
    mov esp, _stack_top_low
    push ebx
    call _multiboot_check
    call _cpuid_check
    call _long_mode_check
    call _setup_page_table
    call _enable_paging
    mov eax, _gdt64_pointer_low
    lgdt [eax]
    pop ebx
    call _ua64_mode_entry

_boot_error:
    mov dword [0xb8000], 0x4f524f45
    mov dword [0xb8004], 0x4f3a4f52
    mov dword [0xb8008], 0x4f204f20
    mov byte  [0xb800a], al
    hlt

_multiboot_check:
    ; check if we were loaded by Multiboot compliant bootloader
        cmp eax, 0x36d76289
        jne .no_multiboot
        ret
    .no_multiboot:
        mov al, '0'
        jmp _boot_error

_cpuid_check:
    ; Check if CPUID is supported by attempting to flip the ID bit (bit 21)
    ; in the FLAGS register. If we can flip it, CPUID is available.

    ; Copy FLAGS in to EAX via stack
    pushfd
    pop eax

    ; Copy to ECX as well for comparing later on
    mov ecx, eax

    ; Flip the ID bit
    xor eax, 1 << 21

    ; Copy EAX to FLAGS via the stack
    push eax
    popfd

    ; Copy FLAGS back to EAX (with the flipped bit if CPUID is supported)
    pushfd
    pop eax

    ; Restore FLAGS from the old version stored in ECX (i.e. flipping the
    ; ID bit back if it was ever flipped).
    push ecx
    popfd

    ; Compare EAX and ECX. If they are equal then that means the bit
    ; wasn't flipped, and CPUID isn't supported.
    cmp eax, ecx
    je .no_cpuid
    ret
.no_cpuid:
    mov al, "1"
    jmp _boot_error

_long_mode_check:
    ; check if processor supports CPUID
    check_long_mode:
        ; test if extended processor info in available
        mov eax, 0x80000000    ; implicit argument for cpuid
        cpuid                  ; get highest supported argument
        cmp eax, 0x80000001    ; it needs to be at least 0x80000001
        jb .no_long_mode       ; if it's less, the CPU is too old for long mode

        ; use extended info to test if long mode is available
        mov eax, 0x80000001    ; argument for extended processor info
        cpuid                  ; returns various feature bits in ecx and edx
        test edx, 1 << 29      ; test if the LM-bit is set in the D-register
        jz .no_long_mode       ; If it's not set, there is no long mode
        ret
    .no_long_mode:
        mov al, '2'
        jmp _boot_error

_setup_page_table:
    ; map first P4 entry to P3 table
        mov eax, _p3_table_low
        or eax, 7 ; present + writable + user
        mov [_p4_table_low], eax

        ; map first P3 entry to P2 table
        mov eax, _p2_table_0_low
        or eax, 3 ; present + writable
        mov [_p3_table_low], eax
        mov [_p3_table_low + 24], eax
        add eax, 4096
        mov [_p3_table_low + 32], eax
        add eax, 4096
        mov [_p3_table_low + 40], eax
        add eax, 4096
        mov [_p3_table_low + 48], eax

        ; map each P2 entry to a huge 2MiB page
        mov ecx, 0         ; counter variable

    .map_p2_table:
        ; map ecx-th P2 entry to a huge page that starts at address 2MiB*ecx
        mov eax, 0x200000  ; 2MiB
        mul ecx            ; start address of ecx-th page
        or eax, 0x83 ; present + writable + huge
        mov esi, ecx
        shl esi, 3
        lea edi, [_p2_table_0_low]
        add esi, edi
        mov dword [esi], eax     ; map ecx-th entry
        inc ecx            ; increase counter
        cmp ecx, 2048      ; if counter == 512, the whole P2 table is mapped
        jne .map_p2_table  ; else map the next entry

        ret

_enable_paging:
    ; load P4 to cr3 register (cpu uses this to access the P4 table)
    mov eax, _p4_table_low
    mov cr3, eax
    xor eax, eax
    mov eax, cr3

    ; enable PAE-flag in cr4 (Physical Address Extension)
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; set the long mode bit in the EFER MSR (model specific register)
    ; also enable System Call Extensions (SCE) to be able to use the syscall opcode
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1
    or eax, 1 << 8
    wrmsr

    ; enable paging in the cr0 register
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    ret

_ua64_mode_entry:
    mov edx, 0xC0000000
    add esp, edx
    mov eax, _ua64_mode_entry_high
    jmp eax
    _ua64_mode_entry_high:
    mov dword [_p3_table], 0
    mov edi, ebx
    jmp _gdt64_code_off:ua64_mode_start

section .rodata
gdt64:
    dq 0 ; zero entry
_gdt64_code_off: equ $ - gdt64 ; new
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
_gdt64_pointer:
    dw $ - gdt64 - 1
    dq gdt64
_gdt64_pointer_low: equ _gdt64_pointer - 0xC0000000

section .bss
align 4096
_p4_table:
    resb 4096
_p3_table:
    resb 4096
_p2_table_0:
    resb 4096
_p2_table_1:
    resb 4096
_p2_table_2:
    resb 4096
_p2_table_3:
    resb 4096
_stack_bottom:
    resb 1024*40
_stack_top:

_p4_table_low: equ _p4_table - 0xC0000000
_p3_table_low: equ _p3_table - 0xC0000000
_p2_table_0_low: equ _p2_table_0 - 0xC0000000
_p2_table_1_low: equ _p2_table_1 - 0xC0000000
_p2_table_2_low: equ _p2_table_2 - 0xC0000000
_p2_table_3_low: equ _p2_table_3 - 0xC0000000
_stack_top_low: equ _stack_top - 0xC0000000
