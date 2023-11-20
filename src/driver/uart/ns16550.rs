use tock_registers::interfaces::*;
use tock_registers::register_bitfields;
use tock_registers::register_structs;
use tock_registers::registers::*;

register_bitfields! {
    u8,

    /// Bitfields of the `RHR_THR_DLL` register.
    pub RHR_THR_DLL [
        /// The Transmit Holding Register.
        ///
        /// It holds the characters to be transmitted by the UART.
        /// In FIFO mode, a write to this FIFO places the data at
        /// the end of the FIFO.
        ///
        /// NOTE: These bits are write-only.
        THR OFFSET(0) NUMBITS(8) [],

        /// The Receive Buffer Register.
        ///
        /// Rx data can be read from here.
        ///
        /// NOTE: These bits are read-only.
        RHR OFFSET(0) NUMBITS(8) [],

        /// The Divisor Latch LSB register.
        ///
        /// The value is represented by the low 8 bits of the 16-bit
        /// Baud Divisor.
        ///
        /// NOTE: These bits are read-only.
        DLL_LSB OFFSET(0) NUMBITS(8) []
    ],

    /// Bitfields of the `IER_DLM` register.
    pub IER_DLM [
        /// Interrupt Enable for End of Received Data.
        IE_EORD OFFSET(5) NUMBITS(1) [],

        /// Interrupt Enable for Rx FIFO timeout.
        IE_RX_TIMEOUT OFFSET(4) NUMBITS(1) [],

        /// Interrupt Enable for Modem Status Interrupt.
        IE_MSI OFFSET(3) NUMBITS(1) [],

        /// Interrupt Enable for Receiver Line Status Interrupt.
        IE_RXS OFFSET(2) NUMBITS(1) [],

        /// Interrupt Enable for Transmitter Holding Register Empty Interrupt.
        IE_THR OFFSET(1) NUMBITS(1) [],

        /// Interrupt Enable for Receive Data Interrupt.
        IE_RHR OFFSET(0) NUMBITS(1) []
    ],

    /// Bitfields of the `ISR_FCR` register.
    pub ISR_FCR [
        /// FIFO Mode Status.
        EN_FIFO OFFSET(6) NUMBITS(2) [
            /// 16450 Mode.
            ///
            /// This mode disables FIFOs.
            Mode16450 = 0,
            /// 16550 Mode.
            ///
            /// This mode enables FIFOs.
            Mode16550 = 1
        ],

        RX_TRIG OFFSET(6) NUMBITS(2) [
            FifoCountGreater1 = 0,
            FifoCountGreater4 = 1,
            FifoCountGreater8 = 2,
            FifoCountGreater16 = 3
        ],

        TX_TRIG OFFSET(4) NUMBITS(2) [
            FifoCountGreater16 = 0,
            FifoCountGreater8 = 1,
            FifoCountGreater4 = 2,
            FifoCountGreater1 = 3
        ],

        /// Whether Encoded Interrupt IDs should be enabled or not.
        IS_PRI2 OFFSET(3) NUMBITS(1) [],

        /// The DMA mode to use.
        DMA OFFSET(3) NUMBITS(1) [
            /// DMA Mode 0.
            ///
            /// This is the default mode.
            DmaMode0 = 0,
            /// DMA Mode 1.
            DmaMode1 = 1
        ],

        /// Whether Encoded Interrupt IDs should be enabled or not.
        IS_PRI1 OFFSET(2) NUMBITS(1) [],

        /// Clears the contents of the transmit FIFO and resets its counter logic to 0.
        TX_CLR OFFSET(2) NUMBITS(1) [
            /// Indicates that the FIFOs were cleared.
            NoClear = 0,
            /// Indicates that the FIFOs should be cleared or are being cleared right now.
            Clear = 1
        ],

        /// Whether Encoded Interrupt IDs should be enabled or not.
        IS_PRI0 OFFSET(1) NUMBITS(1) [],

        /// Clears the contents of the receive FIFO and resets the counter logic to 0.
        RX_CLR OFFSET(1) NUMBITS(1) [
            /// Indicates that the FIFOs were cleared.
            NoClear = 0,
            /// Indicates that the FIFOs should be cleared or are being cleared right now.
            Clear = 1
        ],

        /// Checks whether an interrupt is pending.
        IS_STA OFFSET(0) NUMBITS(1) [
            /// An interrupt is pending.
            IntrPend = 0,
            /// No interrupt is pending.
            NoIntrPend = 1
        ],

        /// Enables the transmit and receive FIFOs.
        ///
        /// This bit should always be enabled.
        FCR_EN_FIFO OFFSET(0) NUMBITS(1) []
    ],

    /// Bitfields of the `LCR` register.
    pub LCR [
        /// Whether the Divisor Latch Access Bit should be enabled.
        ///
        /// NOTE: Set this bit to allow programming of the DLH and DLM Divisors.
        DLAB OFFSET(7) NUMBITS(1) [],

        /// Whether a BREAK condition should be set.
        ///
        /// NOTE: The transmitter sends all zeroes to indicate a BREAK.
        SET_B OFFSET(6) NUMBITS(1) [],

        /// Whether parity should be set (forced) to the value in LCR.
        SET_P OFFSET(5) NUMBITS(1) [],

        /// Whether the even parity format should be used for number representation.
        ///
        /// NOTE: There will always be an even number of 1s in the binary representation.
        EVEN OFFSET(4) NUMBITS(1) [],

        /// Whether parity should be sent or not.
        PAR OFFSET(3) NUMBITS(1) [],

        /// Whether 2 stop bits should be transmitted instead of 1.
        ///
        /// NOTE: The receiver always checks for 1 stop bit.
        STOP OFFSET(2) NUMBITS(1) [],

        /// The Word Length size.
        WD_SIZE OFFSET(0) NUMBITS(2) [
            /// Word length of 5.
            WordLength5 = 0,
            /// Word length of 6.
            WordLength6 = 1,
            /// Word length of 7.
            WordLength7 = 2,
            /// Word length of 8.
            WordLength8 = 3
        ]
    ],

    /// Bitfields of the `MCR` register.
    pub MCR [
        /// Whether the old qualified CTS in TX state machine should be used.
        DEL_QUAL_CTS_EN OFFSET(7) NUMBITS(1) [],

        /// Whether RTS Hardware Flow Control should be enabled.
        RTS_EN OFFSET(6) NUMBITS(1) [],

        /// Whether CTS Hardware Flow Control should be enabled.
        CTS_EN OFFSET(5) NUMBITS(1) [],

        /// Whether internal loop back of Serial Out to In should be enabled.
        LOOPBK OFFSET(4) NUMBITS(1) [],

        /// nOUT2 (Not Used).
        OUT2 OFFSET(3) NUMBITS(1) [],

        /// nOUT1 (Not Used).
        OUT1 OFFSET(2) NUMBITS(1) [],

        /// Whether RTS should be forced to high if RTS hardware flow control wasn't enabled.
        RTS OFFSET(1) NUMBITS(1) [],

        /// Whether DTR should be forced to high or not.
        DTR OFFSET(0) NUMBITS(1) []
    ],

    /// Bitfields of the `LSR` register.
    pub LSR [
        /// Denotes a Receive FIFO error, if set to 1.
        FIFOE OFFSET(7) NUMBITS(1) [],

        /// Denotes a Transmit Shift Register empty status, if set to 1.
        TMTY OFFSET(6) NUMBITS(1) [],

        /// Denotes that the Transmit Holding Register is empty, if set to 1.
        ///
        /// This means that data can be written.
        THRE OFFSET(5) NUMBITS(1) [],

        /// Denotes that a BREAK condition was detected on the line, if set to 1.
        BRK OFFSET(4) NUMBITS(1) [],

        /// Denotes a Framing Error, if set to 1.
        FERR OFFSET(3) NUMBITS(1) [],

        /// Denotes a Parity Error, if set to 1.
        PERR OFFSET(2) NUMBITS(1) [],

        /// Denotes a Receiver Overrun Error, if set to 1.
        OVRF OFFSET(1) NUMBITS(1) [],

        /// Denotes that Receiver Data are in FIFO, if set to 1.
        ///
        /// This means that data are available to read.
        RDR OFFSET(0) NUMBITS(1) []
    ],

    /// Bitfields of the `MSR` register.
    pub MSR [
        /// State of Carrier detect pin.
        CD OFFSET(7) NUMBITS(1) [],

        /// State of Ring Indicator pin.
        RI OFFSET(6) NUMBITS(1) [],

        /// State of Data set ready pin.
        DSR OFFSET(5) NUMBITS(1) [],

        /// State of Clear to send pin.
        CTS OFFSET(4) NUMBITS(1) [],

        /// Change (Delta) in CD state detected.
        DCD OFFSET(3) NUMBITS(1) [],

        /// Change (Delta) in RI state detected.
        DRI OFFSET(2) NUMBITS(1) [],

        /// Change (Delta) in DSR state detected.
        DDSR OFFSET(1) NUMBITS(1) [],

        /// Change (Delta) in CTS detected.
        DCTS OFFSET(0) NUMBITS(1) []
    ],

    /// Bitfields of the `SPR` register.
    pub SPR [
        /// Scratchpad register (not used internally).
        SPR_A OFFSET(0) NUMBITS(8) []
    ],
}

// register_structs! {
//     /// Representation of the UART registers.
//     #[allow(non_snake_case)]
//     pub Ns16550MmioBlock {
//         (0x00 => pub RHR_THR_DLL: ReadWrite<u8, RHR_THR_DLL::Register>),
//         (0x01 => pub IER_DLM: ReadWrite<u8, IER_DLM::Register>),
//         (0x02 => pub ISR_FCR: ReadWrite<u8, ISR_FCR::Register>),
//         (0x03 => pub LCR: ReadWrite<u8, LCR::Register>),
//         (0x04 => pub MCR: ReadWrite<u8, MCR::Register>),
//         (0x05 => pub LSR: ReadOnly<u8, LSR::Register>),
//         (0x06 => pub MSR: ReadWrite<u8, MSR::Register>),
//         (0x07 => pub SPR: ReadWrite<u8, SPR::Register>),
//         (0x08 => @END),
//     }
// }

register_structs! {
    /// Representation of the UART registers.
    #[allow(non_snake_case)]
    pub Ns16550Mmio32Block {
        (0x00 => pub RHR_THR_DLL: ReadWrite<u8, RHR_THR_DLL::Register>),
        (0x01 => _reserved_0),
        (0x04 => pub IER_DLM: ReadWrite<u8, IER_DLM::Register>),
        (0x05 => _reserved_1),
        (0x08 => pub ISR_FCR: ReadWrite<u8, ISR_FCR::Register>),
        (0x09 => _reserved_2),
        (0x0c => pub LCR: ReadWrite<u8, LCR::Register>),
        (0x0d => _reserved_3),
        (0x10 => pub MCR: ReadWrite<u8, MCR::Register>),
        (0x11 => _reserved_4),
        (0x14 => pub LSR: ReadOnly<u8, LSR::Register>),
        (0x15 => _reserved_5),
        (0x18 => pub MSR: ReadWrite<u8, MSR::Register>),
        (0x19 => _reserved_6),
        (0x1c => pub SPR: ReadWrite<u8, SPR::Register>),
        (0x1d => _reserved_7),
        (0x20 => @END),
    }
}

// pub struct Ns16550Mmio<const BASE_ADDR: usize>;

// impl<const BASE_ADDR: usize> core::ops::Deref for Ns16550Mmio<BASE_ADDR> {
//     type Target = Ns16550MmioBlock;

//     fn deref(&self) -> &Self::Target {
//         unsafe { &*(BASE_ADDR as *const _) }
//     }
// }

pub struct Ns16550Mmio32<const BASE_ADDR: usize>;

impl<const BASE_ADDR: usize> core::ops::Deref for Ns16550Mmio32<BASE_ADDR> {
    type Target = Ns16550Mmio32Block;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(BASE_ADDR as *const _) }
    }
}

impl<const BASE_ADDR: usize> super::UartOperation for Ns16550Mmio32<BASE_ADDR> {
    #[inline]
    fn init(&self) {
        self.ISR_FCR.write(ISR_FCR::EN_FIFO::Mode16550);
    }

    #[inline]
    fn send(&self, byte: u8) {
        while !self.LSR.is_set(LSR::THRE) {
            core::hint::spin_loop();
        }
        self.RHR_THR_DLL.set(byte);
    }
}
