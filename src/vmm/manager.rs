use crate::arch::gicc_clear_current_irq;
use crate::arch::power_arch_vm_shutdown_secondary_cores;
use crate::config::vm_cfg_entry;
use crate::device::create_fdt;
use crate::kernel::{active_vcpu_id, active_vm, current_cpu, vcpu_run, Vm, vm_if_list_set_ivc_arg, vm_if_list_set_ivc_arg_ptr, vm_ipa2pa};
use crate::kernel::{active_vm_id, vm_if_list_get_cpu_id, vm_list_size};
use crate::kernel::{ipi_send_msg, IpiInnerMsg, IpiMessage, IpiType, IpiVmmMsg};
use crate::vmm::{vmm_boot, vmm_init_image, vmm_setup_fdt};

#[derive(Copy, Clone)]
pub enum VmmEvent {
    VmmBoot,
    VmmReboot,
    VmmShutdown,
}

pub fn vmm_shutdown_secondary_vm() {
    println!("Shutting down all VMs...");
}

pub fn vmm_boot_vm(vm_id: usize) {
    if vm_id >= vm_list_size() {
        println!("vmm_boot_vm: target vm {} not exist", vm_id);
        return;
    }

    let phys_id = vm_if_list_get_cpu_id(vm_id);

    if current_cpu().active_vcpu.clone().is_some() && vm_id == active_vm_id() {
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

pub fn vmm_reboot_vm(vm: Vm) {
    if vm.id() == 0 {
        vmm_shutdown_secondary_vm();
        crate::board::platform_sys_reboot();
    }
    let vcpu = current_cpu().active_vcpu.clone().unwrap();
    println!("VM {} reset...", vm.id());
    power_arch_vm_shutdown_secondary_cores(vm.clone());
    println!("Core {} (vm {} vcpu {}) shutdown ok", current_cpu().id, vm.id(), active_vcpu_id());

    let config = vm_cfg_entry(vm.id());
    if !vmm_init_image(&config.image, vm.clone()) {
        panic!("vmm_reboot_vm: vmm_init_image failed");
    }
    if vm.id() != 0 {
        // init vm1 dtb
        match create_fdt(config.clone()) {
            Ok(dtb) => {
                let offset = config.image.device_tree_load_ipa
                    - vm.config().memory.region.as_ref().unwrap()[0].ipa_start;
                println!("dtb size {}", dtb.len());
                println!("pa 0x{:x}", vm.pa_start(0) + offset);
                crate::lib::memcpy_safe(
                    (vm.pa_start(0) + offset) as *const u8,
                    dtb.as_ptr(),
                    dtb.len(),
                );
            }
            _ => {
                panic!("vmm_setup_config: create fdt for vm{} fail", vm.id());
            }
        }
    } else {
        unsafe {
            vmm_setup_fdt(config.clone(), vm.clone());
        }
    }
    vm_if_list_set_ivc_arg(vm.id(), 0);
    vm_if_list_set_ivc_arg_ptr(vm.id(), 0);

    crate::arch::interrupt_arch_clear();
    crate::arch::vcpu_arch_init(vm.clone(), vm.vcpu(0));
    vcpu.reset_state();
    vcpu_run();
}

pub fn get_vm_id(id_ipa: usize) -> bool {
    let id_pa = vm_ipa2pa(active_vm().unwrap(), id_ipa);
    if id_pa == 0 {
        println!("illegal id_pa {:x}", id_pa);
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
