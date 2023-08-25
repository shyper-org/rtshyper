use alloc::sync::Arc;

use crate::arch::INTERRUPT_IRQ_HYPERVISOR_TIMER;
use crate::kernel::current_cpu;
use crate::util::timer_list::{TimerEvent, TimerTickValue};

pub fn timer_init() {
    crate::arch::timer_arch_init();
    timer_enable(false);

    crate::util::barrier();
    if current_cpu().id == 0 {
        crate::kernel::interrupt_reserve_int(INTERRUPT_IRQ_HYPERVISOR_TIMER, timer_irq_handler);
        info!("Timer frequency: {}Hz", crate::arch::timer_arch_get_frequency());
        info!("Timer init ok");
    }
}

pub fn timer_enable(val: bool) {
    debug!("Core {} EL2 timer {}", current_cpu().id, val);
    super::interrupt::interrupt_cpu_enable(INTERRUPT_IRQ_HYPERVISOR_TIMER, val);
}

#[allow(dead_code)]
pub fn gettime_ns() -> usize {
    crate::arch::gettime_ns()
}

pub const fn gettimer_tick_ms() -> usize {
    10
}

fn timer_notify_after(ms: usize) {
    use crate::arch::{timer_arch_enable_irq, timer_arch_set};
    if ms == 0 {
        return;
    }

    timer_arch_set(ms);
    timer_arch_enable_irq();
}

fn check_timer_event(current_tick: TimerTickValue) {
    while let Some((_timeout_tick, event)) = current_cpu().timer_list.get_mut().unwrap().pop(current_tick) {
        event.callback(current_cpu().sys_tick);
    }
}

pub fn timer_irq_handler() {
    use crate::arch::timer_arch_disable_irq;

    timer_arch_disable_irq();
    current_cpu().sys_tick += 1;

    check_timer_event(current_cpu().sys_tick);

    current_cpu().vcpu_array.resched();

    timer_notify_after(gettimer_tick_ms());
}

#[allow(dead_code)]
pub fn start_timer_event(period: TimerTickValue, event: Arc<dyn TimerEvent>) {
    let timeout_tick = current_cpu().sys_tick + period;
    current_cpu().timer_list.get_mut().unwrap().push(timeout_tick, event);
}

#[allow(dead_code)]
pub fn remove_timer_event<F>(condition: F)
where
    F: Fn(&Arc<dyn TimerEvent>) -> bool,
{
    current_cpu().timer_list.get_mut().unwrap().remove_all(condition);
}
