# Road following v1 — DS13 contract

**Next action:** implement via DS13 handoff. This extends M05 with a constrained road-following prototype.

## Scope and provenance

- **Goal:** opt-in --road-follow makes the DS11 car follow the height and tangent plane of the DS12 surface underneath its rear axle. Avoid continuing under slopes as seen in the owner screenshot.
- **Evidence:** [scope record](../records/research/ROAD_FOLLOW_2026-10-06.md), [DS12 measurements](../records/research/DS12_GROUND_QUERY_2026-10-06.md), [DS12 review](../reviews/DS12_REVIEW_2026-10-06.md). No new original-engine mechanics are inferred.
- **Label:** "Road-follow prototype — no suspension or collisions". This is kinematic surface following, not wheel contact, grip, airborne or original handling parity.
- **Deferred:** gravity, airborne motion, tire/suspension dynamics, traction changes on slopes, barriers, friction/TDF, banking-induced forces, collisions, lap timing. DS11 forces/speed/yaw stay in horizontal XZ; do not silently add slope acceleration.

## CLI and preflight

- --road-follow requires SCN + --car + --drive. Reject missing combinations before a window opens. It loads the same selected surfaces/index as ground-probe.
- Without --road-follow preserve both DS11 flat mode and DS12 --ground-probe diagnostic mode. When both probe/follow flags are supplied, follow behavior wins and the HUD reports the current follow reference, not the old fixed grid reference.
- Spawn selection: query the DS11 rear-axle XZ with grid Y and max distance 2 m. Require a hit, named failure otherwise. Initialize reference to that hit height, not grid Y. Print source/height/normal/initial grid difference once.
- Ground selection/loading rules remain DS12. No mesh-name heuristics, enlarging the height band on failure or fallback plane.

## Headless follower and fixed steps

- Follower data and logic must be testable without Bevy, with the same ground-query set used by the viewer. Choose API/module placement; no general physics dependency. Accepted Sim/Session tests must remain valid.
- Surface height/normal/pose are persistent simulation state. Update them at each 120 Hz simulation step, not once per render frame. Pause/zero-step frames preserve them.
- Before accepting each motion step, propose the ordinary DS11 step (including the queued shift consumption), query at proposed rear-axle XZ with **last accepted surface height** as reference, max distance 2 m.
- Accept only a hit whose height differs from last accepted height by <= **0.25 m per fixed step**. This explicit project bound prevents large step/bridge snaps. It does not prove continuous road identity or physical contact; closely spaced overlapping surfaces can still be ambiguous.
- Valid hit: accept planar state and new surface state/pose together. No partial update. Keep existing shift timing, reset/pause/focus and catch-up cap behavior.
- Missing hit/query error/height jump: reject the proposed position/yaw/steer/gear, keep last valid pose and gear, set speed zero and RPM appropriate to zero speed, clear queued input/backlog, and latch a distinct **surface lost** state. Show "Surface lost — R to reset". Do not auto-resume on throttle or P; R is required. Logging only on state transition.
- Reset: restore selected spawn planar state, original spawn surface and pose, first gear/idle, clear loss/backlog/input. Preserve user pause state and wait for held controls to release before moving. Reset/pause logic must not accidentally consume a stale shift.
- No frame-dependent interpolation/smoothing in this batch. Immediate per-step normal/height following is sufficient; render current accepted pose. Camera and sky continue following correctly.

## Pose in game axes

Use f64 headless pose and units. Rear contact anchor C = (sim rear X, hit height, sim rear Z). Unit surface normal U points upward. Horizontal requested forward H = (-sin(yaw), 0, -cos(yaw)). Forward tangent F is normalized projection of H onto the surface plane. Left basis L = F cross U; rear basis B = -F. These form the local model axes (+X left, +Y up, +Z rear). Reject non-finite/degenerate basis rather than producing a broken transform.

The car's existing local geometry has its lowest point at Y=0. Define rear local anchor A=(PM rear axle X, 0, PM rear axle Z). Mesh origin O=C-L*A.x-B*A.z. Thus transforming A gives C exactly. This anchor/tangent-plane alignment is a prototype placement choice; it does not guarantee all four tires contact uneven road. Do not use PM wheel Y or radii to add a second ride-height offset.

At zero slope U=(0,1,0), this reproduces DS11 X/Z/yaw placement, with mesh Y replaced by surface height. Convert the full basis and translation to viewer coordinates by the existing Z reflection exactly once. Do not reflect translation and then apply an unchanged game-space rotation. Validate basis orthonormality, handedness and the transformed anchor. Camera targets mesh origin + world Y*1 and sits behind the horizontal heading by 8 m, +world Y*3, as before.

## HUD and controls

- HUD shows road-follow label, speed/gear/RPM/paused, surface height (m), source, candidate count and following/lost state. Current normal/tilt optional. Labels must fit the window; use a readable dark translucent panel or wrap concise lines (owner screenshots showed text competing with scenery).
- W/S/A/D, E/Q, R/P/T/L/Esc remain. T/L preserve follower/index/pose. Focus loss pauses. Lost state overrides movement; P changes pause but cannot clear loss.
- Keep the old flat mode label and probe-only label when their modes are active. No full-lap acceptance claim.

## Invented oracle and required tests

- **Flat compatibility:** broad Y=1 plane, identical held/step-indexed inputs: planar speed/yaw/XZ equal DS11; only render height changes. Spawn/reset anchor transforms back to contact.
- **Slope oracle:** plane Y=1+0.5X, rear at X=1,Z=1, yaw=0, rear local anchor (0,0,1.5). Hit height 1.5 m; U=(-0.447213595499958,0.894427190999916,0), F=(0,0,-1), L=(0.894427190999916,0.447213595499958,0), B=(0,0,1), origin=(1,1.5,-0.5). Transform (0,0,1.5) gives (1,1.5,1). Tolerance 1e-9.
- **Yaw slope:** nonzero yaw and nonzero local rear X; projected tangent and mirrored viewer pose preserve anchor, orthonormal axes and sign. No double mirror.
- **Continuity:** gradual ascending/descending and banked invented surfaces update reference after each fixed step; cumulative elevation exceeds 2 m while each accepted jump stays within 0.25 m. Bridge layers separated by >2 m never snap due to stale grid reference.
- **Loss:** hole, >0.25 m height discontinuity, invalid query/basis: last valid position/pose retained, speed zero, loss latched, no stale shifts. No movement until reset; reset while paused stays paused.
- **Timing:** identical step-indexed inputs at 30/60/144 FPS give matching planar/follower state within 1e-8. Include zero-step and multi-step frames, pause/reset/focus/backlog and rendering toggles preserving index/state.
- **Runtime:** configured Adelaide/Ferrari/grid0 initializes surface following. A reproducible headless straight-throttle scenario should show bounded per-step height changes and either accepted poses or a correctly latched surface loss. Report duration, inputs, successful steps, height range/max jump, final source/loss reason with units. Do not require a successful full straight or widen limits if data loses coverage. Missing configured sample fails; unset root may skip.
- **Checks:** fmt, clippy -D warnings, workspace tests and release build pass; DS11/DS12 regressions pass. Owner drives briefly to confirm following tilt/height and readable HUD; reset/pause/T/L work; lost surface stops visibly if encountered.
