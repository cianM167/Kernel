use alloc::boxed::Box;
use x86_64::{PhysAddr, VirtAddr, registers::control::{Cr3, Cr3Flags}, structures::paging::{Mapper, Page, PageTableFlags, PhysFrame}};

use crate::{allocator::{MemoryManager, USER_CODE_START, with_memory}, println, threads::scheduler::SCHEDULER};

pub mod scheduler;

// ^-^
// o.o
//\   /
// | |
pub static USER_PROG: &[u8] = include_bytes!("../../user_programs/hello_world.elf");// ignore how awful the path is

pub struct Thread {
    pub context: Context,
    pub address_space: PhysFrame,
    pub state: ThreadState,
    pub stack_top: VirtAddr,
    pub kernel_stack_top: VirtAddr,
    pub heap_start: VirtAddr,
    pub heap_end: VirtAddr,
    pub mmap_top: u64,
}

impl Thread {
    pub fn new(_entry: u64) -> Self {

        let (address_space, stack_top, entry, kernel_stack_top) = 
            with_memory(|memory| {
                let kernel_stack_top = memory.alloc_kernel_stack();
                let pml4_frame = memory.new_address_space();
                // memory.map_user_pages(pml4_frame).expect("shat the bed");
                // memory.map_user_code(pml4_frame, VirtAddr::new(USER_CODE_START)).expect("shat þe bed");
                let entry = memory.load_elf(pml4_frame, USER_PROG);

                // let old = Cr3::read().0;

                // unsafe {
                //     Cr3::write(pml4_frame, Cr3Flags::empty());
                // }

                // memory.write_user_code(VirtAddr::new(USER_CODE_START));

                // unsafe {
                //     Cr3::write(old, Cr3Flags::empty());
                // }
                // println!("about to alloc user stack");
                let stack_top = memory.alloc_user_stack(pml4_frame, entry);

                (pml4_frame, stack_top, entry, kernel_stack_top)
            });

        let context = Context::new_user(entry, stack_top);

        let heap_start = VirtAddr::new(0x0000_0000_2000_000);
        

        Self {
            context,
            state: ThreadState::Ready,
            stack_top,
            address_space,
            kernel_stack_top,
            heap_start,
            heap_end: heap_start,
            mmap_top: 0x0000_0000_4000_0000,
        }
    }
}

// pub fn yield_now() {
//     SCHEDULER.lock().schedule();
// }

#[derive(Clone, Copy, PartialEq)]
enum ThreadState {
    Ready,
    Running,
    Blocked,
    Finished,
}


#[derive(Default)]
#[repr(C)]
struct Context {
    pub rsp: u64,
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub rip: u64,
    pub rflags: u64,
}

impl Context {
    pub fn new_user(entry: u64, stack_top: VirtAddr) -> Self {
        Context {
            rip: entry,
            rsp: stack_top.as_u64(),
            rflags: 0x202,
            ..Default::default()
        }
    }
}