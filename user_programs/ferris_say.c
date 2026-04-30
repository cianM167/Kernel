#include "mini_c_stdlib/syscalls.h"

//to compile with simple stdlib

// gcc -nostdlib -static -fno-pie -no-pie -fno-builtin -fno-stack-protector \
    printf_test.c \
    mini_c_stdlib/syscalls.c \
    -o printf_test.elf

void _start() {
    char buffer[100];

    printf("Please input a string you want to display on the screen\n");
    scanf("%s", &buffer); 
    printf("\n\n%s\n        \\\n         \\\n            _~^~^~_\n        \\) /  o o  \\ (/\n          '_   -   _'\n          / '-----' \\ \n", buffer);
    exit(0);
}