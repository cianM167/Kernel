use alloc::{collections::VecDeque, vec::Vec};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{VirtAddr, registers::control::{Cr3, Cr3Flags}};

use crate::{allocator::{KERNEL_OFFSET, debug_walk, with_memory}, gdt::{GDT, TSS, set_rsp0}, println, syscall::CPU_LOCAL, threads::{self, Context, Thread, ThreadState}};

lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

pub struct Scheduler {
    pub threads: Vec<Thread>,
    pub run_queue: VecDeque<usize>,
    pub current: Option<usize>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self { 
            threads: Vec::new(),
            run_queue: VecDeque::new(),
            current: None,
        }
    }

    pub fn spawn(&mut self, thread: Thread) {
        let id = self.threads.len();
        self.threads.push(thread);
        self.run_queue.push_back(id);
    }
  
}

unsafe extern "C" {
    fn switch_context(old: *mut Context, new: *const Context);
}

unsafe fn start_first_thread(ctx: &Context) -> ! {
    unsafe {
        core::arch::asm!(
            "mov rsp, [{0} + 0x00]",
            "mov r15, [{0} + 0x08]",
            "mov r14, [{0} + 0x10]",
            "mov r13, [{0} + 0x18]",
            "mov r12, [{0} + 0x20]",
            "mov rbx, [{0} + 0x28]",
            "mov rbp, [{0} + 0x30]",
            "mov rax, [{0} + 0x38]",
            "jmp rax",
            in(reg) ctx,
            options(noreturn)
        )
    }
}

pub unsafe fn enter_user_mode(entry: u64, user_stack: u64) -> ! {
    let user_cs = (GDT.1.user_code.0 as u64) | 3;
    let user_ss = (GDT.1.user_data.0 as u64) | 3;

    // println!("USER RIP = {:#x}", entry);
    // println!("USER RSP = {:#x}", user_stack);

    unsafe {
        core::arch::asm!(
            // "cli", uuuhhhhh idk might fix shit

            "push {ss}",
            "push {rsp}",
            "push 0x202",
            "push {cs}",
            "push {rip}",
            "iretq",

            cs = in(reg) user_cs,
            ss = in(reg) user_ss,
            rip = in(reg) entry,
            rsp = in(reg) user_stack,

            options(noreturn)
        );
    }
}

pub fn set_kernel_stack(rsp: u64) {
    unsafe {
        CPU_LOCAL.kernel_rsp = rsp;
    }
}

pub fn schedule() {
    let action = {
        let mut scheduler = SCHEDULER.lock();

        let current_id = scheduler.current;
        let next_id = match scheduler.run_queue.pop_front() {
            Some(id) => id,
            None => return,
        };

        if let Some(id) = current_id {
            let current = &mut scheduler.threads[id];
            if current.state == ThreadState::Running {
                current.state = ThreadState::Ready;
                scheduler.run_queue.push_back(id);
            }
        }

        scheduler.threads[next_id].state = ThreadState::Running;
        scheduler.current = Some(next_id);

        let thread = &scheduler.threads[next_id];
        let entry = thread.context.rip;
        let rsp = thread.context.rsp;
        let frame = thread.address_space;
        let kstack = thread.kernel_stack_top;

        (current_id, next_id, entry, rsp, frame, kstack)
    };

    let (current_id, next_id, entry, rsp, frame, kstack) = action;

    unsafe {
        match current_id {
            Some(id) => {
                //  Über broken 😔
                // let (old_ctx, new_ctx) = if id < next_id {
                //     let (left, right) = self.threads.split_at_mut(next_id);
                //     (&mut left[id].context, &right[0].context)
                // } else {
                //     let (left, right) = self.threads.split_at_mut(id);
                //     (&mut right[0].context, &left[next_id].context)
                // };

                // switch_context(old_ctx, new_ctx)
            }
            None => {              
                Cr3::write(frame, Cr3Flags::empty());
                println!("trying to switch to user mode");

                set_kernel_stack(kstack.as_u64());

                set_rsp0(kstack);

                // unsafe {
                //     *((new_ctx.rsp - 8) as *mut u64) = 0xdeadbeef;
                //     println!("value in RSP = {:#x}", *((new_ctx.rsp - 8) as *mut u64));
                // }

                // with_memory(|memory| {
                //     debug_walk(VirtAddr::new(new_ctx.rsp - 8), memory.phys_mem_offset);
                // });

                enter_user_mode(entry, rsp);
            }
        }
    }
}