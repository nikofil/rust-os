use crate::println;
use lazy_static::lazy_static;

extern crate x86_64;
use x86_64::structures::idt::InterruptStackFrame;
use x86_64::instructions::tables::{lidt, DescriptorTablePointer};
use x86_64::instructions::segmentation;
use x86_64::structures::gdt::SegmentSelector;
use core::mem::size_of;

type IDTHandler = extern "x86-interrupt" fn() -> !;

extern "C" fn div_by_zero() -> ! {
    // println!("div by zero!");
    loop {}
}

lazy_static! {
    static ref INTERRUPT_TABLE: InterruptDescriptorTable = InterruptDescriptorTable([
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
        IDTEntry::new(div_by_zero as *const IDTHandler, segmentation::cs(), 0, true, 0),
    ]);
}

#[repr(C, packed)]
struct InterruptDescriptorTable([IDTEntry; 20]);

impl InterruptDescriptorTable {
    fn load(&'static self) {
        let idt_ptr = DescriptorTablePointer {
            base: &self as *const _ as u64,
            limit: (size_of::<Self>() - 1) as u16
        };
        println!("Setting up IDT with {} entries", INTERRUPT_TABLE.0.len());
        println!("idt ptr: {:x}", &idt_ptr as *const _ as u64);
        println!("Table: {:x}", &INTERRUPT_TABLE.0 as *const _ as u64);
        println!("Entry 0: {:x}", div_by_zero as *const IDTHandler as u64);
        println!("Len: {:x}", INTERRUPT_TABLE.0.len());
        unsafe {
            lidt(&idt_ptr);
        }
        println!("Done");
    }
}

#[repr(C, packed)]
struct IDTEntry {
    handler_low: u16,
    gdt_selector: SegmentSelector,
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
        println!("ptr {:p} u64 {:x} low {:x} mid {:x} hi {:x} options {:x} gdt {:x}", handler, handler_ptr, handler_low, handler_mid, handler_hi, options, gdt_selector.index());
        IDTEntry { handler_low, handler_mid, handler_hi, options, gdt_selector, reserved: 0 }
    }
}

pub fn setup_idt() {
    INTERRUPT_TABLE.load();
}
