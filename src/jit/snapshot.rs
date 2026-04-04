//! Fast CPU state snapshot for JIT speculative-execution rollback.

use crate::mips_core::NanoTlbEntry;
use crate::mips_tlb::{MipsTlb, Tlb};
use crate::mips_exec::MipsExecutor;
use crate::mips_cache_v2::MipsCache;

/// Complete CPU snapshot for JIT speculative-execution rollback.
/// ~2.3 KB. Only allocated for speculative blocks; zero overhead for trusted blocks.
#[derive(Clone)]
pub struct CpuRollbackSnapshot {
    pub gpr: [u64; 32],
    pub pc: u64,
    pub hi: u64,
    pub lo: u64,
    // CP0 subset that JIT blocks can observe or dirty:
    pub cp0_status:   u32,
    pub cp0_cause:    u32,
    pub cp0_epc:      u64,
    pub cp0_count:    u64,
    pub cp0_compare:  u64,
    pub cp0_badvaddr: u64,
    pub cp0_entryhi:  u64,
    pub cp0_context:  u64,
    pub cp0_wired:    u32,
    pub cp0_entrylo0: u64,
    pub cp0_entrylo1: u64,
    pub cp0_pagemask: u64,
    pub nanotlb: [NanoTlbEntry; 3],
    pub in_delay_slot: bool,
    pub delay_slot_target: u64,
    pub cached_pending: u64,
    pub interrupt_check_counter: u8,
    pub tlb: MipsTlb,
}

impl CpuRollbackSnapshot {
    /// Capture current CPU state. Call immediately before running a speculative block.
    /// `tlb` should be obtained via `exec.tlb.clone_as_mips_tlb().unwrap()`.
    pub fn capture<T: Tlb, C: MipsCache>(exec: &MipsExecutor<T, C>, tlb: MipsTlb) -> Self {
        Self {
            gpr:               exec.core.gpr,
            pc:                exec.core.pc,
            hi:                exec.core.hi,
            lo:                exec.core.lo,
            cp0_status:        exec.core.cp0_status,
            cp0_cause:         exec.core.cp0_cause,
            cp0_epc:           exec.core.cp0_epc,
            cp0_count:         exec.core.cp0_count,
            cp0_compare:       exec.core.cp0_compare,
            cp0_badvaddr:      exec.core.cp0_badvaddr,
            cp0_entryhi:       exec.core.cp0_entryhi,
            cp0_context:       exec.core.cp0_context,
            cp0_wired:         exec.core.cp0_wired,
            cp0_entrylo0:      exec.core.cp0_entrylo0,
            cp0_entrylo1:      exec.core.cp0_entrylo1,
            cp0_pagemask:      exec.core.cp0_pagemask,
            nanotlb:           exec.core.nanotlb,
            in_delay_slot:     exec.in_delay_slot,
            delay_slot_target: exec.delay_slot_target,
            cached_pending:    exec.cached_pending,
            interrupt_check_counter: exec.interrupt_check_counter,
            tlb,
        }
    }

    /// Restore CPU state from snapshot. Call on rollback after a speculative block misbehaves.
    pub fn restore<T: Tlb, C: MipsCache>(&self, exec: &mut MipsExecutor<T, C>) {
        exec.core.gpr          = self.gpr;
        exec.core.pc           = self.pc;
        exec.core.hi           = self.hi;
        exec.core.lo           = self.lo;
        exec.core.cp0_status   = self.cp0_status;
        exec.core.cp0_cause    = self.cp0_cause;
        exec.core.cp0_epc      = self.cp0_epc;
        exec.core.cp0_count    = self.cp0_count;
        exec.core.cp0_compare  = self.cp0_compare;
        exec.core.cp0_badvaddr = self.cp0_badvaddr;
        exec.core.cp0_entryhi  = self.cp0_entryhi;
        exec.core.cp0_context  = self.cp0_context;
        exec.core.cp0_wired    = self.cp0_wired;
        exec.core.cp0_entrylo0 = self.cp0_entrylo0;
        exec.core.cp0_entrylo1 = self.cp0_entrylo1;
        exec.core.cp0_pagemask = self.cp0_pagemask;
        exec.core.nanotlb      = self.nanotlb;
        exec.in_delay_slot     = self.in_delay_slot;
        exec.delay_slot_target = self.delay_slot_target;
        exec.cached_pending    = self.cached_pending;
        exec.interrupt_check_counter = self.interrupt_check_counter;
        exec.tlb.restore_from_mips_tlb(&self.tlb);
    }

    /// Compare GPRs between snapshot and current state.
    /// Returns bitmask of register indices that differ (bit i set = gpr[i] changed).
    pub fn compare_gprs<T: Tlb, C: MipsCache>(&self, exec: &MipsExecutor<T, C>) -> u32 {
        let mut mask = 0u32;
        for i in 0..32 {
            if self.gpr[i] != exec.core.gpr[i] {
                mask |= 1u32 << i;
            }
        }
        mask
    }
}
