use core::mem::size_of;

use crate::arch::PAGE_SIZE;
use crate::device::{mediated_blk_notify_handler, mediated_dev_append};
use crate::kernel::{current_cpu, interrupt_vm_inject, ipi_register, ipi_send_msg, IpiHvcMsg, IpiInnerMsg, IpiMessage, IpiType, ivc_update_mq, vm, vm_if_list_get_cpu_id, vm_if_list_ivc_arg, vm_if_list_ivc_arg_ptr, vm_if_list_set_ivc_arg_ptr, VM_NUM_MAX};
use crate::lib::{memcpy_safe, trace};
use crate::vmm::{get_vm_id, vmm_boot_vm};

pub const HVC_SYS: usize = 0;
pub const HVC_VMM: usize = 1;
pub const HVC_IVC: usize = 2;
pub const HVC_MEDIATED: usize = 3;

pub const HVC_IRQ: usize = 32 + 0x20;

#[repr(C)]
pub struct HvcGuestMsg {
    pub fid: usize,
    pub event: usize,
}

pub fn hvc_guest_handler(
    hvc_type: usize,
    event: usize,
    x0: usize,
    x1: usize,
    x2: usize,
    x3: usize,
    x4: usize,
    x5: usize,
    x6: usize,
) -> bool {
    match hvc_type {
        HVC_SYS => {
            hvc_sys_handler(event, x0)
        }
        HVC_VMM => {
            hvc_vmm_handler(event, x0, x1)
        }
        HVC_IVC => {
            hvc_ivc_handler(event, x0, x1, x2, x3, x4)
        }
        HVC_MEDIATED => {
            hvc_mediated_handler(event, x0, x1, x2, x3)
        }
        _ => {
            println!(
                "hvc_guest_handler: unknown hvc type {} event {}",
                hvc_type, event
            );
            false
        }
    }
}

fn hvc_sys_handler(event: usize, x0: usize) -> bool {
    true
}

fn hvc_vmm_handler(event: usize, x0: usize, x1: usize) -> bool {
    match event {
        // HVC_VMM_LIST_VM
        0 => {
            todo!();
            true
        }
        // HVC_VMM_GET_VM_STATE
        1 => {
            todo!();
            true
        }
        // HVC_VMM_BOOT_VM
        2 => {
            vmm_boot_vm(x0);
            true
        }
        // HVC_VMM_SHUTDOWN_VM
        3 => {
            todo!();
            true
        }
        // HVC_VMM_REBOOT_VM
        4 => {
            todo!();
            true
        }
        // HVC_VMM_GET_VM_ID
        8 => {
            get_vm_id(x0)
        }
        _ => {
            println!("hvc_vmm unknown event {}", event);
            false
        }
    }
}

fn hvc_ivc_handler(event: usize, x0: usize, x1: usize, x2: usize, x3: usize, x4: usize) -> bool {
    match event {
        // HVC_IVC_UPDATE_MQ
        0 => {
            return ivc_update_mq(x0, x1);
        }
        _ => {
            println!("hvc_ivc_handler: unknown event {}", event);
            false
        }
    }
}

fn hvc_mediated_handler(event: usize, x0: usize, x1: usize, x2: usize, x3: usize) -> bool {
    match event {
        // HVC_MEDIATED_DEV_APPEND
        48 => {
            // println!("mediated dev_append");
            mediated_dev_append(x0, x1);
        }
        // HVC_DEVICE_USER_NOTIFY
        49 => {
            // println!("mediated notify");
            mediated_blk_notify_handler(x0);
        }
        _ => {
            println!("unknown mediated event {}", event);
            return false;
        }
    }
    true
}

pub fn hvc_send_msg_to_vm(vm_id: usize, guest_msg: &HvcGuestMsg) -> bool {
    let mut target_addr = 0;
    let mut arg_ptr_addr = vm_if_list_ivc_arg_ptr(vm_id);
    let arg_addr = vm_if_list_ivc_arg(vm_id);

    if arg_ptr_addr != 0 {
        arg_ptr_addr += PAGE_SIZE / VM_NUM_MAX;
        if arg_ptr_addr - arg_addr >= PAGE_SIZE {
            vm_if_list_set_ivc_arg_ptr(vm_id, arg_addr);
            target_addr = arg_addr;
        } else {
            vm_if_list_set_ivc_arg_ptr(vm_id, arg_ptr_addr);
            target_addr = arg_ptr_addr;
        }
    }

    if target_addr == 0 {
        println!("hvc_send_msg_to_vm: target VM{} interface is not prepared", vm_id);
        return false;
    }

    if trace() && (target_addr < 0x1000 || (guest_msg as *const _ as usize) < 0x1000) {
        panic!("illegal des addr {:x}, src addr {:x}", target_addr, guest_msg as *const _ as usize);
    }
    memcpy_safe(target_addr as *const u8, guest_msg as *const _ as *const u8, size_of::<HvcGuestMsg>());

    let cpu_trgt = vm_if_list_get_cpu_id(vm_id);
    if cpu_trgt != current_cpu().id {
        println!("cpu {} send hvc msg to cpu {}", current_cpu().id, cpu_trgt);
        let ipi_msg = IpiHvcMsg {
            src_vmid: 0,
            trgt_vmid: vm_id,
            fid: guest_msg.fid,
            event: guest_msg.event,
        };
        if !ipi_send_msg(cpu_trgt, IpiType::IpiTHvc, IpiInnerMsg::HvcMsg(ipi_msg)) {
            println!(
                "hvc_send_msg_to_vm: Failed to send ipi message, target {} type {:#?}",
                cpu_trgt, IpiType::IpiTHvc
            );
        }
    } else {
        hvc_guest_notify(vm_id);
        return true;
    }

    true
}

// notify current cpu's vcpu
pub fn hvc_guest_notify(vm_id: usize) {
    let vm = vm(vm_id);
    match current_cpu().vcpu_pool().pop_vcpu_through_vmid(vm_id) {
        None => {
            println!("hvc_guest_notify: Core {} failed to find vcpu of VM {}", current_cpu().id, vm_id);
        }
        Some(vcpu) => {
            interrupt_vm_inject(vm.clone(), vcpu.clone(), HVC_IRQ, 0);
        }
    };
}

pub fn hvc_ipi_handler(msg: &IpiMessage) {
    match &msg.ipi_message {
        IpiInnerMsg::HvcMsg(msg) => {
            if current_cpu().vcpu_pool().pop_vcpu_through_vmid(msg.trgt_vmid).is_none() {
                println!("hvc_ipi_handler: Core {} failed to find vcpu of VM {}", current_cpu().id, msg.trgt_vmid);
                return;
            }

            match msg.fid {
                HVC_MEDIATED => {
                    hvc_guest_notify(msg.trgt_vmid);
                }
                _ => {
                    todo!();
                }
            }
        }
        _ => {
            println!("vgic_ipi_handler: illegal ipi");
            return;
        }
    }
}

pub fn hvc_init() {
    if !ipi_register(IpiType::IpiTHvc, hvc_ipi_handler) {
        panic!(
            "hvc_init: failed to register hvc ipi {}",
            IpiType::IpiTHvc as usize
        )
    }
}