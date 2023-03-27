use alloc::boxed::Box;
use spin::once::Once;

use crate::arch::{PAGE_SIZE, pt_map_banked_cpu, PTE_PER_PAGE, TlbInvalidate};
use crate::arch::ArchTrait;
use crate::arch::ContextFrame;
use crate::arch::ContextFrameTrait;
// use core::ops::{Deref, DerefMut};
use crate::arch::{cpu_interrupt_unmask, PageTable};
use crate::board::{PLATFORM_CPU_NUM_MAX, SchedRule, PLAT_DESC};
use crate::kernel::{Vcpu, VcpuArray, VcpuState, Vm, Scheduler, SchedulerRR};
use crate::util::trace;

pub const CPU_MASTER: usize = 0;
const CPU_STACK_SIZE: usize = PAGE_SIZE * 128;
const CONTEXT_GPR_NUM: usize = 31;

#[repr(C, align(4096))]
#[derive(Copy, Clone, Debug)]
pub struct CpuPt {
    pub lvl1: [usize; PTE_PER_PAGE],
    pub lvl2: [usize; PTE_PER_PAGE],
    pub lvl3: [usize; PTE_PER_PAGE],
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CpuState {
    Inv = 0,
    Idle = 1,
    Run = 2,
}

#[repr(C, align(4096))]
pub struct Cpu {
    pub id: usize,
    pub cpu_state: CpuState,
    pub active_vcpu: Option<Vcpu>,
    pub ctx: Option<usize>,

    sched: Once<Box<dyn Scheduler>>,
    pub vcpu_array: VcpuArray,
    pub current_irq: usize,
    pub global_pt: Once<PageTable>,
    pub cpu_pt: CpuPt,
    stack: [u8; CPU_STACK_SIZE],
}

// see start.S
const_assert_eq!(offset_of!(Cpu, stack), 0x4000);

#[allow(dead_code)]
impl Cpu {
    const fn default() -> Cpu {
        Cpu {
            id: 0,
            cpu_state: CpuState::Inv,
            active_vcpu: None,
            ctx: None,
            sched: Once::new(),
            vcpu_array: VcpuArray::new(),
            current_irq: 0,
            cpu_pt: CpuPt {
                lvl1: [0; PTE_PER_PAGE],
                lvl2: [0; PTE_PER_PAGE],
                lvl3: [0; PTE_PER_PAGE],
            },
            stack: [0; CPU_STACK_SIZE],
            global_pt: Once::new(),
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
        if let Some(ctx_addr) = self.ctx {
            if trace() && ctx_addr < 0x1000 {
                panic!("illegal ctx addr {:x}", ctx_addr);
            }
            let ctx = ctx_addr as *mut ContextFrame;
            unsafe {
                (*ctx).set_gpr(idx, val);
            }
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

    pub fn get_spsr(&self) -> usize {
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).spsr as usize }
            }
            None => 0,
        }
    }

    pub fn set_elr(&self, val: usize) {
        if let Some(ctx_addr) = self.ctx {
            if trace() && ctx_addr < 0x1000 {
                panic!("illegal ctx addr {:x}", ctx_addr);
            }
            let ctx = ctx_addr as *mut ContextFrame;
            unsafe { (*ctx).set_exception_pc(val) }
        }
    }

    pub fn set_active_vcpu(&mut self, active_vcpu: Option<Vcpu>) {
        self.active_vcpu = active_vcpu;
        if let Some(vcpu) = &self.active_vcpu {
            vcpu.set_state(VcpuState::Active);
        }
    }

    pub fn schedule_to(&mut self, next_vcpu: Vcpu) {
        if let Some(prev_vcpu) = &self.active_vcpu {
            if prev_vcpu.vm_id() != next_vcpu.vm_id() {
                // println!(
                //     "next vm{} vcpu {}, prev vm{} vcpu {}",
                //     next_vcpu.vm_id(),
                //     next_vcpu.id(),
                //     prev_vcpu.vm_id(),
                //     prev_vcpu.id()
                // );
                prev_vcpu.set_state(VcpuState::Pend);
                prev_vcpu.context_vm_store();
            }
        }
        // NOTE: Must set active first and then restore context!!!
        //      because context restore while inject pending interrupt for VM
        //      and will judge if current active vcpu
        self.set_active_vcpu(Some(next_vcpu.clone()));
        next_vcpu.context_vm_restore();
        crate::arch::Arch::install_vm_page_table(next_vcpu.vm_pt_dir(), next_vcpu.vm_id());
    }

    pub fn scheduler(&mut self) -> &mut dyn Scheduler {
        match self.sched.get_mut() {
            Some(scheduler) => scheduler.as_mut(),
            None => panic!("scheduler is None"),
        }
    }

    pub fn assigned(&self) -> bool {
        self.vcpu_array.vcpu_num() != 0
    }

    pub fn pt(&self) -> &PageTable {
        self.global_pt.get().unwrap()
    }
}

#[no_mangle]
#[link_section = ".cpu_private"]
pub static mut CPU: Cpu = Cpu::default();

pub fn current_cpu() -> &'static mut Cpu {
    unsafe { &mut CPU }
}

pub fn active_vcpu_id() -> usize {
    let active_vcpu = current_cpu().active_vcpu.as_ref().unwrap();
    active_vcpu.id()
}

pub fn active_vm_id() -> usize {
    let vm = active_vm().unwrap();
    vm.id()
}

pub fn active_vm() -> Option<Vm> {
    match current_cpu().active_vcpu.as_ref() {
        None => None,
        Some(active_vcpu) => active_vcpu.vm(),
    }
}

pub fn active_vm_ncpu() -> usize {
    match active_vm() {
        Some(vm) => vm.ncpu(),
        None => 0,
    }
}

fn cpu_init_pt() {
    let cpu = current_cpu();
    let pt = PageTable::from_pa(cpu.cpu_pt.lvl1.as_ptr() as usize, false);
    cpu.global_pt.call_once(|| pt);
    crate::arch::Arch::invalid_hypervisor_all();
}

// Todo: add config for base slice
fn cpu_sched_init() {
    match PLAT_DESC.cpu_desc.sched_list[current_cpu().id] {
        SchedRule::RoundRobin => {
            info!("cpu[{}] init Round Robin Scheduler", current_cpu().id);
            current_cpu().sched.call_once(|| Box::new(SchedulerRR::new(1)));
        }
        _ => {
            todo!();
        }
    }
}

pub fn cpu_init() {
    let cpu_id = current_cpu().id;
    if cpu_id == 0 {
        use crate::arch::power_arch_init;
        use crate::board::{PlatOperation, Platform};
        Platform::power_on_secondary_cores();
        power_arch_init();
    }
    // crate::arch::Arch::disable_prefetch();
    cpu_init_pt();
    cpu_sched_init();
    current_cpu().cpu_state = CpuState::Idle;
    let sp = current_cpu().stack.as_ptr() as usize + CPU_STACK_SIZE;
    let size = core::mem::size_of::<ContextFrame>();
    current_cpu().set_ctx((sp - size) as *mut _);
    println!("Core {} init ok", cpu_id);

    crate::util::barrier();
    if cpu_id == 0 {
        println!("Bring up {} cores", PLAT_DESC.cpu_desc.num);
        println!("Cpu init ok");
    }
}

pub fn cpu_idle() -> ! {
    current_cpu().cpu_state = CpuState::Idle;
    cpu_interrupt_unmask();
    loop {
        crate::arch::Arch::wait_for_interrupt();
    }
}

pub static mut CPU_LIST: [Cpu; PLATFORM_CPU_NUM_MAX] = [const { Cpu::default() }; PLATFORM_CPU_NUM_MAX];

pub fn cpu_by_id(cpu_id: usize) -> &'static Cpu {
    unsafe { &mut CPU_LIST[cpu_id] }
}

#[no_mangle]
// #[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self(cpu_id: usize) -> usize {
    let mut cpu = unsafe { &mut CPU_LIST[cpu_id] };
    cpu.id = cpu_id;

    pt_map_banked_cpu(cpu)
}
