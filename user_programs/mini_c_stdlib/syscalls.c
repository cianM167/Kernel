int atoi(const char *strg);

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

void exit(int code) {
    asm volatile (
        "syscall"
        :
        : "a"(60), "D"(code)
        : "rcx", "r11", "memory"
    );
    __builtin_unreachable();
}

void puts(const char* s) {
    const char* p = s;
    while (*p) p++;
    write(1, s, p - s);
}

void print_int(long x) {
    char buf[32];
    int i = 0;

    if (x == 0) {
        write(1, "0", 1);
        return;
    }

    if (x < 0) {
        write(1, "-", 1);
        x = -x;
    }

    while (x > 0) {
        buf[i++] = '0' + (x % 10);
        x /= 10;
    }

    // reverse
    for (int j = i - 1; j >= 0; j--) {
        write(1, &buf[j], 1);
    }
}

#include <stdarg.h>

void print_hex(unsigned long x) {
    char hex[] = "0123456789abcdef";
    char buf[16];
    int i = 0;

    if (x == 0) {
        write(1, "0", 1);
        return;
    }

    while (x > 0) {
        buf[i++] = hex[x & 0xf];
        x >>= 4;
    }

    write(1, "0x", 2);

    for (int j = i - 1; j >= 0; j--) {
        write(1, &buf[j], 1);
    }
}

void printf(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);

    for (const char* p = fmt; *p; p++) {
        if (*p != '%') {
            write(1, p, 1);
            continue;
        }

        p++; // skip %
        
        switch (*p) {
            case 's' : {
                const char* s = va_arg(args, const char*);
                puts(s);
                break;
            }
            case 'd': {
                long x = va_arg(args, long);
                print_int(x);
                break;
            }
            case 'x': {
                unsigned long x = va_arg(args, unsigned long);
                print_hex(x);
                break;
            }
            case 'c': {
                char c = (char)va_arg(args, int);
                write(1, &c, 1);
                break;
            }
            case '%': {
                write(1, "%", 1);
                break;
            }
            default:
                // unkown
                write(1, "?", 1);
        }
    }

    va_end(args);
}

long read(int fd, void* buf, long len) {
    long ret;
    asm volatile (
        "syscall"
        : "=a"(ret)
        : "a"(0), "D"(fd), "S"(buf), "d"(len)
        : "rcx", "r11", "memory"
    );

    return ret;
}

char getchar() {
    char c;
    read(0, &c, 1);
    return c;
}

int gets(char* buf, int max) {
    int i = 0;

    while (i < max - 1) {
        char c = getchar();

        if (c == '\n') {
            break;
        }

        buf[i++] = c;
    }

    buf[i] = 0;
    return i;
}

void scanf(const char* fmt, ...) {
    char buffer[128];
    gets(buffer, sizeof(buffer));

    va_list args;
    va_start(args, fmt);

    const char* p = fmt;
    char* input = buffer;

    while (*p) {
        if (*p == '%') {
            p++;// skipping %

            switch (*p) {
                case 'd': {
                    int* out = va_arg(args, int*);
                    *out = atoi(input);

                    while (*input && *input != ' ') input++;
                    if (*input == ' ') input++;
                    break;
                }
                case 's': {
                    char* out = va_arg(args, char*);
                    while (*input && *input != '\n') {
                        *out++ = *input++;
                    }
                    *out = 0;
                    if (*input == '\n') input++;
                }
            }
        }
        p++;
    }
    va_end(args);
}

int atoi(const char *strg) {// geeksforgeeks my beloved
    // Initialize res to 0
    int res = 0;
    int i = 0;

    // Iterate through the string strg and compute res
    while (strg[i] != '\0')
    {
        res = res * 10 + (strg[i] - '0');
        i++;
    }

    return res;
}