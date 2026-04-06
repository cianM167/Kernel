use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;
use x86_64::PrivilegeLevel;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::PageFaultErrorCode;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::{hlt_loop, print};
use crate::println;
use crate::gdt;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static TIMER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe {
    ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)
});

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        idt.breakpoint.set_handler_fn(breakpoint_handler)
            .set_privilege_level(PrivilegeLevel::Ring3);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        unsafe {
            idt.general_protection_fault.set_handler_fn(general_protection_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

            idt.stack_segment_fault.set_handler_fn(unknown_exception)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::Timer.as_u8()]
            .set_handler_fn(timer_interrupt_handler);

        idt[InterruptIndex::Keyboard.as_u8()]
            .set_handler_fn(keyboard_interrupt_handler);

        idt[0].set_handler_fn(unknown_exception_no_error);  // Divide by zero
        idt[6].set_handler_fn(unknown_exception_no_error);  // Invalid opcode

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame
) {
    TIMER.fetch_add(1, Ordering::Relaxed);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame,
) {
    use x86_64::instructions::port::Port;
    
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::task::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame,
) {
    loop {}
    // println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT (O_O)!\nSHITTING THE BED\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64
) {
    println!("GPF! Error code = {:#x}, RIP={:#x}", error_code, stack_frame.instruction_pointer.as_u64());
    hlt_loop();
}

extern "x86-interrupt" fn invalid_opcode_handler(
    stack_frame: InterruptStackFrame
) {
    println!("Invalid opcode at RIP={:#x}", stack_frame.instruction_pointer.as_u64());
    hlt_loop();
}


extern "x86-interrupt" fn unknown_exception(
    stack_frame: InterruptStackFrame, 
    error_code: u64
) {
    println!("Unknown exception!");
    println!("RIP={:#x}", stack_frame.instruction_pointer.as_u64());
    println!("CS={:?}", stack_frame.code_segment);
    println!("RFLAGS={:#x}", stack_frame.cpu_flags);
    println!("Error code={:#x}", error_code);
    hlt_loop();
}

extern "x86-interrupt" fn unknown_exception_no_error(stack_frame: InterruptStackFrame) {
    println!("Unknown exception (no error code)!");
    println!("RIP={:#x}", stack_frame.instruction_pointer.as_u64());
    hlt_loop();
}

#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}