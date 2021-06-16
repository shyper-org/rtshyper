disk = /home/ohmr/work/hypervisor/disk-img

qemu:
	RUSTFLAGS="-C llvm-args=-global-isel=false" \
	cargo build -Z build-std=core,alloc --target aarch64.json --features qemu
	aarch64-linux-gnu-objdump -d target/aarch64/debug/rust_hypervisor > target/aarch64/debug/t.txt

tx2:
	RUSTFLAGS="-C llvm-args=-global-isel=false" \
	cargo build -Z build-std=core,alloc --target aarch64.json --features tx2
	aarch64-linux-gnu-objdump -d target/aarch64/debug/rust_hypervisor > target/aarch64/debug/t.txt

release:
	RUSTFLAGS="-C llvm-args=-global-isel=false" \
	cargo build -Z build-std=core,alloc --target aarch64.json --release --features tx2
	aarch64-linux-gnu-objdump -d target/aarch64/debug/rust_hypervisor > target/aarch64/debug/t.txt

run:
	/usr/share/qemu/bin/qemu-system-aarch64 \
		-machine virt,virtualization=on,gic-version=2\
		-drive file=${disk}/disk.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-m 8g \
		-cpu cortex-a57 \
		-smp 8 \
		-kernel target/aarch64/debug/rust_hypervisor \
		-global virtio-mmio.force-legacy=false \
		-serial stdio \
		-serial tcp:127.0.0.1:12345 \
		-serial tcp:127.0.0.1:12346 \
		-display none

debug:
	/usr/share/qemu/bin/qemu-system-aarch64 \
		-machine virt,virtualization=on,gic-version=2\
		-drive file=${disk}/disk.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
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
	aarch64-linux-gnu-gdb -x gdb/aarch64.gdb

clean:
	rm -rf target