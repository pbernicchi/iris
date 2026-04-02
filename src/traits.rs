#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BusStatus {
    Ready,
    Busy,
    Error,
    Data(u32),
    Data64(u64),
    Data16(u16),
    Data8(u8),
    VirtualCoherencyException,  // R4000 Virtual Coherency Exception (VCEI/VCED)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Signal {
    Reset(bool),
    Interrupt(u32, bool),
}

/// Unified bus device interface supporting 8/16/32/64-bit accesses
///
/// Devices implement only the widths they natively support.
/// Unimplemented widths return BusStatus::Error by default.
pub trait BusDevice: Send + Sync {
    // 8-bit access
    fn read8(&self, _addr: u32) -> BusStatus { BusStatus::Error }
    fn write8(&self, _addr: u32, _val: u8) -> BusStatus { BusStatus::Error }

    // 16-bit access
    fn read16(&self, _addr: u32) -> BusStatus { BusStatus::Error }
    fn write16(&self, _addr: u32, _val: u16) -> BusStatus { BusStatus::Error }

    // 32-bit access
    fn read32(&self, _addr: u32) -> BusStatus { BusStatus::Error }
    fn write32(&self, _addr: u32, _val: u32) -> BusStatus { BusStatus::Error }

    // 64-bit access
    fn read64(&self, _addr: u32) -> BusStatus { BusStatus::Error }
    fn write64(&self, _addr: u32, _val: u64) -> BusStatus { BusStatus::Error }
}

pub trait FifoDevice: Send + Sync {
    fn read_fifo(&self) -> u8;
    fn write_fifo(&self, val: u8, notify: bool);
}

/// Status bits returned by DMA read/write/advance operations.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct DmaStatus(pub u32);

impl DmaStatus {
    pub const OK:         u32 = 0x00; // no flags — normal transfer
    pub const EOP:        u32 = 0x01; // end-of-packet descriptor boundary reached
    pub const EOX:        u32 = 0x02; // end-of-chain: descriptor chain exhausted, channel deactivated
    pub const IRQ:        u32 = 0x04; // DMA interrupt raised (xie was set on EOX)
    pub const NOT_ACTIVE: u32 = 0x08; // transfer refused — channel not active
    pub const ROWN:       u32 = 0x10; // write refused — ROWN=0, host owns descriptor
    pub const OVERFLOW:   u32 = 0x20; // byte count exhausted mid-transfer

    pub fn ok()          -> Self { Self(Self::OK) }
    pub fn is_ok(self)   -> bool { self.0 == Self::OK }
    pub fn eop(self)     -> bool { self.0 & Self::EOP        != 0 }
    pub fn eox(self)     -> bool { self.0 & Self::EOX        != 0 }
    pub fn irq(self)     -> bool { self.0 & Self::IRQ        != 0 }
    pub fn refused(self) -> bool { self.0 & (Self::NOT_ACTIVE | Self::ROWN | Self::OVERFLOW) != 0 }
}

impl std::ops::BitOr for DmaStatus {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}
impl std::ops::BitOrAssign for DmaStatus {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

pub trait DmaClient: Send + Sync {
    /// Returns (value, status, writeback).
    /// writeback is an optional (addr, val16) memory write to be executed by the caller
    /// under its own lock (e.g. SeeqState) for atomicity with state updates.
    fn read(&self) -> Option<(u32, DmaStatus, Option<(u32, u16)>)>;
    /// Write a value to the DMA channel.
    /// Returns (status, writeback) where writeback is an optional (addr, val16) memory write
    /// to be executed by the caller under its own lock (e.g. SeeqState) for atomicity.
    fn write(&self, val: u32, eop: bool) -> (DmaStatus, Option<(u32, u16)>);
}

/// Asynchronous system-level events sent from devices to the machine event loop.
#[derive(Debug)]
pub enum MachineEvent {
    /// Full system reset (SIN bit in CPUCTRL0).
    HardReset,
    /// Soft power-off (front panel power state = 0).
    PowerOff,
}

/// Restore hardware to power-on state.
/// Called with all device threads stopped.
pub trait Resettable {
    fn power_on(&self);
}

/// Serialize / deserialize device register state to/from TOML.
/// Memory bulk data (RAM) is handled separately as raw binary.
pub trait Saveable {
    fn save_state(&self) -> toml::Value;
    fn load_state(&self, v: &toml::Value) -> Result<(), String>;
}

pub trait Device: Send + Sync {
    fn step(&self, cycles: u64);
    fn stop(&self);
    fn start(&self);
    fn is_running(&self) -> bool;
    fn get_clock(&self) -> u64;

    fn signal(&self, _signal: Signal) {}

    fn register_commands(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn execute_command(&self, _cmd: &str, _args: &[&str], _writer: Box<dyn std::io::Write + Send>) -> Result<(), String> {
        Err("Command not found".to_string())
    }
}