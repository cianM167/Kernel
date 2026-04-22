use x86_64::{VirtAddr, registers::model_specific::{Efer, EferFlags, LStar, Msr}};

use crate::{print, println, task::keyboard::STDIN_BUFFER};

static mut KERNEL_STACK: [u8; 4096 * 4] = [0; 4096 * 4];

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

            // save user rsp fixme later
            "mov r12, rsp",

            // switch to kernel stack
            "lea rsp, [{stack} + {size}]",

            // align stack
            "and rsp, -16",

            // save return state
            "push rcx",
            "push r11",

            // preserve original registers first
            "mov r13, rdi",   // save arg1 (fd)
            "mov r14, rsi",   // save arg2 (buf)
            "mov r15, rdx",   // save arg3 (len)

            // now set up Rust ABI
            "mov rdi, rax",   // num
            "mov rsi, r13",   // arg1
            "mov rdx, r14",   // arg2
            "mov rcx, r15",   // arg3

            "call {handler}",

            "pop r11",
            "pop rcx",

            // restore user stack
            "mov rsp, r12",

            "swapgs",
            "sysretq",

            stack = sym KERNEL_STACK,
            size = const 4096 * 4,
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
    match num {
        0 => sys_read(arg1, arg2 as *mut u8, arg3),
        1 => sys_write(arg1, arg2 as * const u8, arg3),
        60 => sys_exit(arg1),
        _ => -1i64 as u64,
    }
}

fn sys_write(fd: u64, buf: *const u8, len: u64) -> u64 {
    if fd != 1 && fd != 2 {
        return -1i64 as u64; // EBADF later
    }

    if buf.is_null() {
        return -1i64 as u64;
    }

    let slice = unsafe {
        core::slice::from_raw_parts(buf, len as usize)
    };

    for &b in slice {
        print!("{}", b as char);
    }

    len
}

fn sys_exit(code: u64) -> u64 {
    println!("Process exited: {}", code);
    loop {}
}

fn sys_read(fd: u64, buf: *mut u8, len: u64) -> u64 {
    if fd != 0 {
        return -1i64 as u64;
    }

    let mut stdin = STDIN_BUFFER.lock();

    let mut i = 0;

   while i < len {
        match stdin.pop_front() {
            Some(byte) => unsafe {
                *buf.add(i as usize) = byte;
                i += 1;
            },
            None => break,
        }
   }

    i as u64
}