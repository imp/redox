use core::intrinsics::{volatile_load, volatile_store};
use common::debug;
use drivers::pio::*;
use drivers::mmio::*;
use super::common::config::*;
use super::common::command::*;

/// Generic PCI device

/// BAR access structure
enum BarAccess {
    IO,
    MEMORY,
}

enum Base {
    x32BIT {base: u32, size: u32},
    x64BIT {base: u64, size: u64},
}

pub struct Bar {
    access: BarAccess,
    base32: u32,
    length: u32,
}

impl Bar {
    pub fn get8(&self, offset: u32) -> u8 {
        assert!(offset < self.length);
        let addr = self.base32 + offset;
        unsafe {
            match self.access {
                BarAccess::IO => Pio::<u8>::new(addr as u16).read(),
                BarAccess::MEMORY => (&mut *(addr as *mut Mmio<u8>)).read(),
            }
        }
    }

    pub fn get16(&self, offset: u32) -> u16 {
        assert!(offset < self.length);
        let addr = self.base32 + offset;
        unsafe {
            match self.access {
                BarAccess::IO => Pio::<u16>::new(addr as u16).read(),
                BarAccess::MEMORY => (&mut *(addr as *mut Mmio<u16>)).read(),
            }
        }
    }

    pub fn get32(&self, offset: u32) -> u32 {
        assert!(offset < self.length);
        let addr = self.base32 + offset;
        unsafe {
            match self.access {
                BarAccess::IO => Pio::<u32>::new(addr as u16).read(),
                BarAccess::MEMORY => (&mut *(addr as *mut Mmio<u32>)).read(),
            }
        }
    }

    pub fn get64(&self, offset: u32) -> u64 {
        assert!(offset < self.length);
        let addr = self.base32 + offset;
        unsafe {
            match self.access {
                BarAccess::IO => panic!("No 64 bit PIO"),
                BarAccess::MEMORY => (&mut *(addr as *mut Mmio<u64>)).read(),
            }
        }
    }
}

#[derive(Debug, Default)]
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
    bar: [usize; 6],
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
        dev.parse_config();
        dev
    }

    fn set_config_address(&self, offset: u8) {
        let address = PCI_CONFIG_ADDRESS_ENABLE |
                      (self.bus as u32) << PCI_BUS_OFFSET |
                      (self.slot as u32) << PCI_SLOT_OFFSET |
                      (self.func as u32) << PCI_FUNC_OFFSET |
                      (offset as u32 & 0xFC);
        Pio::<u32>::new(PCI_CONFIG_ADDRESS).write(address);
    }

    /// Read 8 bit value from the given offset of PCI Configuration Space
    fn config_get8(&self, offset: u8) -> u8 {
        self.set_config_address(offset);
        Pio::<u8>::new(PCI_CONFIG_DATA + (offset & 0x03) as u16).read()
    }

    /// Read 16 bit value from the given offset of PCI Configuration Space
    fn config_get16(&self, offset: u8) -> u16 {
        self.set_config_address(offset);
        Pio::<u16>::new(PCI_CONFIG_DATA + (offset & 0x02) as u16).read()
    }

    /// Read 32 bit value from the given offset of PCI Configuration Space
    fn config_get32(&self, offset: u8) -> u32 {
        self.set_config_address(offset);
        Pio::<u32>::new(PCI_CONFIG_DATA).read()
    }

    /// Write 8 bit value at the given offset of PCI Configuration Space
    fn config_put8(&self, offset: u8, value: u8) {
        self.set_config_address(offset);
        Pio::<u8>::new(PCI_CONFIG_DATA + offset as u16 & 0x03).write(value);
    }

    /// Write 16 bit value at the given offset of PCI Configuration Space
    fn config_put16(&self, offset: u8, value: u16) {
        self.set_config_address(offset);
        Pio::<u16>::new(PCI_CONFIG_DATA + offset as u16 & 0x02).write(value);
    }

    /// Write 32 bit value at the given offset of PCI Configuration Space
    fn config_put32(&self, offset: u8, value: u32) {
        self.set_config_address(offset);
        Pio::<u32>::new(PCI_CONFIG_DATA).write(value);
    }

    fn parse_config(&mut self) {
        self.vendor = self.config_get16(PCI_CFG_VENDOR_ID);
        self.device = self.config_get16(PCI_CFG_DEVICE_ID);
        self.revision = self.config_get8(PCI_CFG_REVISION_ID);
        self.progif = self.config_get8(PCI_CFG_PROG_INTERFACE);
        self.subclass = self.config_get8(PCI_CFG_SUBCLASS);
        self.class = self.config_get8(PCI_CFG_BASECLASS);
        self.subvendor = self.config_get16(PCI_CFG_SUBSYSTEM_VENDOR_ID);
        self.subsystem = self.config_get16(PCI_CFG_SUBSYSTEM_ID);

        self.set_command(IO_SPACE_ENABLE | MEMORY_SPACE_ENABLE);
    }

    pub fn get_vendor(&self) -> u16 {
        self.vendor
    }
    pub fn get_device(&self) -> u16 {
        self.device
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

    pub fn get_status(&self) -> u16 {
        self.config_get16(PCI_CFG_STATUS)
    }

    /// Set specified bits in the PCI COMMAND register
    pub fn set_command(&self, value: u16) {
        let mut cmd = self.config_get16(PCI_CFG_COMMAND);
        cmd |= value;
        self.config_put16(PCI_CFG_COMMAND, cmd);
    }

    /// Clear specified bits in the PCI COMMAND register
    pub fn clear_command(&self, value: u16) {
        let mut cmd = self.config_get16(PCI_CFG_COMMAND);
        cmd &= !value;
        self.config_put16(PCI_CFG_COMMAND, cmd);
    }

    /// Enable INTx for this device
    pub fn enable_intx(&self) {
        self.clear_command(INTX_DISABLE);
    }

    /// Disable INTx for this device
    pub fn disable_intx(&self) {
        self.set_command(INTX_DISABLE);
    }

    /// Report this device to the console
    pub fn report(&self) {
        debug!("PCI [{:X}:{:X}:{:X}] {:X}:{:X}:{:X}",
               self.bus,
               self.slot,
               self.func,
               self.vendor,
               self.device,
               self.revision);
        debug::dl();
    }
}

pub fn Device(bus: u8, slot: u8, func: u8) -> Function {
    Function::new(bus as u8, slot as u8, func as u8)
}
