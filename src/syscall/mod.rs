use x86_64::{VirtAddr, registers::model_specific::{Efer, EferFlags, LStar, Msr}};

use crate::{print, println};

const IA32_STAR: u32 = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_FMASK: u32 = 0xC000_0084;

pub fn enable_syscall() {
    unsafe {
        Efer::update(|efer| {
            efer.insert(EferFlags::SYSTEM_CALL_EXTENSIONS);
        });
    }
}

pub fn init_syscalls(syscall_entry: u64) {
    unsafe {
        // STAR: segment selector

        // Bits:
        // 63:48 = kernel CS
        // 47:32 = user CS (with RPL=3)

        let kernel_cs: u64 = 0x08;
        let user_cs: u64 = 0x18;

        let star_value = 
            (kernel_cs << 48) |
            (user_cs << 32);

        Msr::new(IA32_STAR).write(star_value);

        Msr::new(IA32_LSTAR).write(syscall_entry);

        Msr::new(IA32_FMASK).write(1 << 9);
    }
}

#[unsafe(naked)]
pub extern "C" fn syscall_entry() {
    unsafe {
        core::arch::naked_asm!(
            "swapgs",

            // save user return context
            "mov r11, rcx",   // save RIP (optional but useful)
            "mov rcx, r11",   // keep symmetry

            "call {handler}",

            "swapgs",
            "sysretq",

            handler = sym syscall_handler,
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn syscall_handler(
    num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> u64 {
    println!("RDI={:#x} RSI={:#x} RDX={:#x}", arg1, arg2, arg3);
    match num {
        1 => sys_write(arg1, arg2 as * const u8, arg3),
        60 => sys_exit(arg1),
        _ => -1i64 as u64,
    }
}

fn sys_write(fd: u64, buf: *const u8, len: u64) -> u64 {
    println!("about to write");
    println!("fd is: {}, buffer is: {}", fd, (buf as u8) as char);
    if fd != 1 && fd != 2 {
        return -1i64 as u64; // EBADF later
    }

    println!("fd passed");

    if buf.is_null() {
        return -1i64 as u64;
    }

    println!("buffer not null");

    let slice = unsafe {
        core::slice::from_raw_parts(buf, len as usize)
    };

    println!("i havent exited early");
    println!("buf pointer = {:#x}", buf as u64);
    println!("len = {}", len);

    // for &b in slice {
    //     // print!("{}", b as char);
    // }

    len
}

fn sys_exit(code: u64) -> u64 {
    println!("Process exiited: {}", code);
    loop {}
}