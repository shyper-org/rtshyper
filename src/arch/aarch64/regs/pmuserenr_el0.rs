use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
};

/// Performance Monitors User Enable Register
register_bitfields! {u64,
    pub PMUSERENR_EL0 [
        /// Event counter Read. Traps EL0 access to event counters to EL1, or to EL2 when it is implemented and enabled for the current Security state and HCR_EL2.TGE is 1.
        /// In AArch64 state, trapped accesses are reported using EC syndrome value 0x18.
        /// In AArch32 state, trapped accesses are reported using EC syndrome value 0x03.
        /// 0b0 EL0 using AArch64: EL0 reads of the PMXEVCNTR_EL0 and PMEVCNTR<n>_EL0, and EL0 read/write accesses to the PMSELR_EL0, are trapped if PMUSERENR_EL0.EN is also 0.
        ///     EL0 using AArch32: EL0 reads of the PMXEVCNTR and PMEVCNTR<n>, and EL0 read/write accesses to the PMSELR, are trapped if PMUSERENR_EL0.EN is also 0.
        /// 0b1 Overrides PMUSERENR_EL0.EN and enables:
        ///     RO access to PMXEVCNTR_EL0 and PMEVCNTR<n>_EL0 at EL0.
        ///     RW access to PMSELR_EL0 at EL0.
        ///     RW access to PMSELR at EL0.
        ER OFFSET(3) NUMBITS(1) [
            Trap = 0,
            EnableAccess = 1,
        ],

        CR OFFSET(2) NUMBITS(1) [
            Trap = 0,
            EnableAccess = 1,
        ],

        SW OFFSET(1) NUMBITS(1) [
            Trap = 0,
            EnableAccess = 1,
        ],

        EN OFFSET(0) NUMBITS(1) [
            Trap = 0,
            EnableAccess = 1,
        ],
    ]
}

pub struct Reg;

impl Readable for Reg {
    type T = u64;
    type R = PMUSERENR_EL0::Register;

    #[inline]
    fn get(&self) -> Self::T {
        let reg;
        mrs!(reg, PMUSERENR_EL0);
        reg
    }
}

impl Writeable for Reg {
    type T = u64;
    type R = PMUSERENR_EL0::Register;

    #[inline]
    fn set(&self, value: Self::T) {
        msr!(PMUSERENR_EL0, value);
    }
}

pub const PMUSERENR_EL0: Reg = Reg {};
