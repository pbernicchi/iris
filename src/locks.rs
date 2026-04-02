/// Global named lock registry.
///
/// Every parking_lot Mutex/RwLock in the system registers itself here via
/// `register_lock()`. The `locks` monitor command calls `is_locked()` on
/// each one without acquiring anything, giving a live snapshot of which
/// locks are held at the moment of the query.

use std::sync::{Arc, OnceLock};
use parking_lot::Mutex as PLMutex;
use crate::traits::Device;

/// One registered lock entry.
pub struct LockEntry {
    pub name: String,
    /// Returns true if the lock is currently held by any thread.
    pub is_locked: Box<dyn Fn() -> bool + Send + Sync>,
}

/// Global registry. Populated at startup; read-only thereafter.
static REGISTRY: OnceLock<PLMutex<Vec<LockEntry>>> = OnceLock::new();

fn registry() -> &'static PLMutex<Vec<LockEntry>> {
    REGISTRY.get_or_init(|| PLMutex::new(Vec::new()))
}

/// Register a named lock. `is_locked` should call `.is_locked()` on the
/// underlying parking_lot Mutex/RwLock. Call this once per lock at init time.
pub fn register_lock(name: impl Into<String>, is_locked: impl Fn() -> bool + Send + Sync + 'static) {
    registry().lock().push(LockEntry {
        name: name.into(),
        is_locked: Box::new(is_locked),
    });
}

// ── Convenience helpers ───────────────────────────────────────────────────────

/// Register a parking_lot Mutex by cloning its Arc.
pub fn register_mutex<T: Send + 'static>(name: impl Into<String>, m: &Arc<PLMutex<T>>) {
    let m = m.clone();
    register_lock(name, move || m.is_locked());
}

/// Register a parking_lot Mutex that is not Arc-wrapped but lives in an Arc
/// of its parent struct — use a closure that captures whatever you need.
pub fn register_lock_fn(name: impl Into<String>, f: impl Fn() -> bool + Send + Sync + 'static) {
    register_lock(name, f);
}

// ── Monitor device ─────────────────────────────────────────────────────────

pub struct LockMonitor;

impl Device for LockMonitor {
    fn step(&self, _cycles: u64) {}
    fn stop(&self) {}
    fn start(&self) {}
    fn is_running(&self) -> bool { false }
    fn get_clock(&self) -> u64 { 0 }

    fn register_commands(&self) -> Vec<(String, String)> {
        vec![
            ("locks".into(), "Show status of all registered system locks".into()),
        ]
    }

    fn execute_command(&self, cmd: &str, _args: &[&str], mut writer: Box<dyn std::io::Write + Send>) -> Result<(), String> {
        if cmd != "locks" {
            return Err(format!("Unknown command: {}", cmd));
        }

        let reg = registry().lock();
        writeln!(writer, "=== System Lock Status ({} registered) ===", reg.len()).unwrap();
        let mut any_locked = false;
        for entry in reg.iter() {
            let locked = (entry.is_locked)();
            if locked { any_locked = true; }
            writeln!(writer, "  {:50} {}", entry.name, if locked { "LOCKED" } else { "free" }).unwrap();
        }
        if !any_locked {
            writeln!(writer, "  (all locks free)").unwrap();
        }
        Ok(())
    }
}
