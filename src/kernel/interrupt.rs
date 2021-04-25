use crate::arch::{interrupt_arch_ipi_send, interrupt_arch_vm_inject};
use crate::driver::putc;
use crate::kernel::Vm;
use crate::kernel::{cpu_id, ipi_irq_handler};
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

pub fn interrupt_cpu_ipi_send(target_cpu: usize, ipi_id: usize) {
    interrupt_arch_ipi_send(target_cpu, ipi_id);
}

pub fn interrupt_reserve_int(int_id: usize, handler: InterruptHandler) {
    if int_id < INTERRUPT_NUM_MAX {
        let mut irq_handler_lock = INTERRUPT_HANDLERS.lock();
        let mut hyper_bitmap_lock = INTERRUPT_HYPER_BITMAP.lock();
        let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
        irq_handler_lock[int_id] = handler;
        use crate::lib::{BitAlloc16, BitAlloc256, BitAlloc4K, BitMap};
        hyper_bitmap_lock.set(int_id);
        glb_bitmap_lock.set(int_id);
    }
}

fn interrupt_is_reserved(int_id: usize) -> bool {
    let mut hyper_bitmap_lock = INTERRUPT_HYPER_BITMAP.lock();
    hyper_bitmap_lock.get(int_id) != 0
}

pub fn interrupt_cpu_enable(int_id: usize, en: bool) {
    use crate::arch::interrupt_arch_enable;
    interrupt_arch_enable(int_id, en);
}

pub fn interrupt_init() {
    use crate::arch::interrupt_arch_init;
    interrupt_arch_init();

    let cpu_id = super::cpu_id();
    if cpu_id == 0 {
        interrupt_reserve_int(
            INTERRUPT_IRQ_IPI,
            InterruptHandler::IpiIrqHandler(ipi_irq_handler),
        );
    }
    interrupt_cpu_enable(INTERRUPT_IRQ_IPI, true);
}

use crate::arch::{interrupt_arch_vm_register, GIC_PRIVINT_NUM};
pub fn interrupt_vm_register(vm: Vm, id: usize) -> bool {
    println!("VM {} register interrupt {}", vm.vm_id(), id);
    let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
    if glb_bitmap_lock.get(id) != 0 && id > GIC_PRIVINT_NUM {
        println!(
            "interrupt_vm_register: VM {} interrupts conflict, id = {}",
            vm.vm_id(),
            id
        );
        return false;
    }

    interrupt_arch_vm_register(vm.clone(), id);
    vm.set_int_bit_map(id);
    glb_bitmap_lock.set(id);
    true
}

fn interrupt_vm_inject(vm: Vm, id: usize, source: usize) {
    interrupt_arch_vm_inject(vm, id, source);
}

pub fn interrupt_handler(int_id: usize, src: usize) -> bool {
    use crate::kernel::active_vm;
    match active_vm() {
        Ok(vm) => {
            if vm.has_interrupt(int_id) {
                // TODO: interrupt_handler
                interrupt_vm_inject(vm.clone(), int_id, src);
                return false;
            }
        }
        Err(_) => {}
    }

    if interrupt_is_reserved(int_id) {
        let mut irq_handler = INTERRUPT_HANDLERS.lock();
        match irq_handler[int_id] {
            InterruptHandler::IpiIrqHandler(irq_handler) => {
                irq_handler();
            }
            InterruptHandler::GicMaintenanceHandler(_) => {}
            InterruptHandler::TimeIrqHandler(_) => {}
            InterruptHandler::None => {}
        }
        return true;
    } else {
        println!(
            "interrupt_handler: core {} receive unsupported int {}",
            cpu_id(),
            int_id
        );
        return false;
    }
}
