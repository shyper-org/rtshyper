use crate::driver::putc;

use crate::lib::{BitAlloc, BitAlloc256, BitAlloc4K, BitMap};
use spin::Mutex;

pub const INTERRUPT_NUM_MAX: usize = 1024;
pub const INTERRUPT_IRQ_HYPERVISOR_TIMER: usize = 26;
pub const INTERRUPT_IRQ_GUEST_TIMER: usize = 27;
pub const INTERRUPT_IRQ_IPI: usize = 1;

pub static INTERRUPT_HYPER_BITMAP: Mutex<BitMap<BitAlloc256>> = Mutex::new(BitAlloc4K::default());
pub static INTERRUPT_GLB_BITMAP: Mutex<BitMap<BitAlloc256>> = Mutex::new(BitAlloc4K::default());
pub static INTERRUPT_HANDLERS: Mutex<[InterruptHandler; INTERRUPT_NUM_MAX]> =
    Mutex::new([InterruptHandler::None; INTERRUPT_NUM_MAX]);

#[derive(Copy, Clone)]
pub enum InterruptHandler {
    IpiIrqHandler(fn()),
    GicMaintenanceHandler(fn(usize, usize)),
    TimeIrqHandler(fn(usize, usize)),
    None,
}

impl InterruptHandler {
    pub fn call(&self, arg0: usize, arg1: usize) {
        match self {
            InterruptHandler::IpiIrqHandler(irq_handler) => irq_handler(),
            InterruptHandler::GicMaintenanceHandler(gic_handler) => gic_handler(arg0, arg1),
            InterruptHandler::TimeIrqHandler(time_handler) => time_handler(arg0, arg1),
            InterruptHandler::None => panic!("Call An Empty Interrupt Hanlder!"),
        }
    }
}

fn interrupt_reserve_int(int_id: usize, handler: InterruptHandler) {
    if int_id < INTERRUPT_NUM_MAX {
        let mut irq_handler_lock = INTERRUPT_HANDLERS.lock();
        let mut hyper_bitmap_lock = INTERRUPT_HYPER_BITMAP.lock();
        let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
        irq_handler_lock[int_id] = handler;
        use crate::lib::{BitAlloc16, BitAlloc256, BitAlloc4K, BitMap};
        (*hyper_bitmap_lock).set(int_id);
        (*glb_bitmap_lock).set(int_id);
        drop(irq_handler_lock);
        drop(hyper_bitmap_lock);
        drop(glb_bitmap_lock);
    }
}

fn interrupt_cpu_enable(int_id: usize, en: bool) {
    use crate::arch::interrupt_arch_enable;
    interrupt_arch_enable(int_id, en);
}

// TODO
fn ipi_irq_handler() {}

pub fn interrupt_init() {
    use crate::arch::interrupt_arch_init;
    interrupt_arch_init();

    let cpu_id = super::cpu_id();
    println!("cpu id is {}", cpu_id);
    if cpu_id == 0 {
        // TODO: change handler
        interrupt_reserve_int(
            INTERRUPT_IRQ_IPI,
            InterruptHandler::IpiIrqHandler(ipi_irq_handler),
        );
    }
    interrupt_cpu_enable(INTERRUPT_IRQ_IPI, true);
}
