// to compile run : gcc -nostdlib -static -fno-pie -no-pie syscall_test.c -o syscall_test.elf

static inline long write(int fd, const char* buf, long len) {
    register long rax __asm__("rax") = 1;
    register long rdi __asm__("rdi") = fd;
    register long rsi __asm__("rsi") = (long)buf;
    register long rdx __asm__("rdx") = len;

    asm volatile (
        "syscall"
        : "+r"(rax)
        : "r"(rdi), "r"(rsi), "r"(rdx)
        : "rcx", "r11", "memory"
    );

    return rax;
}

static inline void exit(int code) {
    asm volatile (
        "syscall"
        :
        : "a"(60), "D"(code)
        : "rcx", "r11", "memory"
    );
    __builtin_unreachable();
}

void _start() {
    write(1, "Hello World!!!!\n", 16);
    exit(0);
}

