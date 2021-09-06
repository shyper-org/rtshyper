use super::Vm;
use crate::arch::INTERRUPT_IRQ_IPI;
use crate::board::PLAT_DESC;
use crate::kernel::{cpu_id, interrupt_cpu_ipi_send, CPU_IF_LIST};
use alloc::vec::Vec;
use spin::Mutex;

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

#[derive(Copy, Clone)]
pub struct IpiPowerMessage {
    pub event: PowerEvent,
    pub entry: usize,
    pub context: usize,
}

#[derive(Copy, Clone, Debug)]
pub enum IpiType {
    IpiTIntc = 0,
    IpiTPower = 1,
    IpiTEthernetMsg = 2,
    IpiTEthernetAck = 3,
    IpiTHvc = 4,
}

#[derive(Copy, Clone)]
pub enum IpiInnerMsg {
    Initc(IpiInitcMessage),
    Power(IpiPowerMessage),
    None,
}

pub struct IpiMessage {
    pub ipi_type: IpiType,
    pub ipi_message: IpiInnerMsg,
}

const IPI_HANDLER_MAX: usize = 16;
pub type ipi_handler = fn(&IpiMessage);

pub struct IpiHandler {
    pub handler: ipi_handler,
    pub ipi_type: IpiType,
}

impl IpiHandler {
    fn new(handler: ipi_handler, ipi_type: IpiType) -> IpiHandler {
        IpiHandler { handler, ipi_type }
    }
}

static IPI_HANDLER_LIST: Mutex<Vec<IpiHandler>> = Mutex::new(Vec::new());

pub fn ipi_irq_handler() {
    // println!("Core[{}] ipi_irq_handler", cpu_id());
    let cpu_id = cpu_id();
    let mut cpu_if_list = CPU_IF_LIST.lock();
    let mut msg: Option<IpiMessage> = cpu_if_list[cpu_id].pop();
    drop(cpu_if_list);
    let ipi_handler_list = IPI_HANDLER_LIST.lock();

    while !msg.is_none() {
        let ipi_msg = msg.unwrap();
        let ipi_type = ipi_msg.ipi_type as usize;
        if ipi_handler_list.len() <= ipi_type {
            println!("illegal ipi type {}", ipi_type)
        } else {
            // println!("ipi type is {:#?}", ipi_msg.ipi_type);
            (ipi_handler_list[ipi_type].handler)(&ipi_msg);
        }
        let mut cpu_if_list = CPU_IF_LIST.lock();
        msg = cpu_if_list[cpu_id].pop();
    }
}

pub fn ipi_register(ipi_type: IpiType, handler: ipi_handler) -> bool {
    // check handler max
    let mut ipi_handler_list = IPI_HANDLER_LIST.lock();
    for i in 0..ipi_handler_list.len() {
        if ipi_type as usize == ipi_handler_list[i].ipi_type as usize {
            println!("ipi_register: try to cover exist ipi handler");
            return true;
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

    let mut cpu_if_list = CPU_IF_LIST.lock();
    cpu_if_list[target_id].msg_queue.push(msg);
    interrupt_cpu_ipi_send(target_id, INTERRUPT_IRQ_IPI);

    true
}

pub fn ipi_send_msg(target_id: usize, ipi_type: IpiType, ipi_message: IpiInnerMsg) -> bool {
    let msg = IpiMessage {
        ipi_type,
        ipi_message,
    };
    // if ipi_type as usize == 0 {
    //     match ipi_message {
    //         IpiInnerMsg::Initc(message) => {
    //             println!(
    //                 "Core[{}] send intc ipi to Core[{}], event {:#?}, int {}, val {}",
    //                 cpu_id(),
    //                 target_id,
    //                 message.event,
    //                 message.int_id,
    //                 message.val
    //             );
    //         }
    //         _ => {}
    //     }
    // }
    ipi_send(target_id, msg)
}

pub fn ipi_intra_broadcast_msg(vm: Vm, ipi_type: IpiType, msg: IpiInnerMsg) -> bool {
    let mut i = 0;
    let mut n = 0;
    while n < (vm.cpu_num() - 1) {
        if ((1 << i) & vm.ncpu()) != 0 && i != cpu_id() {
            n += 1;
            if !ipi_send_msg(i, ipi_type, msg) {
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
