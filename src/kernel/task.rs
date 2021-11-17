use alloc::sync::Arc;
use alloc::vec::Vec;

use cortex_a::asm::ret;
use spin::Mutex;

use crate::device::{
    BLK_IRQ, BlkIov, mediated_blk_list_get, mediated_blk_read, mediated_blk_write, virtio_blk_notify_handler, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT, Virtq,
};
use crate::kernel::{active_vm_id, current_cpu, interrupt_vm_inject, ipi_send_msg, IpiInnerMsg, IpiMediatedMsg, IpiType, vm, vm_if_list_get_cpu_id};
use crate::lib::memcpy_safe;

pub struct UsedInfo {
    pub desc_chain_head_idx: u32,
    pub used_len: u32,
}

#[derive(Clone)]
pub struct IoMediatedMsg {
    pub src_vmid: usize,
    pub vq: Virtq,
    pub io_type: usize,
    pub blk_id: usize,
    pub sector: usize,
    pub count: usize,
    pub cache: usize,
    pub iov_list: Arc<Vec<BlkIov>>,
}

// #[derive(Clone)]
// pub struct Task {
//     pub task_type: TaskType,
// }

#[derive(Clone)]
pub enum Task {
    MediatedIpiTask(IpiMediatedMsg),
    MediatedIoTask(IoMediatedMsg),
}

impl Task {
    pub fn handler(&self) {
        match self {
            Task::MediatedIpiTask(msg) => {
                if current_cpu().id == 0 {
                    virtio_blk_notify_handler(msg.vq.clone(), msg.blk.clone(), vm(msg.src_id));
                } else {
                    ipi_send_msg(0, IpiType::IpiTMediatedDev, IpiInnerMsg::MediatedMsg(msg.clone()));
                }
            }
            Task::MediatedIoTask(msg) => match msg.io_type {
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
        }
    }
}

static MEDIATED_USED_INFO_LIST: Mutex<Vec<UsedInfo>> = Mutex::new(Vec::new());
static MEDIATED_IPI_TASK_LIST: Mutex<Vec<Task>> = Mutex::new(Vec::new());
static MEDIATED_IO_TASK_LIST: Mutex<Vec<Task>> = Mutex::new(Vec::new());

pub fn add_task(task: Task) {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();

    match task {
        Task::MediatedIpiTask(_) => {
            ipi_list.push(task.clone());
            // println!("add ipi task, len {}", ipi_list.len());
            if current_cpu().id != 0 && ipi_list.len() == 1 && io_list.is_empty() {
                drop(ipi_list);
                drop(io_list);
                // println!("add and run ipi task");
                task.handler();
            }
        }
        Task::MediatedIoTask(_) => {
            let len = io_list.len();
            if len == 0 {
                io_list.push(task.clone());
            } else {
                let last_task = io_list[len - 1].clone();
                match merge_io_task(last_task.clone(), task.clone()) {
                    None => {
                        io_list.push(task.clone());
                    }
                    Some(new_task) => {
                        // println!("merge success");
                        io_list.pop();
                        io_list.push(new_task.clone());
                    }
                }
            }

            // io_list.push(task.clone());
            // println!("add io task, io list len {}", io_list.len());
            if io_list.len() == 1 && ipi_list.is_empty() {
                drop(ipi_list);
                drop(io_list);
                // println!("add and run io task");
                task.handler();
            }
        }
    }
}

pub fn finish_task(ipi: bool) {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();

    let task_finish = if ipi {
        ipi_list.remove(0)
    } else {
        io_list.remove(0)
    };
    // println!("finish {} task, ipi len {}, io len {}", if ipi { "ipi" } else { "io" }, ipi_list.len(), io_list.len());

    if !ipi_list.is_empty() {
        let task = ipi_list[0].clone();
        // println!("run ipi task, ipi len {}", ipi_list.len());
        drop(io_list);
        drop(ipi_list);
        task.handler();
    } else if !io_list.is_empty() {
        let task = io_list[0].clone();
        // println!("run io task, io len {}", io_list.len());
        drop(io_list);
        drop(ipi_list);
        task.handler();
    } else {
        match task_finish {
            Task::MediatedIoTask(task) => {
                let target_id = vm_if_list_get_cpu_id(task.src_vmid);
                // println!("notify target {}", target_id);
                handle_used_info(task.vq.clone());
                ipi_send_msg(target_id, IpiType::IpiTMediatedNotify, IpiInnerMsg::None);
            }
            Task::MediatedIpiTask(task) => {
                println!("do nothing");
            }
        }
    }
}

pub fn io_task_head() -> Option<Task> {
    let io_list = MEDIATED_IO_TASK_LIST.lock();
    if io_list.is_empty() {
        println!("io task list is empty");
        return None;
    }
    Some(io_list[0].clone())
}

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

pub fn merge_io_task(des_task: Task, src_task: Task) -> Option<Task> {
    let mediated_blk = mediated_blk_list_get(0);

    if let Task::MediatedIoTask(src_io_task) = src_task
    {
        if let Task::MediatedIoTask(des_io_task) = des_task {
            if des_io_task.src_vmid == src_io_task.src_vmid &&
                des_io_task.sector + des_io_task.count == src_io_task.sector &&
                des_io_task.count + src_io_task.count < mediated_blk.dma_block_max() &&
                des_io_task.io_type == src_io_task.io_type &&
                des_io_task.blk_id == src_io_task.blk_id {
                let mut iov_list = Vec::new();
                for iov in &*des_io_task.iov_list {
                    iov_list.push(iov.clone());
                }
                for iov in &*src_io_task.iov_list {
                    iov_list.push(iov.clone());
                }
                return Some(
                    Task::MediatedIoTask(IoMediatedMsg {
                        src_vmid: des_io_task.src_vmid,
                        vq: des_io_task.vq.clone(),
                        io_type: des_io_task.io_type,
                        blk_id: des_io_task.blk_id,
                        sector: des_io_task.sector,
                        count: des_io_task.count + src_io_task.count,
                        cache: des_io_task.cache,
                        iov_list: Arc::new(iov_list),
                    })
                );
            }
        } else {
            panic!("merge_io_task: des task is not an io task");
        }
    } else {
        panic!("merge_io_task: src task is not an io task");
    }
    return None;
}