use alloc::ffi::CString;

use crate::arch::interrupt_arch_deactive_irq;
use crate::arch::power_arch_vm_shutdown_secondary_cores;
use crate::config::vm_cfg_entry;
use crate::kernel::{
    active_vcpu_id, active_vm, current_cpu, push_vm, vm, Vm, vm_if_get_state, vm_if_set_ivc_arg, vm_if_set_ivc_arg_ptr,
    vm_id_list,
};
use crate::kernel::{active_vm_id, vm_if_get_cpu_id};
use crate::kernel::{ipi_send_msg, IpiInnerMsg, IpiMessage, IpiType, IpiVmmMsg};
use crate::kernel::{hvc_send_msg_to_vm, HvcGuestMsg, HvcManageMsg};
use crate::kernel::HVC_CONFIG;
use crate::kernel::HVC_CONFIG_UPLOAD_KERNEL_IMAGE;
use crate::kernel::HVC_VMM;
use crate::kernel::HVC_VMM_REBOOT_VM;
use crate::util::bit_extract;
use crate::vmm::{vmm_cpu_assign_vcpu, vmm_boot, vmm_init_image, vmm_setup_config, vmm_cpu_remove_vcpu};

use shyper::{VM_NUM_MAX, VMInfo};

#[derive(Copy, Clone)]
pub enum VmmEvent {
    VmmBoot,
    VmmReboot,
    #[allow(dead_code)]
    VmmShutdown,
    VmmAssignCpu,
    VmmRemoveCpu,
    VmmMapIPA,
    VmmUnmapIPA,
}

fn vmm_shutdown_secondary_vm() {
    println!("Shutting down all VMs...");
}

/* Generate VM structure and push it to VM.
 *
 * @param[in]  vm_id: new added VM id.
 */
fn vmm_push_vm(vm_id: usize) -> Result<Vm, ()> {
    info!("vmm_push_vm: add vm {} on cpu {}", vm_id, current_cpu().id);
    let vm_cfg = match vm_cfg_entry(vm_id) {
        Some(vm_cfg) => vm_cfg,
        None => {
            println!("vmm_push_vm: failed to find config for vm {}", vm_id);
            return Err(());
        }
    };
    push_vm(vm_id, vm_cfg)
}

/* Init VM before boot.
 * Only VM0 will call this function.
 *
 * @param[in] vm_id: target VM id to boot.
 */
pub fn vmm_init_gvm(vm_id: usize) {
    // Before boot, we need to set up the VM config.
    if current_cpu().id == 0 || (active_vm_id() == 0 && active_vm_id() != vm_id) {
        if let Ok(vm) = vmm_push_vm(vm_id) {
            vmm_setup_config(vm);
        } else {
            error!("VM[{}] alloc failed", vm_id);
        }
    } else {
        error!(
            "VM[{}] Core {} should not init VM [{}]",
            active_vm_id(),
            current_cpu().id,
            vm_id
        );
    }
}

/* Boot Guest VM.
 *
 * @param[in] vm_id: target VM id to boot.
 */
pub fn vmm_boot_vm(vm_id: usize) {
    let phys_id = vm_if_get_cpu_id(vm_id).unwrap();
    // println!(
    //     "vmm_boot_vm: current_cpu {} target vm {} get phys_id {}",
    //     current_cpu().id,
    //     vm_id,
    //     phys_id
    // );
    if phys_id != current_cpu().id {
        let m = IpiVmmMsg {
            vmid: vm_id,
            event: VmmEvent::VmmBoot,
        };
        if !ipi_send_msg(phys_id, IpiType::IpiTVMM, IpiInnerMsg::VmmMsg(m)) {
            println!("vmm_boot_vm: failed to send ipi to Core {}", phys_id);
        }
    } else {
        match current_cpu().vcpu_array.pop_vcpu_through_vmid(vm_id) {
            None => {
                panic!(
                    "vmm_boot_vm: VM[{}] does not have vcpu on Core {}",
                    vm_id,
                    current_cpu().id
                );
            }
            Some(vcpu) => {
                interrupt_arch_deactive_irq(true);
                current_cpu().vcpu_array.wakeup_vcpu(vcpu);
                vmm_boot();
            }
        };
    }
}

/**
 * Reboot target vm according to arguments
 *
 * @param arg force ~ (31, 16) ~ [soft shutdown or hard shutdown]
 *            vmid ~ (15, 0) ~ [target vm id]
 */
pub fn vmm_reboot_vm(arg: usize) {
    let vm_id = bit_extract(arg, 0, 16);
    let force = bit_extract(arg, 16, 16) != 0;
    let cur_vm = active_vm().unwrap();

    println!("vmm_reboot VM [{}] force:{}", vm_id, force);

    if force {
        if cur_vm.id() == vm_id {
            vmm_reboot();
        } else {
            let cpu_trgt = vm_if_get_cpu_id(vm_id).unwrap();
            let m = IpiVmmMsg {
                vmid: vm_id,
                event: VmmEvent::VmmReboot,
            };
            if !ipi_send_msg(cpu_trgt, IpiType::IpiTVMM, IpiInnerMsg::VmmMsg(m)) {
                println!("vmm_reboot_vm: failed to send ipi to Core {}", cpu_trgt);
            }
        }
        return;
    }

    let msg = HvcManageMsg {
        fid: HVC_VMM,
        event: HVC_VMM_REBOOT_VM,
        vm_id,
    };
    if !hvc_send_msg_to_vm(vm_id, &HvcGuestMsg::Manage(msg)) {
        println!("vmm_reboot_vm: failed to notify VM 0");
    }
}

/* Reset vm os at current core.
 *
 * @param[in] vm : target VM structure to be reboot.
 */
pub fn vmm_reboot() {
    let vm = active_vm().unwrap();
    // If running MVM, reboot the whole system.
    if vm.id() == 0 {
        vmm_shutdown_secondary_vm();
        use crate::board::{PlatOperation, Platform};
        Platform::sys_reboot();
    }

    // Reset GVM.
    let vcpu = current_cpu().active_vcpu.as_ref().unwrap();
    println!("VM [{}] reset...", vm.id());
    power_arch_vm_shutdown_secondary_cores(&vm);
    println!(
        "Core {} (VM [{}] vcpu {}) shutdown ok",
        current_cpu().id,
        vm.id(),
        active_vcpu_id()
    );

    // Clear memory region.
    println!(
        "Core {} (VM [{}] vcpu {}) reset mem region",
        current_cpu().id,
        vm.id(),
        active_vcpu_id(),
    );
    vm.reset_mem_regions();

    // Reset image.
    if !vmm_init_image(&vm) {
        panic!("vmm_reboot: vmm_init_image failed");
    }

    // Reset ivc arg.
    vm_if_set_ivc_arg(vm.id(), 0);
    vm_if_set_ivc_arg_ptr(vm.id(), 0);

    crate::arch::interrupt_arch_clear();
    crate::arch::vcpu_arch_init(vm.config(), vm.vcpu(0).unwrap());
    vcpu.reset_context();

    vmm_load_image_from_mvm(&vm);
}

fn vmm_load_image_from_mvm(vm: &Vm) {
    let vm_id = vm.id();
    let msg = HvcManageMsg {
        fid: HVC_CONFIG,
        event: HVC_CONFIG_UPLOAD_KERNEL_IMAGE,
        vm_id,
    };
    // println!("mediated_blk_write send msg to vm0");
    if !hvc_send_msg_to_vm(0, &HvcGuestMsg::Manage(msg)) {
        println!("vmm_load_image_from_mvm: failed to notify VM 0");
    }
}

/* Get current VM id.
 *
 * @param[in] id_ipa : vm id ipa.
 */
pub fn get_vm_id(id_ipa: usize) -> bool {
    let vm = active_vm().unwrap();
    let id_pa = vm.ipa2hva(id_ipa);
    if id_pa == 0 {
        println!("illegal id_pa {:x}", id_pa);
        return false;
    }
    unsafe {
        *(id_pa as *mut usize) = vm.id();
    }
    true
}

#[repr(C)]
struct VMInfoList {
    pub vm_num: usize,
    pub info_list: [VMInfo; VM_NUM_MAX],
}

/* List VM info in hypervisor.
 *
 * @param[in] vm_info_ipa : vm info list ipa.
 */
pub fn vmm_list_vm(vm_info_ipa: usize) -> Result<usize, ()> {
    let vm_info_pa = active_vm().unwrap().ipa2hva(vm_info_ipa);
    if vm_info_pa == 0 {
        println!("illegal vm_info_ipa {:x}", vm_info_ipa);
        return Err(());
    }

    let vm_info = unsafe { &mut *(vm_info_pa as *mut VMInfoList) };

    let vm_id_list = vm_id_list();
    // Get VM num.
    vm_info.vm_num = vm_id_list.len();

    for (idx, vmid) in vm_id_list.into_iter().enumerate() {
        let vm = match vm(vmid) {
            Some(vm) => vm,
            None => {
                println!("Failed to get VM config entry for VM[{}]", vmid);
                continue;
            }
        };
        let vm_cfg = vm.config();
        // Get VM type.
        let vm_type = vm.vm_type();
        // Get VM State.
        let vm_state = vm_if_get_state(vmid);

        vm_info.info_list[idx].id = vmid as u32;
        vm_info.info_list[idx].vm_type = vm_type as u32;
        vm_info.info_list[idx].vm_state = vm_state as u32;

        // From Rust to C: CString represents an owned, C-friendly string
        let vm_name_cstring = CString::new(vm_cfg.name.clone()).unwrap();
        let vm_name_with_null = vm_name_cstring.to_bytes_with_nul();
        // ensure that the slice length is equal
        vm_info.info_list[idx].vm_name[..vm_name_with_null.len()].copy_from_slice(vm_name_with_null);
    }
    Ok(0)
}

pub fn vmm_ipi_handler(msg: IpiMessage) {
    match msg.ipi_message {
        IpiInnerMsg::VmmMsg(vmm) => match vmm.event {
            VmmEvent::VmmBoot => {
                vmm_boot_vm(vmm.vmid);
            }
            VmmEvent::VmmReboot => {
                vmm_reboot();
            }
            VmmEvent::VmmAssignCpu => {
                println!(
                    "vmm_ipi_handler: core {} receive assign vcpu request for vm[{}]",
                    current_cpu().id,
                    vmm.vmid
                );
                vmm_cpu_assign_vcpu(vmm.vmid);
            }
            VmmEvent::VmmRemoveCpu => {
                println!(
                    "vmm_ipi_handler: core {} remove vcpu for vm[{}]",
                    current_cpu().id,
                    vmm.vmid
                );
                vmm_cpu_remove_vcpu(vmm.vmid);
            }
            VmmEvent::VmmMapIPA => {
                debug!(
                    "vmm_ipi_handler: core {} map ipa for vm[{}]",
                    current_cpu().id,
                    vmm.vmid
                );
                super::address::vmm_map_ipa_percore(vmm.vmid, false);
            }
            VmmEvent::VmmUnmapIPA => {
                debug!(
                    "vmm_ipi_handler: core {} unmap ipa for vm[{}]",
                    current_cpu().id,
                    vmm.vmid
                );
                super::address::vmm_unmap_ipa_percore(vmm.vmid);
            }
            _ => {
                todo!();
            }
        },
        _ => {
            println!("vmm_ipi_handler: illegal ipi type");
        }
    }
}
