use common::debug;
use drivers::pio::*;
use super::common::config::*;

/// Generic PCI device
pub struct Bar {
    number: u8,
}

pub struct Function {
    bus: u8,
    slot: u8,
    func: u8,
    vendor: u16,
    device: u16,
    revision: u8,
    class: u8,
    subclass: u8,
    progif: u8,
    bar: [u32; 6],
    subvendor: u16,
    subsystem: u16,
}

impl Function {
    pub fn new(bus: u8, slot: u8, func: u8) -> Self {
        let mut dev = Function {
            bus: bus,
            slot: slot,
            func: func,
            vendor: 0xFFFF,
            device: 0xFFFF,
            revision: 0,
            class: 0,
            subclass: 0,
            progif: 0,
            bar: [0, 0, 0, 0, 0, 0],
            subvendor: 0xFFFF,
            subsystem: 0xFFFF,
        };
        unsafe { dev.parse_config() }
        return dev;
    }

    unsafe fn set_config_address(&self, offset: u8) {
        let address = PCI_CONFIG_ADDRESS_ENABLE |
                      (self.bus as u32) << PCI_BUS_OFFSET |
                      (self.slot as u32) << PCI_SLOT_OFFSET |
                      (self.func as u32) << PCI_FUNC_OFFSET |
                      (offset as u32 & 0xFC);
        Pio32::new(PCI_CONFIG_ADDRESS).write(address);
    }

    unsafe fn config_get8(&self, offset: u8) -> u8 {
        self.set_config_address(offset);
        Pio8::new(PCI_CONFIG_DATA + offset as u16 & 0x03).read()
    }

    unsafe fn config_get16(&self, offset: u8) -> u16 {
        self.set_config_address(offset);
        Pio16::new(PCI_CONFIG_DATA + offset as u16 & 0x02).read()
    }

    unsafe fn config_get32(&self, offset: u8) -> u32 {
        self.set_config_address(offset);
        Pio32::new(PCI_CONFIG_DATA).read()
    }

    unsafe fn config_put8(&self, offset: u8, value: u8) {
        self.set_config_address(offset);
        Pio8::new(PCI_CONFIG_DATA + offset as u16 & 0x03).write(value);
    }

    unsafe fn config_put16(&self, offset: u8, value: u16) {
        self.set_config_address(offset);
        Pio16::new(PCI_CONFIG_DATA + offset as u16 & 0x02).write(value);
    }

    unsafe fn config_put32(&self, offset: u8, value: u32) {
        self.set_config_address(offset);
        Pio32::new(PCI_CONFIG_DATA).write(value);
    }

    unsafe fn parse_config(&mut self) {
        self.vendor = self.config_get16(PCI_CFG_VENDOR_ID);
        self.device = self.config_get16(PCI_CFG_DEVICE_ID);
        self.revision = self.config_get8(PCI_CFG_REVISION_ID);
        self.progif = self.config_get8(PCI_CFG_PROG_INTERFACE);
        self.subclass = self.config_get8(PCI_CFG_SUBCLASS);
        self.class = self.config_get8(PCI_CFG_BASECLASS);
        self.subvendor = self.config_get16(PCI_CFG_SUBSYSTEM_VENDOR_ID);
        self.subsystem = self.config_get16(PCI_CFG_SUBSYSTEM_ID);
    }

    pub fn get_vendor(&self) -> u16 {
        self.vendor
    }
    pub fn get_device(&self) -> u16 {
        self.vendor
    }
    pub fn get_subvendor(&self) -> u16 {
        self.subvendor
    }
    pub fn get_subsystem(&self) -> u16 {
        self.subsystem
    }
    pub fn get_revision(&self) -> u8 {
        self.revision
    }
    pub fn get_class(&self) -> u8 {
        self.class
    }
    pub fn get_subclass(&self) -> u8 {
        self.subclass
    }
    pub fn get_progif(&self) -> u8 {
        self.progif
    }

    pub fn report(&self) {
        debugln!("Found PCI [{:X}:{:X}:{:X}]", self.bus, self.slot, self.func);
    }
}

pub fn pci_enumerate_bus() {
    debugln!("PCI bus enumeration started");
    for bus in 0..256 {
        debugln!("Scanning PCI bus {:X}", bus);
        for slot in 0..32 {
            //debugln!("Scanning PCI bus {:X}:{:X}", bus, slot);
            for func in 0..8 {
                //debugln!("Probing PCI device [{:X}:{:X}:{:X}]", bus, slot, func);
                Function::new(bus, slot, func).report();
            }
        }
    }
    debugln!("PCI bus enumeration finished");
}
