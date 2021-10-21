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
            _ => {}
        }
    }
}

static MEDIATED_IPI_TASK_LIST: Mutex<Vec<Task>> = Mutex::new(Vec::new());
static MEDIATED_IO_TASK_LIST: Mutex<Vec<Task>> = Mutex::new(Vec::new());

pub fn add_task(task: Task) {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();
    if ipi_list.is_empty() {
        ipi_list.push(task.clone());
        task.handler();
    } else {
        match task.task_type {
            TaskType::MediatedIpiTask(_) => {
                ipi_list.push(task);
            }
            TaskType::MediatedIoTask(_) => {
                io_list.push(task);
            }
            _ => { panic!("illegal task type"); }
        }
    }
}

pub fn finish_task() {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        ipi_list.remove(0);
        if !ipi_list.is_empty() {
            ipi_list[0].handler();
        }
    } else {
        io_list.remove(0);
        if !io_list.is_empty() {
            io_list[0].handler();
        } else {
            let target_id = vm_if_list_get_cpu_id(1);
            ipi_send_msg(target_id, IpiType::IpiTMediatedNotify, IpiInnerMsg::None);

            ipi_list.remove(0);
            if !ipi_list.is_empty() {
                ipi_list[0].handler();
            }
        }
    }
}

pub fn io_task_head() -> Task {
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();
    io_list[0].clone()
}