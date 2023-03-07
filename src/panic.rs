use core::panic::PanicInfo;

#[cfg_attr(target_os = "none", panic_handler)]
#[no_mangle]
fn panic(info: &PanicInfo) -> ! {
    println!("\u{1B}[1;31m[Panic]");
    println!("{}\u{1B}[0m", info);
    loop {
        core::hint::spin_loop();
    }
}
