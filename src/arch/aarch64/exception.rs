use core::arch::global_asm;

// use alloc::collections::BinaryHeap;
// use spin::{Mutex, Lazy};
use cortex_a::registers::ESR_EL2;
use tock_registers::interfaces::*;

use crate::arch::{ContextFrame, ContextFrameTrait, InterruptController};
use crate::kernel::interrupt_handler;
use crate::kernel::{active_vm, current_cpu};

use super::sync::{data_abort_handler, hvc_handler, smc_handler, sysreg_handler};
use super::{interrupt_arch_deactive_irq, IntCtrl};

global_asm!(include_str!("exception.S"));

#[inline(always)]
pub fn exception_esr() -> usize {
    ESR_EL2.get() as usize
}

#[inline(always)]
fn exception_class() -> usize {
    ESR_EL2.read(ESR_EL2::EC) as usize
}

#[inline(always)]
fn exception_far() -> usize {
    cortex_a::registers::FAR_EL2.get() as usize
}

#[inline(always)]
fn exception_hpfar() -> usize {
    let hpfar: u64;
    mrs!(hpfar, HPFAR_EL2);
    hpfar as usize
}

#[allow(non_upper_case_globals)]
const ESR_ELx_S1PTW_SHIFT: usize = 7;
#[allow(non_upper_case_globals)]
const ESR_ELx_S1PTW: usize = 1 << ESR_ELx_S1PTW_SHIFT;

fn translate_far_to_hpfar(far: usize) -> Result<usize, ()> {
    /*
     * We have
     *	PAR[PA_Shift - 1 : 12] = PA[PA_Shift - 1 : 12]
     *	HPFAR[PA_Shift - 9 : 4]  = FIPA[PA_Shift - 1 : 12]
     */
    // #define PAR_TO_HPFAR(par) (((par) & GENMASK_ULL(PHYS_MASK_SHIFT - 1, 12)) >> 8)
    fn par_to_far(par: u64) -> u64 {
        let mask = ((1 << (52 - 12)) - 1) << 12;
        (par & mask) >> 8
    }

    use cortex_a::registers::PAR_EL1;

    let par = PAR_EL1.get();
    arm_at!("s1e1r", far);
    let tmp = PAR_EL1.get();
    PAR_EL1.set(par);
    if (tmp & PAR_EL1::F::TranslationAborted.value) != 0 {
        Err(())
    } else {
        Ok(par_to_far(tmp) as usize)
    }
}

// addr be ipa
#[inline(always)]
pub fn exception_fault_addr() -> usize {
    let far = exception_far();
    let hpfar = if (exception_iss() & ESR_ELx_S1PTW) == 0 && exception_data_abort_is_permission_fault() {
        translate_far_to_hpfar(far).unwrap_or_else(|_| {
            error!("error happen in translate_far_to_hpfar");
            0
        })
    } else {
        exception_hpfar()
    };
    (far & 0xfff) | (hpfar << 8)
}

/// \return 1 means 32-bit instruction, 0 means 16-bit instruction
#[inline(always)]
fn exception_instruction_length() -> usize {
    ESR_EL2.read(ESR_EL2::IL) as usize
}

#[inline(always)]
pub fn exception_next_instruction_step() -> usize {
    2 + 2 * exception_instruction_length()
}

#[inline(always)]
pub fn exception_iss() -> usize {
    ESR_EL2.read(ESR_EL2::ISS) as usize
}

#[inline(always)]
pub fn exception_data_abort_handleable() -> bool {
    (!(exception_iss() & (1 << 10)) | (exception_iss() & (1 << 24))) != 0
}

#[inline(always)]
pub fn exception_data_abort_is_translate_fault() -> bool {
    (exception_iss() & 0b111111 & (0xf << 2)) == 4
}

#[inline(always)]
pub fn exception_data_abort_is_permission_fault() -> bool {
    (exception_iss() & 0b111111 & (0xf << 2)) == 12
}

#[inline(always)]
pub fn exception_data_abort_access_width() -> usize {
    1 << ((exception_iss() >> 22) & 0b11)
}

#[inline(always)]
pub fn exception_data_abort_access_is_write() -> bool {
    (exception_iss() & (1 << 6)) != 0
}

#[inline(always)]
pub fn exception_data_abort_access_in_stage2() -> bool {
    (exception_iss() & (1 << 7)) != 0
}

#[inline(always)]
pub fn exception_data_abort_access_reg() -> usize {
    (exception_iss() >> 16) & 0b11111
}

#[inline(always)]
pub fn exception_data_abort_access_reg_width() -> usize {
    4 + 4 * ((exception_iss() >> 15) & 1)
}

#[inline(always)]
pub fn exception_data_abort_access_is_sign_ext() -> bool {
    ((exception_iss() >> 21) & 1) != 0
}

#[no_mangle]
extern "C" fn current_el_sp0_synchronous() {
    panic!("current_el_sp0_synchronous");
}

#[no_mangle]
extern "C" fn current_el_sp0_irq() {
    // lower_aarch64_irq(ctx);
    panic!("current_el_sp0_irq");
}

#[no_mangle]
extern "C" fn current_el_sp0_serror() {
    panic!("current_el0_serror");
}

#[no_mangle]
#[inline(never)]
extern "C" fn current_el_spx_synchronous(ctx: *mut ContextFrame) {
    info!("{}", unsafe { *ctx });
    panic!(
        "current_elx_synchronous elr_el2 {:016x} sp_el0 {:016x} sp_el1 {:016x} sp_sel {}",
        cortex_a::registers::ELR_EL2.get(),
        cortex_a::registers::SP_EL0.get(),
        cortex_a::registers::SP_EL1.get(),
        cortex_a::registers::SPSel.get(),
    );
}

#[no_mangle]
extern "C" fn current_el_spx_irq(ctx: *mut ContextFrame) {
    trace!(">>> core {} current_el_spx_irq", current_cpu().id);
    lower_aarch64_irq(ctx);
}

#[no_mangle]
extern "C" fn current_el_spx_serror() {
    panic!("current_elx_serror");
}

#[no_mangle]
extern "C" fn lower_aarch64_synchronous(ctx: *mut ContextFrame) {
    trace!("lower_aarch64_synchronous");
    let prev_ctx = current_cpu().set_ctx(ctx);
    match exception_class() {
        0x24 => {
            trace!("Core[{}] data_abort_handler", current_cpu().id);
            data_abort_handler();
        }
        0x17 => {
            smc_handler();
        }
        0x16 => {
            hvc_handler();
        }
        0x18 => sysreg_handler(exception_iss() as u32),
        _ => unsafe {
            info!(
                "x0 {:x}, x1 {:x}, x29 {:x}",
                (*ctx).gpr(0),
                (*ctx).gpr(1),
                (*ctx).gpr(29)
            );
            panic!(
                "core {} vm {}: handler not presents for EC_{} @ipa {:#x}, @pc {:#x}",
                current_cpu().id,
                active_vm().unwrap().id(),
                exception_class(),
                exception_fault_addr(),
                (*ctx).exception_pc()
            );
        },
    }
    if let Some(ctx) = prev_ctx {
        current_cpu().set_ctx(ctx as *mut _);
    }
}

#[no_mangle]
#[cfg(feature = "preempt")]
fn interrupt_enter() {
    use super::{cpu_interrupt_disable, cpu_interrupt_enable};
    let level = cpu_interrupt_disable();
    current_cpu().interrupt_nested += 1;
    cpu_interrupt_enable(level);
    if current_cpu().interrupt_nested > 1 {
        trace!(
            "irq has come, core {} interrupt_nested {}",
            current_cpu().id,
            current_cpu().interrupt_nested,
        );
    }
}

#[no_mangle]
#[cfg(feature = "preempt")]
fn interrupt_leave() {
    use super::{cpu_interrupt_disable, cpu_interrupt_enable};
    if current_cpu().interrupt_nested > 1 {
        trace!(
            "irq is going to leave, core {} interrupt_nested {}",
            current_cpu().id,
            current_cpu().interrupt_nested,
        );
    }
    let level = cpu_interrupt_disable();
    current_cpu().interrupt_nested -= 1;
    cpu_interrupt_enable(level);
}

// #[derive(Clone, PartialEq, Eq)]
// struct PendingIrq {
//     int_id: usize,
//     priority: usize,
//     sender: usize,
// }

// impl PendingIrq {
//     fn new(int_id: usize, priority: usize, sender: usize) -> Self {
//         Self {
//             int_id,
//             priority,
//             sender,
//         }
//     }
// }

// impl PartialOrd for PendingIrq {
//     fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
//         Some(self.cmp(other))
//     }
// }

// impl Ord for PendingIrq {
//     fn cmp(&self, other: &Self) -> core::cmp::Ordering {
//         match self.priority.cmp(&other.priority) {
//             core::cmp::Ordering::Equal => {}
//             ord => return ord,
//         }
//         match self.int_id.cmp(&other.int_id) {
//             core::cmp::Ordering::Equal => {}
//             ord => return ord,
//         }
//         self.sender.cmp(&other.sender)
//     }
// }

// // TODO: currently, this is useless
// static PENDING_IRQ_LIST: Lazy<Mutex<BinaryHeap<PendingIrq>>> = Lazy::new(|| Mutex::new(BinaryHeap::new()));

#[no_mangle]
extern "C" fn lower_aarch64_irq(ctx: *mut ContextFrame) {
    let prev_ctx = current_cpu().set_ctx(ctx);
    if let Some((int_id, _sender)) = IntCtrl::fetch() {
        #[cfg(feature = "preempt")]
        interrupt_enter();
        // let priority = IntCtrl::irq_priority(int_id);

        // PENDING_IRQ_LIST.lock().push(PendingIrq::new(int_id, priority, sender));
        let handled_by_hypervisor = interrupt_handler(int_id);
        // PENDING_IRQ_LIST.lock().pop();

        #[cfg(feature = "preempt")]
        interrupt_leave();
        interrupt_arch_deactive_irq(handled_by_hypervisor);
    }
    if let Some(ctx) = prev_ctx {
        current_cpu().set_ctx(ctx as *mut _);
    }
}

#[no_mangle]
extern "C" fn lower_aarch64_serror() {
    panic!("lower aarch64 serror");
}
