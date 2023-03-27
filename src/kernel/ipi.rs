use alloc::vec::Vec;
use alloc::collections::LinkedList;

use spin::Mutex;
use spin::RwLock;

use crate::arch::INTERRUPT_IRQ_IPI;
use crate::board::PLAT_DESC;
use crate::board::PLATFORM_CPU_NUM_MAX;
use crate::device::{VirtioMmio, Virtq};
use crate::kernel::{current_cpu, interrupt_cpu_ipi_send};
use crate::vmm::VmmEvent;

use super::Vm;

#[derive(Copy, Clone, Debug)]
pub enum InitcEvent {
    VgicdGichEn,
    VgicdSetEn,
    VgicdSetAct,
    VgicdSetPend,
    VgicdSetPrio,
    VgicdSetTrgt,
    VgicdSetCfg,
    VgicdRoute,
    None,
}

#[derive(Copy, Clone)]
pub enum PowerEvent {
    PsciIpiCpuOn,
    PsciIpiCpuOff,
    PsciIpiCpuReset,
}

#[derive(Copy, Clone)]
pub struct IpiInitcMessage {
    pub event: InitcEvent,
    pub vm_id: usize,
    pub int_id: u16,
    pub val: u8,
}

/*
* src: src vm id
*/
#[derive(Copy, Clone)]
pub struct IpiPowerMessage {
    pub src: usize,
    pub event: PowerEvent,
    pub entry: usize,
    pub context: usize,
}

// #[derive(Copy, Clone)]
// pub struct IpiEthernetAckMsg {
//     pub len: usize,
//     pub succeed: bool,
// }

#[derive(Copy, Clone)]
pub struct IpiEthernetMsg {
    pub src_vmid: usize,
    pub trgt_vmid: usize,
}

#[derive(Copy, Clone)]
pub struct IpiVmmMsg {
    pub vmid: usize,
    pub event: VmmEvent,
}

// only support for mediated blk
#[derive(Clone)]
pub struct IpiMediatedMsg {
    pub src_id: usize,
    pub vq: Virtq,
    pub blk: VirtioMmio,
    // pub avail_idx: u16,
}

#[derive(Clone, Copy)]
pub struct IpiMediatedNotifyMsg {
    pub vm_id: usize,
}

#[derive(Clone, Copy)]
pub struct IpiHvcMsg {
    pub src_vmid: usize,
    pub trgt_vmid: usize,
    pub fid: usize,
    pub event: usize,
}

#[derive(Clone, Copy)]
pub struct IpiIntInjectMsg {
    pub vm_id: usize,
    pub int_id: usize,
}

#[derive(Copy, Clone, Debug)]
pub enum IpiType {
    IpiTIntc = 0,
    IpiTPower = 1,
    IpiTEthernetMsg = 2,
    #[deprecated]
    IpiTHyperFresh = 3,
    IpiTHvc = 4,
    IpiTVMM = 5,
    IpiTMediatedDev = 6,
    IpiTIntInject = 8,
}

#[derive(Clone)]
pub enum IpiInnerMsg {
    Initc(IpiInitcMessage),
    Power(IpiPowerMessage),
    EnternetMsg(IpiEthernetMsg),
    VmmMsg(IpiVmmMsg),
    MediatedMsg(IpiMediatedMsg),
    MediatedNotifyMsg(IpiMediatedNotifyMsg),
    HvcMsg(IpiHvcMsg),
    IntInjectMsg(IpiIntInjectMsg),
    HyperFreshMsg(),
    None,
}

pub struct IpiMessage {
    pub ipi_type: IpiType,
    pub ipi_message: IpiInnerMsg,
}

const IPI_HANDLER_MAX: usize = 16;

pub type IpiHandlerFunc = fn(&IpiMessage);

pub struct IpiHandler {
    pub handler: IpiHandlerFunc,
    pub ipi_type: IpiType,
}

impl IpiHandler {
    fn new(handler: IpiHandlerFunc, ipi_type: IpiType) -> IpiHandler {
        IpiHandler { handler, ipi_type }
    }
}

static IPI_HANDLER_LIST: RwLock<Vec<IpiHandler>> = RwLock::new(Vec::new());

struct CpuIf {
    msg_queue: LinkedList<IpiMessage>,
}

impl CpuIf {
    const fn default() -> Self {
        Self {
            msg_queue: LinkedList::new(),
        }
    }

    fn push(&mut self, ipi_msg: IpiMessage) {
        self.msg_queue.push_back(ipi_msg);
    }

    fn pop(&mut self) -> Option<IpiMessage> {
        self.msg_queue.pop_front()
    }
}

static CPU_IF_LIST: [Mutex<CpuIf>; PLATFORM_CPU_NUM_MAX] =
    [const { Mutex::new(CpuIf::default()) }; PLATFORM_CPU_NUM_MAX];

fn ipi_pop_message(cpu_id: usize) -> Option<IpiMessage> {
    let mut cpu_if = CPU_IF_LIST[cpu_id].lock();
    let msg = cpu_if.pop();
    // drop the lock manully
    drop(cpu_if);
    msg
}

pub fn ipi_irq_handler() {
    let cpu_id = current_cpu().id;

    while let Some(ipi_msg) = ipi_pop_message(cpu_id) {
        let ipi_type = ipi_msg.ipi_type as usize;

        let ipi_handler_list = IPI_HANDLER_LIST.read();
        let len = ipi_handler_list.len();
        let handler = ipi_handler_list[ipi_type].handler;
        drop(ipi_handler_list);

        // drop all locks before handler
        if len <= ipi_type {
            println!("illegal ipi type {}", ipi_type)
        } else {
            // println!("ipi type is {:#?}", ipi_msg.ipi_type);
            handler(&ipi_msg);
        }
    }
}

pub fn ipi_register(ipi_type: IpiType, handler: IpiHandlerFunc) -> bool {
    // check handler max
    let mut ipi_handler_list = IPI_HANDLER_LIST.write();
    for i in 0..ipi_handler_list.len() {
        if ipi_type as usize == ipi_handler_list[i].ipi_type as usize {
            println!("ipi_register: try to cover exist ipi handler");
            return false;
        }
    }

    while (ipi_type as usize) >= ipi_handler_list.len() {
        ipi_handler_list.push(IpiHandler::new(handler, ipi_type));
    }
    ipi_handler_list[ipi_type as usize] = IpiHandler::new(handler, ipi_type);
    // ipi_handler_list.push(IpiHandler::new(handler, ipi_type));
    true
}

fn ipi_send(target_id: usize, msg: IpiMessage) -> bool {
    if target_id >= PLAT_DESC.cpu_desc.num {
        println!("ipi_send: core {} not exist", target_id);
        return false;
    }

    CPU_IF_LIST[target_id].lock().push(msg);
    interrupt_cpu_ipi_send(target_id, INTERRUPT_IRQ_IPI);

    true
}

pub fn ipi_send_msg(target_id: usize, ipi_type: IpiType, ipi_message: IpiInnerMsg) -> bool {
    let msg = IpiMessage { ipi_type, ipi_message };
    ipi_send(target_id, msg)
}

pub fn ipi_intra_broadcast_msg(vm: Vm, ipi_type: IpiType, msg: IpiInnerMsg) -> bool {
    let mut i = 0;
    let mut n = 0;
    while n < (vm.cpu_num() - 1) {
        if ((1 << i) & vm.ncpu()) != 0 && i != current_cpu().id {
            n += 1;
            if !ipi_send_msg(i, ipi_type, msg.clone()) {
                println!(
                    "ipi_intra_broadcast_msg: Failed to send ipi request, cpu {} type {}",
                    i, ipi_type as usize
                );
                return false;
            }
        }

        i += 1;
    }
    true
}
