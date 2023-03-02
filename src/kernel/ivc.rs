use crate::arch::PAGE_SIZE;
use crate::kernel::{active_vm, current_cpu, Vm, vm_if_set_ivc_arg, vm_if_set_ivc_arg_ptr, vm_ipa2hva};

use shyper::VM_NUM_MAX;

pub fn ivc_update_mq(receive_ipa: usize, cfg_ipa: usize) -> bool {
    let vm = active_vm().unwrap();
    let vm_id = vm.id();
    let receive_pa = vm_ipa2hva(&vm, receive_ipa);
    let cfg_pa = vm_ipa2hva(&vm, cfg_ipa);

    if receive_pa == 0 {
        println!("ivc_update_mq: invalid receive_pa");
        return false;
    }

    vm_if_set_ivc_arg(vm_id, cfg_pa);
    vm_if_set_ivc_arg_ptr(vm_id, cfg_pa - PAGE_SIZE / VM_NUM_MAX);

    let idx = 0;
    let val = vm_id;
    current_cpu().set_gpr(idx, val);
    // println!("VM {} update message", vm_id);
    true
}

pub fn shyper_init(vm: &Vm, base_ipa: usize, len: usize) -> bool {
    if base_ipa == 0 || len == 0 {
        info!("vm{} shyper base ipa {:x}, len {:x}", vm.id(), base_ipa, len);
        return true;
    }
    false
}
