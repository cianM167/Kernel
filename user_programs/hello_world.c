#include <stdio.h>
// #include <stdlib.h>
// clang --target=x86_64-linux-musl -static -no-pie -fno-pie hello_world.c -o hello_world.elf
// for musl compilation 

int main() {
    printf("Hello from glibc!\n");
    printf("Testing malloc: ");
    
    // char* buf = malloc(100);
    // if (buf) {
    //     snprintf(buf, 100, "allocated at %p", buf);
    //     printf("%s\n", buf);
    //     free(buf);
    // }
    
    return 42;
}