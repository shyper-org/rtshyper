# RT-Shyper

forked form [Rust-Shyper](https://gitee.com/openeuler/rust_shyper)

## How to Build
For cross-compiling, install the gcc-multilib (on Ubuntu)

```bash
sudo apt install -y gcc-multilib
```

Install [cargo-binutils](https://github.com/rust-embedded/cargo-binutils) to use `rust-objcopy` and `rust-objdump` tools:

```bash
cargo install cargo-binutils
```

Install u-boot-tools to use `mkimage`

```bash
sudo apt install -y u-boot-tools
```

Simply run `make`

```bash
make <platform> [PROFILE=debug|release] [FEATURES=...]
```
