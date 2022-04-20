use spin::Mutex;

use crate::arch::{interrupt_arch_ipi_send, interrupt_arch_vm_inject};
use crate::arch::{GIC_PRIVINT_NUM, interrupt_arch_vm_register};
use crate::kernel::{current_cpu, ipi_irq_handler, IpiInnerMsg, IpiMessage, Vcpu, VcpuState};
use crate::kernel::{ipi_register, IpiType, Vm};
use crate::lib::{BitAlloc, BitAlloc256, BitAlloc4K, BitMap};
use crate::vmm::vmm_ipi_handler;

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
    GicMaintenanceHandler(fn(usize)),
    TimeIrqHandler(fn(usize)),
    None,
}

impl InterruptHandler {
    pub fn call(&self, arg0: usize, _arg1: usize) {
        match self {
            InterruptHandler::IpiIrqHandler(irq_handler) => irq_handler(),
            InterruptHandler::GicMaintenanceHandler(gic_handler) => gic_handler(arg0),
            InterruptHandler::TimeIrqHandler(time_handler) => time_handler(arg0),
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
        hyper_bitmap_lock.set(int_id);
        glb_bitmap_lock.set(int_id)
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

    let cpu_id = current_cpu().id;
    if cpu_id == 0 {
        interrupt_reserve_int(INTERRUPT_IRQ_IPI, InterruptHandler::IpiIrqHandler(ipi_irq_handler));

        if !ipi_register(IpiType::IpiTIntInject, interrupt_inject_ipi_handler) {
            panic!(
                "interrupt_init: failed to register int inject ipi {:#?}",
                IpiType::IpiTIntInject
            )
        }
        use crate::arch::vgic_ipi_handler;
        if !ipi_register(IpiType::IpiTIntc, vgic_ipi_handler) {
            panic!("interrupt_init: failed to register intc ipi {:#?}", IpiType::IpiTIntc)
        }
        use crate::device::ethernet_ipi_rev_handler;
        if !ipi_register(IpiType::IpiTEthernetMsg, ethernet_ipi_rev_handler) {
            panic!(
                "interrupt_init: failed to register eth ipi {:#?}",
                IpiType::IpiTEthernetMsg,
            );
        }
        if !ipi_register(IpiType::IpiTVMM, vmm_ipi_handler) {
            panic!("interrupt_init: failed to register ipi vmm");
        }
        println!("Interrupt init ok");
    }
    interrupt_cpu_enable(INTERRUPT_IRQ_IPI, true);
}

pub fn interrupt_vm_register(vm: Vm, id: usize) -> bool {
    // println!("VM {} register interrupt {}", vm.id(), id);
    let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
    if glb_bitmap_lock.get(id) != 0 && id >= GIC_PRIVINT_NUM {
        println!("interrupt_vm_register: VM {} interrupts conflict, id = {}", vm.id(), id);
        return false;
    }

    interrupt_arch_vm_register(vm.clone(), id);
    vm.set_int_bit_map(id);
    glb_bitmap_lock.set(id);
    true
}

pub fn interrupt_vm_remove(vm: Vm, id: usize) {
    let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
    // vgic and vm will be removed with struct vm
    glb_bitmap_lock.clear(id);
    // todo: for interrupt 16~31, need to check by vm config
    if id >= GIC_PRIVINT_NUM {
        interrupt_cpu_enable(id, false);
    }
}

pub fn interrupt_vm_inject(vm: Vm, vcpu: Vcpu, int_id: usize, _source: usize) {
    if vcpu.phys_id() != current_cpu().id {
        println!(
            "interrupt_vm_inject: Core {} failed to find target (VCPU {} VM {})",
            current_cpu().id,
            vcpu.id(),
            vm.id()
        );
        return;
    }
    interrupt_arch_vm_inject(vm, vcpu, int_id);
}

pub fn interrupt_handler(int_id: usize, src: usize) -> bool {
    if interrupt_is_reserved(int_id) {
        let irq_handler_list = INTERRUPT_HANDLERS.lock();
        let irq_handler = irq_handler_list[int_id].clone();
        drop(irq_handler_list);
        match irq_handler {
            InterruptHandler::IpiIrqHandler(ipi_handler) => {
                ipi_handler();
            }
            InterruptHandler::GicMaintenanceHandler(maintenace_handler) => {
                maintenace_handler(int_id);
            }
            InterruptHandler::TimeIrqHandler(timer_irq_handler) => {
                timer_irq_handler(int_id);
            }
            InterruptHandler::None => {
                unimplemented!();
            }
        }
        // drop(irq_handler);
        return true;
    }

    if int_id >= 16 && int_id < 32 {
        if let Some(vcpu) = &current_cpu().active_vcpu {
            if let Some(active_vm) = vcpu.vm() {
                // if active_vm.id() == 1 {
                //     println!("interrupt_handler, before inject {} to vm1", int_id);
                // }
                if active_vm.has_interrupt(int_id) {
                    // if active_vm.id() == 1 {
                    //     println!("interrupt_handler, inject {} to vm1", int_id);
                    // }
                    interrupt_vm_inject(active_vm.clone(), vcpu.clone(), int_id, src);
                    return false;
                } else {
                    return true;
                }
            }
        }
    }

    for idx in 0..current_cpu().vcpu_pool().vcpu_num() {
        let vcpu = current_cpu().vcpu_pool().vcpu(idx);
        match vcpu.vm() {
            Some(vm) => {
                if vm.has_interrupt(int_id) {
                    if vcpu.state() as usize == VcpuState::VcpuInv as usize {
                        return true;
                    }

                    interrupt_vm_inject(vm.clone(), vcpu.clone(), int_id, src);
                    return false;
                }
            }
            None => {}
        }
    }

    println!(
        "interrupt_handler: core {} receive unsupported int {}",
        current_cpu().id,
        int_id
    );
    true
}

pub fn interrupt_inject_ipi_handler(msg: &IpiMessage) {
    match &msg.ipi_message {
        IpiInnerMsg::IntInjectMsg(int_msg) => {
            let vm_id = int_msg.vm_id;
            let int_id = int_msg.int_id;
            match current_cpu().vcpu_pool().pop_vcpu_through_vmid(vm_id) {
                None => {
                    panic!("inject int {} to illegal cpu {}", int_id, current_cpu().id);
                }
                Some(vcpu) => {
                    interrupt_vm_inject(vcpu.vm().unwrap(), vcpu.clone(), int_id, 0);
                }
            }
        }
        _ => {
            println!("interrupt_inject_ipi_handler: illegal ipi type");
            return;
        }
    }
}
