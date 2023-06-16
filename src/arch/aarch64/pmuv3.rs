use core::sync::atomic::AtomicUsize;

use alloc::vec::Vec;

use spin::Mutex;
use tock_registers::interfaces::{ReadWriteable, Writeable, Readable};

use crate::{
    kernel::{interrupt_reserve_int, interrupt_cpu_enable, current_cpu},
    board::{Platform, PlatOperation},
};

use super::regs::{PMCR_EL0, PMUSERENR_EL0, PMCCFILTR_EL0};

const MAX_PMU_COUNTER_VALUE: u32 = u32::MAX;

/// See ARM PMU Events
#[allow(dead_code)]
#[derive(Debug)]
#[repr(u32)]
enum PmuEvent {
    MemAccess = 0x13,      // Data memory access
    L2dCache = 0x16,       // Level 2 data cache access
    L2dCacheRefill = 0x17, // Level 2 data cache refill
}

#[derive(Debug)]
struct PmuEventCounter {
    event: PmuEvent,
    index: u32,
    initial_value: u32,
}

impl PmuEventCounter {
    fn enable(&self) {
        // Disable counter
        msr!(PMCNTENCLR_EL0, 1u64 << self.index);

        // Select event counter
        msr!(PMSELR_EL0, self.index, "x");
        // Set event type
        msr!(PMXEVTYPER_EL0, self.event as u32, "x");
        // Set event conuter initial value
        msr!(PMXEVCNTR_EL0, self.initial_value, "x");

        // Enable interrupt for this counter
        msr!(PMINTENSET_EL1, 1u64 << self.index);

        // Enable counter
        msr!(PMCNTENSET_EL0, 1u64 << self.index);
        isb!();
    }

    #[allow(dead_code)]
    fn read_counter(&self) -> u64 {
        // Select event counter
        msr!(PMSELR_EL0, self.index, "x");
        mrs!(PMXEVCNTR_EL0)
    }
}

struct PmuEventCounterList {
    event_counters_num: AtomicUsize,
    event_counters_list: Mutex<Vec<PmuEventCounter>>,
}

static MEM_ACCESS_EVENT: PmuEventCounter = PmuEventCounter {
    event: PmuEvent::MemAccess,
    index: 0,
    initial_value: MAX_PMU_COUNTER_VALUE - 1,
};

pub fn arch_pmu_init() {
    // EL2 Performance Monitors enabled
    let mdcr = mrs!(MDCR_EL2) | (0b1 << 7);
    msr!(MDCR_EL2, mdcr);

    // disables the cycle counter and all PMEVCNTR<x>
    msr!(PMCNTENCLR_EL0, u32::MAX as u64);

    // clear all the overflows
    msr!(PMOVSCLR_EL0, u32::MAX as u64);

    let event_counters_num = PMCR_EL0.read(PMCR_EL0::N);
    debug!("supports {event_counters_num} event counters");

    /// NOTE: ARM deprecates use of PMCR_EL0.LC = 0.
    /// In an AArch64-only implementation, this field is res1.
    /// When this register has an architecturally-defined reset value, if this field is implemented as an RW field, it resets to a value that is architecturally unknown.
    // if PMCR_EL0.matches_all(PMCR_EL0::LC::Enable) {
    //     warn!("PMCR_EL0 enable long cycle by default");
    // }
    // // disable long cycle, overflow when PMCCNTR_EL0[31] from 1 to 0.
    // PMCR_EL0.modify(PMCR_EL0::LC::Disable);
    // assert!(PMCR_EL0.matches_all(PMCR_EL0::LC::Disable));

    // disable Clock divider
    PMCR_EL0.modify(PMCR_EL0::D::Disable);

    // reset Clock counter and event counter
    PMCR_EL0.modify(PMCR_EL0::P::Reset + PMCR_EL0::C::Reset);

    // enable PMU
    PMCR_EL0.modify(PMCR_EL0::E::Enable);

    // enables the cycle counter
    msr!(PMCNTENSET_EL0, 1u64 << 31);

    // only count EL0 and EL1, don't count EL2
    PMCCFILTR_EL0.write(PMCCFILTR_EL0::P::Count + PMCCFILTR_EL0::U::Count + PMCCFILTR_EL0::NSH::DontCount);

    // software can access PMCCNTR_EL0
    // PMUSERENR_EL0.write(PMUSERENR_EL0::EN::Trap + PMUSERENR_EL0::CR::Trap);
    PMUSERENR_EL0.set(0);

    // register the interrupt handler
    let pmu_irq_list = Platform::pmu_irq_list();
    interrupt_reserve_int(pmu_irq_list[current_cpu().id], pmu_irq_handler);
    interrupt_cpu_enable(pmu_irq_list[current_cpu().id], true);

    MEM_ACCESS_EVENT.enable();
}

fn pmu_irq_handler() {
    info!("pmu_irq_handler: on core {}", current_cpu().id);
    msr!(PMOVSCLR_EL0, u32::MAX as u64);
}

#[allow(dead_code)]
pub fn cpu_cycle_count() -> u64 {
    mrs!(PMCCNTR_EL0)
}
