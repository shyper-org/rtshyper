use alloc::boxed::Box;
use alloc::vec::Vec;
use core::arch::asm;

use spin::Mutex;

use crate::arch::{PAGE_SIZE, pt_map_banked_cpu, PTE_PER_PAGE};
use crate::arch::ContextFrame;
use crate::arch::ContextFrameTrait;
// use core::ops::{Deref, DerefMut};
use crate::arch::cpu_interrupt_unmask;
use crate::board::PLATFORM_CPU_NUM_MAX;
use crate::kernel::{SchedType, Vcpu, VcpuPool, VcpuState, Vm};
use crate::kernel::IpiMessage;
use crate::lib::trace;

pub const CPU_MASTER: usize = 0;
pub const CPU_STACK_SIZE: usize = PAGE_SIZE * 128;
pub const CONTEXT_GPR_NUM: usize = 31;

#[repr(C)]
#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct CpuPt {
    pub lvl1: [usize; PTE_PER_PAGE],
    pub lvl2: [usize; PTE_PER_PAGE],
    pub lvl3: [usize; PTE_PER_PAGE],
}

#[derive(Copy, Clone)]
pub enum CpuState {
    CpuInv = 0,
    CpuIdle = 1,
    CpuRun = 2,
}

pub struct CpuIf {
    pub msg_queue: Vec<IpiMessage>,
}

impl CpuIf {
    pub fn default() -> CpuIf {
        CpuIf {
            msg_queue: Vec::new(),
        }
    }

    pub fn push(&mut self, ipi_msg: IpiMessage) {
        self.msg_queue.push(ipi_msg);
    }

    pub fn pop(&mut self) -> Option<IpiMessage> {
        self.msg_queue.pop()
    }
}

pub static CPU_IF_LIST: Mutex<Vec<CpuIf>> = Mutex::new(Vec::new());

fn cpu_if_init() {
    let mut cpu_if_list = CPU_IF_LIST.lock();
    for _ in 0..PLATFORM_CPU_NUM_MAX {
        cpu_if_list.push(CpuIf::default());
    }
}

// struct CpuSelf {}

// impl Deref<Cpu> for CpuSelf {}

#[repr(C)]
#[repr(align(4096))]
// #[derive(Clone)]
pub struct Cpu {
    pub id: usize,
    pub assigned: bool,
    pub cpu_state: CpuState,
    pub active_vcpu: Option<Vcpu>,
    pub ctx: Option<usize>,

    pub sched: SchedType,
    // pub vcpu_pool: Option<Box<VcpuPool>>,

    pub current_irq: usize,
    pub cpu_pt: CpuPt,
    pub stack: [u8; CPU_STACK_SIZE],
}

impl Cpu {
    const fn default() -> Cpu {
        Cpu {
            id: 0,
            assigned: false,
            cpu_state: CpuState::CpuInv,
            active_vcpu: None,
            ctx: None,
            sched: SchedType::None,
            current_irq: 0,
            cpu_pt: CpuPt {
                lvl1: [0; PTE_PER_PAGE],
                lvl2: [0; PTE_PER_PAGE],
                lvl3: [0; PTE_PER_PAGE],
            },
            stack: [0; CPU_STACK_SIZE],
        }
    }

    pub fn set_ctx(&mut self, ctx: *mut ContextFrame) {
        self.ctx = Some(ctx as usize);
    }

    pub fn clear_ctx(&mut self) {
        self.ctx = None;
    }

    pub fn set_gpr(&self, idx: usize, val: usize) {
        if idx >= CONTEXT_GPR_NUM {
            return;
        }
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe {
                    (*ctx).set_gpr(idx, val);
                }
            }
            None => {}
        }
    }

    pub fn get_gpr(&self, idx: usize) -> usize {
        if idx >= CONTEXT_GPR_NUM {
            return 0;
        }
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).gpr(idx) }
            }
            None => 0,
        }
    }

    pub fn get_elr(&self) -> usize {
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).exception_pc() }
            }
            None => 0,
        }
    }

    pub fn set_elr(&self, val: usize) {
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).set_exception_pc(val) }
            }
            None => {}
        }
    }

    pub fn vcpu_pool(&self) -> VcpuPool {
        match &self.sched {
            SchedType::SchedRR(rr) => {
                rr.pool.clone()
            }
            SchedType::None => {
                panic!("cpu[{}] has no vcpu_pool", self.id);
            }
        }
    }

    pub fn set_active_vcpu(&mut self, vcpu: Vcpu) {
        vcpu.set_state(VcpuState::VcpuAct);
        self.active_vcpu = Some(vcpu);
    }
}

#[no_mangle]
#[link_section = ".cpu_private"]
pub static mut CPU: Cpu = Cpu {
    id: 0,
    assigned: false,
    cpu_state: CpuState::CpuInv,
    active_vcpu: None,
    ctx: None,
    sched: SchedType::None,
    current_irq: 0,
    cpu_pt: CpuPt {
        lvl1: [0; PTE_PER_PAGE],
        lvl2: [0; PTE_PER_PAGE],
        lvl3: [0; PTE_PER_PAGE],
    },
    stack: [0; CPU_STACK_SIZE],
};

pub fn current_cpu() -> &'static mut Cpu {
    unsafe {
        &mut CPU
    }
}

pub fn active_vcpu_id() -> usize {
    let active_vcpu = current_cpu().active_vcpu.clone().unwrap();
    active_vcpu.id()
}

pub fn active_vm_id() -> usize {
    let vm = active_vm().unwrap();
    vm.id()
}

pub fn active_vm() -> Option<Vm> {
    match current_cpu().active_vcpu.clone() {
        None => {
            return None;
        }
        Some(active_vcpu) => {
            return active_vcpu.vm();
        }
    }
}

pub fn active_vm_ncpu() -> usize {
    match active_vm() {
        Some(vm) => vm.ncpu(),
        None => 0,
    }
}

pub fn cpu_init() {
    let cpu_id = current_cpu().id;
    if cpu_id == 0 {
        use crate::arch::power_arch_init;
        use crate::board::platform_power_on_secondary_cores;
        platform_power_on_secondary_cores();
        power_arch_init();
        cpu_if_init();
    }

    let state = CpuState::CpuIdle;
    current_cpu().cpu_state = state;
    println!("Core {} init ok", cpu_id);

    crate::lib::barrier();
    // println!("after barrier cpu init");
    use crate::board::PLAT_DESC;
    if cpu_id == 0 {
        println!("Bring up {} cores", PLAT_DESC.cpu_desc.num);
        println!("Cpu init ok");
    }
}

pub fn cpu_idle() {
    let state = CpuState::CpuIdle;
    current_cpu().cpu_state = state;
    cpu_interrupt_unmask();
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

static mut CPU_LIST: [Cpu; PLATFORM_CPU_NUM_MAX] = [
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
];
// static CPU_LIST: Mutex<[Cpu; PLATFORM_CPU_NUM_MAX]> = Mutex::new([
//     Cpu::default(),
//     Cpu::default(),
//     Cpu::default(),
//     Cpu::default(),
//     Cpu::default(),
//     Cpu::default(),
//     Cpu::default(),
//     Cpu::default(),
// ]);

#[no_mangle]
// #[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self(cpu_id: usize) -> usize {
    // let mut cpu_lock = CPU_LIST.lock();
    // let mut cpu = &mut (*cpu_lock)[cpu_id];
    let mut cpu = unsafe { &mut CPU_LIST[cpu_id] };
    (*cpu).id = cpu_id;

    let lvl1_addr = pt_map_banked_cpu(cpu);

    lvl1_addr
}
