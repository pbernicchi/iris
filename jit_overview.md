# IRIS Adaptive JIT — How We Taught an Emulator to Learn

## The Problem

IRIS emulates an SGI Indy (MIPS R4400) well enough to boot IRIX 6.5 to a graphical desktop. But the interpreter tops out at ~30 MIPS on x86_64. We wanted a Cranelift-based JIT compiler to go faster.

First attempt: compile everything. Result: **hang**. Loads and stores in the same compiled block caused Cranelift to generate bad register spill code on x86_64 (only 15 usable registers vs AArch64's 31). Weeks of debugging.

## The Insight

Instead of fixing one bug and praying, make the JIT **fix itself**.

## How It Works

Every compiled block starts at the safest level and earns its way up:

```
Tier 0 (Alu)    Pure math + branches. Can't go wrong.
Tier 1 (Loads)  Add memory reads. Might hit TLB misses.
Tier 2 (Full)   Add memory writes. Full native speed.
```

**Lifecycle of a block:**
1. First seen → compile at Tier 0, mark **speculative**
2. Before each speculative run → snapshot the entire CPU (~2.3 KB)
3. Block runs clean 50 times → **trusted** (no more snapshots)
4. Trusted for 200 runs → **promote** to next tier (speculative again)
5. Block causes 3 exceptions at new tier → **demote** back, recompile

If a speculative block misbehaves, CPU state is rolled back from the snapshot and the interpreter re-runs the instruction correctly. The system never crashes — it just learns that block isn't ready yet.

## Bugs Found Along the Way

1. **SSA register pressure** — Cranelift's exception paths referenced values across block boundaries. Fixed by flushing modified registers before each helper call.

2. **Delay slot skip** *(the real killer)* — MIPS branches have a "delay slot": the instruction after a branch always executes. The JIT's tracer included load instructions in delay slots but the compiler's tier gate silently skipped them. Every branch with a load delay slot (extremely common in MIPS) produced wrong results. One-line fix.

## Profile Cache

Hot block profiles are saved to `~/.iris/jit-profile.bin` on shutdown. Next boot, blocks are pre-compiled at their proven tier — skipping the entire warmup.

## Results

```
Run with IRIS_JIT=0:  boots ✓  (interpreter only)
Run with IRIS_JIT=1:  boots to graphical desktop ✓
                      73,015 blocks compiled
                      4,036 promotions, 6 demotions, 145 rollbacks
                      0 crashes
```

The JIT is now self-correcting. It starts conservative, learns what's safe, and backs off when it's wrong. The emulator doesn't need us to manually decide what to compile — it figures it out at runtime.
