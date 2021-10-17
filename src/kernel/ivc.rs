use crate::kernel::{active_vm, vm_if_list_set_ivc_arg, vm_if_list_set_ivc_arg_ptr, vm_ipa2pa, VM_NUM_MAX};
use crate::arch::PAGE_SIZE;

pub fn ivc_update_mq(receive_ipa: usize, cfg_ipa: usize) -> bool {
    let vm = active_vm().unwrap();
    let receive_pa = vm_ipa2pa(vm.clone(), receive_ipa);
    let cfg_pa = vm_ipa2pa(vm.clone(), cfg_ipa);

    if receive_pa == 0 {
        println!("ivc_update_mq: invalid receive_pa");
        return false;
    }

    vm_if_list_set_ivc_arg(vm.vm_id(), cfg_pa);
    vm_if_list_set_ivc_arg_ptr(vm.vm_id(), cfg_pa - PAGE_SIZE / VM_NUM_MAX);

    println!("VM {} update message", vm.vm_id());
    true
}