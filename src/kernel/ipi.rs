use alloc::vec::Vec;
use spin::Mutex;

struct IpiInitcMessage {
    event: u32,
    vm_id: usize,
    int_id: u16,
    val: u8,
}

#[derive(Copy, Clone)]
pub enum IpiType {
    IpiTIntc = 0,
    IpiTPower = 1,
    IpiTEthernetMsg = 2,
    IpiTEthernetAck = 3,
    IpiTHvc = 4,
}

pub enum IpiInnerMsg {
    Initc(IpiInitcMessage),
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
    // TODO: ipi irq handler
}

pub fn ipi_register(ipi_type: IpiType, handler: ipi_handler) -> bool {
    // check handler max
    let mut ipi_handler_list = IPI_HANDLER_LIST.lock();
    for i in 0..ipi_handler_list.len() {
        if ipi_type as usize == ipi_handler_list[i].ipi_type as usize {
            println!("ipi_register: try to cover exist ipi handler");
            return false;
        }
    }

    ipi_handler_list.push(IpiHandler::new(handler, ipi_type));
    true
}
