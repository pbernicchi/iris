use std::sync::Arc;
use parking_lot::Mutex;

use crate::traits::{BusStatus, BusDevice, Device};

struct TimerInner {
    clock: u64,
    running: bool,
    counter: u32,
    target: u32,
}

pub struct TimerPort {
    inner: Arc<Mutex<TimerInner>>,
}

pub struct Timer {
    inner: Arc<Mutex<TimerInner>>,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            inner: Arc::new(Mutex::new(TimerInner {
                clock: 0,
                running: false,
                counter: 0,
                target: 0,
            })),
        }
    }

    pub fn get_port(&self) -> TimerPort {
        TimerPort {
            inner: self.inner.clone(),
        }
    }
}

impl Device for Timer {
    fn step(&self, cycles: u64) {
        let mut inner = self.inner.lock();
        if inner.running {
            inner.clock += cycles;
            inner.counter = inner.counter.wrapping_add(cycles as u32);
        }
    }

    fn stop(&self) {
        self.inner.lock().running = false;
    }

    fn start(&self) {
        self.inner.lock().running = true;
    }

    fn is_running(&self) -> bool {
        self.inner.lock().running
    }

    fn get_clock(&self) -> u64 {
        self.inner.lock().clock
    }
}

impl BusDevice for TimerPort {
    fn read8(&self, _addr: u32) -> BusStatus {
        BusStatus::Error
    }

    fn write8(&self, _addr: u32, _val: u8) -> BusStatus {
        BusStatus::Error
    }

    fn read16(&self, _addr: u32) -> BusStatus {
        BusStatus::Error
    }

    fn write16(&self, _addr: u32, _val: u16) -> BusStatus {
        BusStatus::Error
    }

    fn read32(&self, addr: u32) -> BusStatus {
        let inner = self.inner.lock();
        match addr {
            0x00 => BusStatus::Data(inner.counter),
            0x04 => BusStatus::Data(inner.target),
            _ => BusStatus::Error,
        }
    }

    fn write32(&self, addr: u32, val: u32) -> BusStatus {
        let mut inner = self.inner.lock();
        match addr {
            0x00 => inner.counter = val,
            0x04 => inner.target = val,
            _ => return BusStatus::Error,
        }
        BusStatus::Ready
    }

    fn read64(&self, addr: u32) -> BusStatus {
        // Read two consecutive 32-bit words
        let high = match self.read32(addr) {
            BusStatus::Data(val) => val as u64,
            _ => return BusStatus::Error,
        };
        let low = match self.read32(addr + 4) {
            BusStatus::Data(val) => val as u64,
            _ => return BusStatus::Error,
        };
        BusStatus::Data64((high << 32) | low)
    }

    fn write64(&self, addr: u32, val: u64) -> BusStatus {
        let high = (val >> 32) as u32;
        let low = val as u32;

        if self.write32(addr, high) == BusStatus::Error {
            return BusStatus::Error;
        }
        self.write32(addr + 4, low)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::BusStatus;

    #[test]
    fn test_timer_basic() {
        let timer = Timer::new();
        let port = timer.get_port();

        // Test Write/Read Target
        assert_eq!(port.write32(0x04, 0xDEADBEEF), BusStatus::Ready);
        match port.read32(0x04) {
            BusStatus::Data(val) => assert_eq!(val, 0xDEADBEEF),
            _ => panic!("Unexpected bus status"),
        }

        // Test Run
        timer.start();
        assert!(timer.is_running());

        // Step
        timer.step(100);

        timer.stop();
        assert!(!timer.is_running());

        // Counter should match clock in this simple impl
        match port.read32(0x00) {
            BusStatus::Data(val) => assert_eq!(val, 100),
            _ => panic!("Could not read counter"),
        }
    }
}