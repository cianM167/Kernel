use x86_64::{VirtAddr, instructions::hlt, registers::model_specific::{Efer, EferFlags, LStar, Msr}, structures::paging::{Mapper, Page, PageTableFlags, page}};

use crate::{allocator::with_memory, print, println, task::keyboard::STDIN_BUFFER, threads::{self, scheduler::{self, SCHEDULER}}};

const IA32_STAR: u32 = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_FMASK: u32 = 0xC000_0084;

#[repr(C)]
pub struct CpuLocal {
    pub user_rsp: u64,
    pub kernel_rsp: u64,
}

pub static mut CPU_LOCAL: CpuLocal = CpuLocal {
    user_rsp: 0,
    kernel_rsp: 0,
};

const IA32_GS_BASE: u32 = 0xC0000101;
const IA32_KERNEL_GS_BASE: u32 = 0xC0000102;

pub fn init_gs() {
    unsafe {
        let ptr = core::ptr::addr_of!(CPU_LOCAL) as u64;
        Msr::new(IA32_GS_BASE).write(ptr);
        Msr::new(IA32_KERNEL_GS_BASE).write(ptr);
    }
}

pub fn enable_syscall() {
    unsafe {
        Efer::update(|efer| {
            efer.insert(EferFlags::SYSTEM_CALL_EXTENSIONS);
        });
    }
}

pub fn init_syscalls(syscall_entry: u64) {
    unsafe {
        let kernel_cs: u64 = 0x08;
        let user_base: u64 = 0x10;

        let star_value = 
            (user_base << 48) |
            (kernel_cs << 32);

        Msr::new(IA32_STAR).write(star_value);

        Msr::new(IA32_LSTAR).write(syscall_entry);

        Msr::new(IA32_FMASK).write(0);// allow for interrupts pray this doesnt break anything
    }
}

#[unsafe(naked)]
pub extern "C" fn syscall_entry() {
    unsafe {
        core::arch::naked_asm!(
            "swapgs",

            // // save user rsp fixme later
            // "mov r12, rsp",

            // // switch to kernel stack
            // "lea rsp, [{stack} + {size}]",

            // save user rsp in gs
            "mov qword ptr gs:[0], rsp",

            // load kernel RSP from CPU local
            "mov rsp, qword ptr gs:[8]",

            // align stack
            "and rsp, -16",

            // save return state
            "push rcx",
            "push r11",

            // preserve original registers first
            "mov r12, rdi",  // save arg1 before we clobber rdi
            "mov r13, rsi",  // save arg2
            "mov r14, rdx",  // save arg3
            "mov r15, r10",  // save arg4 (r10 on Linux ABI)

            "mov rdi, rax",  // num
            "mov rsi, r12",  // arg1
            "mov rdx, r13",  // arg2
            "mov rcx, r14",  // arg3
            "mov r8,  r15",  // arg4

            "call {handler}",

            "pop r11",
            "pop rcx",

            // restore user stack
            "mov rsp, qword ptr gs:[0]",

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
    match num {
        0 => sys_read(arg1, arg2 as *mut u8, arg3),
        1 => sys_write(arg1, arg2 as * const u8, arg3),
        9 => sys_mmap(arg1, arg2, arg3, arg4, arg5, arg6),
        11 => sys_munmap(arg1, arg2),
        12 => sys_brk(arg1),
        13 => sys_rt_sigaction(arg1, arg2, arg3),
        14 => sys_rt_sigprocmask(arg1, arg2, arg3, arg4),
        218 => sys_set_tid_address(arg1),
        231 => sys_exit_group(arg1),
        39  => sys_getpid(),
        63  => sys_uname(arg1),
        60 => sys_exit(arg1),
        158 => sys_arch_prctl(arg1, arg2),
        _ => -1i64 as u64,
    }
}

fn sys_write(fd: u64, buf: *const u8, len: u64) -> u64 {
    if fd != 1 && fd != 2 {
        return -1i64 as u64;
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

    let mut i = 0;

    // println!("got to loop after scanf call");
    while i < len {
        let byte = loop {
            if let Some(b) = STDIN_BUFFER.lock().pop_front() {
                print!("{}", b as char);
                break b;
            }

            hlt();
            // threads::yield_now();
        };

        unsafe {
            *buf.add(i as usize) = byte;
        }

        i += 1;

        if byte ==b'\n' {
            break;
        }
    }

    i
}

fn sys_brk(new_break: u64) -> u64 {
    let mut scheduler = SCHEDULER.lock();
    let thread = match scheduler.current {
        Some(id) => &mut scheduler.threads[id],
        None => return 0,
    };

    let new_break = VirtAddr::new(new_break);

    if new_break.as_u64() == 0 {
        return thread.heap_end.as_u64();
    }

    if new_break < thread.heap_start || new_break.as_u64() > 0x0000_0000_6000_0000 {
        return thread.heap_end.as_u64();
    }

    if new_break > thread.heap_end {
        let start_page = Page::containing_address(thread.heap_end);
        let end_page = Page::containing_address(new_break- 1);
        let address_space = thread.address_space;

        let old_end = thread.heap_end;
        drop(thread);
        drop(scheduler);

        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE
            | PageTableFlags::NO_EXECUTE;

        with_memory(|memory| {
            let mut mapper = unsafe { memory.mapper_for(address_space) };

            for page in Page::range_inclusive(start_page, end_page) {
                memory.alloc_page(page, flags, &mut mapper)
                    .expect("brk: page alloc failed");
            }
        });

        let mut scheduler = SCHEDULER.lock();
        if let Some(id) = scheduler.current {
            scheduler.threads[id].heap_end = new_break;
        }
    } else {
        thread.heap_end = new_break;
    }

    new_break.as_u64()
}

// mmap flags/prot constants matching Linux ABI
const PROT_READ:    u64 = 0x1;
const PROT_WRITE:   u64 = 0x2;
const PROT_EXEC:    u64 = 0x4;
const MAP_PRIVATE:  u64 = 0x2;
const MAP_ANONYMOUS:u64 = 0x20;
const MAP_FIXED:    u64 = 0x10;

fn sys_mmap(addr: u64, len: u64, prot: u64, flags: u64, fd: u64, offset: u64) -> u64 {
    // file backed mapping is cringe not doin all that
    if flags & MAP_ANONYMOUS == 0 {
        println!("mmap: file-backed not supported");
        return -1i64 as u64;
    }

    if len == 0 {
        return -1i64 as u64;
    }

    let len_aligned = (len + 0xFFF) & !0xFFF;

    let mut page_flags = PageTableFlags::PRESENT
        | PageTableFlags::USER_ACCESSIBLE;

    if prot & PROT_WRITE != 0 {
        page_flags |= PageTableFlags::WRITABLE;
    }

    if prot &  PROT_EXEC == 0 {
        page_flags |= PageTableFlags::NO_EXECUTE;
    }

    let (address_space, map_addr) = {
        let mut scheduler = SCHEDULER.lock();
        let thread = match scheduler.current {
            Some(id) => &mut scheduler.threads[id],
            None => return -1i64 as u64,
        };

        let map_addr = if flags & MAP_FIXED != 0 && addr != 0 {
            // caller specified address
            if addr % 0x1000 != 0 {
                return -1i64 as u64;
            }
            addr
        } else {
            let candidate = thread.mmap_top;
            thread.mmap_top += len_aligned;
            candidate
        };

        (thread.address_space, map_addr)
    };

    with_memory(|memory| {
        let mut mapper = unsafe { memory.mapper_for(address_space) };

        let start_page = Page::containing_address(VirtAddr::new(map_addr));
        let end_page = Page::containing_address(
            VirtAddr::new(map_addr + len_aligned - 1)
        );

        for page in Page::range_inclusive(start_page, end_page) {
            if flags & MAP_FIXED != 0 && mapper.translate_page(page).is_ok() {
                continue;
            }

            memory.alloc_page(page, page_flags, &mut mapper)
                .expect("mmap: alloc_page faile");
        }
    });

    unsafe {
        core::ptr::write_bytes(map_addr as *mut u8, 0, len_aligned as usize);
    }

    map_addr
}

fn sys_munmap(addr: u64, len: u64) -> u64 {
    0// yummy memory leak
}

const ARCH_SET_GS: u64 = 0x1001;
const ARCH_SET_FS: u64 = 0x1002;
const ARCH_GET_FS: u64 = 0x1003;
const ARCH_GET_GS: u64 = 0x1004;

const IA32_FS_BASE: u32 = 0xC0000100;

fn sys_arch_prctl(code: u64, addr: u64) -> u64 {
    unsafe {
        match code {
            ARCH_SET_FS => {
                Msr::new(IA32_FS_BASE).write(addr);
                0
            }
            ARCH_GET_FS => {
                Msr::new(IA32_FS_BASE).read()
            }
            ARCH_SET_GS => {
                println!("arch_prctl: ARCH_SET_GS ignored (kernel uses GS)");
                0
            }
            ARCH_GET_GS => {
                Msr::new(IA32_GS_BASE).read()
            }
            _ => {
                println!("arch_prctl: unknown code {:#x}", code);
                -1i64 as u64
            }
        }
    }
}

fn sys_rt_sigaction(_signum: u64, _act: u64, _oldact: u64) -> u64 {
    0 //pretend it worked
}

fn sys_rt_sigprocmask(_how: u64, _set: u64, _oldset: u64, _sigsetsize: u64) -> u64 {
    0
}

fn sys_set_tid_address(_tidptr: u64) -> u64 {
    1
}

fn sys_exit_group(code: u64) -> u64 {
    println!("Process exited (group): {}", code);
    loop {}
}

fn sys_getpid() -> u64 {
    1    
}

fn sys_uname(buf: u64) -> u64 {
    // struct utsname has 6 fields of 65 bytes each
    // glibc checks sysname and release
    if buf == 0 { return -1i64 as u64; }

    unsafe {
        let base = buf as *mut u8;
        // zero the whole struct first (6 * 65 = 390 bytes)
        core::ptr::write_bytes(base, 0, 390);

        // sysname (offset 0)
        let sysname = b"MeowlOs\0";
        core::ptr::copy_nonoverlapping(sysname.as_ptr(), base, sysname.len());

        // release (offset 65) — glibc parses this as a version number
        let release = b"1.0.0\0";
        core::ptr::copy_nonoverlapping(release.as_ptr(), base.add(65), release.len());

        // machine (offset 260)
        let machine = b"x86_64\0";
        core::ptr::copy_nonoverlapping(machine.as_ptr(), base.add(260), machine.len());
    }

    0
}