# DS11 report — first driving prototype

**Next action — Matias:** go to the open viewer window and do the short
start-area drive (controls below). Then tell the coordinator "check DS".

## What was built

- New headless crate **`crates/physics-drive`** (no Bevy): validated `CarSpec`
  from the parsed physics files, fixed 120 Hz `Sim`, input queue + pause
  `Session`, `FixedStepper` with 0.1 s catch-up cap.
- Extended **`formats-hdv`**: `EngineFile` now counts malformed `RPMTorque`
  rows; `TbcFile` separates `Front:` / `Rear:` `Radius` per compound.
- Viewer **`--drive`** on `.SCN` + `--car` + `--grid`: validates before opening,
  then one dynamic car on a flat plane at the grid Y, chase camera, HUD,
  fixed stepping, queued shifts, pause/reset, texture/lighting toggles.

## Checks (each run individually)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass (exit 0) |
| `cargo test --workspace` | pass; **166 passed, 0 failed** |
| `cargo build --release -p viewer` | pass (Finished release) |

Test counts: formats-aiw 7, formats-gen 19, **formats-hdv 42**, formats-mas 8,
formats-mts 13, formats-scn 10, **physics-drive 30**, **viewer 33**, plus 3
corpus tests and 1 `game_drive` test (all 1 passed; corpus/game tests skip
without their env var).

## Numerical oracle (`specs/DRIVE.md`)

All headless unit-test assertions pass with the spec tolerances:

- Rev-limit speed in the oracle gear: `41.887902048 m/s` (1e-8).
- Uncapped yaw rate at v=10, delta=10°, L=3: `0.587756602362 rad/s` (1e-9).
- First step: speed `0.040440416667 m/s`, forward distance
  `0.000337003472 m` (1e-9); derived acceleration `4.85285 m/s²` and
  resistance `117.72 N` follow from the force model.
- Frame grouping 1/2/3/4/5/12 steps and same 1 s duration at 30/60/144 FPS
  produce matching state within 1e-8.

Other covered checks: limiter cuts positive torque above the coupled limit;
neutral and zero throttle never add positive force; brakes stop without reverse;
brake priority; steering rate and lateral caps; left/right yaw signs; downshift
over-limit RPM stays finite with no speed jump; queued shifts survive a
zero-step frame, are not repeated across catch-up steps, and are cleared by
reset/pause; resume waits for key release; excess wall time is dropped.

## Real-car headless validation (not visual)

Ran with `F1C_GAME_DIR` set (no window opened by the test):

```
validated ...\1994_412T1\1994_Ferrari28.veh:
  mass 590 kg, gears 6, final drive 6.462, rev limit 15900, idle 3763,
  radii front/rear 0.322/0.330, wheelbase 2.875, steer lock 20.0 deg
```

This proves the loader + validation accept Ferrari28. It is **not** a
replacement for the owner's visual check.

## Changed files (uncommitted)

- `Cargo.toml`, `Cargo.lock` — add `physics-drive` member.
- `crates/physics-drive/` (new): `Cargo.toml`, `src/lib.rs`, `src/config.rs`,
  `src/sim.rs`, `tests/game_drive.rs`.
- `crates/formats-hdv/src/engine.rs` — `rpm_torque_invalid`.
- `crates/formats-hdv/src/tbc.rs` — Front/Rear radius + `compound_radii`.
- `crates/viewer/Cargo.toml` — depend on `physics-drive`.
- `crates/viewer/src/lib.rs` — `car_local` helper + tests.
- `crates/viewer/src/main.rs` — `--drive`, drive resources/systems, chase camera,
  HUD, input queue, texture-toggle rebuild at the current pose.
- `README.md`, `crates/viewer/README.md` — drive example and flat-plane limit.

## Omissions and limits (deliberate, per spec)

- Flat infinite plane at grid Y; **no collisions, walls, slopes, tire slip,
  suspension, aero, fuel, damage, reverse, automatic gears or audio.**
- Hard positive-torque cutoff and idle floor instead of the original limiter,
  clutch or stall; EngineMap and engine-braking effects omitted (coasting
  differs from the original).
- The prototype equations/constants are project choices, not original-solver
  reconstruction. No full-lap or handling-parity claim.
- Owner visual approval is **not** inferred; the window was opened for you.

## Owner run

```powershell
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive
```

Controls: **W** gas, **S** brake, **A/D** steer, **E/Q** shift, **R** reset,
**P** pause, **T** textures, **L** lighting, **Esc** exit. Flat ground can clip
into or float above slopes; short start-area drive only. Viewer window is open.
