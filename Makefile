# Path
DISK = /home/ohmr/work/hypervisor/disk-img/disk.img

# Compile
ARCH ?= aarch64
BUILD_STD = core,alloc

# Toolchain
TOOLCHAIN=aarch64-linux-gnu
QEMU = /usr/share/qemu/bin/qemu-system-aarch64
GDB = ${TOOLCHAIN}-gdb
OBJDUMP = ${TOOLCHAIN}-objdump

qemu_debug:
	cargo build -Z build-std=${BUILD_STD} --target aarch64-qemu.json --features qemu
	${OBJDUMP} -d target/aarch64/debug/rust_hypervisor > target/aarch64/debug/t.txt

qemu_release:
	cargo build -Z build-std=${BUILD_STD} --target aarch64-qemu.json --features qemu --release
	${OBJDUMP} -d target/aarch64/release/rust_hypervisor > target/aarch64/release/t.txt

tx2:
	cargo build -Z build-std=${BUILD_STD} --target aarch64-tx2.json --features tx2
	bash upload
	${OBJDUMP} -d target/aarch64-tx2/debug/rust_hypervisor > target/aarch64-tx2/debug/t.txt

tx2_release:
	cargo build -Z build-std=${BUILD_STD} --target aarch64-tx2.json --features tx2 --release
	bash upload_release
	${OBJDUMP} -d target/aarch64-tx2/release/rust_hypervisor > target/aarch64-tx2/release/t.txt

tx2_update:
	cargo build -Z build-std=${BUILD_STD} --target aarch64-tx2-update.json --features "tx2 update" --release
	bash upload_update
	${OBJDUMP} -d target/aarch64-tx2-update/release/rust_hypervisor > target/aarch64-tx2-update/release/update.txt

pi4_release:
	cargo build -Z build-std=${BUILD_STD} --target aarch64-pi4.json --features pi4 --release
	bash pi4_upload_release
	${OBJDUMP} -d target/aarch64-pi4/release/rust_hypervisor > target/aarch64-pi4/release/t.txt

run:
	${QEMU} \
		-machine virt,virtualization=on,gic-version=2\
		-drive file=${DISK},if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-m 8g \
		-cpu cortex-a57 \
		-smp 8 \
		-kernel target/aarch64/debug/rust_hypervisor \
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
		-kernel target/aarch64/release/rust_hypervisor \
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
		-kernel target/aarch64/debug/rust_hypervisor \
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
