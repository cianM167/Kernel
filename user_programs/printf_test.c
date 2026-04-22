#include "mini_c_stdlib/syscalls.h"

//to compile with simple stdlib

// gcc -nostdlib -static -fno-pie -no-pie -fno-builtin -fno-stack-protector \
    printf_test.c \
    mini_c_stdlib/syscalls.c \
    -o printf_test.elf

void _start() {
    printf("hello %s %d %x\n", "world", 42, 0xdeadbeef);
    exit(0);
}