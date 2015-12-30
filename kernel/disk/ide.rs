use alloc::arc::Arc;
use alloc::boxed::Box;

use collections::vec::Vec;
use collections::vec_deque::VecDeque;

use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use common::memory::Memory;

use disk::Disk;

use drivers::pciconfig::PciConfig;
use drivers::pio::*;

use schemes::Result;

use syscall::{SysError, EIO};

use sync::Intex;

/// An disk extent
#[derive(Copy, Clone)]
#[repr(packed)]
pub struct Extent {
    pub block: u64,
    pub length: u64,
}

impl Extent {
    pub fn empty(&self) -> bool {
        return self.block == 0 || self.length == 0;
    }
}

/// A disk request
pub struct Request {
    /// The disk extent
    pub extent: Extent,
    /// The memory location
    pub mem: usize,
    /// The request type
    pub read: bool,
    /// Completion indicator
    pub complete: Arc<AtomicBool>,
}

impl Clone for Request {
    fn clone(&self) -> Self {
        Request {
            extent: self.extent,
            mem: self.mem,
            read: self.read,
            complete: self.complete.clone(),
        }
    }
}

/// Direction of DMA, set if moving from disk to memory, not set if moving from memory to disk
const CMD_DIR: u8 = 8;
/// DMA should process PRDT
const CMD_ACT: u8 = 1;
/// DMA interrupt occured
const STS_INT: u8 = 4;
/// DMA error occured
const STS_ERR: u8 = 2;
/// DMA is processing PRDT
const STS_ACT: u8 = 1;

/// PRDT End of Table
const PRD_EOT: u16 = 0x8000;

/// Physical Region Descriptor
#[repr(packed)]
struct Prd {
    addr: u32,
    size: u16,
    eot: u16,
}

struct Prdt {
    reg: Pio32,
    mem: Memory<Prd>,
}

impl Prdt {
    fn new(port: u16) -> Option<Self> {
        if let Some(mem) = Memory::new_align(8192, 65536) {
            return Some(Prdt {
                reg: Pio32::new(port),
                mem: mem,
            });
        }

        None
    }
}

impl Drop for Prdt {
    fn drop(&mut self) {
        unsafe { self.reg.write(0) };
    }
}

// Status port bits
const ATA_SR_BSY: u8 = 0x80;
const ATA_SR_DRDY: u8 = 0x40;
const ATA_SR_DF: u8 = 0x20;
const ATA_SR_DSC: u8 = 0x10;
const ATA_SR_DRQ: u8 = 0x08;
const ATA_SR_CORR: u8 = 0x04;
const ATA_SR_IDX: u8 = 0x02;
const ATA_SR_ERR: u8 = 0x01;

// Error port bits
const ATA_ER_BBK: u8 = 0x80;
const ATA_ER_UNC: u8 = 0x40;
const ATA_ER_MC: u8 = 0x20;
const ATA_ER_IDNF: u8 = 0x10;
const ATA_ER_MCR: u8 = 0x08;
const ATA_ER_ABRT: u8 = 0x04;
const ATA_ER_TK0NF: u8 = 0x02;
const ATA_ER_AMNF: u8 = 0x01;

// Commands
const ATA_CMD_READ_PIO: u8 = 0x20;
const ATA_CMD_READ_PIO_EXT: u8 = 0x24;
const ATA_CMD_READ_DMA: u8 = 0xC8;
const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
const ATA_CMD_WRITE_PIO: u8 = 0x30;
const ATA_CMD_WRITE_PIO_EXT: u8 = 0x34;
const ATA_CMD_WRITE_DMA: u8 = 0xCA;
const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
const ATA_CMD_CACHE_FLUSH: u8 = 0xE7;
const ATA_CMD_CACHE_FLUSH_EXT: u8 = 0xEA;
const ATA_CMD_PACKET: u8 = 0xA0;
const ATA_CMD_IDENTIFY_PACKET: u8 = 0xA1;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

// Identification
const ATA_IDENT_DEVICETYPE: u8 = 0;
const ATA_IDENT_CYLINDERS: u8 = 2;
const ATA_IDENT_HEADS: u8 = 6;
const ATA_IDENT_SECTORS: u8 = 12;
const ATA_IDENT_SERIAL: u8 = 20;
const ATA_IDENT_MODEL: u8 = 54;
const ATA_IDENT_CAPABILITIES: u8 = 98;
const ATA_IDENT_FIELDVALID: u8 = 106;
const ATA_IDENT_MAX_LBA: u8 = 120;
const ATA_IDENT_COMMANDSETS: u8 = 164;
const ATA_IDENT_MAX_LBA_EXT: u8 = 200;

// Selection
const ATA_MASTER: u8 = 0x00;
const ATA_SLAVE: u8 = 0x01;

// Types
const IDE_ATA: u8 = 0x00;
const IDE_ATAPI: u8 = 0x01;

// Registers
const ATA_REG_DATA: u16 = 0x00;
const ATA_REG_ERROR: u16 = 0x01;
const ATA_REG_FEATURES: u16 = 0x01;
const ATA_REG_SECCOUNT0: u16 = 0x02;
const ATA_REG_LBA0: u16 = 0x03;
const ATA_REG_LBA1: u16 = 0x04;
const ATA_REG_LBA2: u16 = 0x05;
const ATA_REG_HDDEVSEL: u16 = 0x06;
const ATA_REG_COMMAND: u16 = 0x07;
const ATA_REG_STATUS: u16 = 0x07;
const ATA_REG_SECCOUNT1: u16 = 0x08;
const ATA_REG_LBA3: u16 = 0x09;
const ATA_REG_LBA4: u16 = 0x0A;
const ATA_REG_LBA5: u16 = 0x0B;
const ATA_REG_CONTROL: u16 = 0x0C;
const ATA_REG_ALTSTATUS: u16 = 0x0C;
const ATA_REG_DEVADDRESS: u16 = 0x0D;

pub struct Ide;

impl Ide {
    pub fn disks(mut pci: PciConfig) -> Vec<Box<Disk>> {
        let mut ret: Vec<Box<Disk>> = Vec::new();

        unsafe { pci.flag(4, 4, true) }; // Bus mastering

        let busmaster = unsafe { pci.read(0x20) } as u16 & 0xFFF0;

        debugln!("IDE on {:X}", busmaster);

        debug!("Primary Master:");
        if let Some(disk) = IdeDisk::new(busmaster, 0x1F0, 0x3F4, 0xE, true) {
            ret.push(box disk);
        }

        debug!("Primary Slave:");
        if let Some(disk) = IdeDisk::new(busmaster, 0x1F0, 0x3F4, 0xE, false) {
            ret.push(box disk);
        }
        debugln!("");

        debug!("Secondary Master:");
        if let Some(disk) = IdeDisk::new(busmaster + 8, 0x170, 0x374, 0xF, true) {
            ret.push(box disk);
        }
        debugln!("");

        debug!("Secondary Slave:");
        if let Some(disk) = IdeDisk::new(busmaster + 8, 0x170, 0x374, 0xF, false) {
            ret.push(box disk);
        }
        debugln!("");

        ret
    }
}

/// A disk (data storage)
pub struct IdeDisk {
    base: u16,
    ctrl: u16,
    master: bool,
    request: Intex<Option<Request>>,
    requests: Intex<VecDeque<Request>>,
    cmd: Pio8,
    sts: Pio8,
    prdt: Option<Prdt>,
    pub irq: u8,
}

impl IdeDisk {
    pub fn new(busmaster: u16, base: u16, ctrl: u16, irq: u8, master: bool) -> Option<Self> {
        let ret = IdeDisk {
            base: base,
            ctrl: ctrl,
            master: master,
            request: Intex::new(None),
            requests: Intex::new(VecDeque::new()),
            cmd: Pio8::new(busmaster),
            sts: Pio8::new(busmaster + 2),
            prdt: Prdt::new(busmaster + 4),
            irq: irq,
        };

        if unsafe { ret.identify() } {
            Some(ret)
        } else {
            None
        }
    }

    unsafe fn ide_read(&self, reg: u16) -> u8 {
        let ret;
        if reg < 0x08 {
            ret = inb(self.base + reg - 0x00);
        } else if reg < 0x0C {
            ret = inb(self.base + reg - 0x06);
        } else if reg < 0x0E {
            ret = inb(self.ctrl + reg - 0x0A);
        } else {
            ret = 0;
        }
        ret
    }

    unsafe fn ide_write(&self, reg: u16, data: u8) {
        if reg < 0x08 {
            outb(self.base + reg - 0x00, data);
        } else if reg < 0x0C {
            outb(self.base + reg - 0x06, data);
        } else if reg < 0x0E {
            outb(self.ctrl + reg - 0x0A, data);
        }
    }

    unsafe fn ide_poll(&self, check_error: bool) -> u8 {
        self.ide_read(ATA_REG_ALTSTATUS);
        self.ide_read(ATA_REG_ALTSTATUS);
        self.ide_read(ATA_REG_ALTSTATUS);
        self.ide_read(ATA_REG_ALTSTATUS);

        while self.ide_read(ATA_REG_STATUS) & ATA_SR_BSY == ATA_SR_BSY {

        }

        if check_error {
            let state = self.ide_read(ATA_REG_STATUS);
            if state & ATA_SR_ERR == ATA_SR_ERR {
                return 2;
            }
            if state & ATA_SR_DF == ATA_SR_DF {
                return 1;
            }
            if !(state & ATA_SR_DRQ == ATA_SR_DRQ) {
                return 3;
            }
        }

        0
    }

    /// Identify
    pub unsafe fn identify(&self) -> bool {
        if self.ide_read(ATA_REG_STATUS) == 0xFF {
            debug!(" Floating Bus");

            return false;
        }

        while self.ide_read(ATA_REG_STATUS) & ATA_SR_BSY == ATA_SR_BSY {

        }

        if self.master {
            self.ide_write(ATA_REG_HDDEVSEL, 0xA0);
        } else {
            self.ide_write(ATA_REG_HDDEVSEL, 0xB0);
        }

        self.ide_write(ATA_REG_SECCOUNT0, 0);
        self.ide_write(ATA_REG_LBA0, 0);
        self.ide_write(ATA_REG_LBA1, 0);
        self.ide_write(ATA_REG_LBA2, 0);

        self.ide_write(ATA_REG_COMMAND, ATA_CMD_IDENTIFY);

        let status = self.ide_read(ATA_REG_STATUS);
        debug!(" Status: {:X}", status);

        if status == 0 {
            return false;
        }

        let err = self.ide_poll(true);
        if err > 0 {
            debug!(" Error: {:X}", err);

            return false;
        }

        let data = Pio16::new(self.base + ATA_REG_DATA);
        let mut destination = Memory::<u16>::new(256).unwrap();
        for word in 0..256 {
            destination.write(word, data.read());
        }

        debug!(" Serial: ");
        for word in 10..20 {
            let d = destination.read(word);
            let a = ((d >> 8) as u8) as char;
            if a != ' ' {
                debug!("{}", a);
            }
            let b = (d as u8) as char;
            if b != ' ' {
                debug!("{}", b);
            }
        }

        debug!(" Firmware: ");
        for word in 23..27 {
            let d = destination.read(word);
            let a = ((d >> 8) as u8) as char;
            if a != ' ' {
                debug!("{}", a);
            }
            let b = (d as u8) as char;
            if b != ' ' {
                debug!("{}", b);
            }
        }

        debug!(" Model: ");
        for word in 27..47 {
            let d = destination.read(word);
            let a = ((d >> 8) as u8) as char;
            if a != ' ' {
                debug!("{}", a);
            }
            let b = (d as u8) as char;
            if b != ' ' {
                debug!("{}", b);
            }
        }

        let mut sectors = (destination.read(100) as u64) | ((destination.read(101) as u64) << 16) |
                          ((destination.read(102) as u64) << 32) |
                          ((destination.read(103) as u64) << 48);

        if sectors == 0 {
            sectors = (destination.read(60) as u64) | ((destination.read(61) as u64) << 16);
        }

        debug!(" Size: {} MB", (sectors / 2048) as usize);

        true
    }

    /// Send request
    pub fn request(&mut self, new_request: Request) {
        self.requests.lock().push_back(new_request);

        if self.request.lock().is_none() {
            unsafe { self.next_request() };
        }
    }

    pub unsafe fn on_poll(&mut self) {
        let sts = self.sts.read();
        if sts & STS_INT == STS_INT {
            self.sts.write(sts);

            self.next_request();
        }
    }

    unsafe fn next_request(&mut self) {
        let mut requests = self.requests.lock();
        let mut request = self.request.lock();

        if let Some(ref mut req) = *request {
            let cmd = self.cmd.read();
            self.cmd.write(cmd & !0x1);
            req.complete.store(true, Ordering::SeqCst);
        }

        *request = requests.pop_front();

        if let Some(ref req) = *request {
            let mut cmd = self.cmd.read();
            if req.read {
                self.cmd.write(CMD_DIR);
            } else {
                self.cmd.write(0);
            }

            while self.ide_read(ATA_REG_STATUS) & ATA_SR_BSY == ATA_SR_BSY {

            }

            if req.mem > 0 {
                let sectors = (req.extent.length + 511) / 512;
                let mut prdt_set = false;
                if let Some(ref mut prdt) = self.prdt {
                    let mut size = sectors * 512;
                    let mut i = 0;
                    while size >= 65536 && i < 8192 {
                        let eot;
                        if size == 65536 {
                            eot = PRD_EOT;
                        } else {
                            eot = 0;
                        }

                        prdt.mem.store(i,
                                       Prd {
                                           addr: (req.mem + i * 65536) as u32,
                                           size: 0,
                                           eot: eot,
                                       });

                        size -= 65536;
                        i += 1;
                    }
                    if size > 0 && i < 8192 {
                        prdt.mem.store(i,
                                       Prd {
                                           addr: (req.mem + i * 65536) as u32,
                                           size: size as u16,
                                           eot: PRD_EOT,
                                       });

                        size = 0;
                        i += 1;
                    }

                    if i > 0 {
                        if size == 0 {
                            prdt.reg.write(prdt.mem.ptr as u32);
                            prdt_set = true;
                        } else {
                            debug!("IDE Request too large: {} remaining\n", size);
                        }
                    } else {
                        debug!("IDE Request size is 0\n");
                    }
                } else {
                    debug!("PRDT not allocated\n");
                }

                if prdt_set {
                    if self.master {
                        self.ide_write(ATA_REG_HDDEVSEL, 0x40);
                    } else {
                        self.ide_write(ATA_REG_HDDEVSEL, 0x50);
                    }

                    self.ide_write(ATA_REG_SECCOUNT1, ((sectors >> 8) & 0xFF) as u8);
                    self.ide_write(ATA_REG_LBA3, ((req.extent.block >> 24) & 0xFF) as u8);
                    self.ide_write(ATA_REG_LBA4, ((req.extent.block >> 32) & 0xFF) as u8);
                    self.ide_write(ATA_REG_LBA5, ((req.extent.block >> 40) & 0xFF) as u8);

                    self.ide_write(ATA_REG_SECCOUNT0, (sectors & 0xFF) as u8);
                    self.ide_write(ATA_REG_LBA0, (req.extent.block & 0xFF) as u8);
                    self.ide_write(ATA_REG_LBA1, ((req.extent.block >> 8) & 0xFF) as u8);
                    self.ide_write(ATA_REG_LBA2, ((req.extent.block >> 16) & 0xFF) as u8);

                    let mut status = self.ide_read(ATA_REG_STATUS);
                    while (status & ATA_SR_BSY == ATA_SR_BSY) || (status & ATA_SR_DRDY != ATA_SR_DRDY)  {

                    }

                    if req.read {
                        self.ide_write(ATA_REG_COMMAND, ATA_CMD_READ_DMA_EXT);
                    } else {
                        self.ide_write(ATA_REG_COMMAND, ATA_CMD_WRITE_DMA_EXT);
                    }

                    if req.read {
                        self.cmd.write(CMD_ACT | CMD_DIR);
                    } else {
                        self.cmd.write(CMD_ACT);
                    }
                }
            } else {
                debug!("IDE Request mem is 0\n");
            }
        }
    }

    unsafe fn ata_pio_small(&mut self, block: u64, sectors: u16, buf: usize, write: bool) -> Result<usize> {
        if buf > 0 {
            while self.ide_read(ATA_REG_STATUS) & ATA_SR_BSY == ATA_SR_BSY {}

            if self.master {
                self.ide_write(ATA_REG_HDDEVSEL, 0x40);
            } else {
                self.ide_write(ATA_REG_HDDEVSEL, 0x50);
            }

            self.ide_write(ATA_REG_SECCOUNT1, ((sectors >> 8) & 0xFF) as u8);
            self.ide_write(ATA_REG_LBA3, ((block >> 24) & 0xFF) as u8);
            self.ide_write(ATA_REG_LBA4, ((block >> 32) & 0xFF) as u8);
            self.ide_write(ATA_REG_LBA5, ((block >> 40) & 0xFF) as u8);

            self.ide_write(ATA_REG_SECCOUNT0, ((sectors >> 0) & 0xFF) as u8);
            self.ide_write(ATA_REG_LBA0, ((block >> 0) & 0xFF) as u8);
            self.ide_write(ATA_REG_LBA1, ((block >> 8) & 0xFF) as u8);
            self.ide_write(ATA_REG_LBA2, ((block >> 16) & 0xFF) as u8);

            if write {
                self.ide_write(ATA_REG_COMMAND, ATA_CMD_WRITE_PIO_EXT);
            } else {
                self.ide_write(ATA_REG_COMMAND, ATA_CMD_READ_PIO_EXT);
            }

            for sector in 0..sectors as usize {
                let err = self.ide_poll(true);
                if err > 0 {
                    debugln!("IDE Error: {:X}", err);
                    return Err(SysError::new(EIO));
                }

                if write {
                    for word in 0..256 {
                        outw(self.base + ATA_REG_DATA,
                             ptr::read((buf + sector * 512 + word * 2) as *const u16));
                    }

                    self.ide_write(ATA_REG_COMMAND, ATA_CMD_CACHE_FLUSH_EXT);
                    self.ide_poll(false);
                } else {
                    for word in 0..256 {
                        ptr::write((buf + sector * 512 + word * 2) as *mut u16,
                                   inw(self.base + ATA_REG_DATA));
                    }
                }
            }

            Ok(sectors as usize * 512)
        } else {
            debugln!("Invalid request");
            Err(SysError::new(EIO))
        }
    }

    fn ata_pio(&mut self, block: u64, sectors: usize, buf: usize, write: bool) -> Result<usize> {
        debugln!("IDE PIO BLOCK: {:X} SECTORS: {} BUF: {:X} WRITE: {}", block, sectors, buf, write);

        if buf > 0 && sectors > 0 {
            let mut sector: usize = 0;
            while sectors - sector >= 65536 {
                if let Err(err) = unsafe { self.ata_pio_small(block + sector as u64, 0, buf + sector * 512, write) } {
                    return Err(err);
                }

                sector += 65536;
            }
            if sector < sectors {
                if let Err(err) = unsafe { self.ata_pio_small(block + sector as u64, (sectors - sector) as u16, buf + sector * 512, write) } {
                    return Err(err);
                }
            }

            Ok(sectors * 512)
        } else {
            debugln!("Invalid request");
            Err(SysError::new(EIO))
        }
    }
}

impl Disk for IdeDisk {
    fn read(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        self.ata_pio(block, buffer.len()/512, buffer.as_ptr() as usize, false)
    }

    fn write(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        self.ata_pio(block, buffer.len()/512, buffer.as_ptr() as usize, true)
    }
}
