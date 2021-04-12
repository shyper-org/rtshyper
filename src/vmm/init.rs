use crate::kernel::cpu_id;
use crate::lib::barrier;
use spin::Mutex;
use alloc::sync::Arc;

use crate::config::VmCpuConfig;
use crate::kernel::Vm;

fn vmm_init_cpu(config: &VmCpuConfig, vm: &mut Vm) -> bool {
    true
}

pub fn vmm_init() {

    if cpu_id() == 0 {
        super::vmm_init_config();
    }
    barrier();

    use crate::config::{VmConfigTable, DEF_VM_CONFIG_TABLE};
    let vm_cfg_table = DEF_VM_CONFIG_TABLE.lock();
    let vm_num = vm_cfg_table.vm_num;

    use crate::kernel::VM_LIST;
    for i in 0..vm_num {
        let mut vm_list = VM_LIST.lock();
        vm_list[i].id = i;
        vm_list[i].config = Some(vm_cfg_table.entries[i].clone());

        vmm_init_cpu(&vm_cfg_table.entries[i].cpu, &mut vm_list[i]);
        println!("after {} {}", crate::kernel::cpu_id(), vm_num);
    }
    drop(vm_cfg_table);

    if cpu_id() == 0 {
        println!("Sybilla Hypervisor init ok\n\nStart booting VMs ...");
    }

    barrier();
}
