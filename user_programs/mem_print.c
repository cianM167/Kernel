#include "mini_c_stdlib/syscalls.h"

//to compile with simple stdlib

// gcc -nostdlib -static -fno-pie -no-pie -fno-builtin -fno-stack-protector \
    mem_print.c \
    mini_c_stdlib/syscalls.c \
    -o mem_print.elf

void _start() {
    int a, b, c, d;

    printf("a:%d\nb:%d\nc:%dd:%d\nmemory is totally untouched");

    exit(0);
}