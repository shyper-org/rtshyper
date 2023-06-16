use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
};

/// Performance Monitors Control Register
register_bitfields! {u64,
    pub PMCR_EL0 [
        /// Implementer code
        ///
        /// 0x41 ARM.
        ///
        /// This is a read-only field.
        IMP OFFSET(24) NUMBITS(8) [],

        /// Identification code:
        /// 0x01 Cortex-A57 processor.
        // This is a read-only field.
        IDCODE OFFSET(16) NUMBITS(8) [],

        /// Number of event counters.
        /// In Non-secure modes other than Hyp mode, this field reads the value of HDCR.HPMN.
        /// In Secure state and Hyp mode, this field returns 0x6 that indicates the number of counters implemented.
        /// This is a read-only field.
        N OFFSET(11) NUMBITS(5) [],

        /// Long cycle count enable. Selects which PMCCNTR_EL0 bit generates an oveflow recorded in PMOVSR[31]:
        /// 0   Overflow on increment that changes PMCCNTR_EL0[31] from 1 to 0.
        /// 1   Overflow on increment that changes PMCCNTR_EL0[63] from 1 to 0.
        LC OFFSET(6) NUMBITS(1) [
            Disable = 0,
            Enable = 1,
        ],

        /// Disable cycle counter, PMCCNTR_EL0 when event counting is prohibited:
        /// 0 Cycle counter operates regardless of the non-invasive debug authentication settings.
        /// 1 Cycle counter is disabled if non-invasive debug is not permitted and enabled.
        /// This bit is read/write.
        DP OFFSET(5) NUMBITS(1) [
            Enable = 0,
            Disable = 1,
        ],

        /// Export enable. This bit permits events to be exported to another debug device, such as a trace macrocell, over an event bus:
        /// 0 Export of events is disabled.
        /// 1 Export of events is enabled.
        /// This bit is read/write and does not affect the generation of Performance Monitors interrupts, that can be
        /// implemented as a signal exported from the processor to an interrupt controller.
        /// This bit does not affect the generation of Performance Monitors overflow interrupt requests or signaling to a cross-trigger
        /// interface (CTI) that can be implemented as signals exported from the PE.
        X OFFSET(4) NUMBITS(1) [
            Disable = 0,
            Enable = 1,
        ],


        /// Clock divider:
        /// 0 When enabled, PMCCNTR_EL0 counts every clock cycle.
        /// 1 When enabled, PMCCNTR_EL0 counts every 64 clock cycles.
        /// This bit is read/write.
        D OFFSET(3) NUMBITS(1) [
            Disable = 0,
            Enable = 1,
        ],

        /// Clock counter reset:
        /// 0 No action.
        /// 1 Reset PMCCNTR_EL0 to 0.
        C OFFSET(2) NUMBITS(1) [
            No = 0,
            Reset = 1,
        ],

        /// Event counter reset:
        /// 0 No action.
        /// 1 Reset all event counters, not including PMCCNTR_EL0, to 0.
        P OFFSET(1) NUMBITS(1) [
            No = 0,
            Reset = 1,
        ],

        /// Enable bit. This bit does not disable or enable, counting by event counters reserved for Hyp mode by
        /// HDCR.HPMN. It also does not suppress the generation of performance monitor overflow interrupt requests by
        /// those counters:
        /// 0 All counters, including PMCCNTR_EL0, are disabled. This is the reset value.
        /// 1 All counters are enabled.
        /// This bit is read/write.
        E OFFSET(0) NUMBITS(1) [
            Disable = 0,
            Enable = 1,
        ]
    ]
}

pub struct Reg;

impl Readable for Reg {
    type T = u64;
    type R = PMCR_EL0::Register;

    #[inline]
    fn get(&self) -> Self::T {
        let reg;
        mrs!(reg, PMCR_EL0, "x");
        reg
    }
}

impl Writeable for Reg {
    type T = u64;
    type R = PMCR_EL0::Register;

    #[inline]
    fn set(&self, value: Self::T) {
        msr!(PMCR_EL0, value, "x");
    }
}

pub const PMCR_EL0: Reg = Reg {};
