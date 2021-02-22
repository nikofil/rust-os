# rust-os

A very secure OS in Rust! Guaranteed to be unhackable due to no networking. Sometimes it crashes but what can you do.

## Features

### Memory management
Utilizes a buddy allocator, allowing the kernel to dynamically allocate memory after initializing (`buddy_alloc.rs`)!

Memory is divided into huge blocks (as large as the memory areas), and whenever a block of size `x` is allocated, we divide the current smallest block into two buddy blocks recursively until we have a block just large enough for `x` to fit. Then we return that block's virtual address.

Similarly, when two buddy blocks are freed, they are united into a larger block of double size.

Before the buddy allocator is initialized, a frame allocator is used (`frame_alloc.rs`) which doesn't reclaim the freed pages. That's mostly to hold the data structures (ie. vectors) that the buddy allocator needs.

The global allocator is declared in `global_alloc.rs` and allows for switching between the two implementations above.

### Virtual memory

A recursive page table is used to map physical to virtual memory (`mem.rs`). Each process has its own page table and is mapped by default to 0x400000 like the 32 bit processes of old. The higher half of the kernel starting at 0xC0000000 maps the entire physical memory (4 GiB).

### Multiprocessing

The Programmable Interrupt Timer is used with default settings to switch to the next task for preemptive multitasking. That means that around 18 times a second, an interrupt fires and the kernel switches tasks in a round-robin fashion. The context is saved and the context of the next process is restored, then the processor `iretq`s to change to usermode (`scheduler.rs`). Right now executables simply live in the kernel itself (`userspace.rs`) until a filesystem exists and are mapped to 0x400000 to be executed in usermode.

### User interaction

Stuff written on the keyboard causes an IRQ which is caught by the kernel (`interrupts.rs`). Currently the key pressed is simply written to the screen, so there's no real user interaction.

### System calls

System calls are supported using the fast syscall mechanism (`syscall` opcode). When called, the handler saves most registers (except floating-point stuff etc.), allocates a stack and uses that for executing the syscall. After that it restores registers and returns to userspace via `sysretq` (`syscalls.rs`).

### Faults / interrupts

An interrupt descriptor table is used to handle different kinds of interrupts / faults (`interrupts.rs`). Those that can be ignored are, while more serious ones (page faults, double faults, GPFs) cause a hang.

### Filesystem
TODO

## How to run

`make run`  
Might break between Rust toolchains :(  
Last tested with `rustc 1.50.0-nightly (1700ca07c 2020-12-08)`
