use crate::arch::traits::ContextFrameTrait;
use crate::kernel::{Vcpu, Vm};
use crate::kernel::VmType;

pub fn vcpu_arch_init(vm: Vm, vcpu: Vcpu) {
    let vm_inner = vm.inner.lock();
    if let Some(config) = &vm_inner.config {
        let mut vcpu_inner = vcpu.inner.lock();
        match config.os_type {
            VmType::VmTOs => {
                vcpu_inner
                    .vcpu_ctx
                    .set_argument(config.image.device_tree_load_ipa);
            }
            _ => {
                let arg = &config.memory.region[0];
                vcpu_inner.vcpu_ctx.set_argument(arg.ipa_start + arg.length);
            }
        }

        vcpu_inner
            .vcpu_ctx
            .set_exception_pc(config.image.kernel_entry_point);
    } else {
        panic!("vcpu_arch_init failed!");
    }
}
