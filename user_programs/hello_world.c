#include <stdio.h>
#include <stdlib.h>

int main() {
    printf("Hello from glibc!\n");
    printf("Testing malloc: ");
    
    char* buf = malloc(100);
    if (buf) {
        snprintf(buf, 100, "allocated at %p", buf);
        printf("%s\n", buf);
        free(buf);
    }
    
    return 42;
}