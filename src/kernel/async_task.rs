// use alloc::boxed::Box;
use alloc::collections::{BTreeMap, LinkedList};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::mutex::Mutex;

use crate::device::{
    BlkIov, mediated_blk_read, mediated_blk_write, virtio_blk_notify_handler, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT,
    VirtioMmio, Virtq,
};
use crate::kernel::{active_vm_id, ipi_send_msg, IpiInnerMsg, IpiMediatedMsg, IpiType, vm};
use crate::util::{memcpy_safe, sleep, trace};

// use core::future::Future;
// use core::pin::Pin;
// use core::task::Context;

// use woke::{waker_ref, Woke};

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

// pub struct FairQueue<T: TaskOwner> {
//     map: BTreeMap<usize, Arc<RefCell<LinkedList<T>>>>,
//     // reverse_map: BTreeMap<Arc<RefCell<LinkedList<T>>>, usize>,
//     queue: LinkedList<Arc<RefCell<LinkedList<T>>>>,
// }

pub struct FairQueue<T: TaskOwner> {
    len: usize,
    map: BTreeMap<usize, LinkedList<T>>,
    queue: LinkedList<usize>,
}

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
                None => panic!(""),
            },
            None => panic!("front: queue empty"),
        }
    }

    pub fn remove(&mut self, owner: usize) {
        if let Some(sub_queue) = self.map.remove(&owner) {
            self.len -= sub_queue.len();
            self.queue = self.queue.drain_filter(|x| *x == owner).collect();
        }
    }
}

impl<T: TaskOwner> Iterator for FairQueue<T> {
    type Item = T;
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        todo!()
    }
}

#[derive(Clone)]
pub enum AsyncTaskData {
    Ipi(IpiMediatedMsg),
    Io(IoAsyncMsg),
    NoneTask(IoIdAsyncMsg),
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
    // pub task: Arc<Mutex<Pin<Box<dyn Future<Output = ()> + 'static + Send + Sync>>>>,
    pub task: fn(),
}

impl TaskOwner for AsyncTask {
    fn owner(&self) -> usize {
        self.src_vmid
    }
}

// impl Woke for AsyncTask {
//     fn wake(self: Arc<Self>) {
//         todo!()
//     }

//     fn wake_by_ref(_arc_self: &Arc<Self>) {
//         todo!()
//     }
// }

impl AsyncTask {
    pub fn new(task_data: AsyncTaskData, src_vmid: usize, future: fn()) -> AsyncTask {
        AsyncTask {
            task_data,
            src_vmid,
            state: Arc::new(Mutex::new(AsyncTaskState::Pending)),
            task: future,
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
        // let wake: Arc<AsyncTask> = unsafe { Arc::from_raw(self as *mut _) };
        // let waker = waker_ref(&wake);
        // let mut context = Context::from_waker(&*waker);
        // self.task.lock().as_mut().poll(&mut context);
        (self.task)();
        false
    }

    pub fn set_state(&self, state: AsyncTaskState) {
        let mut cur_state = self.state.lock();
        *cur_state = state;
    }
}

// async req function
pub fn async_ipi_req() {
    let ipi_list = ASYNC_IPI_TASK_LIST.lock();
    if ipi_list.is_empty() {
        panic!("ipi_list should not be empty");
    }
    let task = ipi_list.front().unwrap().clone();
    drop(ipi_list);
    if let AsyncTaskData::Ipi(msg) = task.task_data {
        if active_vm_id() == 0 {
            virtio_blk_notify_handler(msg.vq.clone(), msg.blk.clone(), vm(msg.src_id).unwrap());
        } else {
            // add_task_ipi_count();
            // send IPI to target cpu, and the target will invoke `mediated_ipi_handler`
            ipi_send_msg(0, IpiType::IpiTMediatedDev, IpiInnerMsg::MediatedMsg(msg));
        }
    }
}

pub fn async_blk_id_req() {}

// inject an interrupt to service VM
pub fn async_blk_io_req() {
    let io_list = ASYNC_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        panic!("io_list should not be empty");
    }
    let task = io_list.front().unwrap().clone();
    drop(io_list);
    if let AsyncTaskData::Io(msg) = task.task_data {
        match msg.io_type {
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
        }
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
    loop {
        if active_vm_id() == 0 || ASYNC_IPI_TASK_LIST.lock().len() < 1024 {
            break;
        } else {
            sleep(100);
        }
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
    match task.task_data {
        AsyncTaskData::Io(args) => {
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

            update_used_info(&args.vq, task.src_vmid);
            let src_vm = vm(task.src_vmid).unwrap();
            args.dev.notify(src_vm);
        }
        AsyncTaskData::Ipi(_) => {}
        AsyncTaskData::NoneTask(args) => {
            update_used_info(&args.vq, task.src_vmid);
            let src_vm = vm(task.src_vmid).unwrap();
            args.dev.notify(src_vm);
        }
    }
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
    // *io_list = io_list.drain_filter(|x| x.src_vmid == vm_id).collect::<LinkedList<_>>();
    io_list.remove(vm_id);
    *ipi_list = ipi_list
        .drain_filter(|x| x.src_vmid == vm_id)
        .collect::<LinkedList<_>>();
}
