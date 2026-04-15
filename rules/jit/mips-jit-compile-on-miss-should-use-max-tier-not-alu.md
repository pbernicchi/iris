# MIPS JIT compile-on-miss should use max_tier, not Alu

**Keywords:** jit, compile on miss, chain, max_tier, Alu, cache miss, block length
**Category:** jit

# Compile-on-Miss at max_tier (not Alu) for Chain Progression

When a JIT block chain breaks due to cache miss, compile the next block at `max_tier` directly, not at Alu. This is the difference between 3.3% and 8.7% JIT coverage.

## Why
The main dispatch path compiles new blocks at Alu tier and lets them be promoted over thousands of executions. That's fine for hot code. But chain misses happen at arbitrary PCs — often where the first instruction is a load/store, which Alu-tier can't compile. The `trace_block` returns empty, compile fails, the PC stays forever uncached, and every chain break at that PC hits the same miss again.

Measured: with Alu-tier compile-on-miss, ~14K of 107M chain misses actually produced new blocks. With max_tier compile-on-miss, the hit rate on subsequent chains goes up dramatically.

## How to apply
In the chain loop's miss path in `run_jit_dispatch`:
```rust
None => {
    // Compile at max_tier, not Alu — chain targets often start with
    // loads/stores that Alu can't trace past.
    let instrs = trace_block(exec, next_pc, max_tier);
    if !instrs.is_empty() {
        if let Some(mut block) = compiler.compile_block(&instrs, next_pc, max_tier) {
            block.phys_addr = next_phys;
            cache.insert(next_phys, next_pc, block);
            blocks_compiled += 1;
            probe.set_cache_size(cache.len() as u32);
        }
    }
    break;
}
```

Safe because Loads/Full tiers are proven stable (delay-slot fix in place, helper limits set). The "start at Alu and promote" progression is an artifact of older debugging needs.

## History
This single change moved the JIT from 3.3% coverage to 8.7% — biggest single-change win in the optimization pass.

