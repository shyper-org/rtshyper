// TODO
pub fn gic_maintenance_handler(arg: usize, source: usize) {}

// TODO
use crate::device::EmuContext;
pub fn emu_intc_handler(emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    true
}
