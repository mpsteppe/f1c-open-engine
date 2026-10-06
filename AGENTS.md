# F1C Open Engine — agent instructions

Separate workspace from the F1 modding workspace (`C:\F1Research`).
F1 workspace rules apply when touching F1 paths; here these rules apply.

## Cold start

1. Read README.md, CLEAN_ROOM.md, sources.toml, records/STATUS.md.
2. Know your role: spec writer (Claude) or implementer (DeepSeek). Implementers
   read only this workspace; see CLEAN_ROOM.md.
3. Reach F1 material only through `python tools/f1src.py` or the paths in
   sources.toml. Never copy game files or decompile output here.

## Rules

- F1 workspace is read-only from here. Never write into it except records the
  coordinator adds under its own rules.
- Never read `api1.txt` or private keys.
- No hard deletes; this workspace is a git repo, commit small steps.
- Per-run output and extracted game files go to `%TEMP%\f1c_openengine`.
- Each DeepSeek batch: handoff in `handoffs/`, Claude review in `reviews/`,
  status line in records/STATUS.md.
- Text files UTF-8 without BOM.
- Rust: pin Bevy version; `cargo test` must pass before review.

## Research records — reusable for engine and original-game mods

- **Record every investigation:** write or update a dated note under
  `records/research/` before treating research as complete. Chat, tool output
  and a handoff alone are not the permanent research record.
- **Make it findable:** maintain `records/research/INDEX.md` with the question,
  topic, date, result and links to the note, related specs and existing F1
  research records. Reuse an existing note for follow-up work.
- **Keep evidence:** state the question, source tier and exact source location,
  method, reproducible command/check, result with units, confidence and limits.
  Link evidence files; scratch output stays under `%TEMP%\f1c_openengine`.
  Record enough context to reproduce results if scratch files disappear.
- **Record failures too:** keep negative results, rejected hypotheses,
  contradictions, corrections and unresolved questions. Separate observed facts,
  static deductions, assumptions and prototype design choices. Do not describe
  our engine's chosen behaviour as a finding about the original game.
- **Explain both uses:** each note says what the result changes for Open Engine
  and what it could enable or fix in original-game modding. State when a result
  has no known modding use; untested ideas stay labelled as untested.
- **Preserve clean-room separation:** notes here contain behaviour, formats,
  measurements and provenance only. No copied game assets, decompile code,
  pseudocode, addresses or decompile function names. Restricted evidence stays
  in its registered external source; only Team A may read it.
- **Share through the coordinator:** link the distilled note from relevant
  specs/handoffs/reviews. When recording it in the F1 workspace, the coordinator
  follows that workspace's rules and links both records. Writing an F1 research
  record does not authorize changing game files or mods.
- **Continuity:** after research, update NEXT and STATUS with the result and
  record link. Do not commit while DS runs; note pending edits and commit them
  with the next coordinator step after DS has stopped.

## User-facing output: ADHD friendly (all agents, all tools)

Applies to chat replies, reports (`handoffs/*_REPORT.md`, `reviews/`), README
sections, CLI output, viewer/UI text — anything Matias reads.

- Next action first: one line, what Matias does or decides now.
- Short bullets. No walls of text. One idea per bullet.
- Bold the key word in each bullet when it helps scanning.
- Max ~10 bullets per message; put detail in a linked file.
- Plain words; explain jargon once or avoid it.
- Questions to Matias: one at a time, yes/no or numbered options.
- State results plainly: pass/fail, numbers, what is blocked.
- Commands in their own code block, one per block, ready to run.
- UI/CLI: clear defaults, short labels, errors say what to do next.

## Coordinator continuity (Claude): always handoff-ready

- `ROADMAP.md` is the project-wide progression and game-coverage dashboard.
  After every reviewed batch, research result or scope decision, update its
  affected capability rows, milestone gates and evidence links. Keep parsing,
  prototypes and validated original-game behaviour distinct. Do not count
  active/unreviewed DS files as accepted work or invent a whole-game percentage.

Matias must be able to open a new conversation at any time and just say
"continue". No manual handoff request.

- `records/NEXT.md` is the single resume point. Rewrite it (not append) after
  EVERY unit of work: spec written, DS batch reviewed, fix applied, owner
  check received, decision made.
- NEXT.md contents (ADHD format, short):
  1. **Next action** (who does what, exact prompt/command).
  2. **Waiting on** (DS batch, owner check, owner decision).
  3. **Current state** (last accepted batch, commit hash, what works).
  4. **Open questions / known issues.**
  5. **Lessons** that change how to work (one line each).
- Also append one dated line to `records/STATUS.md` (history) and commit when
  DS is not running. While DS runs, save the edits and record the pending commit.
- New conversation cold start: read AGENTS.md, then `records/NEXT.md`, then
  act. Do not re-read old handoffs/reviews unless NEXT.md points to them.
