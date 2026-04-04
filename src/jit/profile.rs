//! JIT profile cache: persists hot block metadata across emulator runs.
//!
//! On shutdown, saves (phys_pc, virt_pc, tier) tuples for all blocks above Alu tier.
//! On startup, loads the profile and eagerly compiles those blocks at their saved tier
//! (still speculative until they prove stable again). Eliminates warmup time.

use std::fs;
use std::io::{self, Read, Write, BufReader, BufWriter};
use std::path::PathBuf;

use super::cache::BlockTier;

/// One entry in the profile: a block that reached a tier worth persisting.
#[derive(Debug, Clone)]
pub struct ProfileEntry {
    pub phys_pc: u64,
    pub virt_pc: u64,
    pub tier: BlockTier,
}

const PROFILE_MAGIC: &[u8; 4] = b"IRJP"; // IRIS JIT Profile
const PROFILE_VERSION: u8 = 1;

/// Default profile path: ~/.iris/jit-profile.bin
fn default_profile_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".iris").join("jit-profile.bin")
    } else {
        PathBuf::from("jit-profile.bin")
    }
}

/// Get the profile path, respecting IRIS_JIT_PROFILE env var override.
pub fn profile_path() -> PathBuf {
    match std::env::var_os("IRIS_JIT_PROFILE") {
        Some(p) => PathBuf::from(p),
        None => default_profile_path(),
    }
}

/// Load profile entries from disk. Returns empty vec on any error.
pub fn load_profile() -> Vec<ProfileEntry> {
    let path = profile_path();
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut reader = BufReader::new(file);

    let mut magic = [0u8; 4];
    if reader.read_exact(&mut magic).is_err() || &magic != PROFILE_MAGIC {
        eprintln!("JIT profile: invalid magic in {:?}, ignoring", path);
        return Vec::new();
    }

    let mut version = [0u8; 1];
    if reader.read_exact(&mut version).is_err() || version[0] != PROFILE_VERSION {
        eprintln!("JIT profile: version mismatch in {:?}, ignoring", path);
        return Vec::new();
    }

    let mut count_buf = [0u8; 4];
    if reader.read_exact(&mut count_buf).is_err() {
        return Vec::new();
    }
    let count = u32::from_le_bytes(count_buf) as usize;

    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let mut buf = [0u8; 17]; // 8 + 8 + 1
        if reader.read_exact(&mut buf).is_err() {
            break;
        }
        let phys_pc = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let virt_pc = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        let tier = match buf[16] {
            0 => BlockTier::Alu,
            1 => BlockTier::Loads,
            2 => BlockTier::Full,
            _ => continue,
        };
        entries.push(ProfileEntry { phys_pc, virt_pc, tier });
    }

    eprintln!("JIT profile: loaded {} entries from {:?}", entries.len(), path);
    entries
}

/// Save profile entries to disk.
pub fn save_profile(entries: &[ProfileEntry]) -> io::Result<()> {
    let path = profile_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = fs::File::create(&path)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(PROFILE_MAGIC)?;
    writer.write_all(&[PROFILE_VERSION])?;
    writer.write_all(&(entries.len() as u32).to_le_bytes())?;

    for entry in entries {
        writer.write_all(&entry.phys_pc.to_le_bytes())?;
        writer.write_all(&entry.virt_pc.to_le_bytes())?;
        let tier_byte = match entry.tier {
            BlockTier::Alu => 0u8,
            BlockTier::Loads => 1u8,
            BlockTier::Full => 2u8,
        };
        writer.write_all(&[tier_byte])?;
    }

    writer.flush()?;
    eprintln!("JIT profile: saved {} entries to {:?}", entries.len(), path);
    Ok(())
}
