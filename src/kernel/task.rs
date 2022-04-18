use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

use cortex_a::asm::ret;
use spin::Mutex;

use crate::config::vm_num;
use crate::device::{
    BLK_IRQ, BlkIov, mediated_blk_list_get, mediated_blk_read, mediated_blk_write, virtio_blk_notify_handler,
    VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT, Virtq,
};
use crate::kernel::{
    current_cpu, ipi_send_msg, IpiInnerMsg, IpiMediatedMsg, IpiMediatedNotifyMsg, IpiType, vm, vm_if_list_get_cpu_id,
};
use crate::kernel::interrupt_vm_inject;
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
                    // println!("Core0 task ipi handler, virtio_blk_notify_handler");
                    virtio_blk_notify_handler(msg.vq.clone(), msg.blk.clone(), vm(msg.src_id).unwrap());
                } else {
                    // println!("mediated task ipi send msg");
                    ipi_send_msg(0, IpiType::IpiTMediatedDev, IpiInnerMsg::MediatedMsg(msg.clone()));
                }
            }
            Task::MediatedIoTask(msg) => match msg.io_type {
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
            },
        }
    }
}

static MEDIATED_USED_INFO_LIST: Mutex<BTreeMap<usize, Vec<UsedInfo>>> = Mutex::new(BTreeMap::new());
static MEDIATED_IPI_TASK_LIST: Mutex<Vec<Task>> = Mutex::new(Vec::new());
static MEDIATED_IO_TASK_LIST: Mutex<Vec<Task>> = Mutex::new(Vec::new());

pub fn add_task(task: Task) {
    let mut ipi_list = MEDIATED_IPI_TASK_LIST.lock();
    let mut io_list = MEDIATED_IO_TASK_LIST.lock();

    match task {
        Task::MediatedIpiTask(_) => {
            ipi_list.push(task.clone());
            if ipi_list.len() > 1 {
                println!("add ipi task, len {}", ipi_list.len());
            }
            if ipi_list.len() == 1 && io_list.is_empty() {
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

    let task_finish = if ipi { ipi_list.remove(0) } else { io_list.remove(0) };

    let task_next = if !ipi_list.is_empty() {
        Some(ipi_list[0].clone())
    } else if !io_list.is_empty() {
        Some(io_list[0].clone())
    } else {
        None
    };
    // println!("finish {} task, ipi len {}, io len {}", if ipi { "ipi" } else { "io" }, ipi_list.len(), io_list.len());
    drop(ipi_list);
    drop(io_list);

    if let Task::MediatedIoTask(task_msg) = task_finish {
        if last_vm_io_task(task_msg.src_vmid) {
            let target_id = vm_if_list_get_cpu_id(task_msg.src_vmid);
            handle_used_info(task_msg.vq.clone(), task_msg.src_vmid);
            if target_id != current_cpu().id {
                // println!("ipi inject blk irq to vm {}", task_msg.src_vmid);
                let msg = IpiMediatedNotifyMsg {
                    vm_id: task_msg.src_vmid,
                };
                ipi_send_msg(
                    target_id,
                    IpiType::IpiTMediatedNotify,
                    IpiInnerMsg::MediatedNotifyMsg(msg),
                );
            } else {
                // println!("inject blk irq to vm {}", task_msg.src_vmid);
                let vm = vm(task_msg.src_vmid).unwrap();
                interrupt_vm_inject(vm.clone(), vm.vcpu(0).unwrap(), BLK_IRQ, 0);
            }
        }
    }

    if let Some(task) = task_next {
        task.handler();
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

pub fn last_vm_io_task(vm_id: usize) -> bool {
    let io_list = MEDIATED_IO_TASK_LIST.lock();
    for io_task in &*io_list {
        match io_task {
            Task::MediatedIoTask(task) => {
                if task.src_vmid == vm_id {
                    return false;
                }
            }
            Task::MediatedIpiTask(_) => {
                panic!("last_vm_io_task: illegal io task type");
            }
        }
    }
    true
}

pub fn push_used_info(desc_chain_head_idx: u32, used_len: u32, src_vmid: usize) {
    let mut used_info_list = MEDIATED_USED_INFO_LIST.lock();
    match used_info_list.get_mut(&src_vmid) {
        Some(info_list) => {
            info_list.push(UsedInfo {
                desc_chain_head_idx,
                used_len,
            });
        }
        None => {
            println!("sync_push_used_info: src_vmid {} not existed", src_vmid);
        }
    }
}

pub fn handle_used_info(vq: Virtq, src_vmid: usize) {
    let mut used_info_list = MEDIATED_USED_INFO_LIST.lock();
    if src_vmid >= used_info_list.len() {
        println!(
            "sync_handle_used_info: src_vmid {} larger than list size {}",
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

pub fn init_mediated_used_info() {
    let vm_num = vm_num();
    let mut used_info_list = MEDIATED_USED_INFO_LIST.lock();
    used_info_list.clear();

    for _ in 0..vm_num {
        used_info_list.push(Vec::new());
    }
}

pub fn merge_io_task(des_task: Task, src_task: Task) -> Option<Task> {
    if let Task::MediatedIoTask(io_task_src) = src_task {
        if let Task::MediatedIoTask(io_task_des) = des_task {
            let des_vm = vm(io_task_des.src_vmid).unwrap();
            let mediated_blk = mediated_blk_list_get(des_vm.med_blk_id());

            if io_task_des.src_vmid == io_task_src.src_vmid
                && io_task_des.sector + io_task_des.count == io_task_src.sector
                && io_task_des.count + io_task_src.count < mediated_blk.dma_block_max()
                && io_task_des.io_type == io_task_src.io_type
                && io_task_des.blk_id == io_task_src.blk_id
            {
                let mut iov_list = Vec::new();
                for iov in &*io_task_des.iov_list {
                    iov_list.push(iov.clone());
                }
                for iov in &*io_task_src.iov_list {
                    iov_list.push(iov.clone());
                }
                return Some(Task::MediatedIoTask(IoMediatedMsg {
                    src_vmid: io_task_des.src_vmid,
                    vq: io_task_des.vq.clone(),
                    io_type: io_task_des.io_type,
                    blk_id: io_task_des.blk_id,
                    sector: io_task_des.sector,
                    count: io_task_des.count + io_task_src.count,
                    cache: io_task_des.cache,
                    iov_list: Arc::new(iov_list),
                }));
            }
        } else {
            panic!("merge_io_task: des task is not an io task");
        }
    } else {
        panic!("merge_io_task: src task is not an io task");
    }
    return None;
}
