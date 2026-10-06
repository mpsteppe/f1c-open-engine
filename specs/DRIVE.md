# Drive v1 — DS11 behaviour contract

**Next action:** implementer follows the DS11 handoff; owner tests motion after review.

## Goal and limits

- **Goal:** one rear-wheel-drive car accelerates, brakes, steers and shifts in the existing track viewer.
- **Label:** `Drive prototype — flat ground, no collisions` stays visible.
- **Ground:** infinite horizontal plane at the selected grid's Y; pitch/roll zero. Roads, walls, slopes and other objects are visual only. Short grid-area tests are appropriate; a full lap is not acceptance.
- **Deferred:** tire slip curves, load transfer, suspension dynamics, aero from HDV, fuel mass, damage, wheel animation, audio, reverse, automatic gears and original-game handling parity.
- **No invented game claims:** the prototype equations and constants below are project choices. They are not a reconstruction of the original solver.

## Evidence ledger

| ID | Source and tier | Supported fact |
|---|---|---|
| D1 | PHYSICS_FILES.md; observed game data, 2026-10-06 | Mass, wheel drive, gear settings, torque samples, rev limit, steering lock, per-wheel brakes |
| D2 | sources.toml `game`, SeasonData/Vehicles/1994_Goodyear.tbc; observed file comments | Front/Rear labels divide tire properties; Radius is tire radius; dry grip and slip curves are separate quantities |
| D3 | sources.toml `game`, SeasonData/Vehicles/1994_f1susp.pm; observed fields/comments | Named wheel-body positions; game axes +X left, +Y up, +Z rear |
| R1 | sources.toml `sdk`, docs/47-engine-map-tokens-live-verified.md; original-game live A/B research | Rev-limit setting affects the running engine; engine-braking map affects coasting; EngineMap setting is inert in the inspected original |
| R2 | sources.toml `mods`, mechanical_failures/docs/NATIVE_SIMULATION_MAP_2026-10-06.md, section 3; research summary, not independently retested here | Original limiter uses a fade band and original engine can stall |
| K1 | [MathWorks bicycle equations](https://www.mathworks.com/help/robotics/ug/mobile-robot-kinematics-equations.html), bicycle section, read 2026-10-06 | Ideal rolling rear-axle translation and yaw rate v tan(delta)/wheelbase |
| P1 | Coordinator design, 2026-10-06 | All prototype simplifications, bounds, timing, UI and tests below |

R1/R2 are distilled behaviour only. Implementers do not open those external research sources. DS11 deliberately uses a hard positive-torque cutoff and idle/launch aid rather than the original limiter, clutch or stall behaviour. EngineMap and engine-braking-map effects are both omitted; omission of the latter changes coasting. [P1, R1, R2]

## Launch and controls [P1]

- **Opt in:** track `.SCN` plus `--car <veh> --drive [--grid N]`. Existing modes without `--drive` keep their controls and placement.
- **Reject clearly:** --drive without track/car, unreadable car, missing selected grid or required physics. Return failure before opening a misleading static viewer; name the bad field/file and say what to fix. No fallback camera-position spawn in drive mode.
- **Pedals:** W throttle, S brake; each 0 or 1. Both held is brake priority: throttle 0, brake 1.
- **Steering:** A left, D right; both or neither gives zero. Full keyboard input requests the parsed steering lock, converted from degrees to radians.
- **Gears:** E up, Q down; neutral N through highest forward gear; start/reset in first. No reverse. Requests act once per press, clamp at ends; simultaneous Q/E cancels.
- **Reset:** R restores selected spawn position/yaw, zero speed/steer, first gear, idle RPM, cleared pending inputs. R no longer toggles yaw sign in drive mode.
- **Pause:** P freezes simulation; resuming starts with cleared input until release/repress. Focus loss pauses and clears input. Reset is allowed while paused and keeps pause state.
- **View:** chase camera only in this batch; fly/orbit/right-drag/model-selection controls are inactive in drive mode. T textures, L lighting, Escape exit remain usable.
- **HUD:** speed km/h, gear N/1..N, RPM, pedals, paused state, controls, and prototype label. HUD values describe current simulated state; update at least 10 times/second. No per-tick console spam.

## Required data and validation [D1–D3, P1]

- **Required:** all five physics files loaded; finite positive HDV mass, rev limit, final drive, steering lock below 80 degrees; WheelDrive=REAR. FRONT/FOUR get a clear unsupported-drive error for this batch.
- **Gears:** at least one and at most 20 forward gears, exactly the declared count of settings and resolved finite positive ratios. A missing/out-of-range middle index must fail instead of shortening/reindexing the list. Validate raw settings before using the convenience resolved list.
- **Torque:** at least two triples; every component finite, RPM non-negative and strictly increasing in source order. No sorting or silently dropping invalid rows during drive configuration. Validate the source engine entries against decoded samples. Maximum torque must be positive somewhere between idle and rev limit.
- **Idle:** finite 0 < low <= high < rev limit from IdleRPMLogic. Prototype idle is the arithmetic mean. No silent generic-car defaults for missing required data.
- **Tires:** extend typed COMPOUND parsing to separate Front and Rear blocks. Read finite positive Radius from each selected compound block. Do not flatten both Radius keys into one value; leave other tire properties and curves out of scope.
- **Brakes:** all four finite non-negative BrakeTorque values, at least one positive. Front corners use front radius, rear corners use rear radius.
- **Geometry:** read PM body positions named fl_wheel, fr_wheel, rl_wheel, rr_wheel, case-insensitive. Average each axle's X/Z. Wheelbase L = rear average Z minus front average Z, finite positive. Positive LeftWheelBase/RightWheelBase overrides are unsupported in DS11: reject rather than silently ignore. Absent/zero overrides use PM geometry.
- **State:** finite spawn position/yaw; scalar forward speed is non-negative. Reject non-finite numeric inputs and keep outputs finite. Never panic on a bad car.

## Coordinates and reference point [D3, K1, P1]

The simulation uses game-space X/Z and the accepted positive grid yaw sign. At yaw zero forward is -Z; positive yaw turns right. Positive steering input means D/right.

The moving position is the **rear-axle centre**, not the mesh origin. Let the PM rear-axle local centre be (a_x, a_z), spawn mesh origin be (o_x, o_z), and yaw be psi:

\[
p_x=o_x+a_x\cos\psi+a_z\sin\psi,\qquad
p_z=o_z-a_x\sin\psi+a_z\cos\psi.
\]

The inverse gives the mesh origin at any pose. Simulation Y stays at grid height. Render position mirrors game Z exactly once. The rendered forward vector is (-sin(psi), 0, cos(psi)). A increases game X at yaw zero; D decreases it after moving. Wrap yaw to [-pi, pi).

## Longitudinal model [D1, P1]

Use double precision in the headless model. Symbols: speed v (m/s), mass m (kg), rear radius r (m), selected gear ratio G, final drive F, throttle t and brake b in [0,1]. Prototype constants: g=9.81 m/s², grip mu=1.3, rolling coefficient c=0.015, quadratic drag k=0.5 kg/m. These constants are project choices, independent of TBC dry grip and HDV aero.

\[
n_{coupled}=\frac{60vGF}{2\pi r},\qquad
n=\max(n_{idle},n_{coupled}).
\]

In neutral RPM equals idle and engine force is zero, regardless of throttle. In a forward gear, linearly interpolate both drag torque T_min and full-throttle torque T_max at n. Clamp lookup to curve endpoints; do not clamp displayed RPM to the rev limit.

\[
T=(1-t)T_{min}(n)+tT_{max}(n),\qquad F_{engine}=\frac{TGF}{r}.
\]

- **Limiter:** when n_coupled >= rev limit, suppress positive T to zero; retain negative drag torque. Do not merely clamp RPM while continuing positive drive force.
- **Launch aid:** the idle floor lets the car pull away without a modeled clutch. No idle-throttle controller, engine inertia or stall. Zero throttle must not create positive engine force, even if the curve's idle drag sample is positive.
- **Traction cap:** clamp engine force to [-mu m g / 2, +mu m g / 2]. Equal static axle load is a prototype assumption.

For wheel brake torques B_i, radii r_i:

\[
F_{brake}=\min\left(b\sum_i B_i/r_i,\mu m g\right),\quad
F_{resist}=c m g+k v^2,\quad
a=(F_{engine}-F_{brake}-F_{resist})/m.
\]

Resistance opposes forward motion and cannot start reverse motion. At zero speed, negative net force leaves v=0. Neutral still coasts/brakes. Speed is clamped to zero after integration; no artificial positive speed cap. Independent lateral and longitudinal caps are a deliberate simplification, not a combined tire-friction model.

## Steering and time [K1, P1]

- **Steering response:** move actual steering toward requested steering at no more than 90 degrees/second, including return to centre. Speed never changes the requested lock.
- **Yaw rate:** v tan(delta)/L, capped in magnitude at mu g / max(v, 0.5). At rest yaw rate is zero. This cap limits prototype cornering; it does not simulate slip.
- **Fixed time:** h=1/120 second. Each step evaluates forces from starting speed, updates steering, then speed, then yaw rate from updated speed/steering.

\[
v_{next}=\max(0,v+a h),\qquad
\psi_{next}=\operatorname{wrap}(\psi+\dot\psi h),
\]
\[
p_{x,next}=p_x-v_{next}\sin(\psi_{next})h,\qquad
p_{z,next}=p_z-v_{next}\cos(\psi_{next})h.
\]

- **RPM:** recompute current RPM from resulting speed and gear after each step; a shift changes the ratio on the next step without changing speed.
- **Catch-up:** admit at most 0.1 seconds of wall time per rendered frame (12 steps). Drop excess time rather than jumping the car after a stall; no pending backlog on pause/reset.
- **Input:** sample held input separately from stepping. Edge events survive a frame with zero steps and are consumed exactly once; reset/pause clear queued shifts. No repeated shifting across catch-up steps.
- **Rendering:** transform car entities from persistent local geometry; no rebuilding meshes or respawning the whole track on each motion step. Texture toggling must recreate/rebind any needed car entities at their current pose without losing simulation state.
- **Camera:** target mesh origin + 1 m up; position 8 m behind heading and 3 m above mesh origin, looking at target. Snap on reset; immediate follow is sufficient. Sky keeps following the camera.

## Numerical acceptance [P1]

Use invented data, never copied game files. One oracle uses m=800 kg, rear radius=0.4 m, G=2, F=4, constant T_max=200 Nm, T_min=-20 Nm, idle=2000 RPM, rev limit=8000 RPM, L=3 m. All other constants are above. Initial speed/steering/yaw zero, throttle 1, brake 0.

| Quantity | Expected |
|---|---|
| Engine force, before/after traction cap | 4000 N |
| Initial resistance | 117.72 N |
| Initial acceleration | 4.85285 m/s² |
| Speed after one step | 0.040440416667 m/s |
| First forward distance | 0.000337003472 m |
| RPM-coupled rev-limit speed in this gear | 41.887902048 m/s |
| Uncapped yaw rate at v=10 m/s, delta=10 degrees, L=3 m | 0.587756602362 rad/s |

Numbers were generated by the coordinator's calculation probe under %TEMP%/f1c_openengine/ds11_spec, not copied from game data. Oracle tolerance 1e-9 absolute for step values, 1e-8 for displayed rounded derived values.

## Required checks [P1]

- **Parsing/config:** front/rear radii differ correctly; missing compound/field, middle gear index, duplicate or descending RPM, non-finite data, missing wheel body and unsupported drivetrain all return clear errors.
- **Movement:** oracle step; straight throttle advances along correct heading; coasting slows; brakes stop without reverse; throttle+brake follows brake priority; neutral cannot accelerate from throttle.
- **Limits:** positive torque cuts above the coupled limiter; steering at rest cannot rotate; left/right signs from yaw zero are correct; steering rate and lateral cap hold; downshift can report over-limit RPM without NaN or a speed jump.
- **Continuity:** same step-indexed inputs and simulated duration at 30/60/144 rendered FPS produce matching state within 1e-8. Test queued shifts on zero-step and multi-step frames, and pause/reset clearing backlog/input. Excess wall-time handling is a separate test.
- **Owner:** accelerate briefly from grid, steer both directions at low speed, shift once, brake, reset, pause. HUD and chase camera follow. Texture/light toggles preserve moving pose. No full-lap or original-feel claim.
