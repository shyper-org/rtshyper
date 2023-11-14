# Path
DISK = ../rust_hypervisor/vm0.img

# Compile
ARCH ?= aarch64
PROFILE ?= release
BOARD ?= tx2
# features, seperate with comma `,`
FEATURES =

# Toolchain
TOOLCHAIN=aarch64-none-elf
GDB = ${TOOLCHAIN}-gdb
OBJDUMP = rust-objdump
OBJCOPY = rust-objcopy
QEMU = qemu-system-aarch64
# used for C-library cross compiling
export CROSS_COMPILE ?= ${TOOLCHAIN}-
export CFLAGS := -mgeneral-regs-only

IMAGE=rtshyper_rs

TARGET_DIR=target/${ARCH}/${PROFILE}

TFTP_SERVER ?= root@192.168.106.153:/tftp

UBOOT_IMAGE ?= RTShyperImage

define tftp_upload
	@${OBJCOPY} ${TARGET_DIR}/${IMAGE} -O binary ${TARGET_DIR}/${IMAGE}.bin
	@mkimage -n ${IMAGE} -A arm64 -O linux -T kernel -C none -a $(1) -e $(1) -d ${TARGET_DIR}/${IMAGE}.bin ${TARGET_DIR}/${UBOOT_IMAGE}
	@echo "*** Upload Image ${UBOOT_IMAGE} ***"
	@scp ${TARGET_DIR}/${UBOOT_IMAGE} ${TFTP_SERVER}/${UBOOT_IMAGE}
	@echo "tftp 0x8a000000 \$${serverip}:${UBOOT_IMAGE}; bootm start 0x8a000000 - 0x80000000; bootm loados; bootm go"
endef

# Cargo flags.
CARGO_FLAGS ?= -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem --target ${ARCH}.json --no-default-features --features ${BOARD},${FEATURES}
ifeq (${PROFILE}, release)
CARGO_FLAGS := ${CARGO_FLAGS} --release
endif

.PHONY: build qemu tx2 pi4 clippy fmt run debug clean gdb

build:
	cargo build ${CARGO_FLAGS}
	${OBJDUMP} --demangle -d ${TARGET_DIR}/${IMAGE} > ${TARGET_DIR}/t.asm

qemu:
	$(MAKE) build BOARD=qemu
	${OBJCOPY} ${TARGET_DIR}/${IMAGE} -O binary ${TARGET_DIR}/${IMAGE}.bin

tx2:
	$(MAKE) build BOARD=tx2
	$(call tftp_upload,0x83000000) 

pi4:
	$(MAKE) build BOARD=pi4
	$(call tftp_upload,0xF0080000)
	scp ./image/pi4_fin.dtb ${TFTP_SERVER}/pi4_dtb

clippy:
	cargo fmt
	cargo clippy --no-deps ${CARGO_FLAGS} # --allow-dirty --allow-staged --fix -- -A clippy::all -W clippy::<xxx>

fmt:
	cargo fmt

QEMU_COMMON_OPTIONS = -machine virt,virtualization=on,gic-version=2 \
	-m 8g -cpu cortex-a57 -smp 4 -display none -global virtio-mmio.force-legacy=false \
	-kernel ${TARGET_DIR}/${IMAGE}.bin

QEMU_SERIAL_OPTIONS = -serial mon:stdio #\
	-serial telnet:localhost:12345,server

QEMU_NETWORK_OPTIONS = -netdev user,id=n0,hostfwd=tcp::5555-:22 -device virtio-net-device,bus=virtio-mmio-bus.24,netdev=n0

QEMU_DISK_OPTIONS = -drive file=${DISK},if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.25

run: qemu
	${QEMU} ${QEMU_COMMON_OPTIONS} ${QEMU_SERIAL_OPTIONS} ${QEMU_NETWORK_OPTIONS} ${QEMU_DISK_OPTIONS}

debug: qemu
	${QEMU} ${QEMU_COMMON_OPTIONS} ${QEMU_SERIAL_OPTIONS} ${QEMU_NETWORK_OPTIONS} ${QEMU_DISK_OPTIONS} \
		-s -S

gdb:
	${GDB} -x gdb/${ARCH}.gdb

clean:
	cargo clean
