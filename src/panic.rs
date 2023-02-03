use core::panic::PanicInfo;

#[cfg_attr(target_os = "none", panic_handler)]
#[no_mangle]
fn panic(info: &PanicInfo) -> ! {
    println!("[Panic]");
    println!("{}", info);
    loop {
        core::hint::spin_loop();
    }
}
