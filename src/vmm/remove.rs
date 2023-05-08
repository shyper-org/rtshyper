use crate::arch::{GIC_SGIS_NUM, interrupt_arch_deactive_irq};
use crate::config::vm_cfg_del_vm;
use crate::device::emu_remove_dev;
use crate::kernel::{
    current_cpu, interrupt_vm_remove, ipi_send_msg, IpiInnerMsg, IpiType, IpiVmmMsg, remove_vm, remove_vm_async_task,
    Vm, cpu_idle, vm,
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
    // emu dev
    vmm_remove_emulated_device(&vm);
    // passthrough dev
    vmm_remove_passthrough_device(&vm);
    // clear async task list
    remove_vm_async_task(vm_id);
    // remove vm cfg
    let _ = vm_cfg_del_vm(vm_id);
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
        interrupt_arch_deactive_irq(true);
        cpu_idle();
    }
}

fn vmm_remove_vcpu(vm: &Vm) {
    for idx in 0..vm.cpu_num() {
        let vcpu = vm.vcpu(idx).unwrap();
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

fn vmm_remove_emulated_device(vm: &Vm) {
    let config = vm.config().emulated_device_list();
    for (idx, emu_dev) in config.iter().enumerate() {
        // mmio / vgic will be removed with struct vm
        if !emu_dev.emu_type.removable() {
            warn!("vmm_remove_emulated_device: cannot remove device {}", emu_dev.emu_type);
            return;
        }
        emu_remove_dev(vm.id(), idx, emu_dev.base_ipa, emu_dev.length);
        // println!(
        //     "VM[{}] removes emulated device: id=<{}>, name=\"{}\", ipa=<{:#x}>",
        //     vm.id(),
        //     idx,
        //     emu_dev.emu_type,
        //     emu_dev.base_ipa
        // );
    }
}

fn vmm_remove_passthrough_device(vm: &Vm) {
    for irq in vm.config().passthrough_device_irqs() {
        if irq > GIC_SGIS_NUM {
            interrupt_vm_remove(vm, irq);
            // println!("VM[{}] remove irq {}", vm.id(), irq);
        }
    }
}
