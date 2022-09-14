use alloc::boxed::Box;
use alloc::collections::{BTreeMap, LinkedList};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::future::Future;
use core::pin::Pin;
use core::task::Context;

use cortex_a::asm::wfi;
use spin::mutex::Mutex;
use woke::{waker_ref, Woke};

use crate::device::{
    BLK_IRQ, BlkIov, mediated_blk_read, mediated_blk_write, virtio_blk_notify_handler, VIRTIO_BLK_T_IN,
    VIRTIO_BLK_T_OUT, Virtq,
};
use crate::kernel::{
    current_cpu, interrupt_vm_inject, ipi_send_msg, IpiInnerMsg, IpiMediatedMsg, IpiMediatedNotifyMsg, IpiType, vm,
    vm_if_get_cpu_id,
};
use crate::lib::{memcpy_safe, sleep, trace};

pub static TASK_IPI_COUNT: Mutex<usize> = Mutex::new(0);
pub static TASK_COUNT: Mutex<usize> = Mutex::new(0);

pub fn add_task_ipi_count() {
    let mut count = TASK_IPI_COUNT.lock();
    *count += 1;
}

pub fn add_task_count() {
    let mut count = TASK_COUNT.lock();
    if *count % 100 == 0 {
        println!("task count {}, ipi count {}", *count, get_task_ipi_count());
    }
    *count += 1;
}

pub fn get_task_ipi_count() -> usize {
    let count = TASK_IPI_COUNT.lock();
    *count
}

pub fn get_task_count() -> usize {
    let count = TASK_COUNT.lock();
    *count
}

#[derive(Clone, Copy, Debug)]
pub enum AsyncTaskState {
    Pending,
    Running,
    Finish,
}

pub struct UsedInfo {
    pub desc_chain_head_idx: u32,
    pub used_len: u32,
}

#[derive(Clone)]
pub struct IoAsyncMsg {
    pub src_vmid: usize,
    pub vq: Virtq,
    pub io_type: usize,
    pub blk_id: usize,
    pub sector: usize,
    pub count: usize,
    pub cache: usize,
    pub iov_list: Arc<Vec<BlkIov>>,
}

#[derive(Clone)]
pub struct IoIdAsyncMsg {
    pub vq: Virtq,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AsyncExeStatus {
    Pending,
    Scheduling,
}

pub static ASYNC_EXE_STATUS: Mutex<AsyncExeStatus> = Mutex::new(AsyncExeStatus::Pending);
pub static ASYNC_IPI_TASK_LIST: Mutex<LinkedList<AsyncTask>> = Mutex::new(LinkedList::new());
pub static ASYNC_IO_TASK_LIST: Mutex<LinkedList<AsyncTask>> = Mutex::new(LinkedList::new());
pub static ASYNC_USED_INFO_LIST: Mutex<BTreeMap<usize, LinkedList<UsedInfo>>> = Mutex::new(BTreeMap::new());

#[derive(Clone)]
pub enum AsyncTaskData {
    AsyncIpiTask(IpiMediatedMsg),
    AsyncIoTask(IoAsyncMsg),
    AsyncNoneTask(IoIdAsyncMsg),
}

fn async_exe_status() -> AsyncExeStatus {
    *ASYNC_EXE_STATUS.lock()
}

fn set_async_exe_status(status: AsyncExeStatus) {
    *ASYNC_EXE_STATUS.lock() = status;
}

#[derive(Clone)]
pub struct AsyncTask {
    pub task_data: AsyncTaskData,
    pub src_vmid: usize,
    pub state: Arc<Mutex<AsyncTaskState>>,
    pub task: Arc<Mutex<Pin<Box<dyn Future<Output = ()> + 'static + Send + Sync>>>>,
}

impl Woke for AsyncTask {
    fn wake(self: Arc<Self>) {
        todo!()
    }

    fn wake_by_ref(_arc_self: &Arc<Self>) {
        todo!()
    }
}

impl AsyncTask {
    pub fn new(
        task_data: AsyncTaskData,
        src_vmid: usize,
        future: impl Future<Output = ()> + 'static + Send + Sync,
    ) -> AsyncTask {
        AsyncTask {
            task_data,
            src_vmid,
            state: Arc::new(Mutex::new(AsyncTaskState::Pending)),
            task: Arc::new(Mutex::new(Box::pin(future))),
        }
    }

    pub fn handle(&mut self) -> bool {
        let mut state = self.state.lock();
        match *state {
            AsyncTaskState::Pending => {
                *state = AsyncTaskState::Running;
            }
            AsyncTaskState::Running => {
                return false;
            }
            AsyncTaskState::Finish => {
                return true;
            }
        }
        drop(state);
        let wake: Arc<AsyncTask> = unsafe { Arc::from_raw(self as *mut _) };
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&*waker);
        self.task.lock().as_mut().poll(&mut context);
        return false;
    }

    pub fn set_state(&self, state: AsyncTaskState) {
        let mut cur_state = self.state.lock();
        *cur_state = state;
    }
}

// async req function
pub async fn async_ipi_req() {
    let ipi_list = ASYNC_IPI_TASK_LIST.lock();
    if ipi_list.is_empty() {
        panic!("ipi_list should not be empty");
    }
    let task = ipi_list.front().unwrap().clone();
    drop(ipi_list);
    match task.task_data {
        AsyncTaskData::AsyncIpiTask(msg) => {
            if current_cpu().id == 0 {
                virtio_blk_notify_handler(msg.vq.clone(), msg.blk.clone(), vm(msg.src_id).unwrap());
            } else {
                // add_task_ipi_count();
                ipi_send_msg(0, IpiType::IpiTMediatedDev, IpiInnerMsg::MediatedMsg(msg.clone()));
            }
        }
        _ => {}
    }
}

pub async fn async_blk_id_req() {}

pub async fn async_blk_io_req() {
    let io_list = ASYNC_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        panic!("io_list should not be empty");
    }
    let task = io_list.front().unwrap().clone();
    drop(io_list);
    match task.task_data {
        AsyncTaskData::AsyncIoTask(msg) => match msg.io_type {
            VIRTIO_BLK_T_IN => {
                mediated_blk_read(msg.blk_id, msg.sector, msg.count);
            }
            VIRTIO_BLK_T_OUT => {
                let mut cache_ptr = msg.cache;
                for idx in 0..msg.iov_list.len() {
                    let data_bg = msg.iov_list[idx].data_bg;
                    let len = msg.iov_list[idx].len as usize;

                    if cache_ptr < 0x1000 || data_bg < 0x1000 {
                        panic!("illegal des addr {:x}, src addr {:x}", cache_ptr, data_bg);
                    }
                    memcpy_safe(cache_ptr as *mut u8, data_bg as *mut u8, len);
                    cache_ptr += len;
                }
                mediated_blk_write(msg.blk_id, msg.sector, msg.count);
            }
            _ => {
                panic!("illegal mediated blk req type {}", msg.io_type);
            }
        },
        _ => {}
    }
}
// end async req function

pub fn set_front_io_task_state(state: AsyncTaskState) {
    let io_list = ASYNC_IO_TASK_LIST.lock();
    match io_list.front() {
        None => {
            panic!("front io task is none");
        }
        Some(task) => {
            *(task.state.lock()) = state;
        }
    }
}

pub fn add_async_task(task: AsyncTask, ipi: bool) {
    // println!("add {} task", if ipi { "ipi" } else { "blk io" });
    let mut ipi_list = ASYNC_IPI_TASK_LIST.lock();
    let mut io_list = ASYNC_IO_TASK_LIST.lock();

    if ipi {
        ipi_list.push_back(task);
    } else {
        io_list.push_back(task);
    }
    // println!("add_async_task: ipi len {} io len {}", ipi_list.len(), io_list.len());
    drop(ipi_list);
    drop(io_list);
    loop {
        if current_cpu().id == 0 || ASYNC_IPI_TASK_LIST.lock().len() < 1024 {
            break;
        } else {
            // sleep(100);
            for _ in 0..100 * 1000 {
                unsafe {
                    asm!("wfi");
                }
            }
        }
    }

    if current_cpu().id != 0
        && ASYNC_IO_TASK_LIST.lock().is_empty()
        && ASYNC_IPI_TASK_LIST.lock().len() == 1
        && async_exe_status() == AsyncExeStatus::Pending
    {
        async_task_exe();
    }
}

// async task executor
pub fn async_task_exe() {
    if current_cpu().id == 0 {
        match async_exe_status() {
            AsyncExeStatus::Pending => {
                set_async_exe_status(AsyncExeStatus::Scheduling);
            }
            AsyncExeStatus::Scheduling => {
                return;
            }
        }
    }
    loop {
        let ipi_list = ASYNC_IPI_TASK_LIST.lock();
        let io_list = ASYNC_IO_TASK_LIST.lock();

        // if !ipi_list.is_empty() {
        //     let state = ipi_list[0].state.lock();
        //     if let AsyncTaskState::Finish = *state {
        //         drop(state);
        //         ipi_list.remove(0);
        //     }
        // }

        let mut task;
        let ipi;
        if io_list.is_empty() {
            if !ipi_list.is_empty() {
                task = ipi_list.front().unwrap().clone();
                ipi = true;
            } else {
                set_async_exe_status(AsyncExeStatus::Pending);
                return;
            }
        } else {
            ipi = false;
            task = io_list.front().unwrap().clone();
        }
        drop(ipi_list);
        drop(io_list);
        if task.handle() || (ipi && current_cpu().id == 0) {
            // task finish
            finish_async_task(ipi);
        } else {
            // wait for notify
            set_async_exe_status(AsyncExeStatus::Pending);
            return;
        }
        if current_cpu().id != 0 {
            return;
        }
    }
}

pub fn finish_async_task(ipi: bool) {
    let mut ipi_list = ASYNC_IPI_TASK_LIST.lock();
    let mut io_list = ASYNC_IO_TASK_LIST.lock();
    let task = match if ipi { ipi_list.pop_front() } else { io_list.pop_front() } {
        None => {
            panic!("there is no {} task", if ipi { "ipi" } else { "io" })
        }
        Some(t) => t,
    };
    // println!("finish_async_task: ipi len {} io len {}", ipi_list.len(), io_list.len());
    drop(io_list);
    drop(ipi_list);
    match task.task_data {
        AsyncTaskData::AsyncIoTask(args) => {
            match args.io_type {
                VIRTIO_BLK_T_IN => {
                    // let mut sum = 0;
                    let mut cache_ptr = args.cache;
                    for idx in 0..args.iov_list.len() {
                        let data_bg = args.iov_list[idx].data_bg;
                        let len = args.iov_list[idx].len as usize;
                        if trace() && (data_bg < 0x1000 || cache_ptr < 0x1000) {
                            panic!("illegal des addr {:x}, src addr {:x}", data_bg, cache_ptr);
                        }
                        memcpy_safe(data_bg as *mut u8, cache_ptr as *mut u8, len);
                        // sum |= check_sum(data_bg, len);
                        cache_ptr += len;
                    }
                    // println!("read check_sum is {:x}", sum);
                }
                _ => {}
            }

            // if last_vm_async_io_task(task.src_vmid) {
            let target_id = vm_if_get_cpu_id(task.src_vmid);
            update_used_info(args.vq.clone(), task.src_vmid);
            if target_id != current_cpu().id {
                let msg = IpiMediatedNotifyMsg { vm_id: task.src_vmid };
                ipi_send_msg(
                    target_id,
                    IpiType::IpiTMediatedNotify,
                    IpiInnerMsg::MediatedNotifyMsg(msg),
                );
            } else {
                match vm(task.src_vmid) {
                    None => {
                        println!("finish_async_task: vm[{}] no exist", task.src_vmid);
                    }
                    Some(vm) => {
                        interrupt_vm_inject(vm.clone(), vm.vcpu(0).unwrap(), BLK_IRQ, 0);
                    }
                }
            }
            // }
        }
        AsyncTaskData::AsyncIpiTask(_) => {}
        AsyncTaskData::AsyncNoneTask(args) => {
            let target_id = vm_if_get_cpu_id(task.src_vmid);
            update_used_info(args.vq.clone(), task.src_vmid);
            if target_id != current_cpu().id {
                let msg = IpiMediatedNotifyMsg { vm_id: task.src_vmid };
                ipi_send_msg(
                    target_id,
                    IpiType::IpiTMediatedNotify,
                    IpiInnerMsg::MediatedNotifyMsg(msg),
                );
            } else {
                match vm(task.src_vmid) {
                    None => {
                        println!("finish_async_task: vm[{}] no exist", task.src_vmid);
                    }
                    Some(vm) => {
                        interrupt_vm_inject(vm.clone(), vm.vcpu(0).unwrap(), BLK_IRQ, 0);
                    }
                }
            }
        }
    }
}

pub fn last_vm_async_io_task(vm_id: usize) -> bool {
    let io_list = ASYNC_IO_TASK_LIST.lock();
    for io_task in &*io_list {
        if io_task.src_vmid == vm_id {
            return false;
        }
    }
    true
}

pub fn push_used_info(desc_chain_head_idx: u32, used_len: u32, src_vmid: usize) {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    match used_info_list.get_mut(&src_vmid) {
        Some(info_list) => {
            info_list.push_back(UsedInfo {
                desc_chain_head_idx,
                used_len,
            });
        }
        None => {
            println!("async_push_used_info: src_vmid {} not existed", src_vmid);
        }
    }
}

pub fn update_used_info(vq: Virtq, src_vmid: usize) {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    match used_info_list.get_mut(&src_vmid) {
        Some(info_list) => {
            let vq_size = vq.num();
            // for info in info_list.iter() {
            // vq.update_used_ring(info.used_len, info.desc_chain_head_idx, vq_size);
            let info = info_list.pop_front().unwrap();
            vq.update_used_ring(info.used_len, info.desc_chain_head_idx, vq_size);
            // }
            // info_list.clear();
        }
        None => {
            println!("async_push_used_info: src_vmid {} not existed", src_vmid);
        }
    }
}

pub fn add_async_used_info(vm_id: usize) {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    used_info_list.insert(vm_id, LinkedList::new());
}

pub fn remove_async_used_info(vm_id: usize) {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    used_info_list.remove(&vm_id);
    // println!("VM[{}] remove async used info", vm_id);
}

pub fn remove_vm_async_task(vm_id: usize) {
    let mut io_list = ASYNC_IO_TASK_LIST.lock();
    let mut ipi_list = ASYNC_IPI_TASK_LIST.lock();
    // io_list.retain(|x| x.src_vmid != vm_id);
    // ipi_list.retain(|x| x.src_vmid != vm_id);
    *io_list = io_list.drain_filter(|x| x.src_vmid == vm_id).collect::<LinkedList<_>>();
    *ipi_list = ipi_list
        .drain_filter(|x| x.src_vmid == vm_id)
        .collect::<LinkedList<_>>();
}
