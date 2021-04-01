enum CpuState {
    CpuInv = 0,
    CpuIdle = 1,
    CpuRun = 2,
}

pub struct Cpu {
    id: usize,
    assigned: bool,
    cpu_state: CpuState,
    // TODO
}

impl Cpu {
    fn default() -> Cpu {
        Cpu {
            id: 0,
            assigned: false,
            cpu_state: CpuState::CpuInv,
        }
    }
}

#[no_mangle]
static CPU: Cpu = Cpu {
    id: 0,
    assigned: false,
    cpu_state: CpuState::CpuInv,
};

pub fn cpu_init() {}