# IRIS Debugger Documentation

The IRIS emulator features a built-in monitor and debugger that allows for interactive inspection and control of the emulated machine. The monitor listens on TCP port 8888 by default.

## Connecting

You can connect to the debugger using `netcat` or `telnet`:

```bash
nc localhost 8888
```

## Execution Control

| Command | Alias | Description |
| :--- | :--- | :--- |
| `start` | | Start the CPU execution thread. |
| `stop` | | Stop the CPU execution thread. |
| `run [addr]` | `c`, `cont` | Continue execution. If an address is provided, runs until that address is hit (temporary breakpoint). |
| `step [count\|addr]` | `s` | Step `count` instructions (default 1). If an address is provided (e.g., `step 0x88001000`), runs until that address. |
| `next [count]` | `n` | Step over function calls (executes `jal`/`bal` as one unit). |
| `finish` | `fin` | Run until the current function returns (detects return address). |
| `jump <addr>` | | Force the PC to a specific address. |

## Breakpoints

Breakpoints can be set on execution (PC), memory reads, or memory writes.

| Command | Alias | Description |
| :--- | :--- | :--- |
| `bp list` | `bl` | List all defined breakpoints. |
| `bp add <addr> [type]` | `b` | Add a breakpoint at `addr`. Type can be `pc` (default), `r` (read), `w` (write), `f` (fetch), `pr` (phys read), `pw` (phys write), `pf` (phys fetch). |
| `bp del <id>` | `bb` | Delete breakpoint with the specified ID. |
| `bp enable <id>` | `be` | Enable a disabled breakpoint. |
| `bp disable <id>` | `bd` | Disable a breakpoint without deleting it. |

## Inspection

### Registers

| Command | Alias | Description |
| :--- | :--- | :--- |
| `regs` | `r` | Dump General Purpose Registers (GPRs), HI/LO, and key CP0 registers (Status, Cause, EPC, BadVAddr). |
| `cop0` | | Dump all Coprocessor 0 (System Control) registers. |
| `cop1` | | Dump Coprocessor 1 (FPU) registers and control/status registers. |

### Memory

| Command | Alias | Description |
| :--- | :--- | :--- |
| `mem <addr> [count]` | `m` | Dump virtual memory at `addr`. Default count is 1 word. |
| `mw <addr> <val> [size]` | | Write `val` to virtual memory at `addr`. Size can be `b` (byte), `h` (half), `w` (word), or `d` (double). Default is word. |
| `ms <addr> [max_len]` | | Read a null-terminated string from virtual memory. |
| `stack [addr] [count]` | | Dump stack memory. Defaults to current `$sp` if address is not provided. |
| `dis <addr> [count]` | `d` | Disassemble instructions at `addr`. |

### Translation & TLB

| Command | Alias | Description |
| :--- | :--- | :--- |
| `translate <addr>` | `t` | Translate a virtual address to a physical address using the current TLB and addressing mode. |
| `tlb dump` | | Dump all TLB entries. |
| `tlb trans <vaddr> [asid]` | | Debug translation of a virtual address with an optional ASID. |
| `tlb debug <on\|off>` | | Enable verbose logging of TLB operations. |

## Undo / Time Travel

The emulator maintains a circular buffer of previous CPU states, allowing you to step backwards in time.

> **⚠️ Warning:** The undo feature is powerful but can be **glitchy**. It tracks register changes and memory writes but may not perfectly restore peripheral state. It must be explicitly enabled.

| Command | Alias | Description |
| :--- | :--- | :--- |
| `undo on` | `u on` | **Enable** the undo buffer. |
| `undo off` | `u off` | Disable the undo buffer. |
| `undo [count]` | `u` | Step back `count` instructions (default 1). Reverses register and memory changes. |
| `undo clear` | | Clear the undo history buffer. |

## Tracing & History

| Command | Alias | Description |
| :--- | :--- | :--- |
| `dt [count]` | | **Disassemble Traceback**: Show the last `count` instructions executed by the CPU. Useful for seeing how you got to the current PC. |
| `bt [frames]` | | **Backtrace**: Attempt to walk the stack frames to show the call stack. |
| `debug <on\|off>` | | Enable verbose CPU instruction tracing (prints every instruction executed). |
| `trace uncached <on\|off>` | | Log all uncached memory accesses (useful for debugging I/O). |

## Exceptions

You can configure the debugger to stop execution when specific exceptions occur.

**Usage:** `exception <class|code|all> <on|off>`

| Class/Code | Description |
| :--- | :--- |
| `all` | All exceptions. |
| `int` | Interrupts. |
| `tlb` | TLB Refill / Invalid / Modified. |
| `addr` | Address Errors (Load/Store). |
| `bus` | Bus Errors (Instruction/Data). |
| `sys` | Syscall / Breakpoint. |
| `ri` | Reserved Instruction / Coprocessor Unusable. |
| `arith` | Arithmetic Overflow / Trap / FPE. |
| `watch` | Watchpoint. |
| `vce` | Virtual Coherency Exceptions. |

Example: `ex tlb on` will stop execution whenever a TLB exception occurs.

## Symbols

The debugger can load symbol maps (e.g., `prom.map`, `unix.map`) to display function names instead of raw addresses.

| Command | Description |
| :--- | :--- |
| `loadsym <file>` | Load a symbol map file (NM output format). |
| `sym <addr>` | Lookup the symbol nearest to `addr`. |

## Cache Debugging

Commands to inspect the internal state of the R4000 caches.

| Command | Description |
| :--- | :--- |
| `l1i <check\|dump> <addr\|index>` | Inspect L1 Instruction Cache. |
| `l1d <check\|dump> <addr\|index>` | Inspect L1 Data Cache. |
| `l2 <check\|dump> <addr\|index>` | Inspect L2 Unified Cache. |

## Example Session

```text
> start
CPU started
> bp add 0x88002000
Breakpoint 1 added at 0000000088002000 (Pc)
> run
... execution ...
PC=0000000088002000: Breakpoint 1 hit
> u on
CPU undo buffer enabled
> s
Exec: 88002000 <func+0x0>: 27bdffd8 addiu sp, sp, -40
> regs
... registers ...
> u
Undid 1 instruction(s), PC now at 0000000088002000
> dt 5
Execution Traceback (last 5 instructions):
...
```
## Execution Modes + +The emulator supports two distinct execution modes: + +1. Threaded Mode (start command):

Runs the CPU in a separate, dedicated thread.
Fastest execution speed, suitable for booting the OS or running applications.
Supports breakpoints (will stop execution when hit).
+2. Debug Mode (run, step, next, finish commands):

Runs the CPU in the monitor's thread.
Slower than threaded mode due to overhead.
Provides full debugging capabilities, including instruction tracing, stepping, and detailed status reporting.
Automatically stops the threaded mode if it was running.

## MCP Server Integration

The emulator includes a Model Context Protocol (MCP) server that exposes monitor commands as tools for AI assistants.

### Starting the Server

1. Start IRIS with the monitor enabled (default port 8888).
2. Run the MCP server script:
   ```bash
   python3 src/iris_mcp.py
   ```

### Available Tools

The MCP server exposes the following tools to connected clients:

| Tool | Description | IRIS Command |
| :--- | :--- | :--- |
| `run_command(cmd)` | Run raw monitor command | (any) |
| `read_memory(addr, count)` | Read memory words | `mem` |
| `write_memory(addr, val, size)` | Write memory | `mw` |
| `read_string(addr, max_len)` | Read string | `ms` |
| `get_registers()` | Dump GPRs | `regs` |
| `read_cop0()` | Dump CP0 regs | `cop0` |
| `read_cop1()` | Dump FPU regs | `cop1` |
| `step(count)` | Step instruction(s) | `step` |
| `next_instruction(count)` | Step over call | `next` |
| `continue_execution(until)` | Run (optional breakpoint) | `run` |
| `finish_function()` | Run until return | `finish` |
| `add_breakpoint(addr, kind)` | Add breakpoint | `bp add` |
| `remove_breakpoint(id)` | Delete breakpoint | `bp del` |
| `list_breakpoints()` | List breakpoints | `bp list` |
| `backtrace(frames)` | Show stack trace | `bt` |
| `traceback(count)` | Show execution history | `dt` |
| `undo(count)` | Undo instructions | `undo` |
| `translate_address(addr)` | VA to PA translation | `translate` |
| `dump_tlb()` | Dump TLB entries | `tlb dump` |
| `lookup_symbol(name_or_addr)` | Symbol lookup | `sym` |
| `disassemble(addr, count)` | Disassemble instructions | `dis` |