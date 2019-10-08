use crate::println;
use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::gdt::{GlobalDescriptorTable, SegmentSelector, Descriptor};
use x86_64::instructions::segmentation::set_cs;
use x86_64::instructions::tables::load_tss;

pub const DOUBLE_FAULT_IST_INDEX: u8 = 0;
const STACK_SIZE: usize = 4096;
pub static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, [SegmentSelector; 2]) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_sel = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_sel = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, [code_sel, tss_sel])
    };
}

pub fn init_gdt() {
    GDT.0.load();
    let stack = unsafe { &STACK as *const _ };
    println!(" - Loaded GDT: {:p} TSS: {:p} Stack {:p} CS segment: {} TSS segment: {}",
        &GDT.0 as *const _,
        &*TSS as *const _,
        stack,
        GDT.1[0].0,
        GDT.1[1].0);
    unsafe {
        set_cs(GDT.1[0]);
        load_tss(GDT.1[1]);
    }
}
