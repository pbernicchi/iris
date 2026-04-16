# JIT profile pre-compilation at startup causes PROM hang

**Keywords:** jit, profile, pre-compilation, PROM, debug_fetch_instr, CpuBusErrorDevice, MC CPU Error, boot hang
**Category:** jit

# JIT Profile Pre-compilation Breaks Boot

Pre-compiling above-Alu JIT blocks from a saved profile at startup causes the PROM to hang in a retry loop. Pre-compiling AFTER PROM exit causes IRIX kernel panics (UTLB miss).

## Why (startup variant)
Profile entries contain kernel/userspace virtual PCs from a previous session. At startup, the kernel isn't loaded yet — those physical addresses are served by `CpuBusErrorDevice` (the bus error catcher for unmapped regions). Each `debug_fetch_instr` during pre-compilation triggers `mc.report_cpu_error()`, dirtying `REG_CPU_ERROR_ADDR` / `REG_CPU_ERROR_STAT` on the emulated Memory Controller. The PROM reads those registers during hardware init, sees errors, and retries forever.

## Why (post-PROM variant, UTLB panic)
Even deferring pre-compilation until after PROM exit and compiling incrementally (64 entries per dispatch batch) triggered IRIX UTLB-miss panics shortly after kernel boot. Exact mechanism unknown — suspected that the bulk `debug_fetch_instr` calls evict L2 lines that the kernel's initial data structures depend on (L2 is inclusive of D-cache on emulated R4000), or nanotlb[Fetch] state interference. Unresolved.

## How to apply
The `load_profile()` call in `run_jit_dispatch` should NOT feed into `compile_block` during boot. Blocks compile on-demand when first hit, which is safe because the kernel is already resident by then.

If you want persistent compiled blocks across sessions, store the **compiled native code bytes** in the profile rather than re-tracing. That avoids `debug_fetch_instr` entirely.

## History
Discovered when investigating why max_tier=1 (Loads) hung in PROM while max_tier=0 (Alu) booted fine. MAX_TIER=0 skipped all pre-compilation (entries get capped to Alu, then `continue`); MAX_TIER≥1 actually traced and compiled. The MC:CPU Error messages during pre-compilation were the smoking gun.

