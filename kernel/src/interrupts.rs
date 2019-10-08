use crate::println;
use lazy_static::lazy_static;

extern crate x86_64;
use x86_64::structures::idt::InterruptStackFrame;
use x86_64::instructions::tables::{lidt, DescriptorTablePointer};
use x86_64::instructions::segmentation;
use x86_64::structures::gdt::SegmentSelector;
use core::mem::size_of;

type IDTHandler = extern "x86-interrupt" fn();

extern "x86-interrupt" fn div_by_zero(stack_frame: &mut InterruptStackFrame) {
    println!("div by zero! {:?}", stack_frame);
}

extern "x86-interrupt" fn page_fault(stack_frame: &mut InterruptStackFrame, err_code: u64) {
    println!("page fault! err code: {} {:?}", err_code, stack_frame);
    loop {}
}

extern "x86-interrupt" fn double_fault(stack_frame: &mut InterruptStackFrame, err_code: u64) {
    println!("double fault! err code: {} {:?}", err_code, stack_frame);
    loop {}
}

lazy_static! {
    static ref INTERRUPT_TABLE: InterruptDescriptorTable = {
        let mut vectors = [
            IDTEntry::empty(); 0x100
        ];
        macro_rules! idt_entry {
            ($i:literal, $e:expr) => { vectors[$i] = IDTEntry::new($e as *const IDTHandler, segmentation::cs(), 0, true, 0); }
        }
        idt_entry!(0, div_by_zero);
        vectors[8] = IDTEntry::new(double_fault as *const IDTHandler, segmentation::cs(), crate::gdt::DOUBLE_FAULT_IST_INDEX + 1, true, 0);
        idt_entry!(14, page_fault);
        InterruptDescriptorTable(vectors)
    };
}

#[repr(C, packed)]
struct InterruptDescriptorTable([IDTEntry; 0x100]);

impl InterruptDescriptorTable {
    fn load(&'static self) {
        let idt_ptr = DescriptorTablePointer {
            base: self as *const _ as u64,
            limit: (size_of::<Self>() - 1) as u16
        };
        println!(" - Setting up IDT with {} entries", INTERRUPT_TABLE.0.len());
        println!(" - IDT ptr address: {:x}", &idt_ptr as *const _ as u64);
        println!(" - IDT address: {:x}", &INTERRUPT_TABLE.0 as *const _ as u64);
        unsafe {
            lidt(&idt_ptr);
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct IDTEntry {
    handler_low: u16,
    gdt_selector: u16,
    options: u16,
    handler_mid: u16,
    handler_hi: u32,
    reserved: u32,
}

impl IDTEntry {
    fn new(handler: *const IDTHandler, gdt_selector: SegmentSelector, int_stack_idx: u8, disable_interrupts: bool, dpl_priv: u8) -> IDTEntry {
        let mut options: u16 = int_stack_idx as u16 & 0b111;
        if !disable_interrupts {
            options |= 1 << 8;
        }
        options |= 1 << 9;
        options |= 1 << 10;
        options |= 1 << 11;
        options |= (dpl_priv as u16 & 0b11) << 13;
        options |= 1 << 15;
        let handler_ptr = handler as u64;
        let handler_low = (handler_ptr & 0xFFFF) as u16;
        let handler_mid = ((handler_ptr >> 16) & 0xFFFF) as u16;
        let handler_hi = (handler_ptr >> 32) as u32;
        let gdt_selector = gdt_selector.0;
        IDTEntry { handler_low, handler_mid, handler_hi, options, gdt_selector, reserved: 0 }
    }

    fn empty() -> IDTEntry {
        IDTEntry { handler_low: 0, handler_mid: 0, handler_hi: 0, options: 0, gdt_selector: segmentation::cs().0, reserved: 0 }
    }
}

pub fn setup_idt() {
    INTERRUPT_TABLE.load();
}
