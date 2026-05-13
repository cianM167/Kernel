#include "mini_c_stdlib/syscalls.h"
#include "mini_c_stdlib/malloc.h"

//to compile with simple stdlib

// gcc -nostdlib -static -fno-pie -no-pie -fno-builtin -fno-stack-protector \
    rock_paper_scissors.c \
    mini_c_stdlib/syscalls.c \
    mini_c_stdlib/malloc.c \
    -o rock_paper_scissors.elf

void _start() {
    int loop = 1;
    int player_choice;
    int computer_choice;
    int n;

    // 1 rock
    // 2 paper
    // 3 scissors

    int* a = malloc(2 * sizeof(int));

    printf("allocated memory:\n");

    a[0] = 10;
    a[1] = 11;
    a[0] = 12;

    printf("a[0]: %d, a[1]: %d, a[100]: %d\n", a[0], a[1], a[10000000000]);

    exit(0);
}
