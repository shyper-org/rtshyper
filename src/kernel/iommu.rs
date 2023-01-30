use crate::arch::{smmu_add_device, smmu_vm_init};
use crate::kernel::Vm;

pub fn iommu_init() {
    #[cfg(feature = "tx2")]
    {
        crate::arch::smmu_init();
        println!("IOMMU init ok");
    }
    #[cfg(feature = "pi4")]
    {
        println!("Platform not support IOMMU");
    }
}

pub fn iommmu_vm_init(vm: Vm) -> bool {
    #[cfg(feature = "tx2")]
    {
        return smmu_vm_init(vm);
    }
    #[cfg(feature = "pi4")]
    {
        println!("Platform not support IOMMU");
        return false;
    }
}

pub fn iommu_add_device(vm: Vm, stream_id: usize) -> bool {
    #[cfg(feature = "tx2")]
    {
        return smmu_add_device(vm.iommu_ctx_id(), stream_id);
    }
    #[cfg(feature = "pi4")]
    {
        println!("Platform not support IOMMU");
        return false;
    }
}
