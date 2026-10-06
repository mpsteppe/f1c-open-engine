# Clean-room rules

Purpose: keep the code publishable later. Not legal advice. Source tiers are in
[sources.toml](sources.toml).

| Tier | Who may read | Example |
|---|---|---|
| data | everyone, at run time only, never committed | game MAS/GDB/AIW files |
| clean | everyone | public docs, ISI modding doc, black-box observations |
| spec | spec writer (Claude, Team A) only | decompile, findings.jsonl, SDK docs, mod sources |
| forbidden | nobody | api1.txt, keys |

Team A (Claude) uses EVERYTHING, including spec tier, and writes specs in
`specs/` that describe behaviour, formats and numbers only: no code, no
pseudocode, no addresses, no function names from the decompile. Each spec
records provenance per fact.

Research must also have a durable, dated record and an entry in
`records/research/INDEX.md`, as required by AGENTS.md. These records are reusable
behaviour/format/measurement notes for both Open Engine and original-game
modding. They follow the same content restrictions as specs. Restricted raw
evidence remains outside this workspace; provenance links do not grant Team B
permission to open spec-tier sources. The coordinator may maintain a fuller
record in the F1 workspace under its own rules and link the two records.

Team B (DeepSeek) reads only this workspace (`specs/`, `handoffs/`, `crates/`)
plus data tier via `tools/f1src.py`. `knowledge` command refuses without
`--spec-writer`. Never paste game data into source or tests; tests read game
files at run time and skip when absent.

Limitation: separation is procedural, not technical. Claude reviews every
Team B diff for leaked addresses/decompile names.
