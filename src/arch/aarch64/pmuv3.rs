use core::sync::atomic::{AtomicUsize, Ordering};

use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

#[cfg(feature = "memory-reservation")]
use crate::kernel::{current_cpu, Vcpu, VcpuState, WeakVcpu};

use super::regs::{PMCCFILTR_EL0, PMCR_EL0, PMUSERENR_EL0};

#[cfg(feature = "memory-reservation")]
const MAX_PMU_COUNTER_VALUE: u32 = u32::MAX;

/// See ARM PMU Events
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
enum PmuEvent {
    MemAccess = 0x13,      // Data memory access
    L2dCache = 0x16,       // Level 2 data cache access
    L2dCacheRefill = 0x17, // Level 2 data cache refill
}

#[cfg(feature = "memory-reservation")]
#[derive(Clone, Debug)]
struct PmuEventCounter {
    event: PmuEvent,
    index: u32,
}

#[cfg(feature = "memory-reservation")]
impl PmuEventCounter {
    fn enable(&self, initial_value: u32) {
        // Disable counter
        msr!(PMCNTENCLR_EL0, 1u64 << self.index);

        // Clear overflows
        msr!(PMOVSCLR_EL0, 1u64 << self.index);

        self.set_counter(initial_value);

        // Enable interrupt for this counter
        msr!(PMINTENSET_EL1, 1u64 << self.index);

        // Enable counter
        msr!(PMCNTENSET_EL0, 1u64 << self.index);
        isb!();
    }

    fn disable(&self) {
        // Disable counter
        msr!(PMCNTENCLR_EL0, 1u64 << self.index);

        // Disbale interrupt for this counter
        msr!(PMINTENCLR_EL1, 1u64 << self.index);

        // Reset the event conuter to 0
        self.set_counter(0);
    }

    fn set_counter(&self, initial_value: u32) {
        // Select event counter
        msr!(PMSELR_EL0, self.index, "x");
        // Set event type
        msr!(PMXEVTYPER_EL0, self.event as u32, "x");
        // Set event conuter initial value
        msr!(PMXEVCNTR_EL0, initial_value, "x");
    }

    fn read_counter(&self) -> u32 {
        // Select event counter
        msr!(PMSELR_EL0, self.index, "x");
        mrs!(PMXEVCNTR_EL0) as u32
    }
}

struct PmuEventCounterList {
    event_counters_num: AtomicUsize,
    // event_counters_list: Mutex<Vec<PmuEventCounter>>,
}

#[cfg(feature = "memory-reservation")]
static MEM_ACCESS_EVENT: PmuEventCounter = PmuEventCounter {
    event: PmuEvent::MemAccess,
    index: 0,
};

static GLOBAL_PMU: PmuEventCounterList = PmuEventCounterList {
    event_counters_num: AtomicUsize::new(0),
    // event_counters_list: Mutex::new(Vec::new()),
};

pub fn arch_pmu_init() {
    // EL2 Performance Monitors enabled
    let mdcr = mrs!(MDCR_EL2) | (0b1 << 7);
    msr!(MDCR_EL2, mdcr);

    // disables the cycle counter and all PMEVCNTR<x>
    msr!(PMCNTENCLR_EL0, u32::MAX as u64);

    // clear all the overflows
    msr!(PMOVSCLR_EL0, u32::MAX as u64);

    let event_counters_num = PMCR_EL0.read(PMCR_EL0::N) as usize;
    debug!("PMU supports {event_counters_num} event counters");
    GLOBAL_PMU
        .event_counters_num
        .store(event_counters_num, Ordering::Relaxed);

    /// NOTE: ARM deprecates use of PMCR_EL0.LC = 0.
    /// In an AArch64-only implementation, this field is res1.
    /// When this register has an architecturally-defined reset value, if this field is
    /// implemented as an RW field, it resets to a value that is architecturally unknown.
    // if PMCR_EL0.matches_all(PMCR_EL0::LC::Enable) {
    //     warn!("PMCR_EL0 enable long cycle by default");
    // }

    // enable Long cycle count, disable Clock divider
    PMCR_EL0.modify(PMCR_EL0::LC::Enable + PMCR_EL0::D::Disable);

    // reset Clock counter and event counter
    PMCR_EL0.modify(PMCR_EL0::P::Reset + PMCR_EL0::C::Reset);

    // enable PMU
    PMCR_EL0.modify(PMCR_EL0::E::Enable);

    // enables the cycle counter
    msr!(PMCNTENSET_EL0, 1u64 << 31);

    // only count EL0 and EL1, don't count EL2
    PMCCFILTR_EL0.write(PMCCFILTR_EL0::P::Count + PMCCFILTR_EL0::U::Count + PMCCFILTR_EL0::NSH::DontCount);

    // software can access PMCCNTR_EL0
    PMUSERENR_EL0.write(PMUSERENR_EL0::EN::Trap + PMUSERENR_EL0::CR::Trap);

    #[cfg(feature = "memory-reservation")]
    {
        use crate::{
            board::{PlatOperation, Platform},
            kernel::{interrupt_cpu_enable, interrupt_reserve_int},
        };
        // register the interrupt handler
        let pmu_irq_list = Platform::pmu_irq_list();
        interrupt_reserve_int(pmu_irq_list[current_cpu().id], pmu_irq_handler);
        interrupt_cpu_enable(pmu_irq_list[current_cpu().id], true);
    }
}

#[cfg(feature = "memory-reservation")]
fn pmu_irq_handler() {
    // Read the overflow register
    let pmovsr = mrs!(PMOVSCLR_EL0);
    trace!("pmu_irq_handler: on core {} pmovsr {pmovsr:#x}", current_cpu().id);

    if pmovsr & (1 << MEM_ACCESS_EVENT.index) != 0 {
        // Clear all the overflows
        msr!(PMOVSCLR_EL0, u32::MAX as u64);
        pmu_mem_access_handler();
    }
}

#[cfg(feature = "memory-reservation")]
fn pmu_mem_access_handler() {
    let vcpu = current_cpu().active_vcpu.as_ref().unwrap();
    // TODO: try apply additional budget here
    trace!(
        "pmu_mem_access_handler: core {} vcpu {} counter {:#x}",
        current_cpu().id,
        vcpu.id(),
        MEM_ACCESS_EVENT.read_counter()
    );
    vcpu.bw_info().reset_remaining_budget();
    #[cfg(feature = "dynamic-budget")]
    if vcpu.bw_info().budget_try_rescue() {
        vcpu_start_pmu(vcpu);
    } else {
        current_cpu().vcpu_array.block_current();
    }
    #[cfg(not(feature = "dynamic-budget"))]
    current_cpu().vcpu_array.block_current();
}

#[allow(dead_code)]
pub fn cpu_cycle_count() -> u64 {
    mrs!(PMCCNTR_EL0)
}

#[cfg(feature = "memory-reservation")]
pub fn vcpu_start_pmu(vcpu: &Vcpu) {
    let remaining_budget = vcpu.bw_info().remaining_budget();
    trace!(
        "vcpu_start_pmu: core {} VM {} Vcpu {} memory budget {:#x}",
        current_cpu().id,
        vcpu.vm().unwrap().id(),
        vcpu.id(),
        remaining_budget
    );
    MEM_ACCESS_EVENT.enable(MAX_PMU_COUNTER_VALUE - remaining_budget);
}

#[cfg(feature = "memory-reservation")]
pub fn vcpu_stop_pmu(vcpu: &Vcpu) {
    // read_counter must before disable event (disabling will reset the counter to 0)
    let current_memory_access_count = MEM_ACCESS_EVENT.read_counter();
    MEM_ACCESS_EVENT.disable();

    let remaining_budget = if current_memory_access_count != 0 {
        MAX_PMU_COUNTER_VALUE - current_memory_access_count
    } else {
        0
    };
    trace!(
        "vcpu_stop_pmu: core {} VM {} Vcpu {} remaining memory budget {:#x}",
        current_cpu().id,
        vcpu.vm().unwrap().id(),
        vcpu.id(),
        remaining_budget
    );
    vcpu.bw_info().update_remaining_budget(remaining_budget);
}

#[cfg(feature = "memory-reservation")]
pub struct PmuTimerEvent(pub WeakVcpu);

#[cfg(feature = "memory-reservation")]
impl crate::util::timer_list::TimerEvent for PmuTimerEvent {
    fn callback(self: alloc::sync::Arc<Self>, now: crate::util::timer_list::TimerValue) {
        if let Some(vcpu) = self.0.upgrade() {
            let period = vcpu.bw_info().period();
            trace!(
                "vm {} vcpu {} supply_budget at {}",
                vcpu.vm_id(),
                vcpu.id(),
                now.as_millis()
            );
            match vcpu.state() {
                VcpuState::Running => {
                    vcpu_stop_pmu(&vcpu);
                    #[cfg(feature = "trace-memory")]
                    {
                        let prev_bw =
                            crate::util::budget2bandwidth(vcpu.bw_info().used_budget(), vcpu.bw_info().period());
                        info!(
                            "VM {} vcpu {} used memory bandwidth {}",
                            vcpu.vm_id(),
                            vcpu.id(),
                            prev_bw
                        );
                    }
                    vcpu.bw_info().supply_budget();
                    vcpu_start_pmu(&vcpu);
                }
                VcpuState::Blocked => {
                    #[cfg(feature = "trace-memory")]
                    {
                        let prev_bw =
                            crate::util::budget2bandwidth(vcpu.bw_info().used_budget(), vcpu.bw_info().period());
                        info!(
                            "VM {} vcpu {} used memory bandwidth {}",
                            vcpu.vm_id(),
                            vcpu.id(),
                            prev_bw
                        );
                    }
                    vcpu.bw_info().supply_budget();
                    current_cpu().vcpu_array.wakeup_vcpu(&vcpu);
                }
                _ => {}
            }
            crate::kernel::timer::start_timer_event(period, self);
        }
    }
}
