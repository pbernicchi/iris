# JIT delay slot fault handling — exclude ALL faulting instructions

**Keywords:** jit, delay slot, loads, stores, cp0_epc, BD bit, branch, ERET, in_delay_slot, trace_block
**Category:** jit

# JIT Delay Slot Fault Handling

Any instruction that can fault (load, store, FPU op, COP0 side effect, etc.) MUST be excluded from JIT-compiled branch delay slots.

## Why
If a delay slot instruction faults (TLB miss, bus error), the JIT exception path runs `sync_to_executor`, which explicitly clears `in_delay_slot` and `delay_slot_target` (context.rs, by design — compiled blocks normally handle their own delay slots).

Then `exec.step()` re-executes at the faulting PC without delay-slot context. If it faults again, `handle_exception` sets `cp0_epc = faulting_PC` with **BD=0**. On ERET, the CPU returns to the faulting load/store, not to the branch. **The branch is permanently skipped** — execution diverges silently until something crashes.

## Symptoms
Process crashes mid-boot, "ugly login screen", graphics corruption, TLB panics. Appears as silent state corruption, not a direct fault — the divergence accumulates before manifesting.

## How to apply
In `src/jit/dispatch.rs` `trace_block`, when inspecting the delay slot instruction after a branch:
```rust
let delay_can_fault = is_compilable_load(&delay_d) || is_compilable_store(&delay_d);
if is_compilable_for_tier(&delay_d, tier) && !delay_can_fault {
    // compile delay slot into block
}
// else: drop delay slot AND the branch (pop both; block ends before branch)
```

Whenever adding a new JIT-compilable instruction type that can fault (e.g., LWL/LWR, LL/SC, FPU loads/stores), extend `delay_can_fault` to exclude it from delay slots.

## History
The codebase already excluded stores from delay slots with a detailed comment, but the same fix wasn't applied to loads. Adding Loads tier silently corrupted IRIX boot for weeks until binary-searching block length (max=1 works, max=3 fails) isolated it.

