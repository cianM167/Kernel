use alloc::{collections::VecDeque, vec::Vec};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{VirtAddr, registers::control::{Cr3, Cr3Flags}};

use crate::{allocator::{KERNEL_OFFSET, debug_walk, with_memory}, gdt::{GDT, TSS}, println, syscall::CPU_LOCAL, threads::{self, Context, Thread, ThreadState}};

lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

pub struct Scheduler {
    threads: Vec<Thread>,
    run_queue: VecDeque<usize>,
    current: Option<usize>,
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

    pub fn schedule(&mut self) {
        let current_id = self.current;

        let next_id = match self.run_queue.pop_front() {
            Some(id) => id,
            None => return,
        };

        if let Some(id) = current_id {
            let current = &mut self.threads[id];

            if current.state == ThreadState::Running {
                current.state = ThreadState::Ready;
                self.run_queue.push_back(id);
            }
        }

        let next: &mut Thread = &mut self.threads[next_id];
        next.state = ThreadState::Running;

        self.current = Some(next_id);

        unsafe {
            match current_id {
                Some(id) => {
                    let (old_ctx, new_ctx) = if id < next_id {
                        let (left, right) = self.threads.split_at_mut(next_id);
                        (&mut left[id].context, &right[0].context)
                    } else {
                        let (left, right) = self.threads.split_at_mut(id);
                        (&mut right[0].context, &left[next_id].context)
                    };

                    // switch_context(old_ctx, new_ctx)
                }
                None => {
                    let thread = &self.threads[next_id];
                    let new_ctx = &self.threads[next_id].context;
                    let frame = &self.threads[next_id].address_space;
                    
                    Cr3::write(*frame, Cr3Flags::empty());
                    println!("trying to switch to user mode");

                    set_kernel_stack(thread.kernel_stack_top.as_u64());

                    // unsafe {
                    //     *((new_ctx.rsp - 8) as *mut u64) = 0xdeadbeef;
                    //     println!("value in RSP = {:#x}", *((new_ctx.rsp - 8) as *mut u64));
                    // }

                    // with_memory(|memory| {
                    //     debug_walk(VirtAddr::new(new_ctx.rsp - 8), memory.phys_mem_offset);
                    // });

                    enter_user_mode(new_ctx.rip, new_ctx.rsp);
                }
            }
        }
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

            options(noreturn, nostack)
        );
    }
}

pub fn set_kernel_stack(rsp: u64) {
    unsafe {
        CPU_LOCAL.kernel_rsp = rsp;
    }
}