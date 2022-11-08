use crate::board::{PLAT_DESC, SchedRule};
use crate::kernel::{current_cpu, SchedType, SchedulerRR, Vcpu, VM_NUM_MAX, interrupt_cpu_enable};

pub struct VcpuArray {
    array: [Option<Vcpu>; VM_NUM_MAX],
    len: usize,
}

impl VcpuArray {
    pub const fn new() -> Self {
        Self {
            array: [None, None, None, None, None, None, None, None],
            len: 0,
        }
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.array.len()
    }

    #[inline]
    pub fn pop_vcpu_through_vmid(&self, vm_id: usize) -> Option<Vcpu> {
        self.array[vm_id].clone()
    }

    #[inline]
    pub fn vcpu_num(&self) -> usize {
        self.len
    }

    pub fn append_vcpu(&mut self, vcpu: Vcpu) {
        // There is only 1 VCPU from a VM in a PCPU
        let vm_id = vcpu.vm_id();
        if vm_id >= self.array.len() {
            panic!("vm_id > self.array.len()");
        }
        if self.array[vm_id].is_some() {
            panic!("self.array[vm_id].is_some()");
        }
        println!(
            "append_vcpu: append VM[{}] vcpu {} on core {}",
            vm_id,
            vcpu.id(),
            current_cpu().id
        );
        self.array[vm_id] = Some(vcpu);
        self.len += 1;
    }

    pub fn remove_vcpu(&mut self, vm_id: usize) {
        if vm_id >= self.array.len() {
            panic!("vm_id > self.array.len()");
        }
        match self.array[vm_id].clone() {
            Some(_) => {
                self.len -= 1;
                self.array[vm_id] = None;
            }
            None => panic!(
                "no vcpu from vm[{}] exist in Core[{}] vcpu_pool",
                vm_id,
                current_cpu().id
            ),
        }
        if self.len == 0 {
            // hard code: remove el1 timer interrupt 27
            interrupt_cpu_enable(27, false);
        }
    }
}

// Todo: add config for base slice
pub fn cpu_sched_init() {
    match PLAT_DESC.cpu_desc.sched_list[current_cpu().id] {
        SchedRule::RoundRobin => {
            println!("cpu[{}] init Round Robin Scheduler", current_cpu().id);
            current_cpu().sched = SchedType::SchedRR(SchedulerRR::new(1));
        }
        _ => {
            todo!();
        }
    }
}

pub fn restore_vcpu_gic(cur_vcpu: Option<Vcpu>, trgt_vcpu: Vcpu) {
    // println!("restore_vcpu_gic");
    match cur_vcpu {
        None => {
            // println!("None cur vmid trgt {}", trgt_vcpu.vm_id());
            trgt_vcpu.gic_restore_context();
        }
        Some(active_vcpu) => {
            if trgt_vcpu.vm_id() != active_vcpu.vm_id() {
                // println!("different vm_id cur {}, trgt {}", active_vcpu.vm_id(), trgt_vcpu.vm_id());
                active_vcpu.gic_save_context();
                trgt_vcpu.gic_restore_context();
            }
        }
    }
}

pub fn save_vcpu_gic(cur_vcpu: Option<Vcpu>, trgt_vcpu: Vcpu) {
    // println!("save_vcpu_gic");
    match cur_vcpu {
        None => {
            trgt_vcpu.gic_save_context();
        }
        Some(active_vcpu) => {
            if trgt_vcpu.vm_id() != active_vcpu.vm_id() {
                trgt_vcpu.gic_save_context();
                active_vcpu.gic_restore_context();
            }
        }
    }
}
