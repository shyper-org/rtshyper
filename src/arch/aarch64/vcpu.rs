use crate::arch::traits::ContextFrameTrait;
use crate::config::VmType;
use crate::kernel::{Vcpu, Vm};
use spin::Mutex;

pub fn vcpu_arch_init(vm: &Vm, vcpu: &mut Vcpu) {
    let vm_inner = vm.inner.lock();
    if let Some(config) = &vm_inner.config {
        match config.os_type {
            VmType::VmTOs => {
                vcpu.vcpu_ctx
                    .set_argument(config.image.device_tree_load_ipa);
            }
            _ => {
                let arg = &config.memory.region.as_ref().unwrap()[0];
                vcpu.vcpu_ctx.set_argument(arg.ipa_start + arg.length);
            }
        }

        vcpu.vcpu_ctx
            .set_exception_pc(config.image.kernel_entry_point);
    } else {
        panic!("vcpu_arch_init failed!");
    }
}
