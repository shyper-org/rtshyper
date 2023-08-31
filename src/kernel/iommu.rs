use crate::{config::VmEmulatedDeviceConfig, device::EmuDev, kernel::Vm};

use alloc::sync::Arc;

use cfg_if::cfg_if;

#[allow(dead_code)]
pub fn iommu_init() {
    cfg_if! {
        if #[cfg(feature = "smmuv2")] {
            crate::arch::smmu_init();
            info!("IOMMU init ok");
        } else {
            warn!("Platform not support IOMMU");
        }
    }
}

#[allow(unused)]
pub fn iommmu_vm_init(vm: &Vm) -> bool {
    cfg_if! {
        if #[cfg(feature = "smmuv2")] {
            crate::arch::smmu_vm_init(vm)
        } else {
            warn!("Platform not support IOMMU");
            false
        }
    }
}

#[allow(unused)]
pub fn iommu_add_device(vm: &Vm, stream_id: usize) -> bool {
    cfg_if! {
        if #[cfg(feature = "smmuv2")] {
            crate::arch::smmu_add_device(vm.iommu_ctx_id(), stream_id)
        } else {
            warn!("Platform not support IOMMU");
            false
        }
    }
}

#[allow(unused)]
pub fn emu_iommu_init(emu_cfg: &VmEmulatedDeviceConfig) -> Result<Arc<dyn EmuDev>, ()> {
    cfg_if! {
        if #[cfg(feature = "smmuv2")] {
            crate::arch::emu_smmu_init(emu_cfg)
        } else {
            Err(())
        }
    }
}
