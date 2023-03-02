# Path
DISK = ../rust_hypervisor/vm0.img

# Compile
ARCH ?= aarch64
PROFILE ?= release
# features, seperate with comma `,`
FEATURES =

# Toolchain
TOOLCHAIN=aarch64-none-elf
GDB = ${TOOLCHAIN}-gdb
OBJDUMP = rust-objdump
OBJCOPY = rust-objcopy
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
	${OBJCOPY} ${TARGET_DIR}/${IMAGE} -O binary ${TARGET_DIR}/${IMAGE}.bin
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

QEMU_COMMON_OPTIONS = -machine virt,virtualization=on,gic-version=2 \
	-m 8g -cpu cortex-a57 -smp 4 -display none -global virtio-mmio.force-legacy=false \
	-kernel ${TARGET_DIR}/${IMAGE}.bin

QEMU_SERIAL_OPTIONS = -serial stdio #\
	-serial telnet:localhost:12345,server

QEMU_NETWORK_OPTIONS = -netdev user,id=n0,hostfwd=tcp::5555-:22 -device virtio-net-device,bus=virtio-mmio-bus.24,netdev=n0

QEMU_DISK_OPTIONS = -drive file=${DISK},if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.25

run: qemu
	${QEMU} ${QEMU_COMMON_OPTIONS} ${QEMU_SERIAL_OPTIONS} ${QEMU_NETWORK_OPTIONS} ${QEMU_DISK_OPTIONS}

debug: qemu
	${QEMU} ${QEMU_COMMON_OPTIONS} ${QEMU_SERIAL_OPTIONS} ${QEMU_NETWORK_OPTIONS} ${QEMU_DISK_OPTIONS} \
		-s -S

gdb:
	${GDB} -x gdb/aarch64.gdb

clean:
	cargo clean
