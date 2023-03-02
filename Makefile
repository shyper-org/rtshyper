# Path
DISK = 

# Compile
ARCH ?= aarch64
PROFILE ?= release
# features, seperate with comma `,`
FEATURES =

# Toolchain
TOOLCHAIN=aarch64-none-elf
GDB = ${TOOLCHAIN}-gdb
OBJDUMP = rust-objdump
QEMU = qemu-system-aarch64

IMAGE=rtshyper_rs

TARGET_DIR=target/${ARCH}/${PROFILE}

# Cargo flags.
ifeq (${PROFILE}, release)
CARGO_FLAGS := ${CARGO_FLAGS} --release --no-default-features
else
CARGO_FLAGS := ${CARGO_FLAGS} --no-default-features
endif

.PHONY: qemu tx2 pi4 clippy fmt run debug clean gdb

qemu:
	cargo build --target ${ARCH}.json ${CARGO_FLAGS} --features $@
	${OBJDUMP} --demangle -d ${TARGET_DIR}/${IMAGE} > ${TARGET_DIR}/t.txt

tx2:
	cargo build --target ${ARCH}.json ${CARGO_FLAGS} --features $@,${FEATURES}
	bash upload ${PROFILE}
	${OBJDUMP} --demangle -d ${TARGET_DIR}/${IMAGE} > ${TARGET_DIR}/t.txt

pi4:
	cargo build --target ${ARCH}.json ${CARGO_FLAGS} --features $@
	bash pi4_upload ${PROFILE}
	${OBJDUMP} --demangle -d ${TARGET_DIR}/${IMAGE} > ${TARGET_DIR}/t.txt

clippy:
	cargo clippy --target ${ARCH}.json ${CARGO_FLAGS} --features tx2
	cargo fmt

fmt:
	cargo fmt

QEMU_OPTIONS = -machine virt,virtualization=on,gic-version=2\
		-drive file=${DISK},if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-m 8g \
		-cpu cortex-a57 \
		-smp 8 \
		-kernel ${TARGET_DIR}/${IMAGE} \
		-global virtio-mmio.force-legacy=false \
		-serial stdio \
		-serial tcp:127.0.0.1:12345 \
		-serial tcp:127.0.0.1:12346 \
		-display none

run:
	${QEMU} ${QEMU_OPTIONS}

debug:
	${QEMU} ${QEMU_OPTIONS}\
		-s -S

gdb:
	${GDB} -x gdb/aarch64.gdb

clean:
	cargo clean
