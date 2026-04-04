//! Adaptive JIT dispatch loop with tiered compilation and speculative execution.
//!
//! Every block starts at Tier 0 (ALU only — safe by construction). Hot blocks
//! are promoted through tiers as they prove stable. If a speculative block
//! misbehaves, CPU state is rolled back from a pre-block snapshot and the block
//! is demoted. Blocks that prove stable graduate to trusted execution with zero
//! snapshot overhead.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::mips_exec::{MipsExecutor, DecodedInstr, EXEC_BREAKPOINT, decode_into};
use crate::mips_tlb::{Tlb, AccessType};
use crate::mips_cache_v2::MipsCache;

use super::cache::{BlockTier, CodeCache, TIER_STABLE_THRESHOLD, TIER_PROMOTE_THRESHOLD, TIER_DEMOTE_THRESHOLD};
use super::compiler::BlockCompiler;
use super::context::{JitContext, EXIT_NORMAL, EXIT_EXCEPTION};
use super::helpers::HelperPtrs;
use super::profile::{self, ProfileEntry};
use super::snapshot::CpuRollbackSnapshot;

const MAX_BLOCK_LEN: usize = 64;

/// How many interpreter steps between cache probes within a batch.
const PROBE_INTERVAL: u32 = 1000;

/// How many interpreter steps in one outer batch.
const BATCH_SIZE: u32 = 10000;

pub fn run_jit_dispatch<T: Tlb, C: MipsCache>(
    exec: &mut MipsExecutor<T, C>,
    running: &AtomicBool,
) {
    let jit_enabled = std::env::var("IRIS_JIT").map(|v| v == "1").unwrap_or(false);

    if !jit_enabled {
        eprintln!("JIT: interpreter-only mode (set IRIS_JIT=1 to enable compilation)");
        interpreter_loop(exec, running);
        return;
    }

    // CRITICAL: Convert &mut to raw pointer. We must never hold &mut MipsExecutor
    // across a JIT block call, because the JIT's memory helpers create their own
    // &mut from the raw pointer. Two simultaneous &mut is UB, and with lto="fat"
    // LLVM exploits the noalias guarantee to cache/hoist loads across the call,
    // causing stale TLB/cache/CP0 state and kernel panics.
    let exec_ptr: *mut MipsExecutor<T, C> = exec as *mut _;

    // IRIS_JIT_MAX_TIER: cap the highest tier blocks can reach (0=Alu, 1=Loads, 2=Full)
    let max_tier = match std::env::var("IRIS_JIT_MAX_TIER").ok().and_then(|v| v.parse::<u8>().ok()) {
        Some(0) => BlockTier::Alu,
        Some(1) => BlockTier::Loads,
        _ => BlockTier::Full,
    };
    // IRIS_JIT_VERIFY=1: after each JIT block, re-run via interpreter and compare
    let verify_mode = std::env::var("IRIS_JIT_VERIFY").map(|v| v == "1").unwrap_or(false);
    eprintln!("JIT: adaptive mode (max_tier={:?}, verify={}, probe every {} steps)",
        max_tier, verify_mode, PROBE_INTERVAL);
    let helpers = HelperPtrs::new::<T, C>();
    let mut compiler = BlockCompiler::new(&helpers);
    let mut cache = CodeCache::new();
    let mut ctx = JitContext::new();
    ctx.executor_ptr = exec_ptr as u64;

    let mut total_jit_instrs: u64 = 0;
    let mut total_interp_steps: u64 = 0;
    let mut blocks_compiled: u64 = 0;
    let mut promotions: u64 = 0;
    let mut demotions: u64 = 0;
    let mut rollbacks: u64 = 0;

    // Load saved profile and eagerly compile hot blocks
    {
        let exec = unsafe { &mut *exec_ptr };
        let profile_entries = profile::load_profile();
        let mut profile_compiled = 0u64;
        for entry in &profile_entries {
            // Cap at max_tier
            let tier = if entry.tier > max_tier { max_tier } else { entry.tier };
            if tier == BlockTier::Alu {
                continue; // Alu blocks compile on first miss anyway
            }
            let instrs = trace_block(exec, entry.virt_pc, tier);
            if !instrs.is_empty() {
                if let Some(mut block) = compiler.compile_block(&instrs, entry.virt_pc, tier) {
                    block.phys_addr = entry.phys_pc;
                    cache.insert(entry.phys_pc, block);
                    blocks_compiled += 1;
                    profile_compiled += 1;
                }
            }
        }
        if profile_compiled > 0 {
            eprintln!("JIT profile: pre-compiled {} blocks from profile", profile_compiled);
        }
    }

    while running.load(Ordering::Relaxed) {
        let mut steps_in_batch: u32 = 0;

        while steps_in_batch < BATCH_SIZE {
            // Interpreter batch — no JIT call happens here
            {
                let exec = unsafe { &mut *exec_ptr };
                #[cfg(feature = "lightning")]
                for _ in 0..PROBE_INTERVAL {
                    exec.step();
                }
                #[cfg(not(feature = "lightning"))]
                for _ in 0..PROBE_INTERVAL {
                    let status = exec.step();
                    if status == EXEC_BREAKPOINT {
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            } // &mut exec dropped here
            steps_in_batch += PROBE_INTERVAL;

            if !running.load(Ordering::Relaxed) { break; }

            // Probe the JIT code cache
            let (pc, in_delay_slot) = {
                let exec = unsafe { &*exec_ptr };
                (exec.core.pc, exec.in_delay_slot)
            };
            let pc32 = pc as u32;

            let in_prom = (pc32 >= 0x9FC00000 && pc32 < 0xA0000000) || (pc32 >= 0xBFC00000);
            let in_exc = pc32 >= 0x80000000 && pc32 < 0x80000400;
            if in_prom || in_exc || in_delay_slot {
                continue;
            }

            let phys_pc = {
                let exec = unsafe { &mut *exec_ptr };
                match translate_pc(exec, pc) {
                    Some(p) => p,
                    None => continue,
                }
            };

            if cache.lookup(phys_pc).is_some() {
                // Cache hit — execute compiled block.
                let block = cache.lookup(phys_pc).unwrap();
                let block_len = block.len_mips;
                let block_tier = block.tier;
                let is_speculative = block.speculative;

                // Snapshot CPU if speculative OR verify mode
                let snapshot = if is_speculative || verify_mode {
                    let exec = unsafe { &*exec_ptr };
                    exec.tlb.clone_as_mips_tlb().map(|tlb| {
                        CpuRollbackSnapshot::capture(exec, tlb)
                    })
                } else {
                    None
                };

                // Sync and run
                {
                    let exec = unsafe { &mut *exec_ptr };
                    ctx.sync_from_executor(exec);
                } // &mut dropped before JIT call

                ctx.exit_reason = 0;
                let entry: extern "C" fn(*mut JitContext) = unsafe {
                    std::mem::transmute(cache.lookup(phys_pc).unwrap().entry)
                };
                entry(&mut ctx); // Helpers create their own &mut from exec_ptr

                {
                    let exec = unsafe { &mut *exec_ptr };
                    ctx.sync_to_executor(exec);

                    if ctx.exit_reason == EXIT_EXCEPTION {
                        if let Some(snap) = &snapshot {
                            if is_speculative {
                                // Speculative block hit an exception — roll back
                                snap.restore(exec);
                                rollbacks += 1;

                                if let Some(block) = cache.lookup_mut(phys_pc) {
                                    block.hit_count += 1;
                                    block.exception_count += 1;
                                    block.stable_hits = 0;

                                    if block.exception_count >= TIER_DEMOTE_THRESHOLD {
                                        if let Some(lower) = block.tier.demote() {
                                            demotions += 1;
                                            eprintln!("JIT: demote {:016x} {:?}→{:?} ({}exc)",
                                                pc, block.tier, lower, block.exception_count);
                                            recompile_block_at_tier(
                                                &mut compiler, &mut cache, exec,
                                                phys_pc, pc, lower,
                                                &mut blocks_compiled,
                                            );
                                        } else {
                                            block.speculative = false;
                                        }
                                    }
                                }
                            } else if verify_mode {
                                // Verify mode but not speculative — restore for verification
                                snap.restore(exec);
                            }
                        }
                        // Interpreter handles the faulting instruction
                        exec.step();
                        steps_in_batch += 1;
                        ctx.exit_reason = 0;
                    } else {
                        // Normal exit — verify if enabled
                        if verify_mode {
                            if let Some(snap) = &snapshot {
                                // Save JIT results
                                let jit_gpr = exec.core.gpr;
                                let jit_pc = exec.core.pc;
                                let jit_hi = exec.core.hi;
                                let jit_lo = exec.core.lo;

                                // Restore pre-block state
                                snap.restore(exec);

                                // Run interpreter for the same number of instructions
                                for _ in 0..block_len {
                                    exec.step();
                                }

                                // Compare
                                let interp_gpr = exec.core.gpr;
                                let interp_pc = exec.core.pc;
                                let interp_hi = exec.core.hi;
                                let interp_lo = exec.core.lo;

                                let mut mismatch = false;
                                for i in 0..32 {
                                    if jit_gpr[i] != interp_gpr[i] {
                                        eprintln!("JIT VERIFY FAIL at {:016x} (tier={:?}, len={}): gpr[{}] jit={:016x} interp={:016x}",
                                            pc, block_tier, block_len, i, jit_gpr[i], interp_gpr[i]);
                                        mismatch = true;
                                    }
                                }
                                if jit_pc != interp_pc {
                                    eprintln!("JIT VERIFY FAIL at {:016x}: pc jit={:016x} interp={:016x}",
                                        pc, jit_pc, interp_pc);
                                    mismatch = true;
                                }
                                if jit_hi != interp_hi {
                                    eprintln!("JIT VERIFY FAIL at {:016x}: hi jit={:016x} interp={:016x}",
                                        pc, jit_hi, interp_hi);
                                    mismatch = true;
                                }
                                if jit_lo != interp_lo {
                                    eprintln!("JIT VERIFY FAIL at {:016x}: lo jit={:016x} interp={:016x}",
                                        pc, jit_lo, interp_lo);
                                    mismatch = true;
                                }

                                if mismatch {
                                    // Dump the block instructions
                                    let instrs = trace_block(exec, pc, block_tier);
                                    eprintln!("JIT VERIFY: block at {:016x} ({} instrs):", pc, instrs.len());
                                    for (idx, (raw, d)) in instrs.iter().enumerate() {
                                        let ipc = pc.wrapping_add(idx as u64 * 4);
                                        eprintln!("  {:016x}: {:08x} op={} rs={} rt={} rd={} funct={} imm={:04x}",
                                            ipc, raw, d.op, d.rs, d.rt, d.rd, d.funct, d.imm as u16);
                                    }
                                    // Leave interpreter state (correct) in place
                                    steps_in_batch += block_len;
                                    total_jit_instrs += block_len as u64;
                                    // Invalidate this block so we don't keep hitting it
                                    cache.invalidate_range(phys_pc, phys_pc + 4);
                                    continue;
                                }
                                // Verification passed — interpreter state is already correct
                                // (we ran the interpreter, so state is authoritative)
                            }
                        }

                        // Update stats and check for promotion
                        if let Some(block) = cache.lookup_mut(phys_pc) {
                            block.hit_count += 1;
                            block.stable_hits += 1;
                            block.exception_count = 0;

                            if block.speculative && block.stable_hits >= TIER_STABLE_THRESHOLD {
                                block.speculative = false;
                            }

                            if !block.speculative && block.stable_hits >= TIER_PROMOTE_THRESHOLD {
                                if let Some(next) = block.tier.promote().filter(|t| *t <= max_tier) {
                                    promotions += 1;
                                    eprintln!("JIT: promote {:016x} {:?}→{:?} ({}hits)",
                                        pc, block.tier, next, block.hit_count);
                                    recompile_block_at_tier(
                                        &mut compiler, &mut cache, exec,
                                        phys_pc, pc, next,
                                        &mut blocks_compiled,
                                    );
                                }
                            }
                        }

                        // Advance cp0_count per-instruction
                        if !verify_mode {
                            // In verify mode, interpreter already advanced these
                            for _ in 0..block_len {
                                let prev = exec.core.cp0_count;
                                exec.core.cp0_count = prev.wrapping_add(exec.core.count_step) & 0x0000_FFFF_FFFF_FFFF;
                                if exec.core.cp0_compare != 0 && prev < exec.core.cp0_compare && exec.core.cp0_count >= exec.core.cp0_compare {
                                    exec.core.cp0_cause |= crate::mips_core::CAUSE_IP7;
                                    exec.core.fasttick_count.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        exec.local_cycles += block_len as u64;
                        steps_in_batch += block_len;
                        total_jit_instrs += block_len as u64;
                    }
                } // &mut dropped
            } else {
                // Cache miss — compile at lowest tier (safe by construction)
                let exec = unsafe { &mut *exec_ptr };
                let instrs = trace_block(exec, pc, BlockTier::Alu);
                if !instrs.is_empty() {
                    if let Some(mut block) = compiler.compile_block(&instrs, pc, BlockTier::Alu) {
                        block.phys_addr = phys_pc;
                        cache.insert(phys_pc, block);
                        blocks_compiled += 1;
                        if blocks_compiled <= 10 || blocks_compiled % 500 == 0 {
                            eprintln!("JIT: compiled #{} at {:016x} ({} instrs, tier=Alu, cache={})",
                                blocks_compiled, pc, instrs.len(), cache.len());
                        }
                    }
                }
            }
        }

        {
            let exec = unsafe { &mut *exec_ptr };
            exec.flush_cycles();
        }
        total_interp_steps += steps_in_batch as u64;

        if total_interp_steps % 10000000 < BATCH_SIZE as u64 {
            let exec = unsafe { &*exec_ptr };
            eprintln!("JIT: {} steps, {} JIT instrs ({:.1}%), {} blocks, {}↑ {}↓ {}⟲, pc={:016x}",
                total_interp_steps, total_jit_instrs,
                if total_interp_steps > 0 { total_jit_instrs as f64 / total_interp_steps as f64 * 100.0 } else { 0.0 },
                blocks_compiled, promotions, demotions, rollbacks, exec.core.pc);
        }
    }

    {
        let exec = unsafe { &mut *exec_ptr };
        exec.flush_cycles();
    }
    eprintln!("JIT: shutdown. {} blocks, {} JIT instrs / {} total ({:.1}%), {}↑ {}↓ {}⟲",
        blocks_compiled, total_jit_instrs, total_interp_steps,
        if total_interp_steps > 0 { total_jit_instrs as f64 / total_interp_steps as f64 * 100.0 } else { 0.0 },
        promotions, demotions, rollbacks);

    // Save profile: all blocks above Alu tier
    let profile_entries: Vec<ProfileEntry> = cache.iter()
        .filter(|(_, block)| block.tier > BlockTier::Alu)
        .map(|(&phys_pc, block)| ProfileEntry {
            phys_pc,
            virt_pc: block.virt_addr,
            tier: block.tier,
        })
        .collect();
    if !profile_entries.is_empty() {
        if let Err(e) = profile::save_profile(&profile_entries) {
            eprintln!("JIT profile: save failed: {}", e);
        }
    }
}

/// Recompile a block at a different tier, replacing the existing cache entry.
fn recompile_block_at_tier<T: Tlb, C: MipsCache>(
    compiler: &mut BlockCompiler,
    cache: &mut CodeCache,
    exec: &mut MipsExecutor<T, C>,
    phys_pc: u64,
    virt_pc: u64,
    tier: BlockTier,
    blocks_compiled: &mut u64,
) {
    let instrs = trace_block(exec, virt_pc, tier);
    if !instrs.is_empty() {
        if let Some(mut block) = compiler.compile_block(&instrs, virt_pc, tier) {
            block.phys_addr = phys_pc;
            cache.replace(phys_pc, block);
            *blocks_compiled += 1;
        }
    }
}

fn interpreter_loop<T: Tlb, C: MipsCache>(
    exec: &mut MipsExecutor<T, C>,
    running: &AtomicBool,
) {
    while running.load(Ordering::Relaxed) {
        #[cfg(feature = "lightning")]
        for _ in 0..1000 {
            exec.step(); exec.step(); exec.step(); exec.step(); exec.step();
            exec.step(); exec.step(); exec.step(); exec.step(); exec.step();
        }
        #[cfg(not(feature = "lightning"))]
        for _ in 0..1000 {
            let status = exec.step();
            if status == EXEC_BREAKPOINT {
                running.store(false, Ordering::SeqCst);
                break;
            }
        }
        exec.flush_cycles();
    }
}

fn translate_pc<T: Tlb, C: MipsCache>(
    exec: &mut MipsExecutor<T, C>,
    virt_pc: u64,
) -> Option<u64> {
    let result = (exec.translate_fn)(exec, virt_pc, AccessType::Fetch);
    if result.is_exception() { None } else { Some(result.phys as u64) }
}

fn trace_block<T: Tlb, C: MipsCache>(
    exec: &mut MipsExecutor<T, C>,
    start_pc: u64,
    tier: BlockTier,
) -> Vec<(u32, DecodedInstr)> {
    let mut instrs = Vec::with_capacity(MAX_BLOCK_LEN);
    let mut pc = start_pc;

    for _ in 0..MAX_BLOCK_LEN {
        let raw = match exec.debug_fetch_instr(pc) {
            Ok(w) => w,
            Err(_) => break,
        };

        let mut d = DecodedInstr::default();
        d.raw = raw;
        decode_into::<T, C>(&mut d);

        if !is_compilable_for_tier(&d, tier) { break; }

        let is_branch = is_branch_or_jump(&d);
        instrs.push((raw, d));

        if is_branch {
            pc = pc.wrapping_add(4);
            let mut delay_ok = false;
            if let Ok(delay_raw) = exec.debug_fetch_instr(pc) {
                let mut delay_d = DecodedInstr::default();
                delay_d.raw = delay_raw;
                decode_into::<T, C>(&mut delay_d);
                if is_compilable_for_tier(&delay_d, tier) {
                    instrs.push((delay_raw, delay_d));
                    delay_ok = true;
                }
            }
            if !delay_ok { instrs.pop(); }
            break;
        }

        pc = pc.wrapping_add(4);
    }

    instrs
}

fn is_compilable_for_tier(d: &DecodedInstr, tier: BlockTier) -> bool {
    if is_compilable_alu(d) || is_branch_or_jump(d) { return true; }
    match tier {
        BlockTier::Alu => false,
        BlockTier::Loads => is_compilable_load(d),
        BlockTier::Full => is_compilable_load(d) || is_compilable_store(d),
    }
}

fn is_compilable_alu(d: &DecodedInstr) -> bool {
    use crate::mips_isa::*;
    match d.op as u32 {
        OP_SPECIAL => matches!(d.funct as u32,
            FUNCT_SLL | FUNCT_SRL | FUNCT_SRA |
            FUNCT_SLLV | FUNCT_SRLV | FUNCT_SRAV |
            FUNCT_MOVZ | FUNCT_MOVN |
            FUNCT_MFHI | FUNCT_MTHI | FUNCT_MFLO | FUNCT_MTLO |
            FUNCT_MULT | FUNCT_MULTU | FUNCT_DIV | FUNCT_DIVU |
            FUNCT_DMULT | FUNCT_DMULTU | FUNCT_DDIV | FUNCT_DDIVU |
            FUNCT_ADDU | FUNCT_SUBU | FUNCT_AND | FUNCT_OR |
            FUNCT_XOR | FUNCT_NOR | FUNCT_SLT | FUNCT_SLTU |
            FUNCT_DADDU | FUNCT_DSUBU |
            FUNCT_DSLL | FUNCT_DSRL | FUNCT_DSRA |
            FUNCT_DSLL32 | FUNCT_DSRL32 | FUNCT_DSRA32 |
            FUNCT_DSLLV | FUNCT_DSRLV | FUNCT_DSRAV |
            FUNCT_SYNC
        ),
        OP_ADDIU | OP_DADDIU | OP_SLTI | OP_SLTIU |
        OP_ANDI | OP_ORI | OP_XORI | OP_LUI => true,
        _ => false,
    }
}

fn is_compilable_load(d: &DecodedInstr) -> bool {
    use crate::mips_isa::*;
    matches!(d.op as u32,
        OP_LB | OP_LBU | OP_LH | OP_LHU | OP_LW | OP_LWU | OP_LD
    )
}

fn is_compilable_store(d: &DecodedInstr) -> bool {
    use crate::mips_isa::*;
    matches!(d.op as u32,
        OP_SB | OP_SH | OP_SW | OP_SD
    )
}

fn is_branch_or_jump(d: &DecodedInstr) -> bool {
    use crate::mips_isa::*;
    match d.op as u32 {
        OP_BEQ | OP_BNE | OP_BLEZ | OP_BGTZ => true,
        OP_J | OP_JAL => true,
        OP_SPECIAL => matches!(d.funct as u32, FUNCT_JR),
        _ => false,
    }
}
