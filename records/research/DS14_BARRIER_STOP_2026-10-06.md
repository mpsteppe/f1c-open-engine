# DS14 barrier-stop prototype — measurements — 2026-10-06

**Next action:** coordinator reviews the DS14 barrier-stop slice; owner short drives
with `--barrier-stop`. No commit.

- **Question:** can a swept sphere proxy stop at selected visible steep
  `CollTarget` geometry, without treating timing geometry as a wall, and what
  geometry does the configured Adelaide sample actually expose?
- **Source tier/location:** registered game data
  `SeasonData/Circuits/Australia/1994_Adelaide/1994_Adelaide.SCN` (data tier, run
  time only). Workspace: `specs/BARRIER_STOP.md`, `specs/ROAD_FOLLOW.md`,
  `specs/GROUND_QUERY.md`, the `formats-scn` parser and viewer loading path. No
  restricted material consulted.
- **Method:** implement the DS14 selection rule and headless swept-sphere query,
  then build both sets from the configured SCN/`.veh`/grid 0 in a runtime test.
  Reproduce with:

```powershell
$env:F1C_GAME_DIR='C:\F1Research\F1 Challenge V10'
cargo test -p viewer --test barrier_stop -- --nocapture
```

## Observed — configured Adelaide / Ferrari / grid 0

- **Barrier selection:** 95 mesh occurrences selected, 699 excluded, 0
  unsupported. Raw active SCN lines had 100 `CollTarget=True` and 694
  `CollTarget=False`; the 5 true-but-not-selected occurrences are hidden
  (`Render=False`) or otherwise excluded by the prototype rule. The 1994 record
  [BARRIER_STOP_2026-10-06.md](BARRIER_STOP_2026-10-06.md) holds the raw counts.
- **Barrier triangles:** retained 48,091, rejected 72,356 (degenerate 1,
  shallow/horizontal 72,355). Shallow triangles are the majority: many
  `CollTarget=True` surfaces are near-horizontal track/kerb geometry, not walls.
- **Spawn proxy:** centre `(309.345, 5.410, -334.392)` m, radius `0.75` m,
  centre offset `+0.75` m along world Y from the road-follow mesh origin. The
  stationary spawn sweep reported **clear** (no overlap).
- **Diagnostic sweep (constructed, not a drive):** first clean face found was
  `TRACK02D.mts` occurrence 21, sweeping from `(284.928, 7.491, -153.255)` to
  `(283.059, 7.472, -153.968)` m, radius `0.75` m, earliest contact `t=0.6250`,
  25 candidates tested. This proves the loaded geometry query; it is **not**
  evidence the car reaches that wall.
- **Bounded straight throttle:** 600/600 steps accepted (5.000 s), latch status
  `Clear`, no barrier contact and no surface loss from grid 0. A negative result:
  a straight launch does not meet a selected barrier, so stop behaviour was
  verified by invented fixtures and is left for the owner's drive.

## Observed — invented oracles

- Wall triangle `X=5`, sphere `(0,0,0)->(10,0,0)`, `r=0.75`: earliest face hit
  `t=0.425`, centre `X=4.25`; reverse direction and reversed winding identical.
  Start at `X=4.5` and touch at `X=4.25` return `t=0`.
- Conservative advancement is tunnel-free: a 100 m high-speed sweep crosses a
  thin wall at `t=0.0425`; finite edge/vertex/grazing/miss fixtures and layered
  walls with deterministic lowest-`SourceId` tie-breaks pass within `1e-8`.
- Integration: a contact rejects the whole proposal (pose, gear, speed, queued
  shift), latches `Barrier stopped`, blocks motion and clears on `R`; surface
  loss wins when a proposal has no valid surface; 30/60/144 FPS match within
  `1e-8`.

## Correction — broad-phase axis bug (caught by the runtime diagnostic)

- **Symptom:** the diagnostic sweep to a retained triangle first returned
  "unexpectedly clear" with 0 candidates; a standalone reproduction with a single
  sliver triangle also returned 0 candidates.
- **Cause:** the swept AABB used `point[1]` (Y) where `point[2]` (Z) belongs, so
  the XZ spatial-index query visited the wrong cells and missed the triangle.
- **Fix:** build the swept box from X and Z; regression test
  `swept_aabb_uses_the_xz_plane_not_y` added. This was a prototype bug, not a
  statement about the original game.
- **Also observed:** a dense cluster can place a neighbour within the `0.75` m
  sphere at the constructed start, giving `t=0`; the diagnostic now searches for
  a clean positive-`t` hit and names its source.

## Confidence and limits

- **High:** selection counts, retained/rejected triangle counts, spawn clearance
  and the constructed sweep are reproducible on the configured sample.
- **Prototype choices, not original semantics:** visible `Render=True`
  `CollTarget=True` selection, the `|normal.y| < 0.5` steep test, `0.75` m sphere
  and conservative previous-pose stop are project rules. No original collision,
  damage or whole-car shape is claimed.
- **Known omissions:** hidden (`Render=False`) barriers are excluded, including
  timing geometry such as the `Xfinish` instance with `Response=VEHICLE,TIMING`;
  one central sphere can allow nose/wheel penetration; no sliding, bounce,
  impulses, moving objects or car-car contact. Full M05 contact remains
  incomplete.

## Uses

- **Open Engine:** opt-in `--barrier-stop` gives a conservative, tunnel-free stop
  at visible steep barriers after road following, with a distinct reset latch.
- **Original-game modding:** the explicit-flag/steep-geometry counts and the
  diagnostic sweep could feed future barrier/flag diagnostics; **untested** and
  not a claim about original `CollTarget` behaviour. No game files changed.

[Spec](../../specs/BARRIER_STOP.md) · [Handoff](../../handoffs/DS14_HANDOFF_barrier_stop_2026-10-06.md) · [Scope record](BARRIER_STOP_2026-10-06.md) · [DS13 review](../../reviews/DS13_REVIEW_2026-10-06.md) · [Report](../../handoffs/DS14_REPORT.md). No external F1 research newly consulted.
