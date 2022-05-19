use core::mem::size_of;
use spin::Mutex;

use crate::arch::{
    Aarch64ContextFrame, GIC_LIST_REGS_NUM, GIC_PRIVINT_NUM, GIC_SGIS_NUM, GIC_SPI_MAX, IrqState, PAGE_SIZE,
    PTE_S2_FIELD_AP_RW, PTE_S2_NORMAL, PTE_S2_RO, Sgis, VmContext,
};
use crate::arch::tlb_invalidate_guest_all;
use crate::device::{EMU_DEV_NUM_MAX, EmuContext, VirtioDeviceType, VirtMmioRegs};
use crate::kernel::{
    active_vm, AllocError, current_cpu, get_share_mem, hvc_send_msg_to_vm, HVC_VMM, HVC_VMM_MIGRATE_START, HvcGuestMsg,
    HvcMigrateMsg, mem_pages_alloc, MIGRATE_BITMAP, MIGRATE_COPY, MIGRATE_FINISH, MIGRATE_SEND, vm, Vm,
    vm_if_copy_mem_map, vm_if_mem_map_cache, vm_if_mem_map_page_num, vm_if_set_mem_map, vm_if_set_mem_map_cache,
};
use crate::lib::round_up;
use crate::mm::PageFrame;

pub struct VMData {
    vm_ctx: VmContext,
    vcpu_ctx: Aarch64ContextFrame,
    emu_devs: [EmuDevData; EMU_DEV_NUM_MAX],
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
    vgicd: VgicdData,
    cpu_priv: VgicCpuPrivData,
}

pub struct VgicdData {
    ctlr: u32,
    typer: u32,
    iidr: u32,
    interrupts: [VgicIntData; GIC_SPI_MAX],
}

pub struct VgicCpuPrivData {
    curr_lrs: [u16; GIC_LIST_REGS_NUM],
    sgis: [Sgis; GIC_SGIS_NUM],
    interrupts: [VgicIntData; GIC_PRIVINT_NUM],
    pend_list: [usize; 16],
    // TODO: 16 is hard code
    act_list: [usize; 16],
}

pub struct VgicIntData {
    owner: usize,
    // vcpu_id
    id: u16,
    hw: bool,
    in_lr: bool,
    lr: u16,
    enabled: bool,
    state: IrqState,
    prio: u8,
    targets: u8,
    cfg: u8,

    in_pend: bool,
    in_act: bool,
}

// virtio mmio migration data
pub struct VirtioMmioData {
    id: usize,
    driver_features: usize,
    driver_status: usize,
    regs: VirtMmioRegs,
}

pub struct VirtDevData {
    activated: bool,
    dev_type: VirtioDeviceType,
    features: usize,
    generation: usize,
    int_id: usize,
    desc: DevDescData,
    // req: reserve; we used nfs, no need to mig blk req data
    // cache: reserve
    // stat: reserve
}

pub enum DevDescData {
    BlkDesc(BlkDescData),
    // reserve
    NetDesc(NetDescData),
    ConsoleDesc(ConsoleDescData),
    None,
}

pub struct BlkDescData {}

pub struct NetDescData {
    mac: [u8; 6],
    status: u16,
}

pub struct ConsoleDescData {
    oppo_end_vmid: u16,
    oppo_end_ipa: u64,
    // vm access
    cols: u16,
    rows: u16,
    max_nr_ports: u32,
    emerg_wr: u32,
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

pub fn migrate_vm_context(vm: Vm) {
    let size = size_of::<VMData>();
    match mem_pages_alloc(round_up(size, PAGE_SIZE)) {
        Ok(pf) => {
            let vm_data = unsafe { pf.pa as *mut VMData };
        }
        Err(_) => {}
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
