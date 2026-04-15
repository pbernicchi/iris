# Cranelift opt_level=none is the right trade for throughput JITs

**Keywords:** cranelift, opt_level, compilation speed, throughput, JIT overhead, perf
**Category:** jit

# Cranelift opt_level=none for Interpreter-First JITs

For a JIT where blocks compile frequently and run a few hundred times each, **`opt_level = "none"` beats `"speed"` for total throughput** by 2-3x. Generated native code is ~10-20% slower per instruction, but compile time drops ~3-5x.

## Why
Profiling showed 66% of MIPS-CPU thread time was inside Cranelift compilation passes. Switching opt_level=none preserved the % but increased total emulator throughput 2.5x — same Cranelift work, more actual execution per second. The generated code slowdown is dwarfed by the compile speedup because every block gets compiled whether it runs 10 times or 10,000 times.

## How to apply
In `BlockCompiler::new` (`src/jit/compiler.rs`):
```rust
flag_builder.set("opt_level", "none").unwrap();
```

This is specifically correct when:
- The JIT is interpreter-first (blocks share execution time with interpreter)
- Most blocks are compiled once and run a moderate number of times
- Chain-compile-on-miss aggressively fills the cache with blocks that are used briefly

It would be wrong if blocks ran billions of times each (classic hot-loop JIT scenario), where spending more compile time for better native code pays off.

## Measurement
Use macOS `sample` (or Linux `perf`) on the running emulator. Count samples inside `cranelift_*` symbols vs total thread samples. Compare instructions/second before and after (log total count, divide by wall-clock).

