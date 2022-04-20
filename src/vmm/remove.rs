use crate::config::vm_cfg_remove_vm_entry;
use crate::device::emu_remove_dev;
use crate::device::EmuDeviceType::*;
use crate::kernel::{
    cpu_idle, current_cpu, interrupt_vm_remove, ipi_send_msg, IpiInnerMsg, IpiType, IpiVmmMsg, mem_vm_region_free,
    remove_async_used_info, remove_vm, remove_vm_async_task, vcpu_remove, vm, Vm,
};
use crate::kernel::vm_if_reset;
use crate::vmm::VmmEvent;

pub fn vmm_remove_vm(vm_id: usize) {
    if vm_id == 0 {
        println!("Rust-Shyper do not support remove vm0");
        return;
    }

    let vm = match vm(vm_id) {
        None => {
            println!("vmm_remove_vm: vm[{}] not exist", vm_id);
            return;
        }
        Some(vm) => vm,
    };

    // vcpu
    vmm_remove_vcpu(vm.clone());
    // reset vm interface
    vm_if_reset(vm_id);
    // free mem
    for idx in 0..vm.region_num() {
        mem_vm_region_free(vm.pa_start(idx), vm.pa_length(idx));
    }
    // emu dev
    vmm_remove_emulated_device(vm.clone());
    // pass dev
    vmm_remove_passthrough_device(vm.clone());
    // clear async task list
    remove_vm_async_task(vm_id);
    // async used info
    remove_async_used_info(vm_id);
    // remove vm: page table / mmio / vgic will be removed with struct vm
    vmm_remove_vm_list(vm_id);
    // remove vm cfg
    vm_cfg_remove_vm_entry(vm_id);
    println!("remove vm[{}] successfully", vm_id);
}

pub fn vmm_remove_vm_list(vm_id: usize) {
    let vm = remove_vm(vm_id);
    vm.clear_vcpu();
}

pub fn vmm_remove_vcpu(vm: Vm) {
    for idx in 0..vm.cpu_num() {
        let vcpu = vm.vcpu(idx).unwrap();
        // remove vcpu from VCPU_LIST
        vcpu_remove(vcpu.clone());
        // remove vcpu from scheduler vcpu_pool
        if vcpu.phys_id() == current_cpu().id {
            current_cpu().vcpu_pool().remove_vcpu(vm.id());
        } else {
            let m = IpiVmmMsg {
                vmid: vm.id(),
                event: VmmEvent::VmmRemoveCpu,
            };
            if !ipi_send_msg(vcpu.phys_id(), IpiType::IpiTVMM, IpiInnerMsg::VmmMsg(m)) {
                println!("vmm_remove_vcpu: failed to send ipi to Core {}", vcpu.phys_id());
            }
        }
    }
}

pub fn vmm_remove_emulated_device(vm: Vm) {
    let config = vm.config().emulated_device_list();
    for (idx, emu_dev) in config.iter().enumerate() {
        let dev_name;
        // mmio / vgic will be removed with struct vm
        match emu_dev.emu_type {
            EmuDeviceTGicd => {
                dev_name = "interrupt controller";
            }
            EmuDeviceTGPPT => {
                dev_name = "partial passthrough interrupt controller";
            }
            EmuDeviceTVirtioBlk => {
                dev_name = "virtio block";
            }
            EmuDeviceTVirtioNet => {
                dev_name = "virtio net";
            }
            EmuDeviceTVirtioConsole => {
                dev_name = "virtio console";
            }
            _ => {
                println!("vmm_remove_emulated_device: unknown emulated device");
                return;
            }
        }
        emu_remove_dev(vm.id(), idx, emu_dev.base_ipa, emu_dev.length);
        println!(
            "VM[{}] removes emulated device: id=<{}>, name=\"{}\", ipa=<0x{:x}>",
            vm.id(),
            idx,
            dev_name,
            emu_dev.base_ipa
        );
    }
}

pub fn vmm_remove_passthrough_device(vm: Vm) {
    for irq in vm.config().passthrough_device_irqs() {
        interrupt_vm_remove(vm.clone(), irq);
        println!("VM[{}] remove irq {}", vm.id(), irq);
    }
}
