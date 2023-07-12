use crate::arch::{gic_cpu_reset, InterruptController, gic_glb_init, gic_cpu_init, gic_maintenance_handler};
use crate::board::{PlatOperation, Platform, PLAT_DESC};
use crate::kernel::{current_cpu, Vcpu, Vm, interrupt_reserve_int};

use super::{GICD, GIC_SGIS_NUM, gicc_get_current_irq, gicc_clear_current_irq};

pub const INTERRUPT_NUM_MAX: usize = 1024;
pub const INTERRUPT_IRQ_HYPERVISOR_TIMER: usize = 26;
pub const INTERRUPT_IRQ_IPI: usize = 1;
pub const INTERRUPT_IRQ_GUEST_TIMER: usize = 27;

pub fn interrupt_arch_init() {
    crate::util::barrier();

    if current_cpu().id == 0 {
        gic_glb_init();
    }

    gic_cpu_init();

    let int_id = PLAT_DESC.arch_desc.gic_desc.maintenance_int_id;
    interrupt_reserve_int(int_id, gic_maintenance_handler);
    interrupt_arch_enable(int_id, true);
}

pub fn interrupt_arch_enable(int_id: usize, en: bool) {
    let cpu_id = current_cpu().id;
    if en {
        GICD.set_prio(int_id, 0x7f);
        GICD.set_trgt(int_id, 1 << Platform::cpuid_to_cpuif(cpu_id));

        GICD.set_enable(int_id, en);
    } else {
        GICD.set_enable(int_id, en);
    }
}

pub fn interrupt_arch_ipi_send(cpu_id: usize, ipi_id: usize) {
    if ipi_id < GIC_SGIS_NUM {
        GICD.send_sgi(Platform::cpuid_to_cpuif(cpu_id), ipi_id);
    }
}

pub fn interrupt_arch_vm_register(vm: &Vm, id: usize) {
    super::vgic_set_hw_int(vm, id);
}

pub fn interrupt_arch_vm_inject(vm: &Vm, vcpu: &Vcpu, int_id: usize) {
    let vgic = vm.vgic();
    // trace!("int {}, cur vcpu vm {}, trgt vcpu vm {}", int_id, active_vm().unwrap().id(), vcpu.vm_id());
    if let Some(cur_vcpu) = current_cpu().active_vcpu.as_ref() {
        if cur_vcpu == vcpu {
            vgic.inject(vcpu, int_id);
            return;
        }
    }

    vcpu.push_int(int_id);
}

pub fn interrupt_arch_clear() {
    gic_cpu_reset();
    interrupt_arch_deactive_irq(true);
}

pub fn interrupt_arch_deactive_irq(for_hypervisor: bool) {
    gicc_clear_current_irq(for_hypervisor);
}

pub(super) struct IntCtrl;

impl InterruptController for IntCtrl {
    const NUM_MAX: usize = INTERRUPT_NUM_MAX;

    const IRQ_IPI: usize = INTERRUPT_IRQ_IPI;

    const IRQ_HYPERVISOR_TIMER: usize = INTERRUPT_IRQ_HYPERVISOR_TIMER;

    const IRQ_GUEST_TIMER: usize = INTERRUPT_IRQ_GUEST_TIMER;

    fn init() {
        todo!()
    }

    fn enable(_int_id: usize, _en: bool) {
        todo!()
    }

    fn fetch() -> Option<(usize, usize)> {
        gicc_get_current_irq()
    }

    fn finish(_int_id: usize) {
        todo!()
    }

    fn irq_priority(int_id: usize) -> usize {
        GICD.prio(int_id)
    }
}
