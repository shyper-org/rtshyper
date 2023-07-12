use spin::Once;

use crate::arch::{PAGE_SIZE, pt_map_banked_cpu, PTE_PER_PAGE, TlbInvalidate};
use crate::arch::ArchTrait;
use crate::arch::ContextFrame;
use crate::arch::ContextFrameTrait;
use crate::arch::PageTable;
use crate::board::{PLATFORM_CPU_NUM_MAX, PLAT_DESC};
use crate::kernel::{Vcpu, Vm};
use crate::util::timer_list::{TimerTickValue, TimerList};

use super::sched::get_scheduler;
use super::vcpu_array::VcpuArray;

pub const CPU_MASTER: usize = 0;
pub const CPU_STACK_SIZE: usize = PAGE_SIZE * 128;
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
    ctx: Option<usize>,

    pub vcpu_array: VcpuArray,
    // timer
    pub(super) sys_tick: TimerTickValue,
    pub(super) timer_list: Once<TimerList>,

    pub current_irq: usize,
    global_pt: Once<PageTable>,
    pub interrupt_nested: usize,
    pub cpu_pt: CpuPt,
    pub _guard_page: [u8; PAGE_SIZE],
    stack: [u8; CPU_STACK_SIZE],
}

pub const CPU_STACK_OFFSET: usize = offset_of!(Cpu, stack);

impl Cpu {
    const fn default() -> Cpu {
        Cpu {
            id: 0,
            cpu_state: CpuState::Inv,
            active_vcpu: None,
            ctx: None,
            vcpu_array: VcpuArray::new(),
            sys_tick: 0,
            timer_list: Once::new(),
            current_irq: 0,
            interrupt_nested: 0,
            global_pt: Once::new(),
            cpu_pt: CpuPt {
                lvl1: [0; PTE_PER_PAGE],
                lvl2: [0; PTE_PER_PAGE],
                lvl3: [0; PTE_PER_PAGE],
            },
            _guard_page: [0; PAGE_SIZE],
            stack: [0; CPU_STACK_SIZE],
        }
    }

    pub fn current_ctx(&self) -> Option<usize> {
        self.ctx
    }

    pub fn set_ctx(&mut self, ctx: *mut ContextFrame) -> Option<usize> {
        self.ctx.replace(ctx as usize)
    }

    pub fn set_gpr(&self, idx: usize, val: usize) {
        if idx >= CONTEXT_GPR_NUM {
            return;
        }
        if let Some(ctx_addr) = self.current_ctx() {
            if ctx_addr < 0x1000 {
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
        match self.current_ctx() {
            Some(ctx_addr) => {
                if ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).gpr(idx) }
            }
            None => 0,
        }
    }

    pub fn get_elr(&self) -> usize {
        match self.current_ctx() {
            Some(ctx_addr) => {
                if ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).exception_pc() }
            }
            None => 0,
        }
    }

    #[allow(dead_code)]
    pub fn get_spsr(&self) -> usize {
        match self.current_ctx() {
            Some(ctx_addr) => {
                if ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).spsr as usize }
            }
            None => 0,
        }
    }

    pub fn set_elr(&self, val: usize) {
        if let Some(ctx_addr) = self.current_ctx() {
            if ctx_addr < 0x1000 {
                panic!("illegal ctx addr {:x}", ctx_addr);
            }
            let ctx = ctx_addr as *mut ContextFrame;
            unsafe { (*ctx).set_exception_pc(val) }
        }
    }

    pub(super) fn set_active_vcpu(&mut self, active_vcpu: Option<Vcpu>) {
        self.active_vcpu = active_vcpu;
    }

    pub fn assigned(&self) -> bool {
        self.vcpu_array.vcpu_num() != 0
    }

    pub fn pt(&self) -> &PageTable {
        self.global_pt.get().unwrap()
    }

    fn init_pt(&self, directory: usize) {
        info!("cpu {} init_pt() pa {:#x}", self.id, directory);
        let pt = PageTable::from_pa(directory, false);
        self.global_pt.call_once(|| pt);
        crate::arch::Arch::invalid_hypervisor_all();
    }

    pub(super) fn reset_pt(&mut self, directory: usize) {
        // reset global pt without calling the destructor of PageTable
        let prev = core::mem::replace(&mut self.global_pt, Once::new());
        core::mem::forget(prev);
        assert!(self.global_pt.get().is_none());
        self.init_pt(directory);
    }

    pub fn stack_top(&self) -> usize {
        self.stack.as_ptr_range().end as usize
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

pub fn active_vm() -> Option<alloc::sync::Arc<Vm>> {
    match current_cpu().active_vcpu.as_ref() {
        None => None,
        Some(active_vcpu) => active_vcpu.vm(),
    }
}

fn cpu_init_pt() {
    let cpu = current_cpu();
    let directory = crate::arch::Arch::mem_translate(cpu.cpu_pt.lvl1.as_ptr() as usize).unwrap();
    cpu.init_pt(directory);
}

// Todo: add config for base slice
fn cpu_sched_init() {
    let rule = PLAT_DESC.cpu_desc.core_list[current_cpu().id].sched;
    info!("cpu[{}] init {rule:?} Scheduler", current_cpu().id);
    current_cpu().vcpu_array.sched.call_once(|| get_scheduler(rule));
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
    crate::kernel::interrupt_irqchip_init();
    crate::kernel::ipi_init();
    crate::arch::arch_pmu_init();
    cpu_init_pt();
    cpu_sched_init();
    current_cpu().timer_list.call_once(TimerList::new);
    current_cpu().cpu_state = CpuState::Idle;
    let sp = current_cpu().stack.as_ptr() as usize + CPU_STACK_SIZE;
    let size = core::mem::size_of::<ContextFrame>();
    current_cpu().set_ctx((sp - size) as *mut _);
    info!("Core {} init ok", cpu_id);

    crate::util::barrier();
    if cpu_id == 0 {
        info!("Bring up {} cores", PLAT_DESC.cpu_desc.num);
        info!("Cpu init ok");
    }
}

static mut CPU_LIST: [Cpu; PLATFORM_CPU_NUM_MAX] = [const { Cpu::default() }; PLATFORM_CPU_NUM_MAX];

#[no_mangle]
// #[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self(cpu_id: usize) -> usize {
    let cpu = unsafe { &mut CPU_LIST[cpu_id] };
    cpu.id = cpu_id;

    pt_map_banked_cpu(cpu)
}
