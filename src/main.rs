#![no_std]
#![no_main]
#![feature(asm)]
#![feature(naked_functions)]

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
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
    _multiboot_check();
    _cpuid_check();
    _long_mode_check();
    unsafe {
        asm!("\
        mov dword ptr [0xb8000], 0x2f402f41
        hlt
        " : : : : "intel");
    }
    let vga_buffer = 0xb8000 as *mut u8;
    let hello: &[u8] = b"Hello world!";
    for (i, &byte) in hello.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0x0b;
        }
    }
    loop {}
}


