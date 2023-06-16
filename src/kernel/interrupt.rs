use alloc::collections::BTreeMap;

use spin::Mutex;

use crate::arch::{
    interrupt_arch_ipi_send, interrupt_arch_vm_inject, INTERRUPT_NUM_MAX, GIC_SGIS_NUM, GIC_PRIVINT_NUM,
    interrupt_arch_vm_register,
};
use crate::kernel::{current_cpu, Vcpu, VcpuState, Vm};
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

pub fn interrupt_irqchip_init() {
    use crate::arch::interrupt_arch_init;
    interrupt_arch_init();
}

pub fn interrupt_vm_register(vm: &Vm, id: usize, hw: bool) -> bool {
    // println!("VM {} register interrupt {}", vm.id(), id);
    if hw {
        let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
        if glb_bitmap_lock.get(id) != 0 && id >= GIC_PRIVINT_NUM {
            println!("interrupt_vm_register: VM {} interrupts conflict, id = {}", vm.id(), id);
            return false;
        }
        glb_bitmap_lock.set(id);
        interrupt_arch_vm_register(vm, id);
    }
    true
}

pub fn interrupt_vm_remove(_vm: &Vm, id: usize) {
    if id >= GIC_SGIS_NUM {
        let mut glb_bitmap_lock = INTERRUPT_GLB_BITMAP.lock();
        // vgic and vm will be removed with struct vm
        glb_bitmap_lock.clear(id);
        // todo: for interrupt 16~31, need to check by vm config
        if id >= GIC_PRIVINT_NUM {
            interrupt_cpu_enable(id, false);
        }
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
