#include "mini_c_stdlib/syscalls.h"
#include "mini_c_stdlib/malloc.h"

//to compile with simple stdlib

// gcc -nostdlib -static -fno-pie -no-pie -fno-builtin -fno-stack-protector \
    rock_paper_scissors.c \
    mini_c_stdlib/syscalls.c \
    -o rock_paper_scissors.elf

int random_buff[100];
int buff_idx = 0;

int rand();

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

    for (int i = 0; i < 100; i++) {
        printf("%d\n", a[i]);
    }

    while (loop == 1) {
        printf("Make your choice of move\n");
        printf("1: rock\n");
        printf("2: paper\n");
        printf("3: scissors\n");

        scanf("%d", &player_choice);
        
        while (player_choice > 3 || player_choice < 1) {
            printf("invalid choice\n");
            scanf("%d", &player_choice);
        }

        n = rand() % 100;

        if (n < 33) {
            computer_choice = 1;
        }

        else if (n > 33 && n < 66) {
            computer_choice = 2;
        }

        else {
            computer_choice = 3;
        }

        if (player_choice == computer_choice) {
            printf("draw\n");
        }

        else if (player_choice == 1 && computer_choice == 3) {
            printf("Computer chose scissors you win\n");
        }

        else if (player_choice == 3 && computer_choice == 1) {
            printf("Computer chose rock you lose\n");
        }

        else {
            printf("unfinished\n");
        }
    }

    exit(0);
}

int rand() {
    if (buff_idx > 99) {
        buff_idx = 0;
    }

    printf("%d\n", random_buff[buff_idx]);

    return random_buff[buff_idx++];
}