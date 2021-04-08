use crate::kernel::cpu_id;
use crate::lib::barrier;

pub fn vmm_init() {
    barrier();

    if cpu_id() == 0 {
        super::vmm_init_config();
    }

    if cpu_id() == 0 {
        println!("Sybilla Hypervisor init ok\n\nStart booting VMs ...");
    }

    barrier();
}