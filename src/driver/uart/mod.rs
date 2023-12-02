use crate::util::device_ref::DeviceRef;

#[cfg(feature = "ns16550")]
mod ns16550;
#[cfg(feature = "pl011")]
#[allow(dead_code)]
mod pl011;

#[cfg(feature = "ns16550")]
use ns16550::Ns16550Mmio32 as Uart;
#[cfg(feature = "pl011")]
use pl011::Pl011Mmio as Uart;

trait UartOperation {
    fn init(&self);
    fn send(&self, byte: u8);
}

use crate::board::{PlatOperation, Platform};

const UART_BASE: usize = Platform::HYPERVISOR_UART_BASE;

const UART: DeviceRef<Uart> = unsafe { DeviceRef::new(UART_BASE as *const _) };

pub fn putc(byte: u8) {
    if byte == b'\n' {
        putc(b'\r');
    }
    UART.send(byte);
}

pub(super) fn init() {
    UART.init();
}
