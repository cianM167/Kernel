use core::cell::UnsafeCell;

use spin::Mutex;
use x86_64::{VirtAddr};
use x86_64::structures::tss::TaskStateSegment;
use lazy_static::lazy_static;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor};
use x86_64::structures::gdt::SegmentSelector;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

pub struct Selectors {
    pub data_selector: SegmentSelector,
    pub code_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
}

pub struct StaticTss(UnsafeCell<TaskStateSegment>);
unsafe impl Sync for StaticTss {}

lazy_static! {
    pub static ref TSS: StaticTss = StaticTss(UnsafeCell::new({
        let mut tss = TaskStateSegment::new();

        // kernel stack for syscalls
        static mut KERNEL_STACK: [u8; 4096 * 5] = [0; 4096 * 5];

        let stack_start = VirtAddr::from_ptr(&raw const KERNEL_STACK);
        let stack_end = stack_start + 4096 * 5;

        tss.privilege_stack_table[0] = stack_end;

        // double fault stack
        static mut DF_STACK: [u8; 4096 * 5] = [0; 4096 * 5];

        let df_start = VirtAddr::from_ptr(&raw const DF_STACK);
        let df_end = df_start + 4096 * 5;

        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = df_end;
        
        tss
    }));
}

pub fn set_rsp0(addr: VirtAddr) {
    // only called during scheduler switches not to be used w concurrency

    unsafe {
        (*TSS.0.get()).privilege_stack_table[0] = addr;
    }
}

lazy_static! {
    pub static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let data_selector = gdt.append(Descriptor::kernel_data_segment());

        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());

        let tss_selector = unsafe {
            gdt.append(Descriptor::tss_segment(&*TSS.0.get()))
        };

        (
            gdt,
            Selectors {
                data_selector,
                code_selector,
                tss_selector,
                user_data: user_data_selector,
                user_code: user_code_selector,
            }
        )
    };
}

pub fn init() {
    use x86_64::instructions::tables::load_tss;
    use x86_64::instructions::segmentation::{CS, Segment};

    GDT.0.load();
    
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}