#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(meowl::test_runner)]
#![reexport_test_harness_main = "test_main"]

// cargo build --target x86_64-meowl_os.json to build :)

// cargo bootimage --release

// run in qemu
// qemu-system-x86_64 -drive format=raw,file=target/x86_64-meowl_os/release/bootimage-meowl.bin

use core::{fmt::Write, panic::PanicInfo};

use meowl::hlt_loop;

use crate::vga_buffer::{WRITER};

mod vga_buffer;
mod serial;

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

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello I am the kernel :)");

    meowl::init();

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