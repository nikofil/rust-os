#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]

extern crate rust_os;

extern "C" {
    static _stack_top: u32;
    static _ua64_mode_entry: u64;
    static _gdt64_pointer: u64;
}

#[naked]
#[no_mangle]
pub extern "C" fn _multiboot_check() {
    // check if we were loaded by Multiboot compliant bootloader
    unsafe {
        asm!("
            cmp eax, 0x36d76289
            jne .no_multiboot
            ret
        .no_multiboot:
            mov al, '0'
            jmp _boot_error
        " :::: "intel");
    }
}

#[naked]
#[no_mangle]
pub extern "C" fn _cpuid_check() {
    // check if processor supports CPUID
    unsafe {
        asm!("
        check_cpuid:
            // Check if CPUID is supported by attempting to flip the ID bit (bit 21)
            // in the FLAGS register. If we can flip it, CPUID is available.

            // Copy FLAGS in to EAX via stack
            pushf
            pop rax

            // Copy to ECX as well for comparing later on
            mov ecx, eax

            // Flip the ID bit
            xor eax, 1 << 21

            // Copy EAX to FLAGS via the stack
            push rax
            popf

            // Copy FLAGS back to EAX (with the flipped bit if CPUID is supported)
            pushf
            pop rax

            // Restore FLAGS from the old version stored in ECX (i.e. flipping the
            // ID bit back if it was ever flipped).
            push rcx
            popf

            // Compare EAX and ECX. If they are equal then that means the bit
            // wasn't flipped, and CPUID isn't supported.
            cmp eax, ecx
            je .no_cpuid
            ret
        .no_cpuid:
            mov al, '1'
            jmp _boot_error
        " :::: "intel");
    }
}

#[naked]
#[no_mangle]
pub extern "C" fn _long_mode_check() {
    // check if processor supports CPUID
    unsafe {
        asm!("
        check_long_mode:
            // test if extended processor info in available
            mov eax, 0x80000000    // implicit argument for cpuid
            cpuid                  // get highest supported argument
            cmp eax, 0x80000001    // it needs to be at least 0x80000001
            jb .no_long_mode       // if it's less, the CPU is too old for long mode

            // use extended info to test if long mode is available
            mov eax, 0x80000001    // argument for extended processor info
            cpuid                  // returns various feature bits in ecx and edx
            test edx, 1 << 29      // test if the LM-bit is set in the D-register
            jz .no_long_mode       // If it's not set, there is no long mode
            ret
        .no_long_mode:
            mov al, '2'
            jmp _boot_error
        " :::: "intel");
    }
}

#[naked]
#[no_mangle]
pub extern "C" fn _setup_page_table() {
    // setup page tables
    unsafe {
        asm!("
            // map first P4 entry to P3 table
            lea eax, _p3_table
            or eax, 3 // present + writable
            mov [_p4_table], eax

            // map first P3 entry to P2 table
            lea eax, _p2_table
            or eax, 3 // present + writable
            mov [_p3_table], eax

            // map each P2 entry to a huge 2MiB page
            mov ecx, 0         // counter variable

        .map_p2_table:
            // map ecx-th P2 entry to a huge page that starts at address 2MiB*ecx
            mov eax, 0x200000  // 2MiB
            mul ecx            // start address of ecx-th page
            or eax, 0x83 // present + writable + huge
            mov esi, ecx
            shl esi, 3
            lea edi, [_p2_table]
            add esi, edi
            mov dword ptr [rsi], eax     // map ecx-th entry
            inc ecx            // increase counter
            cmp ecx, 512       // if counter == 512, the whole P2 table is mapped
            jne .map_p2_table  // else map the next entry

            ret
        " :::: "intel");
    }
}

#[naked]
#[no_mangle]
pub extern "C" fn _enable_paging() {
    // enable paging through appropriate registers
    unsafe {
        asm!("
            // load P4 to cr3 register (cpu uses this to access the P4 table)
            mov rax, _p4_table
            mov cr3, rax
            xor rax, rax
            mov rax, cr3

            // enable PAE-flag in cr4 (Physical Address Extension)
            mov rax, cr4
            or eax, 1 << 5
            mov cr4, rax

            // set the long mode bit in the EFER MSR (model specific register)
            mov ecx, 0xC0000080
            rdmsr
            or eax, 1 << 8
            wrmsr

            // enable paging in the cr0 register
            mov rax, cr0
            or eax, 1 << 31
            mov cr0, rax

            ret
        " :::: "intel");
    }
}

#[naked]
#[no_mangle]
pub extern "C" fn _boot_error() -> ! {
    unsafe {
        asm!("
            mov dword ptr [0xb8000], 0x4f524f45
            mov dword ptr [0xb8004], 0x4f3a4f52
            mov dword ptr [0xb8008], 0x4f204f20
            mov byte  ptr [0xb800a], al
            hlt
        " :::: "intel");
    }
    loop {}
}

#[naked]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        // setup a stack
        asm!("
        lea esp, _stack_top
        " :::: "intel");
    }
    _multiboot_check();
    _cpuid_check();
    _long_mode_check();
    _setup_page_table();
    _enable_paging();
    unsafe {
        asm!("\
        lgdt _gdt64_pointer
        call _ua64_mode_entry
        " :::: "intel");
    }
    loop {}
}
