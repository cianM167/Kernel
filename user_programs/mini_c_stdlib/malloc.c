#include "syscalls.h"
#include "malloc.h"

typedef struct block {
    struct block* next;
    unsigned long size;
    int free;
} Block;

#define ALIGN8(x) (((x) + 7) & ~7)
#define HEADER_SIZE (sizeof(Block))

static Block* heap_head = 0;
static void* heap_base = 0;

static Block* request_space(unsigned long size) {
    void* cur = brk(0);

    if (heap_base == 0) heap_base = cur;

    void* new = (char*)cur + HEADER_SIZE + size;
    void* got = brk(new);

    if (got != new) return 0;

    Block* b = (Block*)cur;
    b->size = size;
    b->free = 0;
    b->next = 0;
    return b;
}

void* malloc(unsigned long size) {
    printf("entered malloc\n");
    if (size == 0) return 0;
    size = ALIGN8(size);

    Block* prev = 0;
    Block* curr = heap_head;

    while (curr) {
        if (curr->free && curr->size >= size) {
            curr->free = 0;
            return (void*)(curr + 1);
        }
        prev = curr;
        curr = curr->next;
    }

    Block* b = request_space(size);
    printf("malloc end\n");
    if (!b) return 0;

    if (prev) prev->next = b;
    else heap_head = b;

    printf("malloc end\n");
    return (void*)(b + 1);
}

void free(void* ptr) {
    if (!ptr) return;

    Block* b = (Block*)ptr - 1;     // step back to the header
    b->free = 1;

    // coalesce adjacent free blocks to avoid fragmentation
    Block* curr = heap_head;
    while (curr && curr->next) {
        if (curr->free && curr->next->free) {
            curr->size += HEADER_SIZE + curr->next->size;
            curr->next  = curr->next->next;
        } else {
            curr = curr->next;
        }
    }
}

void* calloc(unsigned long nmemb, unsigned long size) {
    unsigned long total = nmemb * size;
    void* ptr = malloc(total);
    if (!ptr) return 0;

    // zero the allocation
    char* p = (char*)ptr;
    for (unsigned long i = 0; i < total; i++) p[i] = 0;
    return ptr;
}

void* realloc(void* ptr, unsigned long size) {
    if (!ptr)   return malloc(size);
    if (!size) { free(ptr); return 0; }

    Block* b = (Block*)ptr - 1;
    if (b->size >= size) return ptr;// already big enough

    void* new = malloc(size);
    if (!new) return 0;

    // copy old data
    char* src = (char*)ptr;
    char* dst = (char*)new;
    for (unsigned long i = 0; i < b->size; i++) dst[i] = src[i];

    free(ptr);
    return new;
}

static inline void* brk(void* addr) {
    register long rax __asm__("rax") = 12;
    register long rdi __asm__("rdi") = (long)addr;

    asm volatile (
        "syscall"
        : "+r"(rax)
        : "r"(rdi)
        : "rcx", "r11", "memory"
    );

    return (void*)rax;
}