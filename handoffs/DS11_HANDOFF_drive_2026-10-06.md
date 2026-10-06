# DS11 — first driving prototype

**Next action:** DeepSeek executes this handoff; writes handoffs/DS11_REPORT.md; no commit.

## Work order

- **Role:** Team B implementer. Read AGENTS.md, CLEAN_ROOM.md, specs/DRIVE.md and specs/PHYSICS_FILES.md. Read only this workspace; runtime data through the registered bridge. Do not open the external research references in DRIVE.md. No agents/delegation required.
- **Goal:** --drive adds opt-in flat-ground driving to SCN + --car + --grid. Implement the DRIVE.md contract exactly; preserve ordinary viewer modes.
- **Model:** add a small headless crate physics-drive, independent of Bevy, for validated configuration, state, controls and fixed-step motion. API details are your choice. Keep existing Bevy pinned =0.19.1; avoid a general physics dependency for this prototype.
- **Formats:** extend formats-hdv tire parsing for selected Front/Rear radii; retain existing compound-name behaviour. Validate complete raw gear/torque data and required PM geometry before driving. No silent compacting, invented car defaults or copied game fixtures.
- **Viewer:** CLI validation, fixed steps, queued input edges, manual gears, pause/focus handling, reset, chase camera and concise HUD. Use persistent local car geometry and entity transforms; moving must not rebuild the track or reload assets. Handle T/L while moving.
- **Tests:** implement the meaningful acceptance tests in DRIVE.md, including the numerical oracle and frame grouping. Small invented fixtures only; scratch files under %TEMP%/f1c_openengine. No hard deletes.
- **Documentation:** add a short README drive example and explain the flat-plane/no-collision limit. Report exact checks/counts, changed files, omissions, numeric test results and real-car configuration validation. Distinguish headless validation from owner visual approval.
- **Acceptance:** fmt/clippy/workspace tests and release viewer build pass. Headless runtime validation accepts Ferrari28. Open viewer for owner only after checks pass; report the command and controls. Do not infer visual success from launching.
- **Stop:** after three failed attempts on the same blocker, report the error and unfinished items. Do not guess missing game behaviour or expand into ground/suspension physics. No commit; coordinator reviews every batch.

## Checks — run individually

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

If cargo is absent from PATH, use the installed executable at %USERPROFILE%/.cargo/bin/cargo.exe.

## Owner run

```powershell
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive
```

- **Controls:** W gas, S brake, A/D steer, E/Q shift, R reset, P pause, T textures, L lighting, Escape exit.
- **Owner check:** short start-area drive only. Flat ground can clip into or float above slopes; no barriers/collisions in DS11.
