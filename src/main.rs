#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(meowl::test_runner)]
#![reexport_test_harness_main = "test_main"]

// cargo build --target x86_64-meowl_os.json to build :)

// cargo bootimage --release

// run in qemu
// qemu-system-x86_64 -drive format=raw,file=target/x86_64-meowl_os/release/bootimage-meowl.bin

use core::{panic::PanicInfo};

use alloc::{boxed::Box, vec, rc::Rc, vec::Vec};
use bootloader::{BootInfo, entry_point};
use meowl::{allocator, hlt_loop, memory::BootInfoFrameAllocator};
use x86_64::{VirtAddr, registers::control::Cr3, structures::paging::{Page, PageTable, Translate}};

extern crate alloc;

mod vga_buffer;
mod serial;
mod memory;

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

entry_point!(kernel_main);//telling bootloader what our entrypoint is 

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello I am the kernel :)");

    meowl::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    let heap_value = Box::new(42);
    println!("heap_value at {:p}", heap_value);

    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    core::mem::drop(reference_counted);
    println!("reference count is {} now", Rc::strong_count(&cloned_reference));

    #[cfg(test)]
    test_main();

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