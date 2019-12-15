use crate::serial_println;
use crate::gdt;
use crate::mem;
use lazy_static::lazy_static;
use alloc::vec::Vec;
use spin::Mutex;
use alloc::boxed::Box;
use core::fmt::Display;

#[derive(Debug, Clone)]
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
pub unsafe fn restore_context(ctxr: &Context) {
    asm!("mov rsp, $0;\
    pop rbp; pop rax; pop rbx; pop rcx; pop rdx; pop rsi; pop rdi; pop r8; pop r9;\
    pop r10; pop r11; pop r12; pop r13; pop r14; pop r15; iretq;"
    :: "r"(ctxr) :: "intel", "volatile");
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
    task_pt: Box<mem::PageTable>,
}

impl Task {
    pub fn new(exec_base: mem::VirtAddr, stack_end: mem::VirtAddr, _stack_vec: Vec<u8>, task_pt: Box<mem::PageTable>) -> Task {
        Task {
            ctx: None,
            exec_base,
            stack_end,
            _stack_vec,
            task_pt,
        }
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe {
            write!(f, "PT: {}, Context: {:x?}", self.task_pt.phys_addr(), self.ctx)
        }
    }
}

pub struct Scheduler {
    tasks: Mutex<Vec<Task>>,
    cur_task: Mutex<usize>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            tasks: Mutex::new(Vec::new()),
            cur_task: Mutex::new(usize::max_value()), // so that next task is 0
        }
    }

    pub unsafe fn schedule(&self, fn_addr: mem::VirtAddr) {
        let userspace_fn_phys = fn_addr.to_phys().unwrap().0;
        let page_phys_start = (userspace_fn_phys.addr() >> 12) << 12; // zero out page offset to get which page we should map
        let fn_page_offset = userspace_fn_phys.addr() - page_phys_start;
        let userspace_fn_virt_base = 0x400000;
        let userspace_fn_virt = userspace_fn_virt_base + fn_page_offset;
        serial_println!("Mapping {:x} to {:x}", page_phys_start, userspace_fn_virt_base);
        let mut task_pt = mem::PageTable::new();
        task_pt.map_virt_to_phys(
            mem::VirtAddr::new(userspace_fn_virt_base),
            mem::PhysAddr::new(page_phys_start),
            mem::BIT_PRESENT | mem::BIT_USER);
        task_pt.map_virt_to_phys(
            mem::VirtAddr::new(userspace_fn_virt_base).offset(0x1000),
            mem::PhysAddr::new(page_phys_start).offset(0x1000),
            mem::BIT_PRESENT | mem::BIT_USER);
        let mut stack_space: Vec<u8> = Vec::with_capacity(0x1000);
        let stack_space_phys = mem::VirtAddr::new(stack_space.as_mut_ptr() as *const u8 as u64).to_phys().unwrap().0;
        task_pt.map_virt_to_phys(
            mem::VirtAddr::new(0x800000),
            stack_space_phys,
            mem::BIT_PRESENT | mem::BIT_WRITABLE | mem::BIT_USER);
        let task = Task::new(mem::VirtAddr::new(userspace_fn_virt), mem::VirtAddr::new(0x801000), stack_space, task_pt);
        self.tasks.lock().push(task);
    }

    pub unsafe fn save_current_context(&self, ctxp: *const Context) {
        let ctx = &*ctxp;
        let cur_task = *self.cur_task.lock();
        self.tasks.lock()[cur_task].ctx = Some(ctx.clone());
    }

    pub unsafe fn run_next(&self) {
        let ctx: Option<Context>;
        let exec_base: mem::VirtAddr;
        let stack_end: mem::VirtAddr;
        {
            let next_task = (*self.cur_task.lock() + 1) % self.tasks.lock().len();
            *self.cur_task.lock() = next_task;
            let task = &self.tasks.lock()[next_task];
            serial_println!("Switching to task #{} ({})", next_task, task);
            ctx = task.ctx.clone();
            exec_base = task.exec_base.clone();
            stack_end = task.stack_end.clone();
            task.task_pt.enable();
        }
        if let Some(ctx) = ctx.as_ref() {
            restore_context(ctx);
        } else {
            jmp_to_usermode(exec_base, stack_end);
        }
    }
}

lazy_static! {
    pub static ref SCHEDULER: Scheduler = Scheduler::new();
}
