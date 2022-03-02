use crate::arch::INTERRUPT_IRQ_HYPERVISOR_TIMER;
// use crate::board::PLATFORM_CPU_NUM_MAX;
use crate::kernel::{current_cpu, InterruptHandler, SchedType, SchedulerTrait};

// #[derive(Copy, Clone)]
// struct Timer(bool);

// impl Timer {
//     const fn default() -> Timer {
//         Timer(false)
//     }

//     fn set(&mut self, val: bool) {
//         self.0 = val;
//     }
// }

// static TIMER_LIST: Mutex<[Timer; PLATFORM_CPU_NUM_MAX]> =
//     Mutex::new([Timer::default(); PLATFORM_CPU_NUM_MAX]);

pub fn timer_init() {
    crate::arch::timer_arch_init();
    timer_enable(false);

    crate::lib::barrier();
    if current_cpu().id == 0 {
        crate::kernel::interrupt_reserve_int(
            INTERRUPT_IRQ_HYPERVISOR_TIMER,
            InterruptHandler::TimeIrqHandler(timer_irq_handler),
        );
        println!(
            "Timer frequency: {}Hz",
            crate::arch::timer_arch_get_frequency()
        );
        println!("Timer init ok");
    }
}

pub fn timer_enable(val: bool) {
    println!(
        "Core {} {} EL2 timer",
        current_cpu().id,
        if val { "enable" } else { "disable" }
    );
    super::interrupt::interrupt_cpu_enable(INTERRUPT_IRQ_HYPERVISOR_TIMER, val);
}

fn timer_notify_after(us: usize) {
    use crate::arch::{timer_arch_enable_irq, timer_arch_set};
    if us == 0 {
        return;
    }

    timer_arch_set(us);
    timer_arch_enable_irq();
}

fn timer_irq_handler(_arg: usize) {
    use crate::arch::{
        timer_arch_disable_irq, timer_arch_enable_irq, timer_arch_get_counter, timer_arch_set,
    };

    timer_arch_disable_irq();
    let vcpu_pool = current_cpu().vcpu_pool();

    if vcpu_pool.running() > 1 {
        match &current_cpu().sched {
            SchedType::SchedRR(rr) => {
                rr.schedule();
                // TODO: vcpu_pool_switch(ANY_PENDING_VCPU)
                timer_notify_after(vcpu_pool.slice());
            }
            SchedType::None => {
                panic!("cpu{} sched should not be None", current_cpu().id);
            }
        }
    }
}
