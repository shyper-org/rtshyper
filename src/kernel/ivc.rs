use crate::arch::PAGE_SIZE;
use crate::kernel::{active_vm, current_cpu, vm_if_set_ivc_arg, vm_if_set_ivc_arg_ptr};

use shyper::VM_NUM_MAX;

pub fn ivc_update_mq(receive_ipa: usize, cfg_ipa: usize) -> bool {
    let vm = active_vm().unwrap();
    let vm_id = vm.id();
    let receive_pa = vm.ipa2hva(receive_ipa);
    let cfg_pa = vm.ipa2hva(cfg_ipa);

    if receive_pa == 0 {
        error!("ivc_update_mq: invalid receive_pa");
        return false;
    }

    vm_if_set_ivc_arg(vm_id, cfg_pa);
    vm_if_set_ivc_arg_ptr(vm_id, cfg_pa - PAGE_SIZE / VM_NUM_MAX);

    let idx = 0;
    let val = vm_id;
    current_cpu().set_gpr(idx, val);
    trace!("VM {} update message", vm_id);
    true
}

pub fn shyper_init(vmid: usize, base_ipa: usize, len: usize) -> bool {
    if base_ipa == 0 || len == 0 {
        info!("vm{} shyper base ipa {:x}, len {:x}", vmid, base_ipa, len);
        return true;
    }
    false
}
