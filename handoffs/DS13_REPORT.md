# DS13 report — constrained road-following prototype

**Next action — Matias:** short-drive the open viewer (`--road-follow`): confirm
the car follows surface height/tilt on the slope, the HUD is readable, and
`R`/`P`/`T`/`L` work; if surface is lost the car stops and `R` restores the spawn.
Then tell the coordinator "check DS". No commit.

## What was built

- **`physics-drive` road module** (`src/road.rs`, headless f64): the tangent-plane
  pose `FollowPose` (U normal, F horizontal-heading projection, L=F×U, B=-F,
  origin from the rear local anchor), the spawn-initialized `RoadFollower`
  (surface sample, pose, spawn copy, following/lost status), and the per-step
  `step_follow` propose/accept/reject.
- **`Session` integration** (`src/sim.rs`): optional follower,
  `advance_with_ground`, `set_road_follow`, `follower`; resets restore the spawn
  surface/pose and clear loss; lost state freezes the car until reset; pause/
  shift-queue/catch-up semantics unchanged for plain `advance`.
- **`viewer`**: `--road-follow` flag/validation (needs `.SCN` + `--car` +
  `--drive`), shared DS12 ground build, spawn preflight, per-step
  `advance_with_ground`, tangent-plane render transform, chase camera by
  horizontal heading, road-follow HUD with a dark translucent panel, and a
  `follow_view_transform` reflection helper with tests.
- Ground-probe and flat modes are preserved; when both `--ground-probe` and
  `--road-follow` are supplied, road-follow labelling wins.

## Checks (each run individually)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass (exit 0) |
| `cargo test --workspace` (with `F1C_GAME_DIR`) | pass; **216 passed, 0 failed** |
| `cargo build --release -p viewer` | pass (Finished release) |

Per crate: formats-aiw 7, formats-gen 19, formats-hdv 42, formats-mas 8,
formats-mts 13, formats-scn 18, ground-query 21, **physics-drive 45**,
**viewer 37**; optional runtime/corpus: formats-mas/mts/hdv 1 each,
physics-drive `game_drive` 1, viewer `ground_probe` 1, viewer `road_follow` 1.
Optional tests skip only when `F1C_GAME_DIR` is unset; with it set the sample
must pass. DS11/DS12 regressions stay green.

## Invented oracle and headless results

- **Slope oracle:** plane Y=1+0.5X; rear X=1/Z=1, height 1.5 m, yaw 0, rear local
  anchor (0,0,1.5): U=(-0.447213595499958,0.894427190999916,0), F=(0,0,-1),
  B=(0,0,1), origin (1,1.5,-0.5); anchor (0,0,1.5) -> (1,1.5,1) within 1e-9.
  **Spec correction:** `specs/ROAD_FOLLOW.md` prints L=(0.894,0,0.447), which is
  not orthogonal to F; the contract's L=F×U gives (0.894,0.447,0), the value used.
- Flat follow matches plain DS11 speed/yaw/XZ to 1e-9; only height changes.
  Spawn/reset anchor returns to contact. Yaw/offset basis stays orthonormal and
  right-handed. A gradual ramp climbed >2 m with every step <=0.25 m. Overlapping
  Y=1/Y=5 layers never snap. Hole / 0.5 m step / degenerate query latch loss,
  zero speed, idle RPM, clear queued shifts, and block motion until reset; reset
  while paused stays paused. 30/60/144 FPS match within 1e-8; zero-step frames
  preserve the surface sample.

## Real data — headless straight-throttle scenario

`F1C_GAME_DIR=C:\F1Research\F1 Challenge V10`:

```
selected 52 meshes, excluded 742, unsupported 0; retained 27755 triangles, rejected 14128
spawn surface: height 4.663 m, grid reference 4.709 m, difference -0.046 m,
  source TRACK02C.mts, candidates 371
scenario: 5.000 s, straight full throttle; accepted 600 steps;
  height range 4.561..4.663 m (range 0.102 m), max jump 0.0004 m;
  final following, source TRACK02C.mts, last height 4.561 m
```

Bounded and fully accepted on this sample; **negative result not encountered** —
no surface loss and no rejected step. This is sampled geometry, not original
physics. Full record:
[DS13_ROAD_FOLLOW_2026-10-06.md](../records/research/DS13_ROAD_FOLLOW_2026-10-06.md).

## Changed files (uncommitted)

- `crates/physics-drive/Cargo.toml` — add `ground-query` dependency.
- `crates/physics-drive/src/road.rs` (new) — pose, follower, step logic, tests.
- `crates/physics-drive/src/sim.rs` — `Session` follower integration.
- `crates/physics-drive/src/lib.rs` — module + re-exports.
- `crates/viewer/src/lib.rs` — `follow_view_transform` + tests.
- `crates/viewer/src/main.rs` — `--road-follow`, preflight, resources, render
  transform, HUD, dark panel.
- `crates/viewer/tests/road_follow.rs` (new) — runtime acceptance.
- `crates/viewer/README.md` — road-follow example + label.
- `records/research/DS13_ROAD_FOLLOW_2026-10-06.md`, `records/research/INDEX.md`.
- `Cargo.lock` if the added dependency rewrote it.

## Omissions and limits (deliberate, per spec)

- No gravity, airborne motion, suspension, tire contact, traction, barriers,
  collisions or lap timing; horizontal forces stay DS11.
- One sampled rear-axle anchor; tangent-plane placement does not guarantee all
  four tires contact uneven road. Closely stacked surfaces inside the 0.25 m/2 m
  envelope can still be ambiguous.
- Selection and thresholds are prototype choices, not original HAT/collision
  semantics. No full-lap acceptance claim.
- Owner approval is **not** inferred; the window was opened for you. No commit.

## Owner run

```powershell
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --road-follow
```

Controls as DS11 (**W/S/A/D**, **E/Q**, **R** reset, **P** pause, **T**
textures, **L** lighting, **Esc** exit). HUD adds the road-follow block. The
release window is already open with these arguments.
