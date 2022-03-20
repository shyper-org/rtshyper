#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(core_intrinsics)]
#![feature(default_alloc_error_handler)]
#![feature(alloc_error_handler)]
#![feature(const_fn_trait_bound)]
#![feature(into_future)]

#[macro_use]
extern crate alloc;
extern crate fdt;
#[macro_use]
extern crate lazy_static;
extern crate log;

// extern crate rlibc;

// #[macro_export]
// macro_rules! cpu {
//     () => ($crate::kernel::cpu())
// }

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use spin::Mutex;
use woke::{waker_ref, Woke};

use device::{init_vm0_dtb, mediated_dev_init};
use kernel::{cpu_init, interrupt_init, mem_init, timer_init};
use mm::heap_init;
use vmm::{vmm_boot, vmm_init};

use crate::kernel::{cpu_sched_init, hvc_init};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::lib::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[allow(dead_code)]
mod arch;
#[allow(dead_code)]
mod board;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod device;
#[allow(dead_code)]
mod driver;
#[allow(dead_code)]
mod kernel;
#[allow(dead_code)]
mod lib;
#[allow(dead_code)]
mod mm;
#[allow(dead_code)]
mod panic;
#[allow(dead_code)]
mod vmm;

// use lib::{BitAlloc, BitAlloc256};

pub static SYSTEM_FDT: spin::Once<alloc::vec::Vec<u8>> = spin::Once::new();

pub struct BlkFuture {
    val: usize,
}

impl Future for BlkFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        println!("new poll");
        Poll::Ready(())
    }
}

pub struct TaskTmp {
    pub task: Mutex<Pin<Box<dyn Future<Output=()> + 'static + Send + Sync>>>,
}

impl TaskTmp {
    pub fn new(future: impl Future<Output=()> + 'static + Send + Sync) -> TaskTmp {
        TaskTmp {
            task: Mutex::new(Box::pin(future))
        }
    }
}

impl Woke for TaskTmp {
    fn wake(self: Arc<Self>) {
        println!("wake");
        todo!()
    }

    fn wake_by_ref(arc_self: &Arc<Self>) {
        println!("wake_by_ref");
        todo!();
    }
}

// async func
fn test() -> BlkFuture {
    println!("test");
    BlkFuture {
        val: 306
    }
}

async fn bar() {
    println!("bar");
    test().await;
}

fn tmp() {
    println!("tmp");
    let mut bar = bar();
    let mut task_bar = TaskTmp::new(bar);

    let t: Arc<TaskTmp> = unsafe { Arc::from_raw(&mut task_bar as *mut _) };
    let waker = waker_ref(&t);
    let mut context = Context::from_waker(&*waker);
    println!("before poll");
    let ret = task_bar.task.lock().as_mut().poll(&mut context);
    match ret {
        Poll::Ready(_) => {
            println!("ready");
        }
        Poll::Pending => {
            println!("pending");
        }
    }
}

#[no_mangle]
pub unsafe fn init(cpu_id: usize, dtb: *mut fdt::myctypes::c_void) {
    // println!("core id {}", cpu_id);
    // const UART0: *mut u8 = 0x0900_0000 as *mut u8;
    // let out_str = b"AArch64 Bare Metal";
    // for byte in out_str {
    //     crate::driver::uart::putc(*byte);
    // }
    // tegra_emmc_blk_read(0, 0, 0 as *mut _);
    if cpu_id == 0 {
        #[cfg(feature = "tx2")]
        println!("Welcome to TX2 Sybilla Hypervisor!");
        #[cfg(feature = "qemu")]
        println!("Welcome to Qemu Sybilla Hypervisor!");
        heap_init();
        mem_init();
        // kernel::logger_init();
        init_vm0_dtb(dtb);
        hvc_init();
    }
    cpu_init();
    interrupt_init();
    timer_init();
    cpu_sched_init();
    // if cpu_id == 0 {
    //     tmp();
    // }
    vmm_init();
    if cpu_id == 0 {
        mediated_dev_init();
    }

    crate::lib::barrier();
    if cpu_id != 0 {
        crate::kernel::cpu_idle();
    }
    println!("Start booting Manager VM ...");
    vmm_boot();

    loop {}
}
