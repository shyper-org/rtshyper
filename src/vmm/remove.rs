use crate::arch::{interrupt_arch_deactive_irq, INTERRUPT_IRQ_GUEST_TIMER};
use crate::config::vm_cfg_del_vm;
use crate::kernel::{
    current_cpu, interrupt_vm_remove, ipi_send_msg, IpiInnerMsg, IpiType, IpiVmmMsg, remove_vm, remove_vm_async_task,
    Vm, cpu_idle, vm, interrupt_cpu_enable,
};
use crate::kernel::vm_if_reset;
use crate::vmm::VmmEvent;
use crate::vmm::address::vmm_unmap_ipa2hva;

pub fn vmm_remove_vm(vm_id: usize) {
    if vm_id == 0 {
        warn!("{} do not support remove vm0", env!("CARGO_PKG_NAME"));
        return;
    }

    // remove vm: page table / mmio / vgic will be removed when vm drop
    let vm = vm(vm_id).unwrap();

    // vcpu
    vmm_remove_vcpu(&vm);
    // reset vm interface
    vm_if_reset(vm_id);
    // passthrough dev
    vmm_remove_passthrough_device(&vm);
    // clear async task list
    remove_vm_async_task(vm_id);
    crate::device::remove_virtio_nic(vm_id);
    // remove vm cfg
    let _ = vm_cfg_del_vm(vm_id);
    #[cfg(feature = "unilib")]
    // remove vm unilib
    crate::util::unilib::unilib_fs_remove(vm_id);
    // unmap ipa(hva) percore at last
    vmm_unmap_ipa2hva(&vm);
    remove_vm(vm_id);
    info!("remove vm[{}] successfully", vm_id);
}

pub fn vmm_cpu_remove_vcpu(vmid: usize) {
    let vcpu = current_cpu().vcpu_array.remove_vcpu(vmid);
    if let Some(vcpu) = vcpu {
        // remove vcpu from scheduler
        current_cpu().scheduler().sleep(vcpu);
    }
    if !current_cpu().assigned() {
        // hard code: remove el1 timer interrupt 27
        interrupt_cpu_enable(INTERRUPT_IRQ_GUEST_TIMER, false);
        interrupt_arch_deactive_irq(true);
        cpu_idle();
    }
}

fn vmm_remove_vcpu(vm: &Vm) {
    for vcpu in vm.vcpu_list() {
        if vcpu.phys_id() == current_cpu().id {
            vmm_cpu_remove_vcpu(vm.id());
        } else {
            let m = IpiVmmMsg {
                vmid: vm.id(),
                event: VmmEvent::VmmRemoveCpu,
            };
            if !ipi_send_msg(vcpu.phys_id(), IpiType::IpiTVMM, IpiInnerMsg::VmmMsg(m)) {
                warn!("vmm_remove_vcpu: failed to send ipi to Core {}", vcpu.phys_id());
            }
        }
    }
}

fn vmm_remove_passthrough_device(vm: &Vm) {
    for irq in vm.config().passthrough_device_irqs() {
        interrupt_vm_remove(vm, *irq);
        // println!("VM[{}] remove irq {}", vm.id(), irq);
    }
}
