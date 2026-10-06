# DS14 report — swept barrier-stop prototype

**Next action — Matias:** short-drive the open viewer with `--barrier-stop`:
drive into a visible barrier and confirm it stops (HUD `Barrier stopped — R to
reset`), then `R` restores the spawn. Then tell the coordinator "check DS".
No commit.

## What was built

- **`ground-query` barrier module** (`src/barrier.rs`, headless f64): immutable
  steep-triangle geometry with stable `SourceId`; a bounded XZ spatial index
  whose broad phase covers the **entire swept sphere AABB**; and a conservative
  swept-sphere query (`sweep_sphere`) with face/edge/vertex support, deterministic
  lowest-`SourceId` ties and a named non-convergence failure. Named errors added
  for malformed triangle lists and non-convergent sweeps.
- **`physics-drive`** (`src/barrier.rs`): the `BarrierStop` proxy (one `0.75` m
  sphere, `+0.75` m Y offset from the road-follow mesh origin), spawn-overlap
  preflight (`BarrierError::Overlap`), last-contact record and reset. `road.rs`
  `step_follow` now returns a `StepOutcome` (`Accepted`/`SurfaceLost`/
  `BarrierStopped`) and checks the barrier sweep before committing a proposal;
  `sim.rs` `Session` owns the optional barrier, blocks movement while latched and
  clears it on reset.
- **`viewer`**: `classify_barrier`/`build_barrier_query` (visible `Render=True`
  explicit `CollTarget=True`, top-level/static/non-animated/non-sky; HAT-only and
  hidden excluded), `diagnostic_barrier_sweep`, the `--barrier-stop` flag and
  validation (needs `.SCN` + `--car` + `--drive` + `--road-follow`), preflight
  counts plus spawn/proxy/diagnostic output, and a barrier HUD label and
  `Barrier stopped — R to reset` line.
- Old DS11/DS12/DS13 modes and tests are unchanged; without `--barrier-stop` the
  road-follow HUD label stays `no suspension or collisions`.

## Checks (each run individually)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass (exit 0) |
| `cargo test --workspace` (with `F1C_GAME_DIR`) | pass; **252 passed, 0 failed** |
| `cargo build --release -p viewer` | pass (Finished release) |

Per crate: formats-aiw 7, formats-gen 19, formats-hdv 42, formats-mas 8,
formats-mts 13, formats-scn 18, **ground-query 46**, **physics-drive 51**,
**viewer 41**; optional runtime/corpus: formats-mas/mts/hdv `game_corpus` 1 each,
physics-drive `game_drive` 1, viewer `barrier_stop` 1, `ground_probe` 1,
`road_follow` 1. Optional tests skip only when `F1C_GAME_DIR` is unset; with it
set the sample must pass. DS11/DS12/DS13 regressions stay green.

## Invented oracles

- Wall `X=5`, sphere `(0,0,0)->(10,0,0)`, `r=0.75`: face hit `t=0.425`, centre
  `X=4.25` within `1e-8`; reverse direction and reversed winding identical;
  start `X=4.5` overlap and `X=4.25` touch both `t=0`.
- Tunnel-free conservative advancement: 100 m high-speed sweep hits a thin wall
  at `t=0.0425`. Finite edge-only, vertex-only, offset-face, grazing, parallel,
  zero-length, no-hit and layered-earliest fixtures pass; duplicate geometry
  ties break on the lowest `SourceId`; invalid inputs and malformed/non-finite/
  out-of-range data return named errors.
- Indexed sweeps equal an independent brute-force traversal on varied invented
  geometry; local candidate counts stay below the total; huge/extreme finite
  geometry stays bounded and finite.
- Integration: a contact rejects the proposal whole (pose, gear, speed, queued
  shift), latches `Barrier stopped`, clears on reset; surface loss wins over the
  barrier; paused reset stays paused; 30/60/144 FPS match within `1e-8`; a
  zero-step frame preserves the barrier centre; flat/follow modes without a
  barrier are unchanged.

## Real data — configured Adelaide / Ferrari / grid 0

`F1C_GAME_DIR=C:\F1Research\F1 Challenge V10`:

```
barrier: selected 95 meshes, excluded 699, unsupported 0
barrier: triangles retained 48091, rejected 72356 (degenerate 1, shallow 72355)
barrier preflight: proxy centre (309.345, 5.410, -334.392), radius 0.75 m, spawn clear true
barrier diagnostic sweep: TRACK02D.mts (occurrence 21) from (284.928, 7.491, -153.255)
  to (283.059, 7.472, -153.968), t 0.6250, candidates 25
barrier scenario: accepted 600 steps, status Clear, last contact none
```

The diagnostic is a constructed sweep, not evidence the car reaches that wall.
The 600-step straight launch made **no contact** and had no surface loss (negative
result); stop behaviour is covered by the invented fixtures. Full record:
[DS14_BARRIER_STOP_2026-10-06.md](../records/research/DS14_BARRIER_STOP_2026-10-06.md).

## Correction logged

- The swept broad phase initially used the Y axis where Z belongs, so the XZ
  index missed the target triangle (diagnostic reported `0` candidates). Fixed;
  regression test `swept_aabb_uses_the_xz_plane_not_y`. The diagnostic now
  searches for a clean positive-`t` face hit, since dense clusters can overlap a
  neighbour at the constructed start.

## Changed files (uncommitted)

- `crates/ground-query/src/lib.rs` — barrier module + named errors + re-exports.
- `crates/ground-query/src/barrier.rs` (new) — geometry, index, sweep, tests.
- `crates/physics-drive/src/barrier.rs` (new) — proxy, preflight, stop, tests.
- `crates/physics-drive/src/road.rs` — `StepOutcome`, barrier-aware `step_follow`.
- `crates/physics-drive/src/sim.rs` — `Session` barrier field, latch/reset/step.
- `crates/physics-drive/src/lib.rs` — module + re-exports.
- `crates/viewer/src/lib.rs` — barrier selection/build, diagnostic sweep, tests.
- `crates/viewer/src/main.rs` — `--barrier-stop`, preflight, HUD.
- `crates/viewer/tests/barrier_stop.rs` (new) — runtime acceptance.
- `crates/viewer/README.md` — barrier-stop example + label.
- `records/research/DS14_BARRIER_STOP_2026-10-06.md`, `records/research/INDEX.md`.

## Omissions and limits (deliberate, per spec)

- Hidden (`Render=False`) barriers are excluded, including timing geometry such
  as the `Xfinish` instance with `Response=VEHICLE,TIMING`; this can omit real
  invisible barriers.
- One central `0.75` m sphere can allow nose/wheel penetration; no full chassis
  shape. No sliding, bounce, impulses, damage, moving objects or car-car contact.
- The selection rule and steep test are prototype choices, not original
  `CollTarget` semantics. No full-lap or original-parity claim.
- Owner approval is **not** inferred; the window is to be opened for you.
  No commit.

## Owner run

```powershell
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --road-follow --barrier-stop
```

Controls as DS13 (**W/S/A/D**, **E/Q**, **R** reset, **P** pause, **T**
textures, **L** lighting, **Esc** exit). HUD shows the barrier label, the last
contact source and `Barrier stopped — R to reset`. The release window is to be
opened with these arguments.
