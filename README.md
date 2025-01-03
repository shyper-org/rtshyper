# RT-Shyper

forked form [Rust-Shyper](https://gitee.com/openeuler/rust_shyper)

An embedded hypervisor for mixed-critical system.
- Fully based on Rust.
- High performance and reliability.
- Virtio (blk, net and console) support.
- Supporting strong memory isolation (LLC isolation based on coloring and memory bandwidth control).

## Supported Platform

The list of supported (and work in progress) platforms is presented below:

**aarch64**
- [x] NVIDIA Jetson TX2
- [ ] (work-in-progress) Raspberry Pi 4 Model B
- [ ] (work-in-progress) QEMU

## Build Dependencies

First, install the [Rust](https://www.rust-lang.org/tools/install) toolchain. 

For cross-compiling, install the gcc-multilib (on Ubuntu)

Install **clang** and **LLVM** toolchain. 

Install u-boot-tools to use `mkimage`

```bash
sudo apt install gcc-multilib u-boot-tools clang llvm
```

If you want to build the C library with other cross compiling toolchain, for example, [aarch64-none-elf toolchain](https://developer.arm.com/downloads/-/gnu-a), you can add it to **PATH** and set the **CROSS_COMPILE** on the command line.

## How to Build

Simply run `make`

```bash
make [LLVM=1] [CARGO_ACTION=build|clippy|fix|...] [PROFILE=release|debug] [FEATURES=...] <platform>
```

## Rust FFI programming with C
only freestanding C headers are available: <float.h>, <iso646.h>, <limits.h>, <stdarg.h>, <stdbool.h>, <stddef.h>, and <stdint.h>

see http://cs107e.github.io/guides/gcc/

## See more
[Unishyper](https://gitee.com/unishyper/unishyper): A reliable Rust-based unikernel for embedded scenarios.
