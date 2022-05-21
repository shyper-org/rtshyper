use crate::arch::{
    Aarch64ContextFrame, GIC_LIST_REGS_NUM, GIC_PRIVINT_NUM, GIC_SGIS_NUM, GIC_SPI_MAX, IrqState, PAGE_SIZE,
    PTE_S2_FIELD_AP_RW, PTE_S2_NORMAL, PTE_S2_RO, Sgis, VmContext,
};
use crate::arch::tlb_invalidate_guest_all;
use crate::device::{EMU_DEV_NUM_MAX, EmuContext, VirtioDeviceType, VirtMmioRegs};
use crate::kernel::{
    active_vm, current_cpu, get_share_mem, hvc_send_msg_to_vm, HVC_VMM, HVC_VMM_MIGRATE_START, HvcGuestMsg,
    HvcMigrateMsg, mem_pages_alloc, MIGRATE_BITMAP, MIGRATE_COPY, MIGRATE_FINISH, MIGRATE_SEND, vm, Vm,
    vm_if_copy_mem_map, vm_if_mem_map_cache, vm_if_mem_map_page_num, vm_if_set_mem_map, vm_if_set_mem_map_cache,
};

pub struct VMData {
    pub vm_ctx: VmContext,
    pub vcpu_ctx: Aarch64ContextFrame,
    pub emu_devs: [EmuDevData; EMU_DEV_NUM_MAX],
}

pub enum EmuDevData {
    Vgic(VgicMigData),
    VirtioBlk(VirtioMmioData),
    VirtioNet(VirtioMmioData),
    VirtioConsole(VirtioMmioData),
    None,
}

// virtio vgic migration data
pub struct VgicMigData {
    pub vgicd: VgicdData,
    pub cpu_priv_num: usize,
    pub cpu_priv: [VgicCpuPrivData; 4],
}

impl VgicMigData {
    pub fn default() -> VgicMigData {
        VgicMigData {
            vgicd: VgicdData::default(),
            cpu_priv_num: 0,
            cpu_priv: [VgicCpuPrivData::default(); 4], // TODO: 4 is hardcode for vm cpu num max
        }
    }
}

pub struct VgicdData {
    pub ctlr: u32,
    pub typer: u32,
    pub iidr: u32,
    pub interrupts: [VgicIntData; GIC_SPI_MAX],
}

impl VgicdData {
    pub fn default() -> VgicdData {
        VgicdData {
            ctlr: 0,
            typer: 0,
            iidr: 0,
            interrupts: [VgicIntData::default(); GIC_SPI_MAX],
        }
    }
}

#[derive(Copy, Clone)]
pub struct VgicCpuPrivData {
    pub curr_lrs: [u16; GIC_LIST_REGS_NUM],
    pub sgis: [Sgis; GIC_SGIS_NUM],
    pub interrupts: [VgicIntData; GIC_PRIVINT_NUM],
    pub pend_num: usize,
    pub pend_list: [usize; 16],
    // TODO: 16 is hard code
    pub act_num: usize,
    pub act_list: [usize; 16],
}

impl VgicCpuPrivData {
    pub fn default() -> VgicCpuPrivData {
        VgicCpuPrivData {
            curr_lrs: [0; GIC_LIST_REGS_NUM],
            sgis: [Sgis { pend: 0, act: 0 }; GIC_SGIS_NUM],
            interrupts: [VgicIntData::default(); GIC_PRIVINT_NUM],
            pend_num: 0,
            pend_list: [0; 16],
            act_num: 0,
            act_list: [0; 16],
        }
    }
}

#[derive(Copy, Clone)]
pub struct VgicIntData {
    pub owner: Option<usize>,
    // vcpu_id
    pub id: u16,
    pub hw: bool,
    pub in_lr: bool,
    pub lr: u16,
    pub enabled: bool,
    pub state: IrqState,
    pub prio: u8,
    pub targets: u8,
    pub cfg: u8,

    pub in_pend: bool,
    pub in_act: bool,
}

impl VgicIntData {
    pub fn default() -> VgicIntData {
        VgicIntData {
            owner: None,
            id: 0,
            hw: false,
            in_lr: false,
            lr: 0,
            enabled: false,
            state: IrqState::IrqSInactive,
            prio: 0,
            targets: 0,
            cfg: 0,
            in_pend: false,
            in_act: false,
        }
    }
}

// virtio mmio migration data
pub struct VirtioMmioData {
    pub id: usize,
    pub driver_features: usize,
    pub driver_status: usize,
    pub regs: VirtMmioRegs,
    pub dev: VirtDevData,
    pub vq: [VirtqData; 2], // TODO: 2 is hard code for vq max len
}

impl VirtioMmioData {
    pub fn default() -> VirtioMmioData {
        VirtioMmioData {
            id: 0,
            driver_features: 0,
            driver_status: 0,
            regs: VirtMmioRegs::default(),
            dev: VirtDevData::default(),
            vq: [VirtqData::default(); 2],
        }
    }
}

#[derive(Copy, Clone)]
pub struct VirtqData {
    pub ready: usize,
    pub vq_index: usize,
    pub num: usize,

    pub last_avail_idx: u16,
    pub last_used_idx: u16,
    pub used_flags: u16,

    pub desc_table_addr: usize,
    pub avail_addr: usize,
    pub used_addr: usize,
}

impl VirtqData {
    pub fn default() -> VirtqData {
        VirtqData {
            ready: 0,
            vq_index: 0,
            num: 0,
            last_avail_idx: 0,
            last_used_idx: 0,
            used_flags: 0,
            desc_table_addr: 0,
            avail_addr: 0,
            used_addr: 0,
        }
    }
}

pub struct VirtDevData {
    pub activated: bool,
    pub dev_type: VirtioDeviceType,
    pub features: usize,
    pub generation: usize,
    pub int_id: usize,
    pub desc: DevDescData,
    // req: reserve; we used nfs, no need to mig blk req data
    // cache: reserve
    // stat: reserve
}

impl VirtDevData {
    pub fn default() -> VirtDevData {
        VirtDevData {
            activated: false,
            dev_type: VirtioDeviceType::None,
            features: 0,
            generation: 0,
            int_id: 0,
            desc: DevDescData::None,
        }
    }
}

pub enum DevDescData {
    // reserve blk desc
    BlkDesc(BlkDescData),
    NetDesc(NetDescData),
    ConsoleDesc(ConsoleDescData),
    None,
}

pub struct BlkDescData {}

pub struct NetDescData {
    pub mac: [u8; 6],
    pub status: u16,
}

pub struct ConsoleDescData {
    pub oppo_end_vmid: u16,
    pub oppo_end_ipa: u64,
    // vm access
    pub cols: u16,
    pub rows: u16,
    pub max_nr_ports: u32,
    pub emerg_wr: u32,
}

pub fn migrate_ready(vmid: usize) {
    if vm_if_mem_map_cache(vmid).is_none() {
        let trgt_vm = vm(vmid).unwrap();
        map_migrate_vm_mem(trgt_vm, get_share_mem(MIGRATE_SEND));
        match mem_pages_alloc(vm_if_mem_map_page_num(vmid)) {
            Ok(pf) => {
                // println!("bitmap size to page num {}", vm_if_mem_map_page_num(vmid));
                // map dirty bitmap
                active_vm().unwrap().pt_map_range(
                    get_share_mem(MIGRATE_BITMAP),
                    PAGE_SIZE * vm_if_mem_map_page_num(vmid),
                    pf.pa(),
                    PTE_S2_RO,
                );
                vm_if_set_mem_map_cache(vmid, pf);
            }
            Err(_) => {
                panic!("HVC_VMM_MIGRATE_MEMCPY: mem_pages_alloc failed");
            }
        }
    }
}

pub fn migrate_memcpy(vmid: usize) {
    // copy trgt_vm dirty mem map to kernel module
    println!("migrate_memcpy, vm_id {}", vmid);
    hvc_send_msg_to_vm(
        0,
        &HvcGuestMsg::Migrate(HvcMigrateMsg {
            fid: HVC_VMM,
            event: HVC_VMM_MIGRATE_START,
            vm_id: vmid,
            oper: MIGRATE_COPY,
            page_num: vm_if_mem_map_page_num(vmid),
        }),
    );
}

pub fn map_migrate_vm_mem(vm: Vm, ipa_start: usize) {
    for i in 0..vm.region_num() {
        active_vm()
            .unwrap()
            .pt_map_range(ipa_start, vm.pa_length(i), vm.pa_start(i), PTE_S2_NORMAL);
        // println!(
        //     "ipa {:x}, length {:x}, pa start {:x}",
        //     ipa_start,
        //     vm.pa_length(i),
        //     vm.pa_start(i)
        // );
    }
}

pub fn migrate_finish_ipi_handler(vm_id: usize) {
    println!("Core 0 handle VM[{}] finish ipi", vm_id);
    // TODO: hard code for dst_vm;
    let dst_vm = vm(2).unwrap();

    // copy trgt_vm dirty mem map to kernel module
    let vm = vm(vm_id).unwrap();
    vm_if_copy_mem_map(vm_id);
    vm.context_vm_migrate_save();
    // TODO: migrate vm dev
    // dst_vm.migrate_emu_devs(vm.clone());

    hvc_send_msg_to_vm(
        0,
        &HvcGuestMsg::Migrate(HvcMigrateMsg {
            fid: HVC_VMM,
            event: HVC_VMM_MIGRATE_START,
            vm_id,
            oper: MIGRATE_FINISH,
            page_num: vm_if_mem_map_page_num(vm_id),
        }),
    );
}

pub fn migrate_data_abort_handler(emu_ctx: &EmuContext) {
    if emu_ctx.write {
        // ptr_read_write(emu_ctx.address, emu_ctx.width, val, false);
        let vm = active_vm().unwrap();
        let (pa, len) = vm.pt_set_access_permission(emu_ctx.address, PTE_S2_FIELD_AP_RW);
        let mut bit = 0;
        for i in 0..vm.region_num() {
            let start = vm.pa_start(i);
            let end = start + vm.pa_length(i);
            if emu_ctx.address >= start && emu_ctx.address < end {
                bit += (pa - active_vm().unwrap().pa_start(i)) / PAGE_SIZE;
                vm_if_set_mem_map(current_cpu().id, bit, len / PAGE_SIZE);
                break;
            }
            bit += vm.pa_length(i) / PAGE_SIZE;
            if i + 1 == vm.region_num() {
                panic!(
                    "migrate_data_abort_handler: can not found addr 0x{:x} in vm{} pa region",
                    emu_ctx.address,
                    vm.id()
                );
            }
        }
        // flush tlb for updating page table
        tlb_invalidate_guest_all();
    } else {
        panic!("migrate_data_abort_handler: permission should be read only");
    }
}
