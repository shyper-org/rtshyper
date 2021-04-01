build:
	RUSTFLAGS="-C llvm-args=-global-isel=false" \
	cargo build -Z build-std=core,alloc --target aarch64.json
	aarch64-linux-gnu-objdump -d target/aarch64/debug/rust_hypervisor > target/aarch64/debug/t.txt

run:
	/usr/share/qemu/bin/qemu-system-aarch64 \
		-machine virt,virtualization=on,gic-version=2\
		-m 8g \
		-cpu cortex-a57 \
		-smp 8 \
		-nographic \
		-kernel target/aarch64/debug/rust_hypervisor

gdb:
	aarch64-linux-gnu-gdb -x gdb/aarch64.gdb

clean:
	rm -rf target