use crate::arch::{pt_map_banked_cpu, PAGE_SIZE, PTE_PER_PAGE};
use crate::board::PLATFORM_CPU_NUM_MAX;
use crate::kernel::{Vcpu, VcpuPool, VcpuState, Vm};
use alloc::boxed::Box;
use alloc::vec::Vec;
// use core::ops::{Deref, DerefMut};
use crate::arch::cpu_interrupt_unmask;
use crate::arch::ContextFrame;
use crate::kernel::IpiMessage;
use alloc::sync::Arc;
use spin::Mutex;

pub const CPU_MASTER: usize = 0;
pub const CPU_STACK_SIZE: usize = PAGE_SIZE * 128;

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
    for i in 0..PLATFORM_CPU_NUM_MAX {
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
    pub active_vcpu: Option<Arc<Mutex<Vcpu>>>,
    pub ctx: Option<Arc<Mutex<ContextFrame>>>, // need rebuild
    pub vcpu_pool: Option<Box<VcpuPool>>,

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
            vcpu_pool: None,
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
        unsafe {
            self.ctx = Some(Arc::new(Mutex::new(*ctx)));
        }
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
    vcpu_pool: None,
    current_irq: 0,
    cpu_pt: CpuPt {
        lvl1: [0; PTE_PER_PAGE],
        lvl2: [0; PTE_PER_PAGE],
        lvl3: [0; PTE_PER_PAGE],
    },
    stack: [0; CPU_STACK_SIZE],
};

// set/get CPU
pub fn cpu_id() -> usize {
    unsafe { CPU.id }
}

pub fn cpu_assigned() -> bool {
    unsafe { CPU.assigned }
}

pub fn active_vcpu() -> Arc<Mutex<Vcpu>> {
    unsafe { CPU.active_vcpu.as_ref().unwrap().clone() }
}

pub fn active_vcpu_id() -> usize {
    let active_vcpu_lock = active_vcpu();
    let active_vcpu = active_vcpu_lock.lock();
    active_vcpu.id
}

pub fn active_vm() -> Vm {
    let active_vcpu_lock = active_vcpu();
    let active_vcpu = active_vcpu_lock.lock();
    active_vcpu.vm.as_ref().unwrap().clone()
}

pub fn cpu_vcpu_pool_size() -> usize {
    unsafe {
        let vcpu_pool = CPU.vcpu_pool.as_ref().unwrap();
        vcpu_pool.content.len()
    }
}

pub fn cpu_vcpu_pool() -> &'static Box<VcpuPool> {
    unsafe {
        let vcpu_pool = CPU.vcpu_pool.as_ref().unwrap();
        vcpu_pool
    }
}

pub fn cpu_current_irq() -> usize {
    unsafe { CPU.current_irq }
}

pub fn set_cpu_assign(assigned: bool) {
    unsafe {
        CPU.assigned = assigned;
    }
}

pub fn set_cpu_vcpu_pool(pool: Box<VcpuPool>) {
    unsafe {
        CPU.vcpu_pool = Some(pool);
    }
}

pub fn set_cpu_state(state: CpuState) {
    unsafe {
        CPU.cpu_state = state;
    }
}

pub fn set_active_vcpu(idx: usize) {
    unsafe {
        let vcpu_pool = CPU.vcpu_pool.as_mut().unwrap();
        let vcpu = vcpu_pool.content[idx].vcpu.clone();
        vcpu_pool.active_idx = idx;

        let mut vcpu_inner = vcpu.lock();
        vcpu_inner.state = VcpuState::VcpuAct;
        drop(vcpu_inner);
        CPU.active_vcpu = Some(vcpu);
    }
}

pub fn set_cpu_current_irq(irq: usize) {
    unsafe {
        CPU.current_irq = irq;
    }
}
// end set/get CPU

pub fn cpu_init() {
    let cpu_id = cpu_id();
    // println!("cpu id {}", cpu_id);
    if cpu_id == 0 {
        use crate::board::{platform_power_on_secondary_cores, power_arch_init};
        platform_power_on_secondary_cores();
        power_arch_init();
        cpu_if_init();
    }

    set_cpu_state(CpuState::CpuIdle);
    println!("Core {} init ok", cpu_id);

    use crate::lib::barrier;
    barrier();

    use crate::board::PLAT_DESC;
    if cpu_id == 0 {
        println!("Bring up {} cores", PLAT_DESC.cpu_desc.num);
        println!("Cpu init ok");
    }
}

pub fn cpu_idle() {
    set_cpu_state(CpuState::CpuIdle);
    cpu_interrupt_unmask();
    loop {
        unsafe {
            llvm_asm!("wfi");
        }
    }
}

static CPU_LIST: Mutex<[Cpu; PLATFORM_CPU_NUM_MAX]> = Mutex::new([
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
]);

// static mut CPU_LIST: [Cpu; PLATFORM_CPU_NUM_MAX] = [Cpu::default(); PLATFORM_CPU_NUM_MAX];

#[no_mangle]
#[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self(cpu_id: usize) -> usize {
    let mut cpu_lock = CPU_LIST.lock();
    let mut cpu = &mut (*cpu_lock)[cpu_id];
    // let mut cpu = unsafe { &mut CPU_LIST[cpu_id] };
    (*cpu).id = cpu_id;

    let lvl1_addr = pt_map_banked_cpu(cpu);

    lvl1_addr
}
