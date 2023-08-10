# RT-Shyper

forked form [Rust-Shyper](https://gitee.com/openeuler/rust_shyper)

## How to Build
First, install the [Rust](https://www.rust-lang.org/tools/install) toolchain. 

For cross-compiling, install the gcc-multilib (on Ubuntu)

Install clang. Install u-boot-tools to use `mkimage`

```bash
sudo apt install -y gcc-multilib u-boot-tools clang
```

Download [aarch64-none-elf toolchain](https://developer.arm.com/downloads/-/gnu-a), and add it to PATH.

Install [cargo-binutils](https://github.com/rust-embedded/cargo-binutils) to use `rust-objcopy` and `rust-objdump` tools:

```bash
cargo install cargo-binutils
```

Simply run `make`

```bash
make <platform> [PROFILE=debug|release] [FEATURES=...]
```
