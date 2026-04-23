#include "mini_c_stdlib/syscalls.h"

//to compile with simple stdlib

// gcc -nostdlib -static -fno-pie -no-pie -fno-builtin -fno-stack-protector \
    printf_test.c \
    mini_c_stdlib/syscalls.c \
    -o printf_test.elf

void _start() {
    printf("\n\nHello I am the user program\n        \\\n         \\\n            _~^~^~_\n        \\) /  o o  \\ (/\n          '_   -   _'\n          / '-----' \\ \n");
    exit(0);
}