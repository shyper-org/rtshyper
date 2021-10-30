use crate::kernel::{IpiMediatedMsg, IpiType, IpiInnerMsg, ipi_send_msg, vm_if_list_get_cpu_id};
use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;
use crate::device::{BlkIov, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT, mediated_blk_read, mediated_blk_write};
use crate::lib::memcpy;

#[derive(Clone)]
pub enum TaskType {
    MediatedIpiTask(IpiMediatedMsg),
    MediatedIoTask(IoMediatedMsg),
}

#[derive(Clone)]
pub struct IoMediatedMsg {
    pub io_type: usize,
    pub blk_id: usize,
    pub sector: usize,
    pub count: usize,
    pub cache: usize,
    pub iov_list: Arc<Vec<BlkIov>>,
}

#[derive(Clone)]
pub struct Task {
    pub task_type: TaskType,
}

impl Task {
    pub fn handler(&self) {
        match self.task_type.clone() {
            TaskType::MediatedIpiTask(msg) => {
                ipi_send_msg(0, IpiType::IpiTMediatedDev, IpiInnerMsg::MediatedMsg(msg));
            }
            TaskType::MediatedIoTask(msg) => {
                match msg.io_type {
                    VIRTIO_BLK_T_IN => {
                        mediated_blk_read(msg.blk_id, msg.sector, msg.count);
                    }
                    VIRTIO_BLK_T_OUT => {
                        let mut cache_ptr = msg.cache;
                        for idx in 0..msg.iov_list.len() {
                            let data_bg = msg.iov_list[idx].data_bg;
                            let len = msg.iov_list[idx].len as usize;
                            unsafe {
                                memcpy(cache_ptr as *mut u8, data_bg as *mut u8, len);
                            }
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
    }
}

static MEDIATED_IPI_TASK_LIST: Mutex<Vec<Task>> = Mutex::new(Vec::new());
static MEDIATED_IO_TASK_LIST: Mutex<Vec<Task>> = Mutex::new(Vec::new());

pub fn add_task(task: Task) {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();

    match task.task_type {
        TaskType::MediatedIpiTask(_) => {
            ipi_list.push(task.clone());
            // println!("add ipi task, len {}", ipi_list.len());
            if ipi_list.len() == 1 && io_list.is_empty() {
                drop(ipi_list);
                drop(io_list);
                task.handler();
            }
        }
        TaskType::MediatedIoTask(_) => {
            io_list.push(task.clone());
            // println!("add io task, len {}", io_list.len());
            if io_list.len() == 1 {
                drop(ipi_list);
                drop(io_list);
                task.handler();
            }
        }
    }
}

pub fn finish_task() {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        ipi_list.remove(0);
        // println!("finish ipi task, ipi len {}", ipi_list.len());
        if !ipi_list.is_empty() {
            let task = ipi_list[0].clone();
            drop(ipi_list);
            drop(io_list);
            task.handler();
        }
    } else {
        io_list.remove(0);
        if !io_list.is_empty() {
            let task = io_list[0].clone();
            drop(ipi_list);
            drop(io_list);
            // println!("finish io task, io len {}", io_list.len());
            task.handler();
        } else {
            match &ipi_list[0].task_type {
                TaskType::MediatedIpiTask(task) => {
                    if task.vq.avail_flags() == 0 {
                        let target_id = vm_if_list_get_cpu_id(task.src_id);
                        ipi_send_msg(target_id, IpiType::IpiTMediatedNotify, IpiInnerMsg::None);
                    }
                }
                TaskType::MediatedIoTask(_) => { panic!("illegal ipi task"); }
            }

            ipi_list.remove(0);
            // println!("finish io and ipi task, io len {}, ipi len {}", 0, ipi_list.len());
            if !ipi_list.is_empty() {
                let task = ipi_list[0].clone();
                drop(ipi_list);
                drop(io_list);
                task.handler();
            }
        }
    }
}

pub fn finish_ipi_task() {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();
    io_list.clear();
    ipi_list.remove(0);
    // println!("finish ipi task, ipi len {}", ipi_list.len());
    if !ipi_list.is_empty() {
        ipi_list[0].handler();
    }
}

pub fn io_task_head() -> Task {
    let io_list = MEDIATED_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        panic!("io task list is empty");
    }
    io_list[0].clone()
}