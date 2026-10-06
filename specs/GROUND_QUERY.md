# Ground queries v1 — DS12 contract

**Next action:** implement this contract via the DS12 handoff. This is the first M05 foundation slice.

## Goal and evidence

- **Goal:** identify nearby static road-height triangles and query their height beneath a moving car. Headless geometry first; opt-in diagnostic HUD second.
- **Evidence:** [research record](../records/research/GROUND_QUERY_2026-10-06.md). Registered Adelaide SCN has mesh-line HATTarget and CollTarget flags. Their existence is observed; original-engine selection/default semantics are not validated.
- **Design:** all selection rules, tolerances and query limits here are coordinator prototype choices. No original collision/handling claim.
- **Deferred:** changing car Y/pitch/roll, gravity, airborne motion, suspension, surface grip/TDF, barriers, tire contact and lap timing. DS11 motion remains unchanged.

## SCN metadata

- Keep SceneInstance.meshes and its existing consumers working. Add a parallel typed mesh-reference list or migrate consumers safely; every MeshFile occurrence has its own metadata, source order and stable occurrence ID.
- **Mesh scope:** HATTarget and CollTarget belong to the latest MeshFile in the same instance, including keys on later lines. A new MeshFile starts fresh flag values. Flags never leak across instances, parent/child blocks or View/Light blocks.
- Preserve absent/true/false/invalid distinctly. Keys and boolean values are case-insensitive; comments follow existing SCN rules. An invalid flag produces a diagnostic; do not silently convert it to true. Preserve duplicates with deterministic last-value semantics for that mesh occurrence.
- **Selected:** only explicit HATTarget=True, non-moveable, non-animated, non-sky, top-level instances. Render=False does not remove selected geometry. CollTarget alone does not select it. Missing/invalid HATTarget is excluded and counted. Nested flagged geometry is unsupported and reported, rather than guessed.
- These are conservative project rules; do not claim they reproduce original defaults or hierarchy behavior.

## Geometry and coordinate contract

- Add a small headless geometry module/crate without Bevy or a general physics dependency. Public API and acceleration structure choice are implementer decisions.
- Input: immutable indexed triangles in **game world coordinates**, in metres, plus stable source occurrence/group/triangle IDs and display names. Add MTS position once. Car meshes, sky and decorative non-selected meshes are absent from the query set.
- Query independently of render visibility/material/texture state. Share decoded MTS data when useful, but a hidden selected mesh must still load. Do not retain original file bytes or extracted assets in the repository.
- Convert viewer mirrored geometry back by negating Z exactly once if using it as input; test that adapter. Do not use material normals as geometric normals.
- Reject non-finite vertices and out-of-range indices with named errors. Ignore/count degenerate triangles whose absolute XZ projected cross product is <= 1e-10 square metres. Ignore/count faces with upward geometric unit-normal Y < 0.5 (steeper than 60 degrees). Normalize winding so geometric normals point upward; reversed winding remains queryable.
- No partially hidden failures: opted-in ground probing fails before window creation on a missing/unreadable/unsupported selected mesh, invalid geometry, unsupported selected nested/moving/animated geometry, or zero retained triangles. Name the affected mesh and fix. Ordinary DS11/static modes retain their current behavior.

## Height query

- Query inputs: finite X, Z, reference Y and finite non-negative maximum vertical distance. Invalid inputs return a clear error; absence of a hit returns None, never a fallback plane.
- At projected points inside a retained triangle, interpolate height linearly. Treat barycentric weights >= -1e-9 as inside so shared edges are included. Return finite height, upward unit normal and stable triangle/source ID.
- Accept hits with absolute height minus reference Y <= maximum distance. Choose smallest absolute vertical distance, then lower height on an exact tie, then lower stable triangle ID. Results must be deterministic and independent of index traversal order.
- Query both sides of the triangle: this is a nearby-height query, not a downward ray or physical contact solver. Layer selection follows the reference; changing layers automatically is out of scope.
- Build a reusable spatial index once. Queries must inspect local candidates, rather than scan the whole track each fixed step. Report total triangles and tested candidate count; no machine-dependent millisecond pass threshold. Index design must avoid unbounded allocation for a large triangle or distant coordinate.

## Viewer diagnostic integration

- New **--ground-probe** requires track SCN, --car and --drive. Reject invalid combinations before opening. Without it, DS11 behavior and HUD label remain the same.
- Build the query set during preflight from selected meshes through existing registered runtime loading. Print selected/excluded/unsupported mesh counts and retained/rejected triangle counts once.
- Query at the current simulation **rear-axle X/Z**, using fixed grid Y as reference and a 2 m maximum vertical distance. Preflight must have a hit at the selected spawn rear axle; otherwise fail with "No nearby surface at grid; choose another grid or use flat drive without --ground-probe".
- HUD adds "Ground probe only — car stays on flat plane", hit height in metres, grid-to-surface height difference in metres, source mesh, and candidate count; or "No nearby surface" when driving out of coverage. No per-frame console spam.
- No hit while moving is diagnostic only: do not freeze, teleport or alter simulation, inputs or timing. Reset restores the original query reference. Pause still permits a displayed current result. Texture/lighting toggles must preserve index and query results.
- Store/query game-space f64 geometry; mirror only for display. No mandatory debug overlay in this batch. Document that this mode does not follow roads and cannot prove a safe full lap.

## Invented numerical oracle

One triangle has vertices (0,1,0), (4,3,0), (0,1,4), all in metres. Query (X=1,Z=1,reference Y=2,maximum distance=2) returns height **1.5 m** and upward unit normal **(-0.447213595499958, 0.894427190999916, 0)**. Tolerance: 1e-9 absolute. A reversed triangle yields the same result. At (3,3), no hit. At (2,2), the shared diagonal edge is included.

## Acceptance

- **Parser tests:** multiple meshes and different flags in one instance, later-line flags, absent/invalid/mixed-case booleans, comments, nested blocks, no inheritance/leakage, Render=False, CollTarget-only and ordered duplicate handling.
- **Geometry tests:** oracle height/normal/winding, edges, outside, translation and Z adapter, degenerate/vertical/steep faces, bad indices, NaN/Inf and empty set.
- **Selection tests:** hidden selected mesh included; visible non-selected mesh excluded; source identity preserved; unsupported selected metadata yields an explicit failure.
- **Layers:** two overlapping horizontal triangles at Y=1 and 5. Ref=1.2/max=2 selects 1; ref=4.8 selects 5; ref=3/max=2 selects lower 1; max=1 at ref=3 returns None. Stable-ID tie test for duplicates.
- **Index:** indexed results equal brute-force reference on invented varied triangles/points. A separated synthetic grid shows localized query candidate count smaller than total, including a large triangle and extreme finite coordinates without runaway allocation.
- **Runtime:** configured Adelaide/Ferrari/grid 0 probe must actually load selected surfaces and return a spawn hit. Report counts, source, height/reference/difference/normal with units. This is sampled geometry evidence, not original physics. If the sample fails, report the blocker; do not expand the height band or replace flags with name guesses.
- **Regression:** fmt, clippy -D warnings, workspace tests and release viewer build pass. DS11 oracle/session checks stay green. Owner checks probe HUD, short drive, pause/reset and texture/light toggles after code review; no approval inferred from launching.
