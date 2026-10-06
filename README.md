# F1C Open Engine (working name)

Clean-room, F1 Challenge '99-'02 compatible engine in Rust + Bevy. Loads the
player's own game data; ships no game assets. Owner: Matias.
Coordinator workspace is separate from the F1
modding workspace. Public prototype snapshot published at the owner's request.
Independent legal clearance has not been established; no game assets or decompiled source are included.

Feasibility, legal blockers, token budget:
`C:\F1Research\research\OPEN_SOURCE_REWRITE_FEASIBILITY_2026-10-06.md`

## Roles

- Claude: coordinator, spec writer (Team A) and QA. Reviews every DeepSeek batch.
- DeepSeek: implementer (Team B), specs only. Rules: [CLEAN_ROOM.md](CLEAN_ROOM.md).
- Matias: decisions, human validation, legal.

## Reusing the F1 workspace

[sources.toml](sources.toml) registers every reused F1 asset with its tier.
`python tools/f1src.py` lists sources, finds game files, reads/extracts MAS
entries to `%TEMP%\f1c_openengine`, and (spec writer only) queries the SDK
knowledge base.

## Stage 1 objective

**Current progression:** the original Stage 1 below is complete within its
defined scope. DS11–DS13 motion, surface queries and kinematic road following are accepted prototypes. DS14 barrier stop passed the owner check; final review/rebuild is pending. See reviews/DS14_REVIEW_2026-10-06.md.
See [ROADMAP.md](ROADMAP.md) for full game coverage, remaining systems, evidence
and milestone exit gates. See [records/NEXT.md](records/NEXT.md) for the next action.

1. `formats-mas`: read CUBEMAS4.10 archives ([spec](specs/MAS_FORMAT.md)).
2. `formats-mts`: parse MTS models (F1C uses gMotor 1 .MTS, not GMT).
3. `viewer`: Bevy app opens a car MAS and shows the model.

Non-goals: physics, AI, gameplay, archive writing, shipping game assets.
Acceptance: `cargo test` green; MAS crate reads all installed archives with
exact sizes; Matias sees a car model in the viewer.

## Layout

`specs/` format specs · `handoffs/` DeepSeek work orders · `reviews/` QA
reports · `crates/` code · `records/` status · `tools/` bridge.

`crates/physics-drive` is the headless DS11 driving prototype (validated
configuration, fixed-step motion, input queue); the viewer's `--drive` mode
renders it. See `specs/DRIVE.md` and `crates/viewer/README.md`.

## Contributors

Read [CONTRIBUTING.md](CONTRIBUTING.md), [CLEAN_ROOM.md](CLEAN_ROOM.md) and [ROADMAP.md](ROADMAP.md). Build/test without game files using cargo test --workspace; optional real-data tests use F1C_GAME_DIR. This Windows prototype loads assets from your own installation.

Publication history is in [records/GITHUB_PUBLICATION.md](records/GITHUB_PUBLICATION.md). CI/contribution templates are prepared locally; GitHub checks have not run yet. Code license files match existing MIT OR Apache-2.0 manifests; publication/provenance gates remain to be resolved. See LICENSING.md.
