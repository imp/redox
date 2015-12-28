use alloc::boxed::Box;
use collections::string::String;
use collections::vec::Vec;
use core::str;
use schemes::{Result, KScheme, Resource, Url};

pub struct DeviceId {
    vendor_id: u16,
    device_id: u16,
}

impl DeviceId {
    pub fn new(vid: u16, did: u16) -> Self {
        DeviceId { vendor_id: vid, device_id: did }
    }
}

pub struct Device {
    name: String,
    deviceid: DeviceId,
    attached: bool,
}

impl Device {
    pub fn new() -> Self {
        Device {
            name: "/",
            deviceid: DeviceId::new(0x0001, 0x0002),
            attached: false
        }
    }
    pub fn name(&self) -> &str { self.name; }
}

pub struct DeviceNode {
    device: Device,
    parent: *mut Device,
    children: Vec<*mut Device>
}

impl DeviceNode {
    pub fn new() -> Self {
        DeviceNode {}
    }

    pub fn pseudo() -> Self {
        DeviceNode {}
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
