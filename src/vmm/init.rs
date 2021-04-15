use crate::kernel::cpu_id;
use crate::lib::barrier;
use alloc::sync::Arc;
use spin::Mutex;
use alloc::vec::Vec;
use crate::config::VmCpuConfig;
use crate::kernel::{Vm, VmInner};

fn vmm_init_cpu(config: &VmCpuConfig, vm_arc: &Vm) -> bool {
    use crate::board::PLATFORM_VCPU_NUM_MAX;
    use crate::kernel::Vcpu;
    let vm_lock = vm_arc.inner();
    let mut vm = vm_lock.lock();

    for i in 0..config.num {
        use crate::kernel::vcpu_alloc;
        if let Some(vcpu_arc_mutex) = vcpu_alloc() {
            let mut vcpu = vcpu_arc_mutex.lock();
            vm.vcpu_list.push(vcpu_arc_mutex.clone());
            crate::kernel::vcpu_init(vm_arc, &mut *vcpu, i);
        } else {
            println!("failed to allocte vcpu");
            return false;
        }
    }

    // remain to be init when assigning vcpu
    vm.cpu_num = 0;
    vm.ncpu = 0;
    println!(
        "VM {} init cpu: cores=<{}>, allocat_bits=<{:x}>",
        vm.id, config.num, config.allocate_bitmap
    );

    true
}

pub fn vmm_init() {
    if cpu_id() == 0 {
        super::vmm_init_config();

        use crate::config::{VmConfigTable, DEF_VM_CONFIG_TABLE};
        let vm_cfg_table = DEF_VM_CONFIG_TABLE.lock();
        let vm_num = vm_cfg_table.vm_num;

        use crate::kernel::VM_LIST;
        for i in 0..vm_num {
            let mut vm_list = VM_LIST.lock();
            
            let vm = Vm::new(i);
            vm_list.push(vm);

            let vm_arc = vm_list[i].inner();
            let mut vm = vm_arc.lock();

            vm.config = Some(vm_cfg_table.entries[i].clone());
            drop(vm);

            vmm_init_cpu(&vm_cfg_table.entries[i].cpu, &vm_list[i]);
        }
        drop(vm_cfg_table);
    }
    barrier();

    // TODO vmm_assign_vcpu

    if cpu_id() == 0 {
        println!("Sybilla Hypervisor init ok\n\nStart booting VMs ...");
    }

    barrier();
}
