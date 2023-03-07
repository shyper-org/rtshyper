use alloc::collections::BTreeMap;
use core::mem::size_of;

use spin::Mutex;

use crate::arch::{PAGE_SIZE, PTE_S2_NORMAL};
use crate::arch::gicc_clear_current_irq;
use crate::config::*;
use crate::device::{mediated_blk_notify_handler, mediated_dev_append};
use crate::kernel::{
    active_vm, active_vm_id, current_cpu, DIRTY_MEM_THRESHOLD, interrupt_vm_inject, ipi_register, ipi_send_msg,
    IpiHvcMsg, IpiInnerMsg, IpiMessage, IpiType, ivc_update_mq, map_migrate_vm_mem, migrate_finish_ipi_handler,
    migrate_ready, send_migrate_memcpy_msg, unmap_migrate_vm_mem, vcpu_idle, vm, vm_if_copy_mem_map,
    vm_if_dirty_mem_map, vm_if_get_cpu_id, vm_if_ivc_arg, vm_if_ivc_arg_ptr, vm_if_mem_map_dirty_sum,
    vm_if_mem_map_page_num, vm_if_set_ivc_arg_ptr, VMData,
};
use crate::util::{func_barrier, memcpy_safe, round_up, set_barrier_num, trace};
use crate::util::unilib::*;
use crate::vmm::{get_vm_id, vmm_boot_vm, vmm_list_vm, vmm_migrate_boot, vmm_reboot_vm, vmm_remove_vm};

use shyper::VM_NUM_MAX;

pub static VM_STATE_FLAG: Mutex<usize> = Mutex::new(0);

pub static SHARE_MEM_LIST: Mutex<BTreeMap<usize, usize>> = Mutex::new(BTreeMap::new());
// If succeed, return 0.
const HVC_FINISH: usize = 0;
// If failed, return -1.
const HVC_ERR: usize = usize::MAX;

// share mem type
pub const MIGRATE_BITMAP: usize = 0;
pub const VM_CONTEXT_SEND: usize = 1;
pub const VM_CONTEXT_RECEIVE: usize = 2;
pub const MIGRATE_SEND: usize = 3;
pub const MIGRATE_RECEIVE: usize = 4;
pub const LIVE_UPDATE_IMG: usize = 5;

// hvc_fid
pub const HVC_SYS: usize = 0;
pub const HVC_VMM: usize = 1;
pub const HVC_IVC: usize = 2;
pub const HVC_MEDIATED: usize = 3;
pub const HVC_CONFIG: usize = 0x11;
pub const HVC_UNILIB: usize = 0x12;

// hvc_sys_event
pub const HVC_SYS_REBOOT: usize = 0;
pub const HVC_SYS_SHUTDOWN: usize = 1;
pub const HVC_SYS_UPDATE: usize = 3;
pub const HVC_SYS_TEST: usize = 4;

// hvc_vmm_event
pub const HVC_VMM_LIST_VM: usize = 0;
pub const HVC_VMM_GET_VM_STATE: usize = 1;
pub const HVC_VMM_BOOT_VM: usize = 2;
pub const HVC_VMM_SHUTDOWN_VM: usize = 3;
pub const HVC_VMM_REBOOT_VM: usize = 4;
pub const HVC_VMM_GET_VM_DEF_CFG: usize = 5;
pub const HVC_VMM_GET_VM_CFG: usize = 6;
pub const HVC_VMM_SET_VM_CFG: usize = 7;
pub const HVC_VMM_GET_VM_ID: usize = 8;
pub const HVC_VMM_TRACE_VMEXIT: usize = 9;
// for src vm: send msg to MVM to ask for migrating
pub const HVC_VMM_MIGRATE_START: usize = 10;
pub const HVC_VMM_MIGRATE_READY: usize = 11;
// for sender: copy dirty memory to receiver
pub const HVC_VMM_MIGRATE_MEMCPY: usize = 12;
pub const HVC_VMM_MIGRATE_FINISH: usize = 13;
// for receiver: init new vm but not boot
pub const HVC_VMM_MIGRATE_INIT_VM: usize = 14;
pub const HVC_VMM_MIGRATE_VM_BOOT: usize = 15;
pub const HVC_VMM_VM_REMOVE: usize = 16;

// hvc_ivc_event
pub const HVC_IVC_UPDATE_MQ: usize = 0;
pub const HVC_IVC_SEND_MSG: usize = 1;
pub const HVC_IVC_BROADCAST_MSG: usize = 2;
pub const HVC_IVC_INIT_KEEP_ALIVE: usize = 3;
pub const HVC_IVC_KEEP_ALIVE: usize = 4;
pub const HVC_IVC_ACK: usize = 5;
pub const HVC_IVC_GET_TIME: usize = 6;
pub const HVC_IVC_SHARE_MEM: usize = 7;
pub const HVC_IVC_SEND_SHAREMEM: usize = 0x10;
//共享内存通信
pub const HVC_IVC_GET_SHARED_MEM_IPA: usize = 0x11;
//用于VM获取共享内存IPA
pub const HVC_IVC_SEND_SHAREMEM_TEST_SPEED: usize = 0x12; //共享内存通信速度测试

// hvc_mediated_event
pub const HVC_MEDIATED_DEV_APPEND: usize = 0x30;
pub const HVC_MEDIATED_DEV_NOTIFY: usize = 0x31;
pub const HVC_MEDIATED_DRV_NOTIFY: usize = 0x32;

pub const HVC_UNILIB_FS_INIT: usize = 0;
pub const HVC_UNILIB_FS_OPEN: usize = 1;
pub const HVC_UNILIB_FS_CLOSE: usize = 2;
pub const HVC_UNILIB_FS_READ: usize = 3;
pub const HVC_UNILIB_FS_WRITE: usize = 4;
pub const HVC_UNILIB_FS_LSEEK: usize = 5;
pub const HVC_UNILIB_FS_STAT: usize = 6;
pub const HVC_UNILIB_FS_APPEND: usize = 7;
pub const HVC_UNILIB_FS_FINISHED: usize = 8;

// hvc_config_event
pub const HVC_CONFIG_ADD_VM: usize = 0;
pub const HVC_CONFIG_DELETE_VM: usize = 1;
pub const HVC_CONFIG_CPU: usize = 2;
pub const HVC_CONFIG_MEMORY_REGION: usize = 3;
pub const HVC_CONFIG_EMULATED_DEVICE: usize = 4;
pub const HVC_CONFIG_PASSTHROUGH_DEVICE_REGION: usize = 5;
pub const HVC_CONFIG_PASSTHROUGH_DEVICE_IRQS: usize = 6;
pub const HVC_CONFIG_PASSTHROUGH_DEVICE_STREAMS_IDS: usize = 7;
pub const HVC_CONFIG_DTB_DEVICE: usize = 8;
pub const HVC_CONFIG_UPLOAD_KERNEL_IMAGE: usize = 9;

#[cfg(feature = "tx2")]
pub const HVC_IRQ: usize = 32 + 0x20;
#[cfg(feature = "pi4")]
pub const HVC_IRQ: usize = 32 + 0x10;
#[cfg(feature = "qemu")]
pub const HVC_IRQ: usize = 32 + 0x20;

#[repr(C)]
pub enum HvcGuestMsg {
    Default(HvcDefaultMsg),
    Manage(HvcManageMsg),
    Migrate(HvcMigrateMsg),
    UniLib(HvcUniLibMsg),
}

#[repr(C)]
pub struct HvcDefaultMsg {
    pub fid: usize,
    pub event: usize,
}

#[repr(C)]
pub struct HvcManageMsg {
    pub fid: usize,
    pub event: usize,
    pub vm_id: usize,
}

pub const MIGRATE_START: usize = 0;
pub const MIGRATE_COPY: usize = 1;
pub const MIGRATE_FINISH: usize = 2;

#[repr(C)]
pub struct HvcMigrateMsg {
    pub fid: usize,
    pub event: usize,
    pub vm_id: usize,
    pub oper: usize,
    pub page_num: usize, // bitmap page num
}

#[repr(C)]
pub struct HvcUniLibMsg {
    pub fid: usize,
    pub event: usize,
    pub vm_id: usize,
    pub arg_1: usize,
    pub arg_2: usize,
    pub arg_3: usize,
}

pub fn add_share_mem(mem_type: usize, base: usize) {
    let mut list = SHARE_MEM_LIST.lock();
    list.insert(mem_type, base);
}

pub fn get_share_mem(mem_type: usize) -> usize {
    let list = SHARE_MEM_LIST.lock();
    match list.get(&mem_type) {
        None => {
            panic!("there is not {} type share memory", mem_type);
        }
        Some(tup) => *tup,
    }
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
) -> Result<usize, ()> {
    match hvc_type {
        HVC_SYS => hvc_sys_handler(event, x0),
        HVC_VMM => hvc_vmm_handler(event, x0, x1),
        HVC_IVC => hvc_ivc_handler(event, x0, x1),
        HVC_MEDIATED => hvc_mediated_handler(event, x0, x1),
        HVC_CONFIG => hvc_config_handler(event, x0, x1, x2, x3, x4, x5, x6),
        HVC_UNILIB => hvc_unilib_handler(event, x0, x1, x2),
        _ => {
            println!("hvc_guest_handler: unknown hvc type {} event {}", hvc_type, event);
            Err(())
        }
    }
}

fn hvc_config_handler(
    event: usize,
    x0: usize,
    x1: usize,
    x2: usize,
    x3: usize,
    x4: usize,
    x5: usize,
    x6: usize,
) -> Result<usize, ()> {
    match event {
        HVC_CONFIG_ADD_VM => vm_cfg_add_vm(x0),
        HVC_CONFIG_DELETE_VM => vm_cfg_del_vm(x0),
        HVC_CONFIG_CPU => vm_cfg_set_cpu(x0, x1, x2, x3),
        HVC_CONFIG_MEMORY_REGION => vm_cfg_add_mem_region(x0, x1, x2),
        HVC_CONFIG_EMULATED_DEVICE => vm_cfg_add_emu_dev(x0, x1, x2, x3, x4, x5, x6),
        HVC_CONFIG_PASSTHROUGH_DEVICE_REGION => vm_cfg_add_passthrough_device_region(x0, x1, x2, x3),
        HVC_CONFIG_PASSTHROUGH_DEVICE_IRQS => vm_cfg_add_passthrough_device_irqs(x0, x1, x2),
        HVC_CONFIG_PASSTHROUGH_DEVICE_STREAMS_IDS => vm_cfg_add_passthrough_device_streams_ids(x0, x1, x2),
        HVC_CONFIG_DTB_DEVICE => vm_cfg_add_dtb_dev(x0, x1, x2, x3, x4, x5, x6),
        HVC_CONFIG_UPLOAD_KERNEL_IMAGE => vm_cfg_upload_kernel_image(x0, x1, x2, x3, x4),
        _ => {
            println!("hvc_config_handler unknown event {}", event);
            Err(())
        }
    }
}

fn hvc_sys_handler(event: usize, _x0: usize) -> Result<usize, ()> {
    match event {
        HVC_SYS_UPDATE => {
            todo!()
        }
        HVC_SYS_TEST => {
            let vm = active_vm().unwrap();
            crate::device::virtio_net_announce(vm);
            Ok(0)
        }
        _ => Err(()),
    }
}

fn hvc_vmm_handler(event: usize, x0: usize, _x1: usize) -> Result<usize, ()> {
    match event {
        HVC_VMM_LIST_VM => vmm_list_vm(x0),
        HVC_VMM_GET_VM_STATE => {
            todo!();
        }
        HVC_VMM_BOOT_VM => {
            vmm_boot_vm(x0);
            Ok(HVC_FINISH)
        }
        HVC_VMM_SHUTDOWN_VM => {
            todo!();
        }
        HVC_VMM_REBOOT_VM => {
            vmm_reboot_vm(x0);
            Ok(HVC_FINISH)
        }
        HVC_VMM_GET_VM_ID => {
            get_vm_id(x0);
            Ok(HVC_FINISH)
        }
        HVC_VMM_MIGRATE_START => {
            // demo: migration for bma1
            if x0 == 0 {
                println!("migration for mvm is not supported");
                return Err(());
            }

            hvc_send_msg_to_vm(
                0,
                &HvcGuestMsg::Migrate(HvcMigrateMsg {
                    fid: HVC_VMM,
                    event: HVC_VMM_MIGRATE_START,
                    vm_id: x0,
                    oper: MIGRATE_START,
                    page_num: 0,
                }),
            );
            Ok(HVC_FINISH)
        }
        HVC_VMM_MIGRATE_READY => {
            // init gvm dirty memory bitmap
            // let cpu_trgt = vm_if_get_cpu_id(x0);
            // println!(
            //     "core {} HVC_VMM_MIGRATE_READY, cpu trgt {}, vmid {}",
            //     current_cpu().id,
            //     cpu_trgt,
            //     x0
            // );
            migrate_ready(x0);
            mvm_migrate_memory(x0);
            vm_if_dirty_mem_map(x0);

            // send_hvc_ipi(0, x0, HVC_VMM, HVC_VMM_MIGRATE_READY, cpu_trgt);
            Ok(HVC_FINISH)
        }
        HVC_VMM_MIGRATE_MEMCPY => {
            let dirty_mem_num = vm_if_mem_map_dirty_sum(x0);
            // let cpu_trgt = vm_if_get_cpu_id(x0);
            if dirty_mem_num < DIRTY_MEM_THRESHOLD {
                // Idle live vm, copy dirty mem and vm register struct
                let trgt_vm = vm(x0).unwrap();
                set_barrier_num(trgt_vm.cpu_num());
                for vcpu_id in 0..trgt_vm.cpu_num() {
                    let pcpu_id = trgt_vm.vcpuid_to_pcpuid(vcpu_id).unwrap();
                    send_hvc_ipi(0, x0, HVC_VMM, HVC_VMM_MIGRATE_FINISH, pcpu_id);
                }
            } else {
                mvm_migrate_memory(x0);
                // send_hvc_ipi(0, x0, HVC_VMM, HVC_VMM_MIGRATE_MEMCPY, cpu_trgt);
            }
            Ok(HVC_FINISH)
        }
        HVC_VMM_MIGRATE_INIT_VM => {
            info!("migrate init vm {}", x0);
            // vmm_init_gvm(x0);
            let vm = vm(x0).unwrap();
            map_migrate_vm_mem(vm.clone(), get_share_mem(MIGRATE_RECEIVE));
            vm.context_vm_migrate_init();
            Ok(HVC_FINISH)
        }
        HVC_VMM_MIGRATE_VM_BOOT => {
            let mvm = vm(0).unwrap();
            let vm = vm(x0).unwrap();

            let size = size_of::<VMData>();
            mvm.pt_unmap_range(get_share_mem(VM_CONTEXT_RECEIVE), round_up(size, PAGE_SIZE), true);
            unmap_migrate_vm_mem(vm.clone(), get_share_mem(MIGRATE_RECEIVE));

            vm.context_vm_migrate_restore();
            for vcpu_id in 0..vm.cpu_num() {
                let cpu_trgt = vm.vcpuid_to_pcpuid(vcpu_id).unwrap();
                // send ipi to target vcpu, copy data and boot vm (in ipi copy gic data)
                send_hvc_ipi(0, x0, HVC_VMM, HVC_VMM_MIGRATE_VM_BOOT, cpu_trgt);
            }
            Ok(HVC_FINISH)
        }
        HVC_VMM_MIGRATE_FINISH => {
            let mvm = vm(0).unwrap();
            let trgt_vm = vm(x0).unwrap();
            let size = size_of::<VMData>();
            mvm.pt_unmap_range(get_share_mem(VM_CONTEXT_SEND), round_up(size, PAGE_SIZE), true);
            mvm.pt_unmap_range(
                get_share_mem(MIGRATE_BITMAP),
                PAGE_SIZE * vm_if_mem_map_page_num(x0),
                true,
            );
            unmap_migrate_vm_mem(trgt_vm, get_share_mem(MIGRATE_SEND));
            vmm_remove_vm(x0);
            *VM_STATE_FLAG.lock() = 0;
            Ok(HVC_FINISH)
        }
        HVC_VMM_VM_REMOVE => {
            vmm_remove_vm(x0);
            *VM_STATE_FLAG.lock() = 0;
            Ok(HVC_FINISH)
        }
        _ => {
            println!("hvc_vmm unknown event {}", event);
            Err(())
        }
    }
}

fn hvc_ivc_handler(event: usize, x0: usize, x1: usize) -> Result<usize, ()> {
    match event {
        HVC_IVC_UPDATE_MQ => {
            if ivc_update_mq(x0, x1) {
                Ok(HVC_FINISH)
            } else {
                Err(())
            }
        }
        HVC_IVC_SHARE_MEM => {
            let vm = active_vm().unwrap();
            let base = vm.share_mem_base();
            if x0 == LIVE_UPDATE_IMG {
                // hard code for pa 0x8a000000, x1 should be 0x8000000
                vm.pt_map_range(base, x1, 0x8a000000, PTE_S2_NORMAL, true);
            }
            vm.add_share_mem_base(x1);
            add_share_mem(x0, base);
            info!(
                "VM{} add share mem type {:#x} base {:#x} len {:#x}",
                active_vm_id(),
                x0,
                base,
                x1
            );
            Ok(base)
        }
        _ => {
            println!("hvc_ivc_handler: unknown event {}", event);
            Err(())
        }
    }
}

fn hvc_mediated_handler(event: usize, x0: usize, x1: usize) -> Result<usize, ()> {
    match event {
        HVC_MEDIATED_DEV_APPEND => mediated_dev_append(x0, x1),
        HVC_MEDIATED_DEV_NOTIFY => mediated_blk_notify_handler(x0),
        _ => {
            println!("unknown mediated event {}", event);
            Err(())
        }
    }
}

fn hvc_unilib_handler(event: usize, x0: usize, x1: usize, x2: usize) -> Result<usize, ()> {
    match event {
        HVC_UNILIB_FS_INIT => unilib_fs_init(),
        HVC_UNILIB_FS_OPEN => unilib_fs_open(x0, x1, x2),
        HVC_UNILIB_FS_CLOSE => unilib_fs_close(x0),
        HVC_UNILIB_FS_READ => unilib_fs_read(x0, x1, x2),
        HVC_UNILIB_FS_WRITE => unilib_fs_write(x0, x1, x2),
        HVC_UNILIB_FS_LSEEK => unilib_fs_lseek(x0, x1, x2),
        HVC_UNILIB_FS_STAT => unilib_fs_stat(),
        HVC_UNILIB_FS_APPEND => unilib_fs_append(x0),
        HVC_UNILIB_FS_FINISHED => unilib_fs_finished(x0),
        _ => {
            println!("unknown mediated event {}", event);
            Err(())
        }
    }
}

pub fn hvc_send_msg_to_vm(vm_id: usize, guest_msg: &HvcGuestMsg) -> bool {
    let mut target_addr = 0;
    let mut arg_ptr_addr = vm_if_ivc_arg_ptr(vm_id);
    let arg_addr = vm_if_ivc_arg(vm_id);

    if arg_ptr_addr != 0 {
        arg_ptr_addr += PAGE_SIZE / VM_NUM_MAX;
        if arg_ptr_addr - arg_addr >= PAGE_SIZE {
            vm_if_set_ivc_arg_ptr(vm_id, arg_addr);
            target_addr = arg_addr;
        } else {
            vm_if_set_ivc_arg_ptr(vm_id, arg_ptr_addr);
            target_addr = arg_ptr_addr;
        }
    }

    if target_addr == 0 {
        println!("hvc_send_msg_to_vm: target VM{} interface is not prepared", vm_id);
        return false;
    }

    if trace() && (target_addr < 0x1000 || (guest_msg as *const _ as usize) < 0x1000) {
        panic!(
            "illegal des addr {:x}, src addr {:x}",
            target_addr, guest_msg as *const _ as usize
        );
    }
    let (fid, event) = match guest_msg {
        HvcGuestMsg::Default(msg) => {
            memcpy_safe(
                target_addr as *const u8,
                msg as *const _ as *const u8,
                size_of::<HvcDefaultMsg>(),
            );
            (msg.fid, msg.event)
        }
        HvcGuestMsg::Migrate(msg) => {
            memcpy_safe(
                target_addr as *const u8,
                msg as *const _ as *const u8,
                size_of::<HvcMigrateMsg>(),
            );
            (msg.fid, msg.event)
        }
        HvcGuestMsg::Manage(msg) => {
            memcpy_safe(
                target_addr as *const u8,
                msg as *const _ as *const u8,
                size_of::<HvcManageMsg>(),
            );
            (msg.fid, msg.event)
        }
        HvcGuestMsg::UniLib(msg) => {
            memcpy_safe(
                target_addr as *const u8,
                msg as *const _ as *const u8,
                size_of::<HvcUniLibMsg>(),
            );
            (msg.fid, msg.event)
        }
    };

    let cpu_trgt = vm_if_get_cpu_id(vm_id).unwrap();
    if cpu_trgt != current_cpu().id {
        // println!("cpu {} send hvc msg to cpu {}", current_cpu().id, cpu_trgt);
        let ipi_msg = IpiHvcMsg {
            src_vmid: 0,
            trgt_vmid: vm_id,
            fid,
            event,
        };
        if !ipi_send_msg(cpu_trgt, IpiType::IpiTHvc, IpiInnerMsg::HvcMsg(ipi_msg)) {
            println!(
                "hvc_send_msg_to_vm: Failed to send ipi message, target {} type {:#?}",
                cpu_trgt,
                IpiType::IpiTHvc
            );
        }
    } else {
        hvc_guest_notify(vm_id);
    }

    true
}

// notify current cpu's vcpu
pub fn hvc_guest_notify(vm_id: usize) {
    let vm = vm(vm_id).unwrap();
    match current_cpu().vcpu_array.pop_vcpu_through_vmid(vm_id) {
        None => {
            println!(
                "hvc_guest_notify: Core {} failed to find vcpu of VM {}",
                current_cpu().id,
                vm_id
            );
        }
        Some(vcpu) => {
            interrupt_vm_inject(&vm, &vcpu, HVC_IRQ, 0);
        }
    };
}

pub fn hvc_ipi_handler(msg: &IpiMessage) {
    match &msg.ipi_message {
        IpiInnerMsg::HvcMsg(msg) => {
            if current_cpu().vcpu_array.pop_vcpu_through_vmid(msg.trgt_vmid).is_none() {
                println!(
                    "hvc_ipi_handler: Core {} failed to find vcpu of VM {}",
                    current_cpu().id,
                    msg.trgt_vmid
                );
                return;
            }

            match msg.fid {
                HVC_MEDIATED => {
                    hvc_guest_notify(msg.trgt_vmid);
                }
                HVC_VMM => match msg.event {
                    HVC_VMM_MIGRATE_START => {
                        // in mvm
                        hvc_guest_notify(msg.trgt_vmid);
                    }
                    HVC_VMM_MIGRATE_FINISH => {
                        // 被迁移VM收到该ipi标志vcpu_idle，VM0收到该ipi标志最后一次内存拷贝
                        if current_cpu().id == 0 {
                            migrate_finish_ipi_handler(msg.src_vmid);
                            return;
                        }
                        let trgt_vcpu = match current_cpu().vcpu_array.pop_vcpu_through_vmid(msg.trgt_vmid) {
                            None => {
                                println!(
                                    "Core {} failed to find target vcpu, vmid {}",
                                    current_cpu().id,
                                    msg.trgt_vmid
                                );
                                return;
                            }
                            Some(vcpu) => vcpu,
                        };
                        let vm = trgt_vcpu.vm().unwrap();
                        // println!("Core[{}] clear irq {}", current_cpu().id, current_cpu().current_irq);
                        gicc_clear_current_irq(true);
                        // 当满足下序条件时需要拷贝cpu.ctx
                        // 否则意味着当前核心有多个虚拟机共享，且被迁移虚拟机所在的核心尚未被调度到，寄存器数值无需更新
                        if vm.id() == msg.trgt_vmid {
                            trgt_vcpu.context_vm_store();
                        }
                        // save gic register for each vcpu
                        trgt_vcpu.context_gic_irqs_store();
                        func_barrier();
                        *VM_STATE_FLAG.lock() = 1;
                        if trgt_vcpu.id() == 0 {
                            vm.context_vm_migrate_save();
                            // println!("send finish ipi to core0");
                            send_hvc_ipi(msg.trgt_vmid, 0, HVC_VMM, HVC_VMM_MIGRATE_FINISH, 0);
                        }
                        // println!(
                        //     "VMData size is {:x}, VgicMigData size is {:x}, VirtioMmioData size is {:x}",
                        //     size_of::<VMData>(),
                        //     size_of::<VgicMigData>(),
                        //     size_of::<VirtioMmioData>(),
                        // );
                        vcpu_idle(trgt_vcpu);
                    }
                    HVC_VMM_MIGRATE_VM_BOOT => {
                        // let vm = vm(msg.trgt_vmid).unwrap();
                        // vm.set_migration_state(true);

                        gicc_clear_current_irq(true);
                        match current_cpu().vcpu_array.pop_vcpu_through_vmid(msg.trgt_vmid) {
                            None => {
                                panic!("Core[{}] does not have VM[{}] vcpu", current_cpu().id, msg.trgt_vmid);
                            }
                            Some(vcpu) => {
                                current_cpu().scheduler().yield_to(vcpu.clone());
                                // restore gic register for each vcpu
                                vcpu.context_gic_irqs_restore();
                            }
                        }
                        vmm_migrate_boot();
                    }
                    _ => {}
                },
                HVC_CONFIG => match msg.event {
                    HVC_CONFIG_UPLOAD_KERNEL_IMAGE => {
                        hvc_guest_notify(msg.trgt_vmid);
                    }
                    _ => {
                        todo!();
                    }
                },
                HVC_UNILIB => {
                    hvc_guest_notify(msg.trgt_vmid);
                }
                _ => {
                    todo!();
                }
            }
        }
        _ => {
            println!("vgic_ipi_handler: illegal ipi");
        }
    }
}

fn mvm_migrate_memory(trgt_vmid: usize) {
    let vm = vm(trgt_vmid);
    vm.as_ref().unwrap().pt_read_only();
    // tlb_invalidate_guest_all();
    vm_if_copy_mem_map(trgt_vmid);
    send_migrate_memcpy_msg(trgt_vmid);
}

pub fn hvc_init() {
    if !ipi_register(IpiType::IpiTHvc, hvc_ipi_handler) {
        panic!("hvc_init: failed to register hvc ipi {}", IpiType::IpiTHvc as usize)
    }
}

pub fn send_hvc_ipi(src_vmid: usize, trgt_vmid: usize, fid: usize, event: usize, trgt_cpuid: usize) {
    let ipi_msg = IpiHvcMsg {
        src_vmid,
        trgt_vmid,
        fid,
        event,
    };
    if !ipi_send_msg(trgt_cpuid, IpiType::IpiTHvc, IpiInnerMsg::HvcMsg(ipi_msg)) {
        println!(
            "send_hvc_ipi: Failed to send ipi message, target {} type {:#?}",
            0,
            IpiType::IpiTHvc
        );
    }
}
