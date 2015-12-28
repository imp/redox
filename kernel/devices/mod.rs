use alloc::boxed::Box;
use collections::string::String;
use collections::vec::Vec;
use core::str;
use core::ptr::null;
use schemes::{Result, KScheme, Resource, Url};

pub struct DeviceId {
    vendor_id: u16,
    device_id: u16,
}

impl DeviceId {
    pub fn new(vid: u16, did: u16) -> Self {
        DeviceId { vendor_id: vid, device_id: did }
    }
    pub fn name(&self) -> str {
        format!("pci{vid:x},{did:x}", vid=self.vendor_id, did=self.device_id);
    }
}

pub struct Device {
    name: String,
    deviceid: DeviceId,
    attached: bool,
}

impl Device {
    pub fn root() -> Self {
        Device {
            name: String::from("/"),
            deviceid: DeviceId::new(0x0001, 0x0002),
            attached: false,
        }
    }

    pub fn pseudo() -> Self {
        Device {
            name: String::from("pseudo"),
            deviceid: DeviceId::new(0x0000, 0x0000),
            attached: false,
        }
    }
    pub fn name(&self) -> &str { self.name; }
}

pub struct DeviceNode {
    device: Device,
    parent: &DeviceNode,
    children: Vec<*const Device>
}

impl DeviceNode {
    pub fn new() -> Self {
        DeviceNode {
            device: &Device::root(),
            parent: null(),
            children: vec![],
        }
    }

    pub fn pseudo() -> Self {
        DeviceNode {
            device: &Device::pseudo(),
            parent: null(),
            children: vec![],
        }
    }

    pub fn add_child(&mut self, devnode: DeviceNode) { self.children.push(devnode) }
}

pub struct DeviceManager {
    root: DeviceNode,
    // TODO Replace with something faster like map of some sort
    devices: Vec<*const DeviceNode>
}

impl DeviceManager {
    pub fn new() -> Self {
        let root = DeviceNode::new();
        DeviceManager {
            root: root
        }
    }
    pub fn register(&self, device: Device) {}
}

impl KScheme for DeviceManager {
    fn scheme(&self) -> &str { "devices" }

    fn open(&mut self, url: &Url, flags: usize) -> Result<Box<Resource>> {
        if url.reference() == "/" {
            debugln!("Opening 'devices:'");
        }
    }
}
