use alloc::collections::BTreeMap;

use spin::Mutex;

use super::VirtioMmio;

static MAC2NIC_INFO: Mutex<BTreeMap<MacAddress, (usize, VirtioMmio)>> = Mutex::new(BTreeMap::new());

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct MacAddress([u8; 6]);

impl MacAddress {
    fn new(mac: &[u8]) -> Self {
        let mut this = Self([0; 6]);
        this.0.clone_from_slice(&mac[0..6]);
        this
    }
}

pub(super) fn set_mac_info(mac: &[u8], vmid: usize, nic: VirtioMmio) {
    MAC2NIC_INFO.lock().insert(MacAddress::new(mac), (vmid, nic));
}

pub(super) fn mac_to_vmid(mac: &[u8]) -> Option<usize> {
    MAC2NIC_INFO.lock().get(&MacAddress::new(mac)).map(|info| info.0)
}

pub fn remove_virtio_nic(vmid: usize) {
    MAC2NIC_INFO.lock().retain(|_mac, info| info.0 != vmid);
}
