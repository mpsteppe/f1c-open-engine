# DS14 — swept barrier-stop prototype

**Next action — DeepSeek:** execute this handoff; write handoffs/DS14_REPORT.md. No commit.

- **Read:** AGENTS.md, CLEAN_ROOM.md, specs/BARRIER_STOP.md, specs/ROAD_FOLLOW.md and reviews/DS13_REVIEW_2026-10-06.md. Team B: workspace/registered runtime data only. No delegation required.
- **Goal:** visible steep CollTarget geometry, headless swept-sphere queries, opt-in --barrier-stop and fixed-step atomic latched stop. Follow the contract; preserve pending edits and Bevy =0.19.1.
- **Keep bounded:** one sphere, conservative previous-pose stop. No gravity, bounce, sliding, damage, trigger logic, moving collisions or generic vehicle physics expansion.
- **Test:** meaningful invented face/edge/vertex/high-speed/overlap/timing and state regressions. Runtime configured sample must fail if missing; unset root may skip. A diagnostic sweep against loaded geometry is not an owner driving collision check.
- **Record:** measurements/negative results in records/research/DS14_BARRIER_STOP_2026-10-06.md plus INDEX/report links; exact source locations, commands, units, limits and engine/modding uses. Scratch/assets under %TEMP%/f1c_openengine only. No game writes/copied fixtures.
- **Report:** exact checks/counts, changed files, selected/retained counts, spawn proxy clearance, named runtime diagnostic sweep, limits and owner command. No inferred visual approval.
- **Stop:** after three attempts on a repeated blocker, report it with reproducible failure. Do not hide failures by disabling barriers, including hidden timing meshes or widening proxy/selection rules.
- **Finish:** checks pass and spawn clear, open viewer for owner short drive then stop. No commit, hard deletes or unrelated rewrites.

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

Owner restart:

```powershell
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --road-follow --barrier-stop
```
