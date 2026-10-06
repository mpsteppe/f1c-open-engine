# DS12 — road-height query foundation

**Next action — DeepSeek:** execute this handoff; report in handoffs/DS12_REPORT.md. No commit.

- **Role:** Team B implementer. Read AGENTS.md, CLEAN_ROOM.md, specs/GROUND_QUERY.md, specs/DRIVE.md and reviews/DS11_REVIEW_2026-10-06.md. Read only this workspace and registered runtime game data. No agents/delegation needed.
- **Goal:** implement the GROUND_QUERY contract, including mesh-scoped SCN flags, headless indexed height queries and opt-in --drive --ground-probe diagnostics. This batch does not make the car follow slopes.
- **Preserve:** accepted DS11 coordinator fixes and all pending workspace edits. Bevy stays =0.19.1. Do not rewrite unrelated docs/research, commit, hard-delete or write into F1.
- **Architecture:** geometry independent of Bevy; static index built once; selected hidden surfaces loaded independently of render geometry; explicit preflight errors. API choices are yours within the contract.
- **Tests:** invented values/names, numerical/layer/index/parser oracles and existing regressions. Runtime tests skip only when no game root is configured; a configured acceptance sample that is missing or fails must fail, not report success by skipping.
- **Evidence:** real sample measurements and failures go in records/research/DS12_GROUND_QUERY_2026-10-06.md, linked from INDEX.md and DS12_REPORT.md. State data-tier paths, reproducible commands, units, limits and engine/modding uses. Extracted files/scratch outputs only under %TEMP%/f1c_openengine.
- **Report:** exact commands/results, unit versus optional runtime counts, changed files, surface counts, spawn hit and omissions. Update viewer README with a short example and ground-probe-only label. Do not count a launch as owner validation.
- **Stop:** after three attempts on the same blocker, report reproducible failure and remaining work. Never guess original flag defaults, silently use a flat fallback or select meshes by TRACK name.
- **Finish:** checks below pass; leave a viewer open for the owner if runtime preflight succeeds, give the restart command, then stop. No commit. Coordinator reviews before acceptance.

Run separately:

```powershell
cargo fmt --all -- --check
```

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

```powershell
cargo test --workspace
```

```powershell
cargo build --release -p viewer
```

Use %USERPROFILE%/.cargo/bin/cargo.exe if PATH lacks cargo.

Owner restart after checks:

```powershell
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --ground-probe
```

**Owner check:** surface height/source appears; short drive updates it; reset/pause and T/L work. Car still stays on the flat plane.
