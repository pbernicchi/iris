# JIT block chaining needs MAX_CHAIN_INSTRS cap for interrupt timing

**Keywords:** jit, chaining, interrupt latency, MAX_CHAIN_INSTRS, interpreter burst, cp0_count, timing, ugly login
**Category:** jit

# JIT Chain Length Affects Interrupt Delivery

Block chaining (running one cached block after another without returning to the interpreter burst) must be capped by cumulative instruction count, NOT by chain block count. 32 instructions is safe; 64 causes "ugly login" / timing-dependent corruption.

## Why
The interpreter checks pending interrupts before every single instruction. The JIT defers interrupt checking to post-block bookkeeping (cp0_count advance + merge IP bits into cp0_cause). Without chaining, a block exit returns to the interpreter burst which immediately sees the merged interrupts.

With chaining, multiple blocks execute back-to-back. Each chained block does its own post-block cp0_count advance and IP-bit merge, so timer interrupts get set in cp0_cause correctly — BUT the actual interrupt dispatch is deferred until the chain ends and the interpreter runs again. Worst-case interrupt delivery latency = MAX_CHAIN_INSTRS.

IRIX has code paths that depend on interrupt timing tight enough that 32 instructions is tolerable but 64 is not. Measured empirically: MAX_CHAIN_INSTRS=32 boots cleanly, =64 produces timing-dependent boot failures.

## How to apply
In the chain loop in `run_jit_dispatch`, accumulate `chain_instrs += next_block_len` and `break` when it reaches 32. Don't check "is interrupt pending" inside the chain — IRIX's level-triggered device interrupts (IP2-IP6) are almost always asserted, which would break every chain after one block.

If timing-related crashes reappear after touching chain code, **reduce MAX_CHAIN_INSTRS before debugging codegen**. The user's own heuristic, validated in practice.

## History
Initial chaining implementation checked `interrupts_enabled() && (cp0_cause & cp0_status & IM) != 0` to break the chain. Broke every chain immediately because devices are constantly asserted → JIT% barely moved. Removing the check but keeping MAX_CHAIN_INSTRS=32 gave clean boots with 3-4x more JIT coverage.

