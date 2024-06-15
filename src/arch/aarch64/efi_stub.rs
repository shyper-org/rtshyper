use uefi::prelude::*;

#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init(&mut system_table).unwrap();
    println!("Hello, world!");
    system_table.boot_services().stall(10_000_000);
    Status::SUCCESS
}
