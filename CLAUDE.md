# Claude Instructions

## INVARIANTS (non-negotiable)

These are hard rules. Violating any of these is a session failure.

- **MCP TOOLS BEFORE EVERYTHING**: `search` first!! `search` everything before you do it so you don't flail around.`report_error` before attempting any fix. `check_compat` before writing any compat function. `search` before inventing any technique. NO EXCEPTIONS. Do not skip these because you think you already know the answer — your training data for IRIX is outdated and wrong. **ENFORCED AT TWO LEVELS**: (1) The knowledge MCP tracks MCP tool calls — warning at turn 3, blocking at turn 6+ (nudge_escalation_threshold=2). (2) A Claude Code PreToolUse hook tracks built-in tool calls (Edit, Write, Bash) — warning after 8 calls without MCP search, **Edit/Write BLOCKED after 20 calls**. The hook catches the blind spot where the MCP nudge system can't see built-in tools. Both levels reset when you call search/report_error/check_compat.
- **NO FIXES OUTSIDE MOGRIX RULES**: Every fix goes into `rules/`, `compat/`, or `patches/`. If you `sed` a file during debugging, that fix MUST end up in a YAML rule. If it doesn't, you have failed.
- **NO INLINE C IN YAML**: C files go in `patches/packages/<pkg>/`, referenced via `add_source`. No heredocs generating .c/.h files in `prep_commands`.
- **`add_rule` IMMEDIATELY AFTER FIX CONFIRMED**: The moment a build passes after a fix, call `add_rule` with `file_path` pointing to the authoritative rule file. Do not batch to session end — context pressure causes deferred `add_rule` calls to be dropped.
- **DB IS CACHE, FILES ARE AUTHORITATIVE**: Rule files (`rules/packages/*.yaml`, `rules/generic.yaml`, `compat/catalog.yaml`, `rules/methods/*.md`) are the source of truth. `add_rule` must include `file_path`.
- **DELEGATE LONG DEBUGS**: >2 failed fix attempts for the same error → stop and spawn a sub-agent with `Task()`. Pass it the error text, file paths, and tell it to use MCP tools first. Never let debug trace flood parent context.
- **REDIRECT BUILD OUTPUT**: Never let rpmbuild output flood context. Log to file. Use sub-agents (`Task(model="haiku")`) for reading large build logs.
- **INVOCATION**: `uv run mogrix <command>`. No other invocation method works.

---

## Session Protocol

1. Call `session_start` MCP tool
2. Work — use MCP tools for every error, every symbol, every lookup
3. `add_rule` immediately after each confirmed fix
4. Call `session_handoff` MCP tool before ending

---

## MCP Tool Quick Reference

These are your primary interface. Use them before reading files, before grepping, before guessing.

| When | Tool | What it does |
|------|------|--------------|
| Hit any error | `report_error` | Logs error AND auto-searches rules+compat+errors in one call |
| Need to look something up | `search` (or `knowledge_query`) | FTS5 search across all knowledge, rules, errors, negative knowledge |
| Confirmed a fix | `add_rule` | Stores the fix with `file_path` to authoritative rule file |
| Learned something | `add_knowledge` (or `report_finding`) | Stores findings, decisions, insights |
| Found a dead end | `add_negative` | Stores anti-patterns so they're never repeated |
| Session start | `session_start` | Context summary, last handoff, active tasks |
| Session end | `session_handoff` | Snapshot state for next session |


## Context Management

**Tuned for 1M context (Opus 4.6).** Sessions can safely run 400+ turns. Compaction/handoff urgency is low. Focus is on knowledge capture quality, not checkpoint frequency.

- **Sub-agents for investigation**: Any task requiring >200 lines of output gets a sub-agent. `Task(model="haiku")` for build log reading. Sub-agent investigates and returns a concise summary; parent applies the fix.
- **Re-orientation check every 8 tool calls**: Am I using MCP tools? Am I freestyling a fix that's probably already documented? Have I stored my findings? If unsure, call `session_start`.
- **Store knowledge continuously**: `report_error` when you hit it → fix it → build passes → `add_rule` right then. Don't accumulate findings to store later. The nudge system fires a store reminder after 6 turns without a store.
- **Checkpoint at 30 turns**: `save_snapshot` or `session_handoff` to reset the checkpoint counter. Mandatory stop at 60 turns (enforced, blocks all tools).
- **Batch builds**: Max 2-3 background agents, each with its own rpmbuild directory. Only the orchestrator updates rule files. See `rules/methods/task-tracking.md`.

**MCP enforcement thresholds** (mcm-engine.yaml):
- Store reminder: 6 turns
- Checkpoint: 30 turns
- Mandatory stop: 60 turns (+10 grace)
- Nudge escalation: 2 ignores → blocking
- MCP-first enforcement: warning at turn 3, blocks at turn ~7 if no search/report_error/check_compat called

---

