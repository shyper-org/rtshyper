use crate::arch::{PAGE_SIZE, PTE_S2_NORMAL, PTE_S2_RO};
use crate::kernel::{
    active_vm, get_share_mem, hvc_send_msg_to_vm, HVC_VMM, HVC_VMM_MIGRATE_START, HvcGuestMsg, HvcMigrateMsg,
    mem_pages_alloc, MIGRATE_BITMAP, MIGRATE_COPY, MIGRATE_FINISH, MIGRATE_SEND, vm, Vm, vm_if_clear_mem_map,
    vm_if_cpy_mem_map, vm_if_mem_map_cache, vm_if_mem_map_page_num, vm_if_set_mem_map_cache,
};

pub fn migrate_memcpy(vmid: usize) {
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
    // copy trgt_vm dirty mem map to kernel module
    vm_if_cpy_mem_map(vmid);
    vm_if_clear_mem_map(vmid);

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

pub fn migrate_finish_ipi_handler(vmid: usize) {
    println!("Core 0 handle finish ipi");
    // copy trgt_vm dirty mem map to kernel module
    let vm = vm(vmid).unwrap();
    vm_if_cpy_mem_map(vmid);
    vm_if_clear_mem_map(vmid);
    vm.context_vm_migrate_save();
    hvc_send_msg_to_vm(
        0,
        &HvcGuestMsg::Migrate(HvcMigrateMsg {
            fid: HVC_VMM,
            event: HVC_VMM_MIGRATE_START,
            vm_id: vmid,
            oper: MIGRATE_FINISH,
            page_num: vm_if_mem_map_page_num(vmid),
        }),
    );
}
