use core::sync::atomic::{AtomicBool, Ordering};

use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;

use crate::arch::{PAGE_SIZE, PTE_S1_NORMAL, LVL1_SHIFT};
use crate::kernel::{current_cpu, Vm, IpiInnerMsg, ipi_send_msg, IpiType, IpiVmmPercoreMsg};
use crate::board::PLAT_DESC;
use crate::util::barrier;

use super::VmmPercoreEvent;

// Here, we regrad IPA as part of HVA (Hypervisor VA)
// using the higher bits as VMID to distinguish

// convert ipa to pa and mapping the hva(from ipa) on current cpu()
pub fn vmm_setup_ipa2hva(vm: Arc<Vm>) {
    let mut flag = false;
    for target_cpu_id in 0..PLAT_DESC.cpu_desc.num {
        if target_cpu_id != current_cpu().id {
            let msg = IpiVmmPercoreMsg {
                vm: vm.clone(),
                event: VmmPercoreEvent::MapIPA,
            };
            if !ipi_send_msg(target_cpu_id, IpiType::IpiTVMM, IpiInnerMsg::VmmPercoreMsg(msg)) {
                error!("vmm_setup_ipa2hva: failed to send ipi to Core {}", target_cpu_id);
            }
        } else {
            flag = true;
        }
    }
    // execute after notify all other cores
    if flag {
        vmm_map_ipa_percore(&vm, true);
    }
    info!("vmm_setup_ipa2hva: VM[{}] is ok", vm.id());
}

pub fn vmm_unmap_ipa2hva(vm: Arc<Vm>) {
    vm.reset_mem_regions();
    let mut flag = false;
    for target_cpu_id in 0..PLAT_DESC.cpu_desc.num {
        if target_cpu_id != current_cpu().id {
            let msg = IpiVmmPercoreMsg {
                vm: vm.clone(),
                event: VmmPercoreEvent::UnmapIPA,
            };
            if !ipi_send_msg(target_cpu_id, IpiType::IpiTVMM, IpiInnerMsg::VmmPercoreMsg(msg)) {
                error!("vmm_unmap_ipa2hva: failed to send ipi to Core {}", target_cpu_id);
            }
        } else {
            flag = true;
        }
    }
    // execute after notify all other cores
    if flag {
        vmm_unmap_ipa_percore(&vm);
    }
    info!("vmm_unmap_ipa2hva: VM[{}] is ok", vm.id());
}

pub fn vmm_map_ipa_percore(vm: &Vm, is_master: bool) {
    static SHARED_PTE: RwLock<Vec<(usize, usize)>> = RwLock::new(Vec::new());
    static FINISH: AtomicBool = AtomicBool::new(false);

    trace!("vmm_map_ipa_percore: on core {}, for VM[{}]", current_cpu().id, vm.id());
    let config = vm.config();
    if is_master {
        let mut shared_pte_list = SHARED_PTE.write();
        shared_pte_list.clear();
        for region in config.memory_region().iter() {
            for ipa in region.as_range().step_by(PAGE_SIZE) {
                let hva = vm.ipa2hva(ipa);
                let pa = vm.ipa2pa(ipa).unwrap();
                current_cpu()
                    .pt()
                    .pt_map_range(hva, PAGE_SIZE, pa, PTE_S1_NORMAL, false);
            }

            for ipa in region.as_range().step_by(1 << LVL1_SHIFT) {
                let hva = vm.ipa2hva(ipa);
                let pte = current_cpu().pt().get_pte(hva, 1).unwrap();
                shared_pte_list.push((hva, pte));
            }
        }
        FINISH.store(true, Ordering::Relaxed);
    } else {
        while !FINISH.load(Ordering::Relaxed) {
            core::hint::spin_loop();
        }
        for (hva, pte) in SHARED_PTE.read().iter() {
            current_cpu().pt().set_pte(*hva, 1, *pte);
        }
    }
    barrier();
    if is_master {
        FINISH.store(false, Ordering::Relaxed);
    }
}

pub fn vmm_unmap_ipa_percore(vm: &Vm) {
    trace!(
        "vmm_unmap_ipa_percore: on core {}, for VM[{}]",
        current_cpu().id,
        vm.id()
    );
    let config = vm.config();
    for region in config.memory_region().iter() {
        let hva = vm.ipa2hva(region.ipa_start);
        current_cpu().pt().pt_unmap_range(hva, region.length, false);
    }
    barrier();
}
