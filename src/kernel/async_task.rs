use core::future::Future;
use core::pin::Pin;
use core::task::Context;

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, LinkedList};
use alloc::sync::Arc;
use alloc::task::Wake;
use alloc::vec::Vec;
use spin::mutex::Mutex;

use crate::device::{
    BlkIov, mediated_blk_read, mediated_blk_write, virtio_blk_notify_handler, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT,
    VirtioMmio, Virtq,
};
use crate::kernel::{active_vm_id, ipi_send_msg, IpiInnerMsg, IpiMediatedMsg, IpiType, vm};
use crate::util::{memcpy_safe, sleep};

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
    pub dev: VirtioMmio,
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
    pub dev: VirtioMmio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncExeStatus {
    Pending,
    Scheduling,
}

pub static ASYNC_EXE_STATUS: Mutex<AsyncExeStatus> = Mutex::new(AsyncExeStatus::Pending);
pub static ASYNC_IPI_TASK_LIST: Mutex<LinkedList<AsyncTask>> = Mutex::new(LinkedList::new());
pub static ASYNC_IO_TASK_LIST: Mutex<FairQueue<AsyncTask>> = Mutex::new(FairQueue::new());
pub static ASYNC_USED_INFO_LIST: Mutex<BTreeMap<usize, LinkedList<UsedInfo>>> = Mutex::new(BTreeMap::new());

pub trait TaskOwner {
    fn owner(&self) -> usize;
}

pub struct FairQueue<T: TaskOwner> {
    len: usize,
    map: BTreeMap<usize, LinkedList<T>>,
    queue: LinkedList<usize>,
}

#[allow(dead_code)]
impl<T: TaskOwner> FairQueue<T> {
    pub const fn new() -> Self {
        Self {
            len: 0,
            map: BTreeMap::new(),
            queue: LinkedList::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push_back(&mut self, task: T) {
        let key = task.owner();
        match self.map.get_mut(&key) {
            Some(sub_queue) => sub_queue.push_back(task),
            None => {
                let mut sub_queue = LinkedList::new();
                sub_queue.push_back(task);
                self.map.insert(key, sub_queue);
                self.queue.push_back(key);
            }
        }
        self.len += 1;
    }

    pub fn pop_front(&mut self) -> Option<T> {
        match self.queue.pop_front() {
            Some(owner) => match self.map.get_mut(&owner) {
                Some(sub_queue) => {
                    let res = sub_queue.pop_front();
                    if !sub_queue.is_empty() {
                        self.queue.push_back(owner);
                    } else {
                        self.map.remove(&owner);
                    }
                    self.len -= 1;
                    res
                }
                None => panic!(""),
            },
            None => panic!("front: queue empty"),
        }
    }

    pub fn front(&self) -> Option<&T> {
        match self.queue.front() {
            Some(owner) => match self.map.get(owner) {
                Some(sub_queue) => sub_queue.front(),
                None => None,
            },
            None => None,
        }
    }

    pub fn remove(&mut self, owner: usize) {
        if let Some(sub_queue) = self.map.remove(&owner) {
            self.len -= sub_queue.len();
            self.queue = self.queue.drain_filter(|x| *x == owner).collect();
        }
    }
}

pub trait AsyncCallback {
    #[inline]
    fn preprocess(&self) {}
    #[inline]
    fn finish(&self, _src_vmid: usize) {}
}

impl AsyncCallback for IpiMediatedMsg {
    #[inline]
    fn preprocess(&self) {
        if active_vm_id() == 0 {
            virtio_blk_notify_handler(self.vq.clone(), self.blk.clone(), vm(self.src_id).unwrap());
        } else {
            // send IPI to target cpu, and the target will invoke `mediated_ipi_handler`
            ipi_send_msg(0, IpiType::IpiTMediatedDev, IpiInnerMsg::MediatedMsg(self.clone()));
        }
    }
}

impl AsyncCallback for IoAsyncMsg {
    #[inline]
    // inject an interrupt to service VM
    fn preprocess(&self) {
        if self.io_type == VIRTIO_BLK_T_IN {
            mediated_blk_read(self.blk_id, self.sector, self.count);
        } else if self.io_type == VIRTIO_BLK_T_OUT {
            let mut cache_ptr = self.cache;
            for iov in self.iov_list.iter() {
                let data_bg = iov.data_bg;
                let len = iov.len as usize;
                memcpy_safe(cache_ptr as *mut u8, data_bg as *mut u8, len);
                cache_ptr += len;
            }
            mediated_blk_write(self.blk_id, self.sector, self.count);
        }
    }

    #[inline]
    fn finish(&self, src_vmid: usize) {
        if self.io_type == VIRTIO_BLK_T_IN {
            // let mut sum = 0;
            let mut cache_ptr = self.cache;
            for iov in self.iov_list.iter() {
                let data_bg = iov.data_bg;
                let len = iov.len as usize;
                memcpy_safe(data_bg as *mut u8, cache_ptr as *mut u8, len);
                // sum |= check_sum(data_bg, len);
                cache_ptr += len;
            }
            // println!("read check_sum is {:x}", sum);
        }

        update_used_info(&self.vq, src_vmid);
        let src_vm = vm(src_vmid).unwrap();
        self.dev.notify(src_vm);
    }
}

impl AsyncCallback for IoIdAsyncMsg {
    #[inline]
    fn finish(&self, src_vmid: usize) {
        update_used_info(&self.vq, src_vmid);
        let src_vm = vm(src_vmid).unwrap();
        self.dev.notify(src_vm);
    }
}

fn async_exe_status() -> AsyncExeStatus {
    *ASYNC_EXE_STATUS.lock()
}

fn set_async_exe_status(status: AsyncExeStatus) {
    *ASYNC_EXE_STATUS.lock() = status;
}

#[derive(Clone)]
pub struct AsyncTask {
    pub callback: Arc<dyn AsyncCallback + Send + Sync>,
    pub src_vmid: usize,
    pub state: Arc<Mutex<AsyncTaskState>>,
    pub task: Arc<Mutex<Pin<Box<dyn Future<Output = ()> + 'static + Send + Sync>>>>,
}

impl TaskOwner for AsyncTask {
    fn owner(&self) -> usize {
        self.src_vmid
    }
}

impl Wake for AsyncTask {
    fn wake(self: Arc<Self>) {
        todo!()
    }
}

impl AsyncTask {
    pub fn new(
        callback: impl AsyncCallback + 'static + Send + Sync,
        src_vmid: usize,
        future: impl Future<Output = ()> + 'static + Send + Sync,
    ) -> AsyncTask {
        AsyncTask {
            callback: Arc::new(callback),
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
        let waker = Arc::new(self.clone()).into();
        let mut context = Context::from_waker(&waker);
        let _ = self.task.lock().as_mut().poll(&mut context);
        false
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
    task.callback.preprocess();
}

pub async fn async_blk_id_req() {}

pub async fn async_blk_io_req() {
    let io_list = ASYNC_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        panic!("io_list should not be empty");
    }
    let task = io_list.front().unwrap().clone();
    drop(io_list);
    task.callback.preprocess();
}
// end async req function

pub fn set_front_io_task_state(state: AsyncTaskState) {
    let io_list = ASYNC_IO_TASK_LIST.lock();
    match io_list.front() {
        None => {
            panic!("front io task is none");
        }
        Some(task) => {
            task.set_state(state);
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
    let need_execute = ipi_list.len() == 1
        && io_list.is_empty()
        && active_vm_id() != 0
        && async_exe_status() == AsyncExeStatus::Pending;
    // println!("add_async_task: ipi len {} io len {}", ipi_list.len(), io_list.len());
    drop(ipi_list);
    drop(io_list);
    while active_vm_id() != 0 && ASYNC_IPI_TASK_LIST.lock().len() >= 1024 {
        sleep(1);
    }

    // if this is a normal VM and this is the first IO request
    // (which generate a ipi async task in `virtio_mediated_blk_notify_handler`)
    // invoke the executor to handle it
    if need_execute {
        async_task_exe();
    }
}

// async task executor
pub fn async_task_exe() {
    if active_vm_id() == 0 {
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

        let mut task;
        let ipi;
        if io_list.is_empty() {
            if !ipi_list.is_empty() {
                // other VM start an IO which need to be handled by service VM
                task = ipi_list.front().unwrap().clone();
                ipi = true;
            } else {
                set_async_exe_status(AsyncExeStatus::Pending);
                return;
            }
        } else {
            // if io_list is not empty, prioritize IO requests
            ipi = false;
            task = io_list.front().unwrap().clone();
        }
        drop(ipi_list);
        drop(io_list);
        if task.handle() || (ipi && active_vm_id() == 0) {
            // task finish
            finish_async_task(ipi);
        } else {
            // wait for notify
            set_async_exe_status(AsyncExeStatus::Pending);
            return;
        }
        // not a service VM, end loop
        if active_vm_id() != 0 {
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
    task.callback.finish(task.src_vmid);
}

pub fn push_used_info(desc_chain_head_idx: u32, used_len: u32, src_vmid: usize) {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    let info = UsedInfo {
        desc_chain_head_idx,
        used_len,
    };
    match used_info_list.get_mut(&src_vmid) {
        Some(info_list) => {
            info_list.push_back(info);
        }
        None => {
            let mut list = LinkedList::new();
            list.push_back(info);
            used_info_list.insert(src_vmid, list);
        }
    }
}

fn update_used_info(vq: &Virtq, src_vmid: usize) {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    match used_info_list.get_mut(&src_vmid) {
        Some(info_list) => {
            // for info in info_list.iter() {
            // vq.update_used_ring(info.used_len, info.desc_chain_head_idx, vq_size);
            let info = info_list.pop_front().unwrap();
            vq.update_used_ring(info.used_len, info.desc_chain_head_idx);
            // }
            // info_list.clear();
        }
        None => {
            println!("async_push_used_info: src_vmid {} not existed", src_vmid);
        }
    }
}

fn remove_async_used_info(vm_id: usize) {
    let mut used_info_list = ASYNC_USED_INFO_LIST.lock();
    used_info_list.remove(&vm_id);
    // println!("VM[{}] remove async used info", vm_id);
}

pub fn remove_vm_async_task(vm_id: usize) {
    let mut io_list = ASYNC_IO_TASK_LIST.lock();
    let mut ipi_list = ASYNC_IPI_TASK_LIST.lock();
    io_list.remove(vm_id);
    *ipi_list = ipi_list.drain_filter(|x| x.src_vmid == vm_id).collect();
    remove_async_used_info(vm_id);
}
