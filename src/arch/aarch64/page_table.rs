use crate::kernel::Cpu;

pub fn pt_map_banked_cpu(cpu: &mut Cpu) {
    (*cpu).id = 1;
}