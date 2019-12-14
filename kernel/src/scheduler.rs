use crate::serial_println;
use crate::gdt;
use crate::mem;
use alloc::vec::Vec;

#[derive(Debug)]
pub struct Context {
    pub rbp: u64,
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[naked]
#[inline(always)]
pub unsafe fn get_context() -> *const Context {
    let ctxp: *const Context;
    asm!("push r15; push r14; push r13; push r12; push r11; push r10; push r9;\
    push r8; push rdi; push rsi; push rdx; push rcx; push rbx; push rax; push rbp;\
    mov $0, rsp; sub rsp, 0x400;"
    : "=r"(ctxp) ::: "intel", "volatile");
    ctxp
}

#[naked]
#[inline(always)]
pub unsafe fn set_context(ctxp: *const Context) {
    asm!("mov rsp, $0;\
    pop rbp; pop rax; pop rbx; pop rcx; pop rdx; pop rsi; pop rdi; pop r8; pop r9;\
    pop r10; pop r11; pop r12; pop r13; pop r14; pop r15; iretq;"
    :: "r"(ctxp) :: "intel", "volatile");
}

pub unsafe fn jmp_to_usermode(code: mem::VirtAddr, stack_end: mem::VirtAddr) {
    let (cs_idx, ds_idx) = gdt::set_usermode_segs();
    x86_64::instructions::tlb::flush_all();
    asm!("\
    push rax // stack segment
    push rsi // rsp
    pushfq   // rflags
    push rdx // code segment
    push rdi // ret to virtual addr
    iretq"
    :: "{rdi}"(code.addr()), "{rsi}"(stack_end.addr()), "{dx}"(cs_idx), "{ax}"(ds_idx) :: "intel", "volatile");
}

struct Task {
    ctx: Option<Context>,
    exec_base: mem::VirtAddr,
    stack_end: mem::VirtAddr,
    _stack_vec: Vec<u8>,
}

impl Task {
    pub fn new(exec_base: mem::VirtAddr, stack_end: mem::VirtAddr, _stack_vec: Vec<u8>) -> Task {
        Task {
            ctx: None,
            exec_base,
            stack_end,
            _stack_vec
        }
    }
}

pub struct Scheduler {
    tasks: Vec<Task>,
    cur_task: usize,
    map_offset: u64,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            tasks: Vec::new(),
            cur_task: 0,
            map_offset: 0,
        }
    }

    pub unsafe fn schedule(&mut self, fn_addr: mem::VirtAddr) {
        let map_offset = self.map_offset;
        self.map_offset += 0x100000;
        let userspace_fn_phys = fn_addr.to_phys().unwrap().0;
        let page_phys_start = (userspace_fn_phys.addr() >> 12) << 12; // zero out page offset to get which page we should map
        let fn_page_offset = userspace_fn_phys.addr() - page_phys_start;
        let userspace_fn_virt_base = 0x400000 + map_offset;
        let userspace_fn_virt = userspace_fn_virt_base + fn_page_offset;
        serial_println!("Mapping {:x} to {:x}", page_phys_start, userspace_fn_virt_base);
        mem::get_page_table().map_virt_to_phys(
            mem::VirtAddr::new(userspace_fn_virt_base),
            mem::PhysAddr::new(page_phys_start),
            mem::BIT_PRESENT | mem::BIT_USER);
        mem::get_page_table().map_virt_to_phys(
            mem::VirtAddr::new(userspace_fn_virt_base).offset(0x1000),
            mem::PhysAddr::new(page_phys_start).offset(0x1000),
            mem::BIT_PRESENT | mem::BIT_USER);
        let mut stack_space: Vec<u8> = Vec::with_capacity(0x1000);
        let stack_space_phys = mem::VirtAddr::new(stack_space.as_mut_ptr() as *const u8 as u64).to_phys().unwrap().0;
        mem::get_page_table().map_virt_to_phys(
            mem::VirtAddr::new(0x800000 + map_offset),
            stack_space_phys,
            mem::BIT_PRESENT | mem::BIT_WRITABLE | mem::BIT_USER);
        let task = Task::new(mem::VirtAddr::new(userspace_fn_virt), mem::VirtAddr::new(0x801000), stack_space);
        self.tasks.push(task);
    }

    pub unsafe fn run_next(&mut self) {
        self.cur_task = (self.cur_task + 1) % self.tasks.len();
        serial_println!("Scheduling task #{}", self.cur_task);
        let task = &self.tasks[self.cur_task];
        jmp_to_usermode(task.exec_base, task.stack_end);
    }
}
