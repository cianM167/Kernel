void* malloc(unsigned long size);
void  free(void* ptr);
void* calloc(unsigned long nmemb, unsigned long size);
void* realloc(void* ptr, unsigned long size);
static inline void* brk(void* addr);