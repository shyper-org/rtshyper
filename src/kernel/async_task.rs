use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::future::Future;
use core::pin::Pin;
use core::task::Context;

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
use crate::lib::{memcpy_safe, trace};

#[derive(Clone, Debug)]
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

static ASYNC_IPI_TASK_LIST: Mutex<Vec<AsyncTask>> = Mutex::new(Vec::new());
static ASYNC_IO_TASK_LIST: Mutex<Vec<AsyncTask>> = Mutex::new(Vec::new());
static ASYNC_USED_INFO_LIST: Mutex<Vec<Vec<UsedInfo>>> = Mutex::new(vec![]);

#[derive(Clone)]
pub enum AsyncTaskData {
    AsyncIpiTask(IpiMediatedMsg),
    AsyncIoTask(IoAsyncMsg),
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
        // println!("task state {:#?}", state);
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
}

// async req function
pub async fn async_ipi_req() {
    let ipi_list = ASYNC_IPI_TASK_LIST.lock();
    if ipi_list.is_empty() {
        panic!("ipi_list should not be empty");
    }
    let task = ipi_list[0].clone();
    drop(ipi_list);
    match task.task_data {
        AsyncTaskData::AsyncIpiTask(msg) => {
            if current_cpu().id == 0 {
                virtio_blk_notify_handler(msg.vq.clone(), msg.blk.clone(), vm(msg.src_id).unwrap());
            } else {
                ipi_send_msg(0, IpiType::IpiTMediatedDev, IpiInnerMsg::MediatedMsg(msg.clone()));
            }
        }
        _ => {}
    }
}

pub async fn async_blk_io_req() {
    let io_list = ASYNC_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        panic!("io_list should not be empty");
    }
    let task = io_list[0].clone();
    drop(io_list);
    match task.task_data {
        AsyncTaskData::AsyncIoTask(msg) => {
            match msg.io_type {
                VIRTIO_BLK_T_IN => {
                    // println!("mediated_blk_read");
                    mediated_blk_read(msg.blk_id, msg.sector, msg.count);
                }
                VIRTIO_BLK_T_OUT => {
                    // println!("mediated_blk_write");
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
            }
        }
        _ => {}
    }
}
// end async req function

pub fn set_io_task_state(idx: usize, state: AsyncTaskState) {
    let io_list = ASYNC_IO_TASK_LIST.lock();
    if io_list.len() <= idx {
        panic!("async_io_task_head io_list should not be empty");
    }
    *(io_list[idx].state.lock()) = state;
}

pub fn add_async_task(task: AsyncTask, ipi: bool) {
    // println!("add {} task", if ipi { "ipi" } else { "blk io" });
    let mut ipi_list = ASYNC_IPI_TASK_LIST.lock();
    let mut io_list = ASYNC_IO_TASK_LIST.lock();

    if ipi {
        ipi_list.push(task);
    } else {
        io_list.push(task);
    }
    if current_cpu().id != 0 && io_list.is_empty() && ipi_list.len() == 1 {
        drop(ipi_list);
        drop(io_list);
        async_task_exe();
    } else if current_cpu().id == 0 {
        drop(ipi_list);
        drop(io_list);
        async_task_exe();
    }
}

// async task executor
pub fn async_task_exe() {
    let mut ipi_list = ASYNC_IPI_TASK_LIST.lock();
    let io_list = ASYNC_IO_TASK_LIST.lock();

    if !ipi_list.is_empty() {
        let state = ipi_list[0].state.lock();
        if let AsyncTaskState::Finish = *state {
            drop(state);
            ipi_list.remove(0);
        }
    }

    let mut task;
    let ipi;
    if io_list.is_empty() {
        if !ipi_list.is_empty() {
            task = ipi_list[0].clone();
            ipi = true;
        } else {
            return;
        }
    } else {
        ipi = false;
        task = io_list[0].clone();
    }
    drop(ipi_list);
    drop(io_list);
    if task.handle() {
        // task finish
        // println!("task finish, ipi len {}, io len {}", ipi_list.len(), io_list.len());
        finish_async_task(ipi);
    }
}

pub fn finish_async_task(ipi: bool) {
    let mut ipi_list = ASYNC_IPI_TASK_LIST.lock();
    let mut io_list = ASYNC_IO_TASK_LIST.lock();
    let task = if ipi { ipi_list.remove(0) } else { io_list.remove(0) };
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

            if last_vm_async_io_task(task.src_vmid) {
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
                    let vm = vm(task.src_vmid).unwrap();
                    interrupt_vm_inject(vm.clone(), vm.vcpu(0).unwrap(), BLK_IRQ, 0);
                }
            }
        }
        AsyncTaskData::AsyncIpiTask(_) => {}
    }
    if current_cpu().id == 0 {
        async_task_exe();
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
    if src_vmid >= used_info_list.len() {
        println!(
            "push_used_info: src_vmid {} larger than list size {}",
            src_vmid,
            used_info_list.len()
        );
        return;
    }
    used_info_list[src_vmid].push(UsedInfo {
        desc_chain_head_idx,
        used_len,
    });
}

pub fn update_used_info(vq: Virtq, src_vmid: usize) {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    if src_vmid >= used_info_list.len() {
        println!(
            "handle_used_info: src_vmid {} larger than list size {}",
            src_vmid,
            used_info_list.len()
        );
        return;
    }
    let vq_size = vq.num();
    for info in &*used_info_list[src_vmid] {
        vq.update_used_ring(info.used_len, info.desc_chain_head_idx, vq_size);
    }
    used_info_list[src_vmid].clear();
}

pub fn add_async_used_info() {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    used_info_list.push(Vec::new());
}
