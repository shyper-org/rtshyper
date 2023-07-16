use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use spin::Mutex;

use super::VirtioMmio;

static MAC2NIC_INFO: Mutex<BTreeMap<MacAddress, Arc<VirtioMmio>>> = Mutex::new(BTreeMap::new());

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct MacAddress([u8; 6]);

impl MacAddress {
    fn new(mac: &[u8]) -> Self {
        let mut this = Self([0; 6]);
        this.0.clone_from_slice(&mac[0..6]);
        this
    }
}

pub fn set_mac_info(mac: &[u8], nic: Arc<VirtioMmio>) {
    MAC2NIC_INFO.lock().insert(MacAddress::new(mac), nic);
}

pub fn mac_to_nic(mac: &[u8]) -> Option<Arc<VirtioMmio>> {
    MAC2NIC_INFO.lock().get(&MacAddress::new(mac)).cloned()
}

#[inline]
pub fn virtio_nic_list_walker<F>(mut f: F)
where
    F: FnMut(&Arc<VirtioMmio>),
{
    for nic in MAC2NIC_INFO.lock().values() {
        f(nic);
    }
}

pub fn remove_virtio_nic(vmid: usize) {
    MAC2NIC_INFO.lock().retain(|_mac, nic| {
        if let Some(vm) = nic.upper_vm() {
            vm.id() != vmid
        } else {
            false // if the vm is gone, the nic should be removed
        }
    });
}
