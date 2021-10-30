use crate::arch::gicc_clear_current_irq;
use crate::kernel::{active_vm, vm_ipa2pa};
use crate::kernel::{active_vcpu, active_vm_id, vm_if_list_get_cpu_id, vm_num};
use crate::kernel::{ipi_send_msg, IpiInnerMsg, IpiMessage, IpiType, IpiVmmMsg};
use crate::vmm::vmm_boot;

#[derive(Copy, Clone)]
pub enum VmmEvent {
    VmmBoot,
    VmmReboot,
    VmmShutdown,
}

pub fn vmm_boot_vm(vm_id: usize) {
    if vm_id >= vm_num() {
        println!("vmm_boot_vm: target vm {} not exist", vm_id);
        return;
    }

    let phys_id = vm_if_list_get_cpu_id(vm_id);

    if active_vcpu().is_some() && vm_id == active_vm_id() {
        gicc_clear_current_irq(true);
        vmm_boot();
    } else {
        let m = IpiVmmMsg {
            vmid: vm_id,
            event: VmmEvent::VmmBoot,
        };
        if !ipi_send_msg(phys_id, IpiType::IpiTVMM, IpiInnerMsg::VmmMsg(m)) {
            println!("vmm_boot_vm: failed to send ipi to Core {}", phys_id);
        }
    }
}

pub fn get_vm_id(id_ipa: usize) -> bool {
    let id_pa = vm_ipa2pa(active_vm().unwrap(), id_ipa);
    if id_pa == 0 {
        return false;
    }
    unsafe { *(id_pa as *mut usize) = active_vm_id(); }
    true
}

pub fn vmm_ipi_handler(msg: &IpiMessage) {
    match msg.ipi_message {
        IpiInnerMsg::VmmMsg(vmm) => match vmm.event {
            VmmEvent::VmmBoot => {
                vmm_boot_vm(vmm.vmid);
            }
            _ => {
                todo!();
            }
        },
        _ => {
            println!("vmm_ipi_handler: illegal ipi type");
            return;
        }
    }
}
