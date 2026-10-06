# DS12 ground query measurements — 2026-10-06

**Next action:** coordinator reviews DS12; runtime probe numbers below are sampled
geometry evidence, not original-engine physics.

- **Question:** can the clean-room engine load a track's explicit `HATTarget`
  surfaces, build a headless nearby-height index and return a deterministic hit
  under a car's spawn, without guessing original collision semantics?
- **Source tier/location:** data tier (`sources.toml` game): SeasonData/Circuits/
  Australia/1994_Adelaide/1994_Adelaide.SCN + sibling `.aiw`, and SeasonData/
  Vehicles/Ferrari/1994_412T1/1994_Ferrari28.veh. Workspace code:
  crates/formats-scn, crates/ground-query, crates/viewer. No spec-tier or
  forbidden sources read; no game bytes copied into the repository.
- **Method:** run the DS12 runtime acceptance test with `F1C_GAME_DIR` set to the
  active install. It parses the SCN per-MeshFile flags, resolves the SCN
  `MASFile=` list, loads the explicit `HATTarget=True` meshes through the same
  MAS/MTS loader as the viewer, builds the f64 index, and queries the spawn
  rear-axle X/Z at a 2 m band about the grid Y.

## Observed measurements (acceptance sample)

- MAS archives: **7 found, 0 missing**.
- Mesh occurrences: **52 selected, 742 excluded, 0 unsupported**.
- Triangles: **27,755 retained**, **14,128 rejected** (2,970 degenerate, 11,158
  steeper than 60 degrees from vertical).
- Spawn rear axle (game axes, metres): **X 310.208, Z -335.542**.
- Hit height **4.663 m**, grid reference Y **4.709 m**, difference **-0.046 m**,
  upward normal **(0.001, 1.000, 0.003)**, source mesh **TRACK02C.mts**,
  **371** candidate triangles tested.

Command (PowerShell):

```powershell
$env:F1C_GAME_DIR='C:\F1Research\F1 Challenge V10'; cargo test -p viewer --test ground_probe -- --nocapture
```

## Observed parser/index behaviour

- Per-MeshFile flags are preserved: `HATTarget`/`CollTarget` bind to the latest
  `MeshFile=` in the same instance, including later lines; new `MeshFile=` starts
  fresh values; flags do not leak across instances or nested blocks; an invalid
  value is kept and diagnosed, never coerced to true.
- Selection is explicit `HATTarget=True` on top-level, non-moveable,
  non-animated, non-sky occurrences. `Render=False` does not exclude a selected
  mesh; `CollTarget` alone does not select one.
- Geometry is stored in game world `f64`; the viewer's Z-mirror is undone exactly
  once. Degenerate (|XZ projected cross| <= 1e-10 m^2) and steep
  (|unit normal Y| < 0.5) faces are dropped and counted. Reversed winding is
  normalized upward and stays queryable.
- Query uses a bounded uniform XZ grid with an oversized fallback list, so a
  giant triangle or an extreme finite coordinate cannot allocate unboundedly;
  candidate count is reported per query.

## Static deductions

- The 11,158 steep faces show that explicit `HATTarget=True` geometry is not all
  retained by the prototype slope filter. Face counts alone do not identify walls, kerbs or terrain. The nearby
  height is therefore a candidate surface band, not a validated road.
- Correction after coordinator source check: 54 raw `HATTarget=True`
  lines include two fully commented MeshFile lines (TRACK07LOGO.mts and
  TRACK08LOGO.mts); 52 active lines match the selected occurrences. The prior
  before-MeshFile/outside-instance explanation was unsupported and is superseded.

## Prototype choices (not original-game findings)

- Explicit-true only; top-level static geometry; reference-Y layer choice; 2 m
  diagnostic band; car stays on the flat plane; steeper-than-60-degree drop.
- Not modeled: original HAT defaults/layering, barriers, slope dynamics,
  suspension, tire contact, full contact physics.

## Confidence and limits

- High confidence in the parser, geometry and query behaviour and in the sample
  numbers above (deterministic, reproducible).
- No original-engine semantics, grid-height or full-contact validation. One
  track/car/grid sample only.

## Uses

- **Open Engine:** a testable surface-query foundation for later slope/airborne
  work; measured spawn-to-surface differences can calibrate placement.
- **Original-game modding:** can reveal a mod's explicit HATTarget geometry gaps
  on a loaded track; untested until tried in-game. No original-engine reaction is
  claimed, and no game or mod files were modified.

[Spec](../../specs/GROUND_QUERY.md) · [Handoff](../../handoffs/DS12_HANDOFF_ground_query_2026-10-06.md) · [Scope note](GROUND_QUERY_2026-10-06.md). No related external F1 research newly consulted.
