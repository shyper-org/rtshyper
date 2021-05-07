// pub const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
// pub const VIRTIO_MMIO_VERSION: usize = 0x004;
// pub const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
// pub const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
use crate::device::EmuContext;
use crate::kernel::Vm;

pub fn emu_virtio_mmio_init(vm: Vm, emu_dev_id: usize) -> bool {
    // unimplemented!();
    true
}

pub fn emu_virtio_mmio_handler(emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    unimplemented!();
}
