#[cfg(feature = "ns16550")]
mod ns16550;
#[cfg(feature = "pl011")]
#[allow(dead_code)]
mod pl011;

trait UartOperation {
    fn init(&self);
    fn send(&self, byte: u8);
}

use crate::board::{Platform, PlatOperation};

const UART_BASE: usize = Platform::HYPERVISOR_UART_BASE;

#[cfg(feature = "tx2")]
const UART: ns16550::Ns16550Mmio32<UART_BASE> = ns16550::Ns16550Mmio32;
#[cfg(any(feature = "pi4", feature = "qemu"))]
const UART: pl011::Pl011Mmio<UART_BASE> = pl011::Pl011Mmio;

pub fn putc(byte: u8) {
    if byte == b'\n' {
        putc(b'\r');
    }
    UART.send(byte);
}

pub(super) fn init() {
    UART.init();
}
