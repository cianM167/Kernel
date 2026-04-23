#include "mini_c_stdlib/syscalls.h"

//to compile with simple stdlib

// gcc -nostdlib -static -fno-pie -no-pie -fno-builtin -fno-stack-protector \
    printf_test.c \
    mini_c_stdlib/syscalls.c \
    -o printf_test.elf

int _start(){

    int count = 1;

    while (count < 11) {
        printf("%d\n", count);
        count++;
    }

    exit(0);
}