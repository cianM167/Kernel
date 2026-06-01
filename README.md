# Instructions
## Writing userspace programs
The kernel has a limited number of syscalls callable by user programs.
To make these easier to use there is a small C standard library included with the kernel
C Lib:
```
user_programs/mini_c_stdlib
```

You can include these using the headers:
```C
#include "mini_c_stdlib/syscalls.h"
#include "mini_c_stdlib/malloc.h"
```
