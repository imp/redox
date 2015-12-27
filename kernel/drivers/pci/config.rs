use drivers::io::{Io, Pio};
use super::common::config::*;

/// A PCI configuration
#[derive(Copy, Clone)]
pub struct PciConfig {
    bus: u8,
    slot: u8,
    func: u8,
    addr: Pio<u32>,
    data: Pio<u32>,
}

impl PciConfig {
    /// Create a new configuration
    pub fn new(bus: u8, slot: u8, func: u8) -> Self {
        PciConfig {
            bus: bus,
            slot: slot,
            func: func,
            addr: Pio::<u32>::new(0xCF8),
            data: Pio::<u32>::new(0xCFC),
        }
    }

    unsafe fn set_address(&mut self, offset: u8) -> &mut Self {
        let address = (self.bus as u32) << 16 | (self.slot as u32) << 11 |
                      (self.func as u32) << 8 | (offset as u32 & 0xFC);
        self.addr.write(PCI_CONFIG_ADDRESS_ENABLE | address);
        self
    }

    /// PCI Configuration Read
    pub unsafe fn read(&mut self, offset: u8) -> u32 {
        self.set_address(offset).data.read()
    }

    /// PCI Configuration Write
    pub unsafe fn write(&mut self, offset: u8, value: u32) {
        self.set_address(offset).data.write(value);
    }

    pub unsafe fn flag(&mut self, offset: u8, flag: u32, toggle: bool) {
        let mut value = self.read(offset);
        if toggle {
            value |= flag;
        } else {
            value &= 0xFFFFFFFF - flag;
        }
        self.write(offset, value);
    }

    // TODO: Write functions to get data structures
}
