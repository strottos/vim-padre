A collection of programs that were compiled to be minimal ELF programs on Linux x86_64. This means some unit tests will only run on those platforms for now, ideally we'd find a better way of doing this, for now those on other platforms (myself included) will need to run in docker containers to get *all* unit tests to pass.

The programs were compiled by having `.asm` files as follows:
```
section .data
    node_ver db "v8.10.1", 10
    node_ver_len equ $ - node_ver
section .text
    global _start
    _start:
        mov rax, 1
        mov rdi, 1
        mov rsi, node_ver
        mov rdx, node_ver_len
        syscall
        mov rax, 60
        mov rdi, 0
        syscall
```
We can then compile this as follows:
```
nasm -w+all -f elf64 -o node.o node.asm
ld -o node
```
and we strip out extras for a tiny binary:
```
strip node
```
This way we get a minimal binary at the end that we can commit to git.

The lldb-server binary needs to send to stderr so this has the following variant:
```
section .data
    lldb_ver db `lldb version 6.0.0`, 10
    lldb_ver_len equ $ - lldb_ver
section .text
    global _start
    _start:
        mov rax, 1
        mov rdi, 2
        mov rsi, lldb_ver
        mov rdx, lldb_ver_len
        syscall
        mov rax, 60
        mov rdi, 0
        syscall
```
