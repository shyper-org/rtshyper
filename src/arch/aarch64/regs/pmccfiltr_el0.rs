use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
};

register_bitfields! {u64,
    pub PMCCFILTR_EL0 [
        /// EL1 modes filtering bit. Controls counting in EL1. If EL3 is implemented, then counting in Non-secure EL1 is further controlled by the NSK bit.
        P OFFSET(31) NUMBITS(1) [
            Count = 0,
            DontCount = 1,
        ],

        /// EL0 filtering bit. Controls counting in EL0. If EL3 is implemented, then counting in Non-secure EL0 is further controlled by the NSU bit.
        U OFFSET(30) NUMBITS(1) [
            Count = 0,
            DontCount = 1,
        ],

        /// Non-secure EL1 (kernel) modes filtering bit. Controls counting in Non-secure EL1. If EL3 is not implemented, this bit is res0.
        /// If the value of this bit is equal to the value of P, cycles in Non-secure EL1 are counted.
        /// Otherwise, cycles in Non-secure EL1 are not counted.
        NSK OFFSET(29) NUMBITS(1) [],

        /// Non-secure User mode filtering bit. Controls counting in Non-secure EL0. If EL3 is not implemented, this bit is res0.
        /// If the value of this bit is equal to the value of U, cycles in Non-secure EL0 are counted.
        /// Otherwise, cycles in Non-secure EL0 are not counted.
        NSU OFFSET(28) NUMBITS(1) [],

        /// Non-secure Hyp modes filtering bit. Controls counting in Non-secure EL2. If EL2 is not implemented, this bit is res0.
        NSH OFFSET(27) NUMBITS(1) [
            DontCount = 0,
            Count = 1,
        ],

        /// Secure EL3 filtering bit. Most applications can ignore this bit and set the value to zero. If EL3 is not implemented, this bit is res0.
        /// If the value of this bit is equal to the value of P, cycles in Secure EL3 are counted.
        /// Otherwise, cycles in Secure EL3 are not counted.
        M OFFSET(26) NUMBITS(1) [],
    ]
}

pub struct Reg;

impl Readable for Reg {
    type T = u64;
    type R = PMCCFILTR_EL0::Register;

    #[inline]
    fn get(&self) -> Self::T {
        let reg;
        mrs!(reg, PMCCFILTR_EL0, "x");
        reg
    }
}

impl Writeable for Reg {
    type T = u64;
    type R = PMCCFILTR_EL0::Register;

    #[inline]
    fn set(&self, value: Self::T) {
        msr!(PMCCFILTR_EL0, value, "x");
    }
}

pub const PMCCFILTR_EL0: Reg = Reg {};
