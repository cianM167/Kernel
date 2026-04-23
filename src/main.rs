#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(meowl::test_runner)]
#![reexport_test_harness_main = "test_main"]

// cargo build --target x86_64-meowl_os.json to build :)

// cargo bootimage --release

// run in qemu
// qemu-system-x86_64 -drive format=raw,file=target/x86_64-meowl_os/release/bootimage-meowl.bin

use core::{panic::PanicInfo, sync::atomic::Ordering, arch::asm};

use alloc::{boxed::Box, vec, rc::Rc, vec::Vec};
use bootloader::{BootInfo, entry_point};
use meowl::{MEMORY, allocator::{self, MemoryManager, with_memory}, hlt_loop, interrupts::TIMER, memory::BootInfoFrameAllocator, task::{Task, executor::Executor, keyboard, simple_executor::SimpleExecutor}, threads::{Thread, scheduler::{SCHEDULER, Scheduler}}};
use spin::{Mutex};
use x86_64::{VirtAddr, registers::control::{Cr3, Cr3Flags}, structures::paging::{Mapper, OffsetPageTable, Page, PageTable, Translate}};

extern crate alloc;

mod vga_buffer;
mod serial;
mod memory;

const KERNEL_STACK_SIZE: usize = 4096 * 5;
static mut KERNEL_STACK: [u8; KERNEL_STACK_SIZE] = [0; KERNEL_STACK_SIZE];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

entry_point!(kernel_main);// telling bootloader what our entrypoint is 

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    #[cfg(test)]
    test_main();

    // unsafe {
    //     core::arch::asm!(
    //         "mov rsp, {}",
    //         in(reg) stack_end.as_u64(),
    //         options(nostack)
    //     );
    // }

    meowl::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    {// ignore awful scopejitsu
        let mut memory = MEMORY.lock();

        *memory = Some(unsafe {
            MemoryManager::new(phys_mem_offset, &boot_info.memory_map)
        });
    }
    
    with_memory(|memory| {
        let (pml4, _) = Cr3::read();
        memory.map_vga_buffer(pml4).unwrap();
        memory.init_heap().expect("heap initialization failed");
    });

    println!("Hello I am the kernel\n        \\\n         \\\n            _~^~^~_\n        \\) /  o o  \\ (/\n          '_   -   _'\n          / '-----' \\");

    // unsafe { Cr3::write(pml4_frame, Cr3Flags::empty()) };

    SCHEDULER.lock().spawn(Thread::new(0));
    SCHEDULER.lock().schedule();

    // let mut executor = Executor::new();
    // executor.spawn(Task::new(example_task()));
    // executor.spawn(Task::new(keyboard::print_keypresses()));
    // executor.run();

    println!("I didnt crash Yippee!!!!");
    hlt_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {// eventually will display message
    println!("Mom I frew up: ({})", info);
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    meowl::test_panic_handler(info)
}

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

fn test() -> ! {
    loop {
        unsafe {
            core::arch::asm!("nop");
        }
    }
}

#[inline(always)]
fn read_rsp() -> u64 {
    let rsp: u64;
    unsafe {
        asm!(
            "mov {}, rsp",
            out(reg) rsp,
            options(nomem, nostack, preserves_flags)
        );
    }

    rsp
}


