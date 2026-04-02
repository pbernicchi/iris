// System Snapshot — save and restore full machine state to/from a directory.
//
// Layout of saves/<name>/:
//   cpu.toml       — CPU core (GPRs, CP0, FPU), TLB entries
//   mc.toml        — Memory Controller registers + GIO DMA state
//   ioc.toml       — IOC interrupt registers
//   hpc3.toml      — HPC3 state register, PBUS PIO, DMA channel registers
//   rex3.toml      — REX3 drawing registers, VC2, XMAP9, CMAP palette
//   bank0.bin      — 128 MB RAM bank A (raw u8, big-endian word layout)
//   bank1.bin      — 128 MB RAM bank B
//   bank2.bin      — 128 MB RAM bank C
//   bank3.bin      — 128 MB RAM bank D

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use toml::Value;

pub struct Snapshot {
    pub dir: PathBuf,
}

impl Snapshot {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    // ---- helpers ----

    pub fn write_toml(&self, name: &str, v: &Value) -> std::io::Result<()> {
        let path = self.dir.join(name);
        let s = toml::to_string_pretty(v)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut f = fs::File::create(&path)?;
        f.write_all(s.as_bytes())?;
        Ok(())
    }

    pub fn read_toml(&self, name: &str) -> std::io::Result<Value> {
        let path = self.dir.join(name);
        let mut f = fs::File::open(&path)?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        toml::from_str::<Value>(&s)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    pub fn write_bin(&self, name: &str, data: &[u8]) -> std::io::Result<()> {
        let path = self.dir.join(name);
        fs::write(path, data)?;
        Ok(())
    }

    pub fn read_bin(&self, name: &str) -> std::io::Result<Vec<u8>> {
        let path = self.dir.join(name);
        fs::read(path)
    }

    pub fn ensure_dir(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)
    }
}

// ---- scalar hex helpers ----

/// Encode a u64 as a hex string Value (e.g. "0x000000001234abcd").
pub fn hex_u64(v: u64) -> Value { Value::String(format!("0x{:016x}", v)) }

/// Encode a u32 as a hex string Value (e.g. "0x1234abcd").
pub fn hex_u32(v: u32) -> Value { Value::String(format!("0x{:08x}", v)) }

/// Encode a u16 as a hex string Value (e.g. "0x1234").
pub fn hex_u16(v: u16) -> Value { Value::String(format!("0x{:04x}", v)) }

/// Encode a u8 as a hex string Value (e.g. "0x12").
pub fn hex_u8(v: u8)   -> Value { Value::String(format!("0x{:02x}", v)) }

// ---- TOML helpers ----

/// Build a TOML array of hex strings from a slice of u64.
pub fn u64_slice_to_toml(slice: &[u64]) -> Value {
    Value::Array(slice.iter().map(|&v| hex_u64(v)).collect())
}

/// Build a TOML array of hex strings from a slice of u32.
pub fn u32_slice_to_toml(slice: &[u32]) -> Value {
    Value::Array(slice.iter().map(|&v| hex_u32(v)).collect())
}

/// Build a TOML array of hex strings from a slice of u16.
pub fn u16_slice_to_toml(slice: &[u16]) -> Value {
    Value::Array(slice.iter().map(|&v| hex_u16(v)).collect())
}

/// Build a TOML array of hex strings from a slice of u8.
pub fn u8_slice_to_toml(slice: &[u8]) -> Value {
    Value::Array(slice.iter().map(|&v| hex_u8(v)).collect())
}

/// Parse a hex string or integer TOML value as u64.
pub fn toml_u64(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => u64::from_str_radix(s.trim_start_matches("0x"), 16).ok(),
        Value::Integer(i) => Some(*i as u64),
        _ => None,
    }
}

/// Parse a hex string or integer TOML value as u32.
pub fn toml_u32(v: &Value) -> Option<u32> {
    match v {
        Value::String(s) => u64::from_str_radix(s.trim_start_matches("0x"), 16).ok().map(|x| x as u32),
        Value::Integer(i) => Some(*i as u32),
        _ => None,
    }
}

/// Parse a hex string or integer TOML value as u16.
pub fn toml_u16(v: &Value) -> Option<u16> {
    match v {
        Value::String(s) => u64::from_str_radix(s.trim_start_matches("0x"), 16).ok().map(|x| x as u16),
        Value::Integer(i) => Some(*i as u16),
        _ => None,
    }
}

/// Parse a hex string or integer TOML value as u8.
pub fn toml_u8(v: &Value) -> Option<u8> {
    match v {
        Value::String(s) => u64::from_str_radix(s.trim_start_matches("0x"), 16).ok().map(|x| x as u8),
        Value::Integer(i) => Some(*i as u8),
        _ => None,
    }
}

/// Extract a bool from a TOML Value::Boolean.
pub fn toml_bool(v: &Value) -> Option<bool> {
    v.as_bool()
}

/// Load a u64 slice from a TOML array, filling as many entries as available.
pub fn load_u64_slice(v: &Value, dst: &mut [u64]) {
    if let Value::Array(arr) = v {
        for (i, item) in arr.iter().enumerate() {
            if i >= dst.len() { break; }
            if let Some(x) = toml_u64(item) { dst[i] = x; }
        }
    }
}

/// Load a u32 slice from a TOML array.
pub fn load_u32_slice(v: &Value, dst: &mut [u32]) {
    if let Value::Array(arr) = v {
        for (i, item) in arr.iter().enumerate() {
            if i >= dst.len() { break; }
            if let Some(x) = toml_u32(item) { dst[i] = x; }
        }
    }
}

/// Load a u16 slice from a TOML array.
pub fn load_u16_slice(v: &Value, dst: &mut [u16]) {
    if let Value::Array(arr) = v {
        for (i, item) in arr.iter().enumerate() {
            if i >= dst.len() { break; }
            if let Some(x) = toml_u16(item) { dst[i] = x; }
        }
    }
}

/// Load a u8 slice from a TOML array.
pub fn load_u8_slice(v: &Value, dst: &mut [u8]) {
    if let Value::Array(arr) = v {
        for (i, item) in arr.iter().enumerate() {
            if i >= dst.len() { break; }
            if let Some(x) = toml_u8(item) { dst[i] = x; }
        }
    }
}

/// Get a field from a TOML table by key.
pub fn get_field<'a>(table: &'a Value, key: &str) -> Option<&'a Value> {
    table.as_table()?.get(key)
}
