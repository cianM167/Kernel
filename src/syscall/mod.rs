use x86_64::registers::model_specific::{Efer, EferFlags};

use crate::println;

pub fn init_syscall() {
    unsafe {
        Efer::update(|flags| {
            *flags = EferFlags::SYSTEM_CALL_EXTENSIONS;
        })
    }
}

extern "C" fn syscall_entry() {
    println!("got a syscall");
}