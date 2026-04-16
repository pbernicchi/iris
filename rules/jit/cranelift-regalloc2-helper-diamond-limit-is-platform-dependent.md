# Cranelift regalloc2 helper-diamond limit is platform-dependent

**Keywords:** cranelift, regalloc2, aarch64, x86_64, helper call, ok_block, exc_block, diamond, Full tier, block length
**Category:** jit

# Cranelift Helper-Diamond Limit Differs by Architecture

Full-tier JIT blocks terminate after N load/store helper calls to avoid Cranelift regalloc2 miscompilations. The safe N is platform-specific: **aarch64 tolerates 3, x86_64 only 1**. Bumping above the threshold produces silent miscompilations (confirmed by IRIS_JIT_VERIFY catching real GPR mismatches).

## Why
Each load or store helper call emits an `ok_block` / `exc_block` CFG diamond (helper can return an exception status, so we branch after). Multiple chained diamonds create complex control flow that stresses Cranelift's regalloc2 allocator. On x86_64 (15 GPRs), register pressure plus the CFG complexity hits an edge case and produces wrong code. On aarch64 (30 GPRs), more headroom — 3 diamonds tolerable, 4 starts failing.

## How to apply
In `src/jit/dispatch.rs` `trace_block`:
```rust
let max_helpers: u32 = if cfg!(target_arch = "aarch64") { 3 } else { 1 };
let mut helper_count: u32 = 0;
// ... inside loop ...
if tier == BlockTier::Full && (is_compilable_store(&d) || is_compilable_load(&d)) {
    helper_count += 1;
    instrs.push((raw, d));
    if helper_count >= max_helpers { break; }
}
```

Don't raise the aarch64 limit without running `IRIS_JIT_VERIFY=1` for 500M+ instructions to catch silent miscompilations. GPR mismatches at Full tier, len=3+ with off-by-small-number values (jit=0x97 interp=0x98) are the signature.

## History
Original code had `if has_helper { break; }` unconditionally with a comment citing x86_64 regalloc2 issues. Binary-searched on aarch64: max_helpers=2 works, 3 works, 4 produces real codegen mismatches in verify mode. Kept at 3 for safety margin.

