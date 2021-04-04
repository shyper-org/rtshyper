use crate::driver::putc;

use crate::lib::{BitAlloc256, BitAlloc4K, BitMap};
use spin::Mutex;

pub const INTERRUPT_IRQ_HYPERVISOR_TIMER: usize = 26;
pub const INTERRUPT_IRQ_GUEST_TIMER: usize = 27;
pub const INTERRUPT_IRQ_IPI: usize = 1;

pub const INTERRUPT_HYPER_BITMAP: Mutex<BitMap<BitAlloc256>> = Mutex::new(BitAlloc4K::default());
pub const INTERRUPT_GLB_BITMAP: Mutex<BitMap<BitAlloc256>> = Mutex::new(BitAlloc4K::default());

fn interrupt_reserve_int(int_id: usize, handler: usize) {}

fn interrupt_cpu_enable(int_id: usize, en: bool) {
    use crate::arch::interrupt_arch_enable;
    interrupt_arch_enable(int_id, en);
}

pub fn interrupt_init() {
    use crate::arch::interrupt_arch_init;
    interrupt_arch_init();

    let cpu_id = super::cpu_id();
    if cpu_id == 0 {
        // TODO: change handler
        interrupt_reserve_int(INTERRUPT_IRQ_IPI, 0);
    }
    interrupt_cpu_enable(INTERRUPT_IRQ_IPI, true);
}
