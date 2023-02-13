use crate::arch::{smmu_add_device, smmu_vm_init};
use crate::kernel::Vm;

pub fn iommu_init() {
    if cfg!(feature = "tx2") {
        crate::arch::smmu_init();
        println!("IOMMU init ok");
    } else {
        println!("Platform not support IOMMU");
    }
}

pub fn iommmu_vm_init(vm: &Vm) -> bool {
    if cfg!(feature = "tx2") {
        smmu_vm_init(vm)
    } else {
        println!("Platform not support IOMMU");
        false
    }
}

pub fn iommu_add_device(vm: &Vm, stream_id: usize) -> bool {
    if cfg!(feature = "tx2") {
        smmu_add_device(vm.iommu_ctx_id(), stream_id)
    } else {
        println!("Platform not support IOMMU");
        false
    }
}
