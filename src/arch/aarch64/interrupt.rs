use super::GICD;

pub fn interrupt_arch_init() {
    use crate::arch::{gic_cpu_init, gic_glb_init};
    crate::lib::barrier();

    if crate::kernel::cpu_id() == 0 {
        gic_glb_init();
    }

    gic_cpu_init();

    //TODO
}

pub fn interrupt_arch_enable(int_id: usize, en: bool) {
    // use super::gic::{gicd_set_enable, gicd_set_prio, gicd_set_trgt};
    use crate::board::platform_cpuid_to_cpuif;

    let cpu_id = crate::kernel::cpu_id();
    GICD.set_enable(int_id, en);
    GICD.set_prio(int_id, 0x7f);
    GICD.set_trgt(int_id, 1 << platform_cpuid_to_cpuif(cpu_id));
}
