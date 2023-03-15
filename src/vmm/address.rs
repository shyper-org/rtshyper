use crate::arch::{PAGE_SIZE, PTE_S1_NORMAL};
use crate::kernel::{vm, current_cpu, IpiVmmMsg, vm_ipa2hva, Vm, IpiInnerMsg, ipi_send_msg, IpiType};
use crate::board::PLAT_DESC;
use crate::util::barrier;

use super::VmmEvent;

// Here, we regrad IPA as part of HVA (Hypervisor VA)
// using the higher bits as VMID to distinguish

// convert ipa to pa and mapping the hva(from ipa) on current cpu()
pub fn vmm_setup_ipa2hva(vm: &Vm) {
    let mut flag = false;
    for target_cpu_id in 0..PLAT_DESC.cpu_desc.num {
        if target_cpu_id != current_cpu().id {
            let msg = IpiVmmMsg {
                vmid: vm.id(),
                event: VmmEvent::VmmMapIPA,
            };
            if !ipi_send_msg(target_cpu_id, IpiType::IpiTVMM, IpiInnerMsg::VmmMsg(msg)) {
                println!("vmm_setup_ipa2hva: failed to send ipi to Core {}", target_cpu_id);
            }
        } else {
            flag = true;
        }
    }
    // execute after notify all other cores
    if flag {
        vmm_map_ipa_percore(vm.id());
    }
    info!("vmm_setup_ipa2hva: VM[{}] is ok", vm.id());
}

pub fn vmm_unmap_ipa2hva(vm: &Vm) {
    let mut flag = false;
    for target_cpu_id in 0..PLAT_DESC.cpu_desc.num {
        if target_cpu_id != current_cpu().id {
            let msg = IpiVmmMsg {
                vmid: vm.id(),
                event: VmmEvent::VmmUnmapIPA,
            };
            if !ipi_send_msg(target_cpu_id, IpiType::IpiTVMM, IpiInnerMsg::VmmMsg(msg)) {
                println!("vmm_unmap_ipa2hva: failed to send ipi to Core {}", target_cpu_id);
            }
        } else {
            flag = true;
        }
    }
    // execute after notify all other cores
    if flag {
        vmm_unmap_ipa_percore(vm.id());
    }
    info!("vmm_unmap_ipa2hva: VM[{}] is ok", vm.id());
}

pub fn vmm_map_ipa_percore(vm_id: usize) {
    let vm = match vm(vm_id) {
        None => {
            panic!(
                "vmm_map_ipa_percore: on core {}, VM [{}] is not added yet",
                current_cpu().id,
                vm_id
            );
        }
        Some(vm) => vm,
    };
    info!("vmm_map_ipa_percore: on core {}, for VM[{}]", current_cpu().id, vm_id);
    let config = vm.config();
    for region in config.memory_region().iter() {
        for ipa in region.as_range().step_by(PAGE_SIZE) {
            let hva = vm_ipa2hva(&vm, ipa);
            let pa = vm.ipa2pa(ipa).unwrap();
            current_cpu()
                .pt()
                .pt_map_range(hva, PAGE_SIZE, pa, PTE_S1_NORMAL, false);
        }
    }
    barrier();
}

pub fn vmm_unmap_ipa_percore(vm_id: usize) {
    let vm = match vm(vm_id) {
        None => {
            panic!(
                "vmm_unmap_ipa_percore: on core {}, VM [{}] is not added yet",
                current_cpu().id,
                vm_id
            );
        }
        Some(vm) => vm,
    };
    info!("vmm_unmap_ipa_percore: on core {}, for VM[{}]", current_cpu().id, vm_id);
    let config = vm.config();
    for region in config.memory_region().iter() {
        for ipa in region.as_range().step_by(PAGE_SIZE) {
            let hva = vm_ipa2hva(&vm, ipa);
            current_cpu().pt().pt_unmap_range(hva, PAGE_SIZE, false);
        }
    }
    barrier();
}
