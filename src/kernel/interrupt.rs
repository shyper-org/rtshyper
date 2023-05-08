use alloc::collections::BTreeMap;

use spin::Mutex;

use crate::arch::{interrupt_arch_ipi_send, interrupt_arch_vm_inject, INTERRUPT_IRQ_IPI, INTERRUPT_NUM_MAX};
use crate::arch::{GIC_PRIVINT_NUM, interrupt_arch_vm_register};
use crate::kernel::{current_cpu, ipi_irq_handler, IpiInnerMsg, IpiMessage, Vcpu, VcpuState};
use crate::kernel::{ipi_register, IpiType, Vm};
use crate::util::{BitAlloc, BitAlloc4K};

static INTERRUPT_GLB_BITMAP: Mutex<BitAlloc4K> = Mutex::new(BitAlloc4K::default());
static INTERRUPT_HANDLERS: Mutex<BTreeMap<usize, fn()>> = Mutex::new(BTreeMap::new());

pub fn interrupt_cpu_ipi_send(target_cpu: usize, ipi_id: usize) {
    interrupt_arch_ipi_send(target_cpu, ipi_id);
}

pub fn interrupt_reserve_int(int_id: usize, handler: fn()) {
    if int_id < INTERRUPT_NUM_MAX {
        INTERRUPT_HANDLERS.lock().insert(int_id, handler);
        INTERRUPT_GLB_BITMAP.lock().set(int_id);
    }
}

pub fn interrupt_cpu_enable(int_id: usize, en: bool) {
    use crate::arch::interrupt_arch_enable;
    interrupt_arch_enable(int_id, en);
}

pub fn interrupt_init() {
    use crate::arch::interrupt_arch_init;
    interrupt_arch_init();

    if current_cpu().id == 0 {
        interrupt_reserve_int(INTERRUPT_IRQ_IPI, ipi_irq_handler);

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
        use crate::vmm::vmm_ipi_handler;
        if !ipi_register(IpiType::IpiTVMM, vmm_ipi_handler) {
            panic!("interrupt_init: failed to register ipi vmm");
        }

        println!("Interrupt init ok");
    }
    interrupt_cpu_enable(INTERRUPT_IRQ_IPI, true);
}

pub fn interrupt_vm_register(vm: &Vm, id: usize) -> bool {
    // println!("VM {} register interrupt {}", vm.id(), id);
    let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
    if glb_bitmap_lock.get(id) != 0 && id >= GIC_PRIVINT_NUM {
        println!("interrupt_vm_register: VM {} interrupts conflict, id = {}", vm.id(), id);
        return false;
    }

    interrupt_arch_vm_register(vm, id);
    vm.set_int_bit_map(id);
    glb_bitmap_lock.set(id);
    true
}

pub fn interrupt_vm_remove(_vm: &Vm, id: usize) {
    let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
    // vgic and vm will be removed with struct vm
    glb_bitmap_lock.clear(id);
    // todo: for interrupt 16~31, need to check by vm config
    if id >= GIC_PRIVINT_NUM {
        interrupt_cpu_enable(id, false);
    }
}

pub fn interrupt_vm_inject(vm: &Vm, vcpu: &Vcpu, int_id: usize) {
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

fn interrupt_is_reserved(int_id: usize) -> Option<fn()> {
    INTERRUPT_HANDLERS.lock().get(&int_id).cloned()
}

pub fn interrupt_handler(int_id: usize) -> bool {
    if let Some(irq_handler) = interrupt_is_reserved(int_id) {
        irq_handler();
        return true;
    }

    if (16..GIC_PRIVINT_NUM).contains(&int_id) {
        if let Some(vcpu) = &current_cpu().active_vcpu {
            if let Some(active_vm) = vcpu.vm() {
                if active_vm.has_interrupt(int_id) {
                    interrupt_vm_inject(&active_vm, vcpu, int_id);
                    return false;
                } else {
                    return true;
                }
            }
        }
    }

    for vcpu in current_cpu().vcpu_array.iter().flatten() {
        if let Some(vm) = vcpu.vm() {
            if vm.has_interrupt(int_id) {
                if vcpu.state() == VcpuState::Inv {
                    return true;
                }
                interrupt_vm_inject(&vm, vcpu, int_id);
                return false;
            }
        }
    }

    println!(
        "interrupt_handler: core {} receive unsupported int {}",
        current_cpu().id,
        int_id
    );
    true
}

fn interrupt_inject_ipi_handler(msg: &IpiMessage) {
    match &msg.ipi_message {
        IpiInnerMsg::IntInjectMsg(int_msg) => {
            let vm_id = int_msg.vm_id;
            let int_id = int_msg.int_id;
            match current_cpu().vcpu_array.pop_vcpu_through_vmid(vm_id) {
                None => {
                    panic!("inject int {} to illegal cpu {}", int_id, current_cpu().id);
                }
                Some(vcpu) => {
                    interrupt_vm_inject(&vcpu.vm().unwrap(), &vcpu, int_id);
                }
            }
        }
        _ => {
            println!("interrupt_inject_ipi_handler: illegal ipi type");
        }
    }
}
