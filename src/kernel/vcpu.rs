#[derive(Copy, Clone)]
enum VcpuState {
    VcpuInv = 0,
    VcpuPend = 1,
    VcpuAct = 2,
}

#[derive(Copy, Clone)]
pub struct Vcpu {
    id: usize,
    phys_id: usize,
    state: VcpuState,
    // TODO: VCPU
}

impl Vcpu {
    pub fn default() -> Vcpu {
        Vcpu {
            id: 0,
            phys_id: 0,
            state: VcpuState::VcpuInv,
        }
    }
}
