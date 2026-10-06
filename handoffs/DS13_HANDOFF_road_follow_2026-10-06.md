# DS13 — constrained road-following prototype

**Next action — DeepSeek:** execute this handoff and write handoffs/DS13_REPORT.md. No commit.

- **Role:** Team B. Read AGENTS.md, CLEAN_ROOM.md, specs/ROAD_FOLLOW.md, specs/GROUND_QUERY.md, specs/DRIVE.md and reviews/DS12_REVIEW_2026-10-06.md. Workspace and registered runtime data only. No delegation required.
- **Goal:** opt-in --road-follow updates height/tilt from the DS12 query per fixed step, with last-surface reference and a latched stop on lost/discontinuous surface. Follow ROAD_FOLLOW.md exactly.
- **Preserve:** pending DS11/DS12/coordinator edits; ordinary flat/probe modes; fixed-step/input semantics; Bevy =0.19.1. No general physics dependency, game-folder writes, hard deletes or commit.
- **Headless:** implement/test follower state, atomic proposed-step acceptance, pose basis and loss/reset independently of Bevy. Do not add a render-frame-only height hack or mutate spawn Y as moving state.
- **Viewer:** preflight and full reflected pose, chase camera, readable concise HUD with dark backdrop, following/lost labels; T/L preserve index and state. Road-follow overrides probe-only labeling when both flags are supplied.
- **Evidence:** invented fixtures; configured runtime scenario fails if sample missing. Write records/research/DS13_ROAD_FOLLOW_2026-10-06.md and link INDEX/report. Record reproducible input duration, per-step height range/jump, sources/loss and prototype limits. Scratch/extracted assets under %TEMP%/f1c_openengine only.
- **Checks/report:** run checks separately below; report exact totals, changed files, oracle results, real scenario including negative results, omissions and owner command. No inferred owner approval from launch.
- **Stop:** after three attempts on the same blocker, report it with unfinished items. Do not add gravity/suspension/collisions or silently widen height thresholds to make the sample work.
- **Finish:** if preflight/checks pass, open viewer for owner short drive and stop. No commit; coordinator review required.

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

Use %USERPROFILE%/.cargo/bin/cargo.exe if cargo is absent from PATH.

Owner restart:

```powershell
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --road-follow
```

**Owner check:** brief acceleration/steering, visible slope height/tilt, readable HUD, R/P/T/L; if surface lost appears, car stops and R restores spawn. No full lap needed.
