# DS12 report — road-height query foundation

**Next action — Matias:** owner-check the open viewer (`--ground-probe`): surface
height/source shows, a short drive updates it, reset/pause and T/L work, and the
car still stays on the flat plane. Then tell the coordinator "check DS".

## What was built

- **`formats-scn`**: every `MeshFile=` occurrence now carries typed
  `HATTarget`/`CollTarget` metadata (`MeshRef`, `FlagValue` absent/true/false/
  invalid). Flags bind to the latest mesh in the same instance, including later
  lines; new meshes start fresh; no leakage across instances/nested blocks; an
  invalid value is kept and diagnosed. `SceneInstance.meshes` still works.
- **New headless crate `crates/ground-query`** (no Bevy, no physics): immutable
  indexed triangles in game-world metres, stable source IDs, upward-normal
  normalization, degenerate/steep rejection with counts, a bounded uniform XZ
  grid with an oversized fallback, and a deterministic nearby-height query
  (smallest vertical distance, then lower height, then lower stable ID).
- **`viewer`**: `build_ground_query` selection/loading through the registered
  MAS/MTS path, `--ground-probe` preflight (selected/excluded/unsupported and
  retained/rejected counts + spawn hit), a `GroundProbe` resource and HUD lines.
  DS11 flat motion is unchanged.

## Checks (each run individually)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass (exit 0) |
| `cargo test --workspace` | pass; **202 passed, 0 failed** |
| `cargo build --release -p viewer` | pass (Finished release) |

`cargo test --workspace` was also run with `F1C_GAME_DIR` set. Per crate:
formats-aiw 7, formats-gen 19, formats-hdv 42, formats-mas 8, formats-mts 13,
formats-scn 18, **ground-query 21**, physics-drive 36, viewer 33; optional
runtime/corpus tests: formats-mas/mts/hdv corpus 1 each, physics-drive
`game_drive` 1, viewer `ground_probe` 1. Optional tests only return early when
their game root is unset; with it set, the DS12 sample must pass.

## Invented numerical oracle

`specs/GROUND_QUERY.md` oracle passes: triangle (0,1,0)/(4,3,0)/(0,1,4) at
(1,1) returns height **1.5 m** and normal **(-0.447213595499958,
0.894427190999916, 0)** within 1e-9; reversed winding matches; (3,3) no hit;
(2,2) shared diagonal included; layer/ref/max selection and stable-ID tie
covered; indexed results equal brute force; localized candidate count is smaller
than the total; a giant triangle and extreme finite coordinates stay bounded.

## Real data (sampled geometry evidence)

Ran with `F1C_GAME_DIR` set (headless; no window):

```
MAS found 7, missing 0
selected 52 meshes, excluded 742, unsupported 0
retained 27755 triangles, rejected 14128 (degenerate 2970, steep 11158)
spawn rear axle (310.208, -335.542): height 4.663 m, reference 4.709 m,
difference -0.046 m, normal (0.001, 1.000, 0.003), source TRACK02C.mts,
candidates 371
```

This is sampled geometry, not original physics. Full record:
[DS12_GROUND_QUERY_2026-10-06.md](../records/research/DS12_GROUND_QUERY_2026-10-06.md).

## Changed files (uncommitted)

- `Cargo.toml`, `Cargo.lock` — add `ground-query` member.
- `crates/ground-query/` (new): `Cargo.toml`, `src/lib.rs`.
- `crates/formats-scn/src/lib.rs` — typed per-mesh flags, diagnostics, tests.
- `crates/viewer/Cargo.toml` — `ground-query` dep + test dev-deps.
- `crates/viewer/src/lib.rs` — `MeshSelection`/`classify_mesh`, `resolve_grid`,
  `build_ground_query`, `load_ground_mesh`, re-exports, tests.
- `crates/viewer/src/main.rs` — `--ground-probe` arg/validation, preflight
  counts + spawn hit, `GroundProbe` resource, HUD lines.
- `crates/viewer/tests/ground_probe.rs` (new) — runtime acceptance.
- `crates/viewer/README.md` — ground-probe example + label.
- `records/research/DS12_GROUND_QUERY_2026-10-06.md`, `records/research/INDEX.md`.

## Omissions and limits (deliberate, per spec)

- Car Y/pitch/roll, gravity, airborne, suspension, grip/TDF, barriers, tire
  contact and lap timing are untouched. The car still stays on the flat plane.
- Selection, tolerances, 2 m band, 60-degree drop and index are prototype
  choices, not original collision/handling reconstruction.
- Nested/moveable/animated/sky meshes marked `HATTarget=True` are unsupported and
  fail with a named error; none occurred in the Adelaide sample.
- Owner approval is not inferred; the window was opened for you.

## Owner run

```powershell
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --ground-probe
```

Controls as in DS11 (**W/S/A/D**, **E/Q**, **R** reset, **P** pause, **T**
textures, **L** lighting, **Esc** exit). HUD adds the ground-probe block.
