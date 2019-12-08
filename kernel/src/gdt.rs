use crate::println;
use lazy_static::lazy_static;
use x86_64::instructions::segmentation::{set_cs, load_ds};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{PrivilegeLevel, VirtAddr};

pub const DOUBLE_FAULT_IST_INDEX: u8 = 0;
const STACK_SIZE: usize = 4096;
pub static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
pub static mut PRIV_TSS_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

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
    static ref USER_TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.privilege_stack_table[0] = {
            let stack_start = VirtAddr::from_ptr(unsafe { &PRIV_TSS_STACK });
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, [SegmentSelector; 5]) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_sel = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_sel = gdt.add_entry(Descriptor::tss_segment(&TSS));
        let mut user_code_sel = gdt.add_entry(Descriptor::user_code_segment());
        let mut user_data_sel = gdt.add_entry(Descriptor::user_data_segment());
        let mut user_tss_sel = gdt.add_entry(Descriptor::tss_segment(&USER_TSS));
        user_code_sel.0 |= PrivilegeLevel::Ring3 as u16;
        user_data_sel.0 |= PrivilegeLevel::Ring3 as u16;
        user_tss_sel.0 |= PrivilegeLevel::Ring3 as u16;
        (gdt, [code_sel, tss_sel, user_code_sel, user_data_sel, user_tss_sel])
    };
}

pub fn init_gdt() {
    GDT.0.load();
    let stack = unsafe { &STACK as *const _ };
    println!(
        " - Loaded GDT: {:p} TSS: {:p} Stack {:p} CS segment: {} TSS segment: {}",
        &GDT.0 as *const _, &*TSS as *const _, stack, GDT.1[0].0, GDT.1[1].0
    );
    unsafe {
        set_cs(GDT.1[0]);
        load_tss(GDT.1[1]);
    }
}

#[inline(always)]
pub unsafe fn set_usermode_segs() -> (u16, u16) {
    load_ds(GDT.1[3]);
    load_tss(GDT.1[4]);
    (GDT.1[2].0, GDT.1[3].0)
}
