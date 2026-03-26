struct Thread {
    context: Context,
    stack_ptr: usize,
    state: ThreadState,
    address_space:  *mut AddressSpace,
}

#[repr(C)]
struct Context {
    rsp: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    rip: u64,
}