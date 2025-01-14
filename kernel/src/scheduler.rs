use core::arch::asm;
use crate::gdt;
use crate::mem;
use crate::port;
use crate::serial_println;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Display;
use core::pin::Pin;
use lazy_static::lazy_static;
use spin::Mutex;

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

#[inline(always)]
pub unsafe fn get_context() -> *const Context {
    let ctxp: *const Context;
    asm!("push r15; push r14; push r13; push r12; push r11; push r10; push r9;\
    push r8; push rdi; push rsi; push rdx; push rcx; push rbx; push rax; push rbp;\
    mov {}, rsp; sub rsp, 0x400;",
    out(reg) ctxp);
    ctxp
}

#[inline(always)]
pub unsafe fn restore_context(ctxr: &Context) {
    asm!("mov rsp, {};\
    pop rbp; pop rax; pop rbx; pop rcx; pop rdx; pop rsi; pop rdi; pop r8; pop r9;\
    pop r10; pop r11; pop r12; pop r13; pop r14; pop r15; iretq;",
    in(reg) ctxr);
}

#[inline(never)]
pub unsafe fn jmp_to_usermode(code: mem::VirtAddr, stack_end: mem::VirtAddr) {
    let (cs_idx, ds_idx) = gdt::set_usermode_segs();
    x86_64::instructions::tlb::flush_all(); // flush the TLB after address-space switch
    asm!("\
    push rax   // stack segment
    push rsi   // rsp
    push 0x200 // rflags (only interrupt bit set)
    push rdx   // code segment
    push rdi   // ret to virtual addr
    iretq",
    in("rdi") code.addr(), in("rsi") stack_end.addr(), in("dx") cs_idx, in("ax") ds_idx);
}

#[derive(Clone, Debug)]
enum TaskState {
    // a task's state can either be
    SavedContext(Context),                      // a saved context
    StartingInfo(mem::VirtAddr, mem::VirtAddr), // or a starting instruction and stack pointer
}

pub struct Task {
    state: TaskState,             // the current state of the task
    task_pt: Box<mem::PageTable>, // the page table for this task
    _data_bytes: Pin<Box<[u8]>>,          // a vector to keep the task's data to be mapped
    _stack_bytes: Pin<Box<[u8]>>,          // a vector to keep the task's stack space
}

impl Task {
    pub fn new(
        exec_base: mem::VirtAddr,
        stack_end: mem::VirtAddr,
        task_pt: Box<mem::PageTable>,
        _data_bytes: Pin<Box<[u8]>>,
        _stack_bytes: Pin<Box<[u8]>>,
    ) -> Task {
        // ask for the vecs to be pinned as we take the pointer to the data above
        // and we don't want the data to be moved around in physical memory while we've mapped it to virtual memory
        Task {
            state: TaskState::StartingInfo(exec_base, stack_end),
            task_pt,
            _data_bytes,
            _stack_bytes,
        }
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe {
            write!(
                f,
                "PT: {}, Context: {:x?}",
                self.task_pt.phys_addr(),
                self.state
            )
        }
    }
}

pub struct Scheduler {
    tasks: Mutex<Vec<Task>>,
    cur_task: Mutex<Option<usize>>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            tasks: Mutex::new(Vec::new()),
            cur_task: Mutex::new(None), // so that next task is 0
        }
    }

    pub unsafe fn schedule_data(&self, prog_data: Vec<u8>, entry_offset: usize) {
        let prog_bytes = Pin::new(prog_data.into_boxed_slice());
        let fn_addr = mem::VirtAddr::new(prog_bytes.as_ptr() as usize);
        let userspace_fn_phys = fn_addr.to_phys().unwrap().0; // virtual address to physical
        let page_phys_start = (userspace_fn_phys.addr() >> 12) << 12; // zero out page offset to get which page we should map
        let fn_page_offset = userspace_fn_phys.addr() - page_phys_start; // offset of function from page start
        let userspace_fn_virt_base = 0x400000; // target virtual address of page
        let userspace_fn_virt = userspace_fn_virt_base + fn_page_offset + entry_offset; // target virtual address of function
        serial_println!(
            "Mapping {:x} to {:x}",
            page_phys_start,
            userspace_fn_virt_base
        );
        let mut task_pt = mem::PageTable::new(); // copy over the kernel's page tables
        task_pt.map_virt_to_phys(
            mem::VirtAddr::new(userspace_fn_virt_base),
            mem::PhysAddr::new(page_phys_start),
            mem::BIT_PRESENT | mem::BIT_USER,
        ); // map the program's code
        task_pt.map_virt_to_phys(
            mem::VirtAddr::new(userspace_fn_virt_base).offset(mem::FRAME_SIZE),
            mem::PhysAddr::new(page_phys_start).offset(mem::FRAME_SIZE),
            mem::BIT_PRESENT | mem::BIT_USER,
        ); // also map another page to be sure we got the entire function in
        let mut stack_space = Pin::new(Box::new([0u8; mem::FRAME_SIZE])); // allocate some pinned memory to use for the stack
        let stack_space_phys = mem::VirtAddr::new(stack_space.as_mut_ptr() as *const u8 as usize)
            .to_phys()
            .unwrap()
            .0;
        // take physical address of stack
        task_pt.map_virt_to_phys(
            mem::VirtAddr::new(0x800000),
            stack_space_phys,
            mem::BIT_PRESENT | mem::BIT_WRITABLE | mem::BIT_USER,
        ); // map the stack memory to 0x800000
        let task = Task::new(
            mem::VirtAddr::new(userspace_fn_virt),
            mem::VirtAddr::new(0x801000),
            task_pt,
            prog_bytes,
            stack_space,
        ); // create task struct
        self.schedule_task(task); // schedule the task
    }


    pub fn schedule_task(&self, task: Task) {
        self.tasks.lock().push(task); // push task struct to list of tasks
    }

    pub unsafe fn save_current_context(&self, ctxp: *const Context) {
        self.cur_task.lock().map(|cur_task_idx| {
            // if there is a current task
            let ctx = (*ctxp).clone();
            self.tasks.lock()[cur_task_idx].state = TaskState::SavedContext(ctx);
            // replace its context with the given one
        });
    }

    pub unsafe fn run_next(&self) {
        let tasks_len = self.tasks.lock().len(); // how many tasks are available
        if tasks_len > 0 {
            let task_state = {
                let mut cur_task_opt = self.cur_task.lock(); // lock the current task index
                let cur_task = cur_task_opt.get_or_insert(0); // default to 0
                let next_task = (*cur_task + 1) % tasks_len; // next task index
                *cur_task = next_task;
                let task = &self.tasks.lock()[next_task]; // get the next task
                serial_println!("Switching to task #{} ({})", next_task, task);
                task.task_pt.enable(); // enable task's page table
                task.state.clone() // clone task state information
            }; // release held locks
            match task_state {
                TaskState::SavedContext(ctx) => {
                    restore_context(&ctx) // either restore the saved context
                }
                TaskState::StartingInfo(exec_base, stack_end) => {
                    jmp_to_usermode(exec_base, stack_end) // or initialize the task with the given instruction, stack pointers
                }
            }
        }
        loop {} // no task to jump to
    }
}

lazy_static! {
    pub static ref SCHEDULER: Scheduler = Scheduler::new();
}

pub unsafe extern "sysv64" fn context_switch(ctx: *const Context) {
    SCHEDULER.save_current_context(ctx);
    port::end_of_interrupt(32);
    SCHEDULER.run_next();
}