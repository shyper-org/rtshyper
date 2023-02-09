# Path
DISK = /home/ohmr/work/hypervisor/disk-img/disk.img

# Compile
ARCH ?= aarch64
BUILD_STD = core,alloc
PROFILE ?= release
# features, seperate with comma `,`
FEATURES =

# Toolchain
TOOLCHAIN=aarch64-none-elf
QEMU = /usr/share/qemu/bin/qemu-system-aarch64
GDB = ${TOOLCHAIN}-gdb
OBJDUMP = ${TOOLCHAIN}-objdump

IMAGE=rtshyper_rs

# Cargo flags.
ifeq (${PROFILE}, release)
CARGO_FLAGS := ${CARGO_FLAGS} --release --no-default-features
else
CARGO_FLAGS := ${CARGO_FLAGS} --no-default-features
endif

qemu_debug:
	cargo build -Z build-std=${BUILD_STD} --target aarch64-qemu.json --features qemu
	${OBJDUMP} --demangle -d target/aarch64/debug/${IMAGE} > target/aarch64/debug/t.txt

qemu_release:
	cargo build -Z build-std=${BUILD_STD} --target aarch64-qemu.json --features qemu --release
	${OBJDUMP} --demangle -d target/aarch64/release/${IMAGE} > target/aarch64/release/t.txt

tx2:
	cargo build --target ${ARCH}-$@.json -Z build-std=${BUILD_STD} ${CARGO_FLAGS} --features $@,${FEATURES}
	bash upload ${PROFILE}
	${OBJDUMP} --demangle -d target/${ARCH}-$@/${PROFILE}/${IMAGE} > target/${ARCH}-$@/${PROFILE}/t.txt

tx2_update:
	cargo build --target ${ARCH}-tx2-update.json -Z build-std=${BUILD_STD} ${CARGO_FLAGS} --features "tx2 update"
	bash upload_update
	${OBJDUMP} --demangle -d target/${ARCH}-tx2-update/${PROFILE}/${IMAGE} > target/${ARCH}-tx2-update/${PROFILE}/update.txt

pi4:
	cargo build --target ${ARCH}-$@.json -Z build-std=${BUILD_STD} ${CARGO_FLAGS} --features $@
	bash pi4_upload ${PROFILE}
	${OBJDUMP} --demangle -d target/${ARCH}-$@/${PROFILE}/${IMAGE} > target/${ARCH}-$@/${PROFILE}/t.txt

run:
	${QEMU} \
		-machine virt,virtualization=on,gic-version=2\
		-drive file=${DISK},if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-m 8g \
		-cpu cortex-a57 \
		-smp 8 \
		-kernel target/aarch64/debug/${IMAGE} \
		-global virtio-mmio.force-legacy=false \
		-serial stdio \
		-serial tcp:127.0.0.1:12345 \
		-serial tcp:127.0.0.1:12346 \
		-display none

run_release:
	${QEMU} \
		-machine virt,virtualization=on,gic-version=2\
		-drive file=${DISK},if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-m 8g \
		-cpu cortex-a57 \
		-smp 8 \
		-kernel target/aarch64/release/${IMAGE} \
		-global virtio-mmio.force-legacy=false \
		-serial stdio \
		-serial tcp:127.0.0.1:12345 \
		-serial tcp:127.0.0.1:12346 \
		-display none

debug:
	${QEMU} \
		-machine virt,virtualization=on,gic-version=2\
		-drive file=${DISK},if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-m 8g \
		-cpu cortex-a57 \
		-smp 8 \
		-kernel target/aarch64/debug/${IMAGE} \
		-global virtio-mmio.force-legacy=false \
		-serial stdio \
		-serial tcp:127.0.0.1:12345 \
		-serial tcp:127.0.0.1:12346 \
		-display none \
		-s -S


gdb:
	${GDB} -x gdb/aarch64.gdb

clean:
	cargo clean
