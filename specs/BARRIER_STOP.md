# Barrier stop v1 — prototype contract


## Goal and provenance

- **Goal:** opt-in --barrier-stop prevents a simple moving car proxy from crossing selected steep visible CollTarget triangles. Stop and latch on contact; R resets.
- **Evidence:** maintainer research notes. Adelaide has explicit CollTarget flags and hidden timing geometry with CollTarget=True. This does not establish original collision rules.
- **Design:** proxy, mesh/face selection, collision algorithm tolerances and stop response are project choices. No original collision, damage or whole-car shape claim.
- **Deferred:** sliding, bounce, contact impulses, damage, car-car collisions, hidden physical barriers, moving objects, gravity/suspension/tire dynamics, trigger/race logic.

## Selection and loading

- Reuse per-MeshFile metadata and game-world placement from prototype. Select explicit CollTarget=True in visible (Render=True), static, non-animated, top-level, non-sky instances. HATTarget alone never selects a barrier. Ignore/count absent/false/invalid flags with existing diagnostics.
- Explicit-true visible candidates in nested/moving/animated/sky contexts are unsupported and fail in this opt-in mode. Render=False occurrences are excluded, including timing triggers; this can omit real invisible barriers and must be documented.
- Retain only nondegenerate 3D triangles whose absolute unit geometric normal Y is **<0.5**. Horizontal/shallower triangles are excluded/count separately, including track surfaces. Both windings/sides collide; no normal-Y upward conversion needed.
- Non-finite vertices, malformed triangle-list length, bad indices, missing/unreadable/unsupported selected mesh are named errors. Zero retained barrier triangles fails opt-in preflight. Ordinary modes unchanged.
- Headless immutable f64 barrier geometry with stable source identity; reusable spatial acceleration built once. Broad phase considers the entire swept sphere AABB, not only the destination cell. Bound index memory, handle large triangles without huge cell duplication, report tested candidates. No per-tick asset loading.

## Proxy and swept query

- Car proxy is **one sphere**, radius **0.75 m**. Centre = road-follow mesh origin + **world Y*0.75 m**, in game axes. This approximates only the central part of the car and can allow nose/wheel penetration; it is not a full chassis collider.
- Query start/end finite centres and finite strictly positive radius. Return earliest contact fraction t in [0,1], contact point/normal where defined, stable source ID/name and candidate count; None on no contact. Invalid inputs error. Outputs finite.
- Collision exists where sphere-to-triangle minimum distance is <= radius at any point along the straight centre sweep. Include face interior, finite edges and vertices; no endpoint-only test, infinite-plane test or arbitrary fixed substeps that can tunnel. Earliest contact tolerance **1e-8 fraction**, distance tolerance **1e-8 m** for invented ordinary-scale fixtures.
- Initial overlap/touch returns t=0. Zero-length motion checks overlap. Either winding and travel direction work. Exact equal-time ties choose lower stable source ID (occurrence/group/triangle); do not rely on traversal order.
- Use an analytic sweep or a conservative method with a defined convergence bound/error. If unable to resolve safely, return a named failure rather than claim clear. Implementation math/API is your choice; a general physics dependency is not required.

## Fixed-step integration and stop behavior

- --barrier-stop requires SCN + --car + --drive + --road-follow. Ground probe may also be supplied; follow labels win as before. Fail invalid combinations before window creation.
- Preflight ground follower then barrier set and stationary proxy overlap at spawn; overlap fails with source and "Choose another grid or use road follow without --barrier-stop". No fallback to disabling barriers.
- Each fixed step proposes planar/follower state as prototype. Surface loss still wins if proposal has no valid surface. If surface proposal is valid, sweep proxy from last accepted pose to proposed pose before accepting either state.
- No contact: accept planar, follower and any consumed shift together. Contact/query failure: discard entire proposal (including queued shift), retain last valid pose/position/gear, speed=0/idle RPM, clear pending shifts/input/time and latch distinct **Barrier stopped** state. No t-based repositioning in this batch; stopping at previous pose is conservative.
- Preserve Surface lost separately. Both latches block movement until R; P/focus/T/L do not clear them. Reset restores spawn state/proxy, clears latch and waits for held controls to release; paused reset stays paused.
- Barrier integration runs at 120 Hz. No query on unchanged render frames required. Existing prototype/prototype/prototype modes/tests stay valid. Keep last contact/source/reason for HUD/report; log only state transition, no tick spam.

## HUD and owner behavior

- Label: "Barrier-stop prototype — sphere proxy, no damage" alongside road-follow status. No "no collisions" label in enabled barrier mode. Keep visible loss/stop messages and readable dark panel.
- Show following/barrier-stopped status and last contact source, concise controls "Barrier stopped — R to reset". Optional diagnostic proxy rendering is permitted but not required; accurately document proxy dimensions/limitations.
- Camera, sky, R/P/T/L and old modes continue working. T/L must not rebuild/reset the headless index or stop state.

## Invented oracles/tests

- Vertical wall triangle at X=5: (5,-5,-5),(5,5,-5),(5,0,5). Sphere from (0,0,0) to (10,0,0), r=0.75: earliest face hit **t=0.425**, centre X=4.25; reverse direction t=0.425; reversed winding identical. At start X=4.5 overlapping, t=0; centre X=4.25 touching, t=0. Tolerance 1e-8.
- No hit: same path at Y=20. Finite-triangle boundary tests must miss where the infinite plane would hit. Edge-only, vertex-only, grazing/tangent, parallel and zero-length overlap/nonoverlap need meaningful fixtures and expected outcomes.
- High-speed sweep crosses a thin wall with endpoint entirely beyond it and still detects earliest hit. Layered multiple walls return earliest; exact duplicate/source ties are deterministic independent of index traversal.
- Parser/selection: visible CollTrue selected even if HATFalse; HAT-only excluded; hidden timing trigger excluded; unsupported visible selected contexts fail. Vertical faces retained, horizontal faces excluded, malformed/non-finite data named errors.
- Index: compare indexed sweeps to independent brute-force candidate traversal on invented varied geometry. Local candidate counts lower than total; large triangles and extreme finite inputs stay bounded. Non-finite derived math fails safely.
- Integration: contact rejects proposal atomically, preserves prior pose/gear, stops/idle/latches/clears input; no movement without reset. Surface loss precedence, paused reset, queued shifts in zero/multistep frames and 30/60/144 FPS result equality within 1e-8. Flat/follow modes without barriers unchanged.
- Runtime: configured Adelaide/Ferrari/grid0 builds barrier set and has clear spawn. Report selected/excluded counts, retained/degenerate/shallow triangles, proxy centre/radius and overlap check. A reproducible headless sweep to a **loaded retained triangle's face interior** proves geometry query; name source and constructed endpoints. It is a diagnostic sweep, not evidence the car reaches that wall in a drive. If no clear spawn/usable triangle, report failure rather than widen/change rules.
- Checks: fmt, clippy -D warnings, workspace tests and release viewer build pass. Owner short-drive toward a visible supported barrier, confirm stop→R if encountered; do not infer this from launching or require a full lap.
