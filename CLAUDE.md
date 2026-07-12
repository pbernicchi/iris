# CLAUDE.md — IRIS (SGI Indy emulator)

## What this is
Rust emulator of an SGI Indy (MIPS R4400) that boots IRIX 6.5 and 5.3, with
networking, Newport/REX3 graphics, audio, and optional Cranelift JITs.
Single crate; binaries `iris` (src/main.rs) and `coffdump` (src/coffdump.rs).

## Key commands (verified)
```
cargo run --release                                  # normal run (needs iris.toml + prom.bin + scsi1.raw)
cargo run --release --features developer             # intrusive debug helpers (slower)
cargo run --release --features lightning,rex-jit     # fastest; lightning disables breakpoints
IRIS_JIT=1 cargo run --release --features jit        # MIPS JIT (feature at build, env at runtime)
cargo test                                           # unit tests (CI runs build + test, ubuntu)
cargo build --profile developer                      # release opt + debug symbols
./jit-diag.sh [-f features] [jit|verify|nojit|interp|perf|perf-jit|smoke]  # logs to jit-diag-*.log
./profile.sh                                         # cargo flamegraph, --profile profiling
```
Features (Cargo.toml): `jit`, `rex-jit`, `lightning`, `developer`, `developer_ip7`,
`developerx`, `debug_cache`, `mouseabs`. JIT env vars: `IRIS_JIT`, `IRIS_JIT_MAX_TIER`,
`IRIS_JIT_VERIFY`, `IRIS_JIT_PROBE`.

## Runtime interfaces
- Config: `iris.toml` (paths relative to cwd; CLI flags override). `--headless`, `--noaudio`, `--2x`.
- Monitor/debugger: TCP 127.0.0.1:8888 (`nc localhost 8888`). Commands in `debug.md`
  and `HELP.md`; `help` inside the monitor lists all (incl. `cow`, which is only there).
- Serial ports: 8880 (A), 8881 (B). Guest telnet via port-forward: `telnet 127.0.0.1 2323`.
- `src/iris_mcp.py` is a FastMCP server ("iris-monitor") that drives the monitor on 8888.
- Required local assets (gitignored, not in repo history): `scsi1.raw` (2 GB IRIX disk image),
  `prom.bin`. Never delete or rewrite them.

## Architecture & key files
- `HACKING.md` — read first: data path, concurrency model, bus traits, ExecStatus bitfield.
- `src/mips_core.rs` (register state), `src/mips_exec.rs` (executor), `src/mips_cache_v2.rs`,
  `src/mips_tlb.rs`, `src/physical.rs` (64KB-page address decode, deliberate unsafe).
- Devices: `mc.rs` (crossbar), `hpc3.rs`, `rex3.rs`/`vc2.rs`/`xmap9.rs` (graphics),
  `scsi.rs`/`wd33c93a.rs`, `seeq8003.rs`+`net.rs` (ethernet), `hal2.rs` (audio),
  `z85c30.rs` (serial), `ioc.rs`/`ps2.rs` (input).
- JITs: `src/jit/` (MIPS), `src/rex3_jit/` (graphics shaders).
- `docs/` — SGI hardware datasheets/PDFs. `rules/` — hard-won debugging lessons (markdown).

## Conventions & gotchas
- **Read `rules/jit/dispatch-architecture.md` before touching the JIT dispatch loop.**
  Other lessons: `rules/jit/*.md`, `rules/irix/*.md`, `rules/testing/*.md`.
- Endianness: word-transparent architecture; swapping happens only at the edge
  (PROM/disk I/O). Never suggest `.to_be()`/`.to_le()` in memory or register logic.
- Each device runs in its own thread and locks its own state; deadlocks live in
  child→parent callbacks (e.g. SCSI → HPC3). DRAM access is deliberately unsynchronized.
- `ExecStatus` is a `u32` bitfield, not an enum (see HACKING.md §6).
- Not cycle-accurate anywhere, on purpose. Don't add accuracy that slows things down.
- When a confirmed fix teaches a durable lesson, record it as a markdown file under
  the matching `rules/` subdirectory (and via the knowledge MCP `add_rule` with that
  file_path, if the server is connected). Files are authoritative; the DB is a cache.
- `mcm-engine.yaml` configures the per-project knowledge server (db at
  `.claude/knowledge.db`); the repo has no committed `.mcp.json`, so wiring is local.

## Hard rules (do-nots)
- **Never boot/test against the bare base disk image.** Keep `overlay = true` for
  `[scsi.1]` in iris.toml. Emulator crashes corrupt the XFS filesystem on `scsi1.raw`,
  and the resulting TLBMISS panics are indistinguishable from JIT bugs
  (see `rules/testing/disk-image-hygiene.md`).
- Never run the monitor command `cow commit` unless the user explicitly asks — it
  permanently merges overlay writes into the base image. `cow reset` discards them.
- Never let emulator or build output flood the conversation: redirect long runs to a
  log file (`jit-diag.sh` already does this) and read the log selectively or via a
  subagent.
- Don't use the `lightning` feature for interactive debugging — it removes breakpoints
  and traceback.
- Don't commit disk images, overlays, `prom.bin`, or logs (already gitignored).
