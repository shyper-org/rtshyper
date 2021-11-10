use crate::device::{
    mediated_blk_read, mediated_blk_write, Virtq, BlkIov, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT,
};
use crate::kernel::{ipi_send_msg, vm_if_list_get_cpu_id, IpiInnerMsg, IpiMediatedMsg, IpiType};
use crate::lib::memcpy;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Clone)]
pub enum TaskType {
    MediatedIpiTask(IpiMediatedMsg),
    MediatedIoTask(IoMediatedMsg),
}

pub struct UsedInfo {
    pub desc_chain_head_idx: u32,
    pub used_len: u32,
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
                // todo: if cpu_id == target_id (0)
                ipi_send_msg(0, IpiType::IpiTMediatedDev, IpiInnerMsg::MediatedMsg(msg));
            }
            TaskType::MediatedIoTask(msg) => match msg.io_type {
                VIRTIO_BLK_T_IN => {
                    // println!(
                    //     "blk read {} sector {} count {}",
                    //     msg.blk_id, msg.sector, msg.count
                    // );
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
            },
        }
    }
}

static MEDIATED_USED_INFO_LIST: Mutex<Vec<UsedInfo>> = Mutex::new(Vec::new());
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

pub fn finish_task(check: bool) {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();
    if check && !io_list.is_empty() {
        panic!("finish_task: io list should be empty");
    }

    // if io_list.is_empty() {
    //     ipi_list.remove(0);
    //     if !ipi_list.is_empty() {
    //         let task = ipi_list[0].clone();
    //         drop(ipi_list);
    //         drop(io_list);
    //         task.handler();
    //     }
    // } else {
    if !io_list.is_empty() {
        io_list.remove(0);
    }
    if !io_list.is_empty() {
        let task = io_list[0].clone();
        drop(ipi_list);
        drop(io_list);
        // println!("finish io task, io len {}", io_list.len());
        task.handler();
    } else {
        match ipi_list.remove(0).task_type {
            TaskType::MediatedIpiTask(task) => {
                if ipi_list.is_empty() {
                    handle_used_info(task.vq.clone());
                    // println!("notify");
                    let target_id = vm_if_list_get_cpu_id(task.src_id);
                    ipi_send_msg(target_id, IpiType::IpiTMediatedNotify, IpiInnerMsg::None);
                }
            }
            TaskType::MediatedIoTask(_) => {
                panic!("illegal ipi task");
            }
        }

        // println!(
        //     "finish last io task, and ipi task, ipi len {}",
        //     ipi_list.len()
        // );
        // println!("finish io and ipi task, io len {}, ipi len {}", 0, ipi_list.len());
        if !ipi_list.is_empty() {
            let task = ipi_list[0].clone();
            drop(ipi_list);
            drop(io_list);
            task.handler();
        }
    }
    // }
}

// pub fn finish_ipi_task() {
//     let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
//     let mut io_list = MEDIATED_IO_TASK_LIST.lock();
//     if io_list.len() != 0 {
//         panic!("illegle finish ipi task, len {}", io_list.len());
//     }
//     io_list.clear();
//     let task = ipi_list.remove(0);
//     // println!("finish ipi task, ipi len {}", ipi_list.len());
//     if !ipi_list.is_empty() {
//         ipi_list[0].handler();
//     } else {
//         // println!("notify");
//         if let TaskType::MediatedIpiTask(ipi_task) = task.task_type {
//             let target_id = vm_if_list_get_cpu_id(ipi_task.src_id);
//             // if ipi_task.notify {
//             ipi_send_msg(target_id, IpiType::IpiTMediatedNotify, IpiInnerMsg::None);
//             // }
//         }
//     }
// }

pub fn io_task_head() -> Task {
    let io_list = MEDIATED_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        panic!("io task list is empty");
    }
    io_list[0].clone()
}

// pub fn set_ipi_notify(idx: usize, notify: bool) {
//     let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
//     if ipi_list.len() <= idx {
//         panic!("set_ipi_notify: illegal idx for ipi task list");
//     }
//     match &mut ipi_list[idx].task_type {
//         TaskType::MediatedIpiTask(task) => {
//             task.notify = notify;
//         }
//         TaskType::MediatedIoTask(_) => {
//             panic!("illegal ipi task");
//         }
//     }
// }

pub fn io_list_len() -> usize {
    let io_list = MEDIATED_IO_TASK_LIST.lock();
    io_list.len()
}

pub fn ipi_list_len() -> usize {
    let ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    ipi_list.len()
}

pub fn push_used_info(desc_chain_head_idx: u32, used_len: u32) {
    let mut used_info_list = MEDIATED_USED_INFO_LIST.lock();
    used_info_list.push(UsedInfo {
        desc_chain_head_idx,
        used_len,
    });
}

pub fn handle_used_info(vq: Virtq) {
    let mut used_info_list = MEDIATED_USED_INFO_LIST.lock();
    let vq_size = vq.num();
    for info in &*used_info_list {
        if info.used_len == 0 {
            println!("used_len {}, chain_head_idx {}", info.used_len, info.desc_chain_head_idx);
        }
        vq.update_used_ring(info.used_len, info.desc_chain_head_idx, vq_size);
    }
    used_info_list.clear();
}

// pub fn set_used_len(len: u32, idx: usize) {
//     let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
//     if ipi_list.len() <= idx {
//         panic!("set_ipi_notify: illegal idx for ipi task list");
//     }
//     match &mut ipi_list[idx].task_type {
//         TaskType::MediatedIpiTask(task) => {
//             task.notify = notify;
//         }
//         TaskType::MediatedIoTask(_) => {
//             panic!("illegal ipi task");
//         }
//     }
// }