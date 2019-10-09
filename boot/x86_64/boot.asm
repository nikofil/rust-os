extern long_mode_start
extern ua64_mode_start

global _stack_top
global _p2_table
global _p3_table
global _p4_table
global _gdt64_code_off
global _gdt64_pointer
global _ua64_mode_entry

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

_ua64_mode_entry:
mov edx, 0xC0000000
add esp, edx
mov eax, _ua64_mode_entry_high
add eax, edx
jmp eax
_ua64_mode_entry_high:
mov dword [_p3_table], 0
jmp _gdt64_code_off:ua64_mode_start + 0xC0000000

section .rodata
gdt64:
    dq 0 ; zero entry
_gdt64_code_off: equ $ - gdt64 ; new
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
_gdt64_pointer:
    dw $ - gdt64 - 1
    dq gdt64

section .bss
align 4096
_p4_table:
    resb 4096
_p3_table:
    resb 4096
_p2_table:
    resb 4096
_stack_bottom:
    resb 1024*40
_stack_top:
