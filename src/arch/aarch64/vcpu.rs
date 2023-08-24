use cortex_a::registers::*;

use crate::arch::traits::ContextFrameTrait;
use crate::config::VmConfigEntry;
use crate::kernel::Vcpu;
use crate::kernel::VmType;

pub fn vcpu_arch_init(config: &VmConfigEntry, vcpu: &Vcpu) {
    let mut vcpu_inner = vcpu.inner_mut.lock();
    match config.os_type {
        VmType::VmTOs => {
            vcpu_inner.vcpu_ctx.set_argument(config.device_tree_load_ipa());
        }
        _ => {
            let arg = &config.memory_region()[0];
            vcpu_inner.vcpu_ctx.set_argument(arg.ipa_start + arg.length);
        }
    }

    vcpu_inner.vcpu_ctx.set_exception_pc(config.kernel_entry_point());
    vcpu_inner.vcpu_ctx.spsr =
        (SPSR_EL2::M::EL1h + SPSR_EL2::I::Masked + SPSR_EL2::F::Masked + SPSR_EL2::A::Masked + SPSR_EL2::D::Masked)
            .value;
}
