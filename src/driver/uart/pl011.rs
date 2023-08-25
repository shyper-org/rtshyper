use tock_registers::interfaces::*;
use tock_registers::register_structs;
use tock_registers::registers::*;

const UART_FR_RXFF: u32 = 1 << 4;
const UART_FR_TXFF: u32 = 1 << 5;

register_structs! {
  #[allow(non_snake_case)]
  pub Pl011MmioBlock {
    (0x000 => pub Data: ReadWrite<u32>),
    (0x004 => pub RecvStatusErrClr: ReadWrite<u32>),
    (0x008 => _reserved_1),
    (0x018 => pub Flag: ReadOnly<u32>),
    (0x01c => _reserved_2),
    (0x020 => pub IrDALowPower: ReadWrite<u32>),
    (0x024 => pub IntBaudRate: ReadWrite<u32>),
    (0x028 => pub FracBaudRate: ReadWrite<u32>),
    (0x02c => pub LineControl: ReadWrite<u32>),
    (0x030 => pub Control: ReadWrite<u32>),
    (0x034 => pub IntFIFOLevel: ReadWrite<u32>),
    (0x038 => pub IntMaskSetClr: ReadWrite<u32>),
    (0x03c => pub RawIntStatus: ReadOnly<u32>),
    (0x040 => pub MaskedIntStatus: ReadOnly<u32>),
    (0x044 => pub IntClear: WriteOnly<u32>),
    (0x048 => pub DmaControl: ReadWrite<u32>),
    (0x04c => _reserved_3),
    (0x1000 => @END),
  }
}

pub struct Pl011Mmio<const BASE_ADDR: usize>;

impl<const BASE_ADDR: usize> core::ops::Deref for Pl011Mmio<BASE_ADDR> {
    type Target = Pl011MmioBlock;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(BASE_ADDR as *const _) }
    }
}

impl<const BASE_ADDR: usize> super::UartOperation for Pl011Mmio<BASE_ADDR> {
    #[inline]
    fn init(&self) {}

    #[inline]
    fn send(&self, byte: u8) {
        while self.Flag.get() & UART_FR_TXFF != 0 {
            core::hint::spin_loop();
        }
        self.Data.set(byte as u32);
    }
}
