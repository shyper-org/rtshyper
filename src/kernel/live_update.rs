pub fn update_request() {
    println!("src hypervisor send update request");
    extern "C" {
        pub fn update_request();
    }
    unsafe { update_request() };
}

#[no_mangle]
pub unsafe extern "C" fn rust_shyper_update() {
    println!("in rust_shyper_update");
}
