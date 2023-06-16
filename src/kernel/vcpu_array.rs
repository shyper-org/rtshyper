use alloc::slice::{Iter, IterMut};
use crate::kernel::{current_cpu, Vcpu, CONFIG_VM_NUM_MAX};

pub struct VcpuArray {
    array: [Option<Vcpu>; CONFIG_VM_NUM_MAX],
    len: usize,
}

#[allow(dead_code)]
impl VcpuArray {
    pub const fn new() -> Self {
        Self {
            array: [const { None }; CONFIG_VM_NUM_MAX],
            len: 0,
        }
    }

    #[inline]
    pub fn pop_vcpu_through_vmid(&self, vm_id: usize) -> Option<Vcpu> {
        self.array[vm_id].clone()
    }

    #[inline]
    pub(super) fn vcpu_num(&self) -> usize {
        self.len
    }

    pub fn append_vcpu(&mut self, vcpu: Vcpu) {
        // There is only 1 VCPU from a VM in a PCPU
        let vm_id = vcpu.vm_id();
        match self.array.get_mut(vm_id) {
            Some(x) => match x {
                Some(_) => panic!("self.array[vm_id].is_some()"),
                None => {
                    debug_assert_eq!(current_cpu().id, vcpu.phys_id());
                    info!(
                        "append_vcpu: append VM[{}] vcpu {} on core {}",
                        vm_id,
                        vcpu.id(),
                        current_cpu().id
                    );
                    *x = Some(vcpu);
                    self.len += 1;
                }
            },
            None => panic!("vm_id > self.array.len()"),
        }
    }

    pub fn remove_vcpu(&mut self, vm_id: usize) -> Option<Vcpu> {
        match self.array.get_mut(vm_id) {
            Some(x) => x.take().map(|vcpu| {
                self.len -= 1;
                vcpu
            }),
            None => None,
        }
    }

    pub fn iter(&self) -> Iter<'_, Option<Vcpu>> {
        self.array.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, Option<Vcpu>> {
        self.array.iter_mut()
    }
}
