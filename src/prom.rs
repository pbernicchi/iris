use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

use crate::traits::{BusStatus, BusDevice, Device};

const PROM_BASE: u32 = 0x1FC00000;
const PROM_SIZE: u32 = 1024 * 1024; // 1MB
const PROM_FILENAME: &str = "070-9101-011.bin";

struct PromInner {
    data: Vec<u32>,
    clock: AtomicU64,
    running: AtomicBool,
}

pub struct PromPort {
    inner: Arc<PromInner>,
}

pub struct Prom {
    inner: Arc<PromInner>,
}

impl Prom {
    pub fn new() -> Self {
        let path = Path::new(PROM_FILENAME);
        let bytes = fs::read(path).unwrap_or_else(|_| {
            eprintln!("Warning: Could not read PROM file: {}", PROM_FILENAME);
            Vec::new()
        });

        Self::from_bytes(&bytes)
    }

    /// Load PROM from `path`; if that fails, fall back to the embedded binary.
    pub fn from_file_or_embedded(path: &str) -> Self {
        match fs::read(path) {
            Ok(bytes) => {
                println!("Loaded PROM from {}", path);
                Self::from_bytes(&bytes)
            }
            Err(e) => {
                eprintln!("Warning: Could not read PROM file '{}': {} — using embedded PROM", path, e);
                Self::from_bytes(&crate::prombin::PROM0709101011)
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        // Pack bytes into u32 (Big Endian)
        let mut data = Vec::with_capacity((bytes.len() + 3) / 4);
        for chunk in bytes.chunks(4) {
            let mut buf = [0u8; 4];
            for (i, &b) in chunk.iter().enumerate() {
                buf[i] = b;
            }
            data.push(u32::from_be_bytes(buf));
        }

        Prom {
            inner: Arc::new(PromInner {
                data,
                clock: AtomicU64::new(0),
                running: AtomicBool::new(false),
            }),
        }
    }

    pub fn get_port(&self) -> PromPort {
        PromPort {
            inner: self.inner.clone(),
        }
    }
}

impl Device for Prom {
    fn step(&self, _cycles: u64) {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);
    }

    fn stop(&self) {
        self.inner.running.store(false, Ordering::SeqCst);
    }

    fn start(&self) {
        self.inner.running.store(true, Ordering::SeqCst);
    }

    fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::SeqCst)
    }

    fn get_clock(&self) -> u64 {
        self.inner.clock.load(Ordering::Relaxed)
    }
}

impl BusDevice for PromPort {
    fn read8(&self, addr: u32) -> BusStatus {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);

        if addr < PROM_BASE || addr >= PROM_BASE + PROM_SIZE {
            return BusStatus::Data8(0xFF);
        }

        let offset = (addr - PROM_BASE) as usize;
        let word_index = offset / 4;
        let byte_offset = offset % 4;

        if word_index < self.inner.data.len() {
            let word = self.inner.data[word_index];
            let byte = ((word >> (24 - byte_offset * 8)) & 0xFF) as u8;
            BusStatus::Data8(byte)
        } else {
            BusStatus::Data8(0xFF)
        }
    }

    fn write8(&self, _addr: u32, _val: u8) -> BusStatus {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);
        BusStatus::Ready
    }

    fn read16(&self, addr: u32) -> BusStatus {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);

        if addr < PROM_BASE || addr >= PROM_BASE + PROM_SIZE {
            return BusStatus::Data16(0xFFFF);
        }

        let offset = (addr - PROM_BASE) as usize;
        let word_index = offset / 4;
        let byte_offset = offset % 4;

        if word_index < self.inner.data.len() {
            let word = self.inner.data[word_index];
            let halfword = ((word >> (16 - byte_offset * 8)) & 0xFFFF) as u16;
            BusStatus::Data16(halfword)
        } else {
            BusStatus::Data16(0xFFFF)
        }
    }

    fn write16(&self, _addr: u32, _val: u16) -> BusStatus {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);
        BusStatus::Ready
    }

    fn read32(&self, addr: u32) -> BusStatus {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);

        if addr < PROM_BASE || addr >= PROM_BASE + PROM_SIZE {
            return BusStatus::Data(0xFFFFFFFF);
        }

        let offset = (addr - PROM_BASE) as usize;
        let index = offset / 4;

        if index < self.inner.data.len() {
            BusStatus::Data(self.inner.data[index])
        } else {
            BusStatus::Data(0xFFFFFFFF)
        }
    }

    fn write32(&self, _addr: u32, _val: u32) -> BusStatus {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);
        BusStatus::Ready
    }

    fn read64(&self, addr: u32) -> BusStatus {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);

        // Read two consecutive 32-bit words
        let high = match self.read32(addr) {
            BusStatus::Data(val) => val as u64,
            _ => 0xFFFFFFFF,
        };
        let low = match self.read32(addr + 4) {
            BusStatus::Data(val) => val as u64,
            _ => 0xFFFFFFFF,
        };

        BusStatus::Data64((high << 32) | low)
    }

    fn write64(&self, _addr: u32, _val: u64) -> BusStatus {
        self.inner.clock.fetch_add(1, Ordering::Relaxed);
        BusStatus::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prom_behavior() {
        // Mock data: F0 0B F0 00 (Big Endian u32: 0xF00BF000)
        let data = vec![0xF0, 0x0B, 0xF0, 0x00];
        let prom = Prom::from_bytes(&data);
        let port = prom.get_port();

        // 1. Check if [0] is F0 0B F0 00
        match port.read32(PROM_BASE) {
            BusStatus::Data(val) => assert_eq!(val, 0xF00BF000, "First word mismatch"),
            _ => panic!("Expected BusStatus::Data"),
        }

        // 2. Check reads outside range return 0xFFFFFFFF
        // Inside mapped range but outside data length
        match port.read32(PROM_BASE + 4) {
            BusStatus::Data(val) => assert_eq!(val, 0xFFFFFFFF, "Read past data end mismatch"),
            _ => panic!("Expected BusStatus::Data"),
        }
        // Outside mapped range
        match port.read32(PROM_BASE - 4) {
            BusStatus::Data(val) => assert_eq!(val, 0xFFFFFFFF, "Read before base mismatch"),
            _ => panic!("Expected BusStatus::Data"),
        }

        // 3. Check writes do nothing
        assert_eq!(port.write32(PROM_BASE, 0xDEADBEEF), BusStatus::Ready);
        // Verify data is unchanged
        match port.read32(PROM_BASE) {
            BusStatus::Data(val) => assert_eq!(val, 0xF00BF000, "Write modified ROM data"),
            _ => panic!("Expected BusStatus::Data"),
        }
    }

    #[test]
    fn test_prom_disassembly() {
        use crate::mips_dis;

        let prom = Prom::new();
        let port = prom.get_port();

        println!("\nDisassembling first 256 words from PROM:\n");

        // PROM is at physical address 0x1FC00000
        // But CPU starts at 0xBFC00000 (KSEG1 uncached mapping)
        const KSEG1_OFFSET: u32 = 0xA0000000;

        for i in 0..256 {
            let phys_addr = PROM_BASE + (i * 4);
            let kseg1_addr = phys_addr + KSEG1_OFFSET; // 0xBFC00000 + offset

            match port.read32(phys_addr) {
                BusStatus::Data(word) => {
                    let disasm = mips_dis::disassemble(word, kseg1_addr as u64, None);
                    println!("0x{:08x}: 0x{:08x}: {}", kseg1_addr, word, disasm);
                }
                _ => {
                    println!("0x{:08x}: ERROR reading", kseg1_addr);
                }
            }
        }
    }
}