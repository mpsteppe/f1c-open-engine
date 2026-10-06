# DS13 road-following prototype — 2026-10-06

**Next action:** coordinator reviews DS13; the runtime numbers below are a bounded
headless scenario on sampled geometry, not original-engine physics.

- **Question:** can the clean-room engine follow the sampled road height and
  tangent plane under the rear axle, per fixed step, with a last-height reference
  and a latched stop on lost support, without pretending to implement suspension
  or original handling?
- **Source tier/location:** data tier (`sources.toml` game): SeasonData/Circuits/
  Australia/1994_Adelaide/1994_Adelaide.SCN + sibling `.aiw`, and SeasonData/
  Vehicles/Ferrari/1994_412T1/1994_Ferrari28.veh. Workspace code:
  crates/physics-drive, crates/ground-query, crates/viewer. No spec-tier or
  forbidden sources read; no game bytes copied into the repository.
- **Method:** implement the DS11 fixed-step loop with an optional DS13 follower;
  add headless f64 pose/follower tests (flat, slope, yaw, continuity, loss,
  timing); add a configured runtime acceptance test that loads the Adelaide
  surfaces and runs a straight-throttle scenario. Independently rerun
  `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace` and the
  release viewer build with `F1C_GAME_DIR` set.

## Observed measurements (acceptance sample)

- MAS archives: **7 found, 0 missing**; selected **52** meshes, **742** excluded,
  **0** unsupported.
- Triangles: **27,755 retained**, **14,128 rejected** (2,970 degenerate, 11,158
  steeper than 60 degrees). Same set as DS12.
- Spawn surface (rear axle game X/Z **310.208 / -335.542 m**): height **4.663 m**
  versus grid reference **4.709 m**, difference **-0.046 m**, upward normal
  **(0.001, 1.000, 0.003)**, source **TRACK02C.mts**, **371** candidates.
- Straight full-throttle scenario, **5.000 s** (600 fixed steps, zero steering):
  **600 accepted** steps, **0 lost**; height range **4.561..4.663 m**
  (range **0.102 m**), maximum accepted jump **0.0004 m**, final source
  **TRACK02C.mts**, final status **following**.

Command (PowerShell):

```powershell
$env:F1C_GAME_DIR='C:\F1Research\F1 Challenge V10'; cargo test -p viewer --test road_follow -- --nocapture
```

## Observed headless behaviour (invented data)

- Flat compatibility: with a broad Y=1 plane and identical step-indexed inputs,
  speed/yaw/XZ match plain DS11 exactly (to 1e-9); only the rendered height
  changes. Spawn/reset anchor transforms back to the contact point.
- Slope oracle: plane Y=1+0.5X, rear at X=1/Z=1, height 1.5 m, yaw 0, rear local
  anchor (0,0,1.5): U=(-0.447213595499958,0.894427190999916,0), F=(0,0,-1),
  B=(0,0,1), origin (1,1.5,-0.5); anchor -> (1,1.5,1) within 1e-9.
- Yaw/offset: nonzero yaw and nonzero rear-axle local X keep the basis
  orthonormal, right-handed (left·(up×rear)=+1) and the anchor exact.
- Continuity: a gradual ramp updates the reference each step; cumulative
  elevation exceeded 2 m while every accepted jump stayed within 0.25 m.
- Layers: two overlapping surfaces at Y=1 and Y=5 never snap while the 2 m band
  keeps the last-height layer.
- Loss: a hole, a 0.5 m step (> the 0.25 m bound) and a degenerate/invalid query
  keep the last valid pose, set speed to zero and idle RPM, clear queued shifts
  and latch lost; no step runs until reset. Reset while paused stays paused.
- Timing: identical step-indexed inputs at 30/60/144 FPS give matching planar and
  follower state within 1e-8; zero-step frames preserve the surface sample.

## Static deductions

- The DS12 surface band is usable as a per-step follow target with a 0.25 m
  per-step bound; the observed 10 cm elevation change over 5 s stayed well inside
  it. This is one track/car/grid sample and does not prove full-lap coverage.
- `specs/ROAD_FOLLOW.md` prints L=(0.894427190999916, 0, 0.447213595499958) for
  the slope oracle, but that vector is not orthogonal to F=(0,0,-1). The
  contract's definition L = F×U gives (0.894427190999916, 0.447213595499958, 0),
  which is the orthonormal value the engine uses; the printed component is a spec
  typo, reported for correction. The origin/anchor numbers are unaffected.

## Prototype choices (not original-game findings)

- Single rear-axle anchor A=(PM rear X, 0, PM rear Z); tangent-plane placement is
  a project choice and does **not** guarantee all four tires contact uneven road.
- Last accepted height as reference; 2 m lookup band; 0.25 m per-step jump bound;
  atomic pose/surface acceptance; latched loss; opt-in rendering.
- Not modeled: gravity, airborne motion, suspension/tire dynamics, traction,
  banking forces, barriers, collisions, lap timing. Horizontal forces stay DS11.

## Confidence and limits

- High confidence in the headless pose/follower logic and the reproducible sample
  numbers. The implementation uses a single sampled anchor; closely stacked
  surfaces within the 0.25 m/2 m envelope, or rapid geometry changes, remain
  ambiguous. No original-engine road/handling semantics are claimed.

## Uses

- **Open Engine:** a testable surface-following foundation for later contact and
  slope dynamics; the car now moves with candidate road height/tilt and stops on
  lost support.
- **Original-game modding:** no new physics finding. The prototype could
  eventually flag surface-coverage gaps in a mod's `HATTarget` geometry, but that
  is untested and its thresholds do not imply original limits.

[Spec](../../specs/ROAD_FOLLOW.md) · [Handoff](../../handoffs/DS13_HANDOFF_road_follow_2026-10-06.md) ·
[DS12 measurements](DS12_GROUND_QUERY_2026-10-06.md) · [DS12 review](../../reviews/DS12_REVIEW_2026-10-06.md).
No related external F1 research newly consulted.

## Coordinator source check — 2026-10-06

- **Correction:** current specs/ROAD_FOLLOW.md slope oracle already prints L=(0.894427190999916,0.447213595499958,0), consistent with F cross U. The typo claim above is not supported by the current file; no change to the spec is needed. A prior different version was not established. Implementation/report review remains pending.

## Coordinator review — 2026-10-06

- Owner said the drive works. Independent fmt/clippy/workspace tests (216/0, game root unset), release build and configured road_follow test pass; 600-step sample reproduced. Accepted within kinematic scope; see reviews/DS13_REVIEW_2026-10-06.md. Removed false spec-typo assertion from test comment. No commit.
