#include "mini_c_stdlib/syscalls.h"
#include "mini_c_stdlib/malloc.h"

//to compile with simple stdlib

// gcc -nostdlib -static -fno-pie -no-pie -fno-builtin -fno-stack-protector \
    rock_paper_scissors.c \
    mini_c_stdlib/syscalls.c \
    mini_c_stdlib/malloc.c \
    -o rock_paper_scissors.elf

void _start() {
    printf("rock started");
    int loop = 1;
    int player_choice;
    int computer_choice;
    int n;

    // 1 rock
    // 2 paper
    // 3 scissors

    int* a = malloc(100 * sizeof(int));

    printf("allocated memory:\n");

    for (int i = 0; i < 100; i++) {
        printf("%d\n", a[i]);
    }

    exit(0);
}
