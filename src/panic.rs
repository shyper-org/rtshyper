use core::panic::PanicInfo;
use crate::kernel::current_cpu;

#[cfg_attr(target_os = "none", panic_handler)]
#[no_mangle]
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
