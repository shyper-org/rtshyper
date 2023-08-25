use crate::kernel::current_cpu;
use core::panic::PanicInfo;

#[cfg_attr(target_os = "none", panic_handler)]
fn panic(info: &PanicInfo) -> ! {
    println!(
        concat!("\u{1B}[1;31m[Panic] on Core {}\n", "{}\u{1B}[0m"),
        current_cpu().id,
        info
    );
    loop {
        core::hint::spin_loop();
    }
}
