static inline long write(int fd, const char* buf, long len) {
    long ret;
    asm volatile (
        "syscall"
        : "=a"(ret)
        : "a"(1), "D"(fd), "S"(buf), "d"(len)
        : "rcx", "r11", "memory"
    );
    return ret;
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
    write(1, "hello\n", 6);
    exit(0);
}