use alloc::slice::{Iter, IterMut};
use crate::kernel::{current_cpu, Vcpu, interrupt_cpu_enable, CONFIG_VM_NUM_MAX};

pub struct VcpuArray {
    array: [Option<Vcpu>; CONFIG_VM_NUM_MAX],
    len: usize,
}

impl VcpuArray {
    pub const fn new() -> Self {
        Self {
            array: [const { None }; CONFIG_VM_NUM_MAX],
            len: 0,
        }
    }

    #[deprecated]
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
        vcpu.set_phys_id(current_cpu().id);
        info!(
            "append_vcpu: append VM[{}] vcpu {} on core {}",
            vm_id,
            vcpu.id(),
            current_cpu().id
        );
        self.array[vm_id] = Some(vcpu);
        self.len += 1;
    }

    pub fn remove_vcpu(&mut self, vm_id: usize) -> Option<Vcpu> {
        if vm_id >= self.array.len() {
            panic!("vm_id > self.array.len()");
        }
        match self.array[vm_id].clone() {
            Some(vcpu) => {
                self.len -= 1;
                self.array[vm_id] = None;
                if self.len == 0 {
                    // hard code: remove el1 timer interrupt 27
                    interrupt_cpu_enable(27, false);
                }
                Some(vcpu)
            }
            None => panic!(
                "no vcpu from vm[{}] exist in Core[{}] vcpu_pool",
                vm_id,
                current_cpu().id
            ),
        }
    }

    pub fn iter(&self) -> Iter<'_, Option<Vcpu>> {
        self.array.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, Option<Vcpu>> {
        self.array.iter_mut()
    }
}

pub fn restore_vcpu_gic(cur_vcpu: Option<Vcpu>, trgt_vcpu: &Vcpu) {
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

pub fn save_vcpu_gic(cur_vcpu: Option<Vcpu>, trgt_vcpu: &Vcpu) {
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
