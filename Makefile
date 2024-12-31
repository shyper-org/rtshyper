# Compile
ARCH ?= aarch64
PROFILE ?= release
BOARD ?= tx2
# features, seperate with comma `,`
FEATURES ?=
export TEXT_START ?= 0x83000000

# CROSS_COMPILE can be set on the command line
export CROSS_COMPILE ?= aarch64-none-elf-
export CFLAGS := -ffreestanding -mgeneral-regs-only

# export CC for C-library cross compiling
ifneq ($(LLVM),)
unexport CROSS_COMPILE
export CC	= clang
LD			= ld.lld
AR			= llvm-ar
NM			= llvm-nm
READEL		= llvm-readelf
STRIP		= llvm-strip
OBJDUMP		= llvm-objdump
OBJCOPY		= llvm-objcopy
GDB			= lldb
else
export CC	= $(CROSS_COMPILE)gcc
LD			= $(CROSS_COMPILE)ld
AR			= $(CROSS_COMPILE)ar
NM			= $(CROSS_COMPILE)nm
READELF		= $(CROSS_COMPILE)readelf
STRIP		= $(CROSS_COMPILE)strip
OBJDUMP		= $(CROSS_COMPILE)objdump
OBJCOPY		= $(CROSS_COMPILE)objcopy
GDB			= ${CROSS_COMPILE}gdb
endif
QEMU = qemu-system-$(ARCH)

IMAGE=rtshyper_rs

export EFISTUB
ifeq ($(EFISTUB),)
unexport EFISTUB
CARGO_TARGET = ${ARCH}.json
else
CARGO_TARGET = ${ARCH}-unknown-uefi
IMAGE := $(IMAGE).efi
FEATURES := $(FEATURES),efi-stub
endif

TARGET_DIR=target/$(basename $(CARGO_TARGET))/${PROFILE}

APP = ${TARGET_DIR}/${IMAGE}

TFTP_SERVER ?= root@192.168.106.153:/tftp

UBOOT_IMAGE ?= RTShyperImage

# Cargo flags.
CARGO_FLAGS += -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem --target $(CARGO_TARGET) --no-default-features --features "${BOARD},${FEATURES}"
ifeq (${PROFILE}, release)
CARGO_FLAGS += --release
else ifneq (${PROFILE}, debug)
$(error bad PROFILE: ${PROFILE})
endif

.PHONY: build clippy upload qemu tx2 pi4 run debug clean gdb

clippy:
	cargo clippy ${CARGO_FLAGS}

build:
	cargo fmt
	cargo build ${CARGO_FLAGS}
	${OBJDUMP} --demangle -d $(APP) > ${TARGET_DIR}/t.asm
	${OBJCOPY} $(APP) -O binary $(APP).bin

upload: build
	@mkimage -n ${IMAGE} -A arm64 -O linux -T kernel -C none -a $(TEXT_START) -e $(TEXT_START) -d $(APP).bin ${TARGET_DIR}/${UBOOT_IMAGE}
	@echo "*** Upload Image ${UBOOT_IMAGE} ***"
	@scp ${TARGET_DIR}/${UBOOT_IMAGE} ${TFTP_SERVER}/${UBOOT_IMAGE}
	@echo "tftp 0x8a000000 \$${serverip}:${UBOOT_IMAGE}; bootm start 0x8a000000 - 0x80000000; bootm loados; bootm go"

qemu:
	$(MAKE) build BOARD=qemu TEXT_START=0x40080000

tx2:
	$(MAKE) upload BOARD=tx2 TEXT_START=0x83000000

pi4:
	$(MAKE) upload BOARD=pi4 TEXT_START=0xF0080000
	scp ./image/pi4_fin.dtb ${TFTP_SERVER}/pi4_dtb

clean:
	cargo clean

###########################
# QEMU setup
###########################

# DISK is the path to the ROOTFS for VM0 when `make run` on QEMU
# DISK can be set on the command line
DISK ?= vm0.img

BIOS ?= QEMU_EFI.fd

PARTITION_PREFIX = esp/efi/boot/

ifeq ($(ARCH),aarch64)
QEMU_OPTIONS += -machine virt,virtualization=on,gic-version=2 -m 8g -cpu cortex-a57 -smp 4
PARTITION = $(PARTITION_PREFIX)/bootaa64.efi
else
$(error bad arch: $(ARCH))
endif

QEMU_OPTIONS += -display none -global virtio-mmio.force-legacy=false -kernel ${TARGET_DIR}/${IMAGE}.bin

QEMU_SERIAL_OPTIONS = -serial mon:stdio #\
	-serial telnet:localhost:12345,server
QEMU_OPTIONS += $(QEMU_SERIAL_OPTIONS)

QEMU_NETWORK_OPTIONS = -netdev user,id=n0,hostfwd=tcp::5555-:22 -device virtio-net-device,bus=virtio-mmio-bus.24,netdev=n0
QEMU_OPTIONS += $(QEMU_NETWORK_OPTIONS)

QEMU_DISK_OPTIONS = -drive file=${DISK},if=none,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.25
QEMU_OPTIONS += $(QEMU_DISK_OPTIONS)

ifeq ($(EFISTUB),)
QEMU_OPTIONS += -kernel $(APP).bin
else
# bios
QEMU_OPTIONS += -bios $(BIOS)
QEMU_OPTIONS += -drive format=raw,file=fat:rw:esp
endif

QEMU_EFI.fd:
	@wget https://releases.linaro.org/components/kernel/uefi-linaro/latest/release/qemu64/QEMU_EFI.fd -O $@

ifneq ($(EFISTUB),)
$(PARTITION): qemu
	mkdir -p $(dir $@)
	cp $(APP) $@

run: $(BIOS) $(PARTITION)
else
run: qemu
endif
ifeq ($(wildcard $(DISK)),)
	$(error $(DISK) not found)
else
	$(QEMU) ${QEMU_OPTIONS}
endif

debug: QEMU_OPTIONS += -s -S
debug: run

gdb:
	${GDB} -x gdb/${ARCH}.gdb
