# Project roadmap and game coverage

**Next action:** finish DS14 release rebuild after viewer closes, then wrap up phase/publication audit. No DS15.

Snapshot: **2026-10-06**. Accepted working implementation: DS13 kinematic road following, uncommitted, after review/checks and owner short drive. [DS13 review](reviews/DS13_REVIEW_2026-10-06.md). Last accepted implementation commit: DS10, `8a36928`; HEAD: `089f09f`. [DS11 review](reviews/DS11_REVIEW_2026-10-06.md).

## Where we are

- **Working:** load installed game archives, display textured car/track geometry, place a car on a grid, apply track lighting/sky, read linked physics files.
- **Owner verified:** Ferrari assembly, Adelaide track, car placement/yaw and track appearance. These are sample checks, not proof for every mod/car/circuit.
- **Driving:** DS11 accepted prototype; flat plane, no collisions, simplified motion. It is not original-game physics parity.
- **Game loop:** no accepted lap timing, sessions, opponents, pit stops, race rules, championship, replay or multiplayer.
- **Open implementation:** original assets remain in the owner's game installation. Loading assets does not mean those assets became open source; no original game code is copied.
- **Release:** not ready for distribution; legal review remains required by README. Development can continue while that gate remains open.

## How progress is counted

| Status | Meaning | Promotion requirement |
|---|---|---|
| Not started | No accepted implementation for this capability | Research record, behaviour spec and scoped handoff |
| Researched | Durable behaviour/evidence record exists | Reviewed spec ready for implementation |
| Spec ready | Written contract and acceptance checks exist | Implementation completed |
| In progress | Active batch or unreviewed code | Coordinator review and required checks |
| Prototype | Accepted limited substitute for intended behaviour | Document gaps, implement and validate real behaviour |
| Partial | Accepted subset of the intended capability | Close listed gaps and broaden coverage |
| Accepted scope | Defined slice passes review/checks | Does not imply all-content or original-game parity |
| Validated parity | Matching original-game behaviour demonstrated in defined cases | Preserve measurements, tolerances and scope |
| Deferred | Explicitly outside current release path | Owner/project decision to schedule it |

Track **three separate dimensions**: file compatibility, playable features, and behaviour fidelity. A parser, an approximate simulation and a parity-tested simulation earn different labels. Existing external F1 research is a source lead; it is not an implemented feature or a completed research record here.

There is **no defensible whole-game completion percentage yet**. The rows below are a scope inventory, not equal units of effort. File-count percentages apply only to their named corpus. Future estimates require sized tasks and a stable release scope; do not derive completion from lines of code, batch count or number of crates.

## Measured coverage

| Measure | Accepted evidence | What it does not prove |
|---|---|---|
| MAS archive reading | DS01: 685 archives, 75,578 entries read; [review](reviews/DS01_REVIEW_2026-10-06.md) | Every archive from every mod/version; archive writing |
| MTS model reading | DS02: 46,236 supported, 3,047 unsupported, 0 errors among supported files; 12,511,688 triangles; [review](reviews/DS02_REVIEW_2026-10-06.md) | Legacy 4.01 support, animation or full material parity |
| MTS observed corpus share | 46,236 / 49,283 = **93.8%** supported in that recorded corpus | 93.8% of the game implemented; current installation totals may differ |
| Ferrari texture lookup | DS05 reported body lookup 25/25; owner later approved the car; [history](records/STATUS.md) | All materials/textures across all vehicles |
| Adelaide scene | DS07: 766 meshes shown, 28 skipped, 0 missing model lookups; textures 3,029/3,059 in that run; [review](reviews/DS07_REVIEW_2026-10-06.md) | Skipped content compatibility; every circuit; image-identical rendering |
| HDV corpus | DS10: 155 seen, 155 parsed, 0 failed; [review](reviews/DS10_FIX_REVIEW_2026-10-06.md) | Typed-value validity or linked-file resolution across all 155 cars |
| Complete physics link sample | Ferrari28 HDV + engine + gears + tires + PM resolve; fresh reviewer check | All cars, original dynamic behaviour or complete tire/suspension data |
| Owner sample approvals | Car, track, placement and lighting recorded in STATUS | A systematic compatibility matrix or handling comparison |

These are **dated evidence snapshots**, not newly rerun corpus measurements. Counts stay attached to the report that produced them.

## Capability inventory

IDs stay stable so research notes, handoffs and reviews can refer to them.

### Data and content

| ID | Original-game capability | Current state | Remaining work / proof |
|---|---|---|---|
| DATA-01 | Read MAS archives | Accepted scope — CUBEMAS4.10 | Broader mod/version matrix; corrupt-input regressions |
| DATA-02 | Read model geometry/materials | Partial — MTS 4.10 | MTS 4.01 and other observed variants; complete material semantics |
| DATA-03 | Vehicle definitions and GEN lookup | Partial — assembly/search paths | Complete VEH metadata and selection; season isolation; hierarchy/lookup rules |
| DATA-04 | Track SCN and AIW | Partial — scene and grid | DS12 per-mesh flags/query foundation accepted; surfaces, racing path graph, sectors, pit/grid metadata |
| DATA-05 | HDV/engine/gears/TBC/PM | Partial — DS10 typed subset plus DS11 Front/Rear radii | Tire curves/remaining front-rear fields, constraints, setups, drivetrain/aero/thermal fields |
| DATA-06 | Track/race metadata and surfaces | Not started | GDB/TDF and related settings; terrain grip, track rules and session defaults |
| DATA-07 | Drivers, seasons, championships | Not started | Talent/rosters, calendar, scoring and season definitions |
| DATA-08 | Sound, cameras, controls, configuration | Not started as game-format support | Original configuration/sound/camera formats; migration and defaults |
| DATA-09 | Mod and content discovery | Partial — supplied paths | Season-aware catalog, missing-data diagnostics, compatibility manifest |

### Presentation and input

| ID | Original-game capability | Current state | Remaining work / proof |
|---|---|---|---|
| VIEW-01 | Textured cars and tracks | Accepted scope — sampled static scenes | Content matrix, transparency/material variants and visual regression references |
| VIEW-02 | Light, fog, sky | Partial — SCN sun/ambient/linear fog/sky | Other light/fog modes, shadows, time progression and weather |
| VIEW-03 | Object hierarchy and animation | Partial — static assembly; animated instances skipped | Correct parent transforms, driver/wheel/suspension animation, moving objects |
| VIEW-04 | Driving cameras and mirrors | Prototype — DS11 chase only | Cockpit, onboard, trackside, replay, camera configuration and mirrors |
| VIEW-05 | Cockpit, instruments and HUD | Prototype — DS11 basic telemetry only | Wheel displays, warning states, race/pit HUD and cockpit interactions |
| VIEW-06 | Sound | Not started | Engine RPM/load sound, tires, wind, impacts, pits and commentary |
| VIEW-07 | Effects | Not started | Smoke, dust, sparks, rain spray, skidmarks, damage appearance |
| VIEW-08 | Input devices and feedback | Prototype — DS11 keyboard controls | Wheel/gamepad/pedals, bindings, calibration, force feedback and assists |
| VIEW-09 | Menus and accessibility | Not started | Car/track/session selection, settings, garage UI, scalable/readable UI |

### Car and track simulation

| ID | Original-game capability | Current state | Remaining work / proof |
|---|---|---|---|
| SIM-01 | Basic acceleration/braking/steering | Prototype — DS11 flat-ground substitute | Replace simplified prototype forces with validated dynamics |
| SIM-02 | Ground contact and surface response | Partial — DS12 queries accepted; DS13 following accepted; DS14 owner passed/review fix tested; release rebuild pending | Road queries, slopes, crests, kerbs, off-track terrain and lost contact |
| SIM-03 | Chassis and suspension | Not started — PM bodies read, constraints counted | Rigid-body motion, suspension kinematics, springs/dampers, ride height/bottoming |
| SIM-04 | Tires | Not started as simulation | Slip, combined forces, load/camber/pressure, heat/wear and wet grip |
| SIM-05 | Engine and drivetrain | Prototype — DS11 lookup/ratios substitute | Inertia, clutch, launch/stall, limiter fidelity, differential, reverse, drivetrain types |
| SIM-06 | Brakes | Prototype — DS11 torque substitute | Bias, lockup, temperature/fade, wear/failure and assists |
| SIM-07 | Aero and fuel | Not started | Wings/body/downforce/drag, ride-height effects, fuel mass and consumption |
| SIM-08 | Collisions and damage | In progress — DS14 owner stop passed; final rebuild pending | Barriers/car contact, impact response, part damage/detachment and repairs |
| SIM-09 | Mechanical reliability | Not started | Documented wear/failure models and event consequences, validated against original |
| SIM-10 | Garage/setup and assistance | Not started | Setup ranges/persistence, assists, limits and repeatable configuration |
| SIM-11 | Determinism and simulation performance | Accepted scope — DS11 fixed-step tests | Broader changing-input scenarios; multi-car profiling; reproducible scenario harness |

### Racing and persistence

| ID | Original-game capability | Current state | Remaining work / proof |
|---|---|---|---|
| RACE-01 | Lap/sector timing and track limits | Not started | Directed crossing, pit routing, lap validity, timing and off-track rules |
| RACE-02 | Practice/qualifying/race lifecycle | Not started | Session setup, countdown, starts, finish and results |
| RACE-03 | AI navigation and driving | Not started — grid parsing is not navigation | Path graph, pace, steering/pedals, recovery and multi-car scenarios |
| RACE-04 | Opponent tactics and strategy | Not started | Traffic, passing, defense, fuel/tire decisions and difficulty |
| RACE-05 | Pits and servicing | Not started | Pit path/box, request, speed limits, service/repair and release |
| RACE-06 | Flags, penalties and safety car | Not started | Original supported rules, incidents, neutralisation and restart sequencing |
| RACE-07 | Scoring, standings and championship | Not started | Classification, points, calendar, season progression and save/load |
| RACE-08 | Replay and telemetry | Not started | Recording/playback, export, camera playback and saved-session compatibility decision |
| RACE-09 | Multiplayer | Deferred from first offline slice | Authority/sync, interpolation, lobbies, rules, disconnects and network validation |

### Project readiness

| ID | Capability | Current state | Remaining work / proof |
|---|---|---|---|
| SHIP-01 | Clean-room provenance/research | Partial — rules/specs/reviews present | Audit inherited fixtures, index research, track corrections and main-game reuse |
| SHIP-02 | Automated verification | Partial — local Rust checks/corpus samples | CI, fresh-machine builds, long runs, malformed data and subsystem regressions |
| SHIP-03 | Performance/resource budgets | Partial — sample scene load result | Target hardware/frame budgets, memory, large grids, streaming and stability |
| SHIP-04 | Installation/distribution | Not started — distribution gated | Legal/license audit, dependency notices, packaging and user-data discovery |
| SHIP-05 | Platform and mod compatibility | Not started as a matrix | Define supported platforms/game editions/mods; reproduce results per combination |

## Ordered milestones and exit gates

Future milestone names are planning labels, **not promised DS batch numbers or time estimates**. One milestone may need several spec/implementation/review cycles.

| Gate | State | Deliverable and pass condition | Depends on |
|---|---|---|---|
| M01 — asset foundation | Accepted scope | MAS and MTS 4.10 parsing plus rendered geometry; dated corpus counts and malformed-input tests | DS01–DS03 |
| M02 — scene foundation | Accepted scope | Textured assembled car on sampled track/grid, correct yaw, lighting/sky; owner approves | M01; DS04–DS09 |
| M03 — physics input foundation | Accepted scope | Linked physics files load, typed subset tested, missing-file handling and real-car summary | M02; DS10 |
| M04 — first motion | Accepted prototype | DS11 contract passes review/numeric tests and owner short-drive check; limitations visible | M03 |
| M05 — road and contact | Partial — DS12 queries accepted; DS13 following accepted; DS14 owner passed/review fix tested; release rebuild pending | Drive over slopes/kerbs; off-road and airborne states defined; no accidental surface snapping; barrier response specified/tested | M04, DATA-04/06 research |
| M06 — credible single-car dynamics | Not started | Suspension/tires/engine/brakes/aero/fuel/setup implemented with measured handling tests; controls calibrated | M05; subsystem research |
| M07 — solo lap/time trial | Not started | Complete valid lap with sectors, invalidation, reset, results, cockpit/HUD and usable driving sound | M05; necessary M06 subset |
| M08 — offline event | Not started | Select car/track, configure session, start/finish, classify and persist results; basic garage | M07; metadata/menu work |
| M09 — opponents | Not started | Multi-car AI completes laps, handles traffic/recovery and pits; stable bounded frame cost | M06/M08; full AIW graph |
| M10 — complete race operation | Not started | Race starts, rules/flags/penalties, pits/servicing, damage/retirement and finish state tested together | M08/M09 |
| M11 — seasons and content breadth | Not started | Original supported seasons/championship flow; representative cars/tracks/mods validated; visual/audio gaps closed | M10; content/persistence work |
| M12 — network play | Deferred | Defined original-feature target and network authority; full-session multi-client test | Stable offline simulation/rules; owner scope decision |
| M13 — distribution readiness | Not started | Legal/provenance gate clear, clean installation/build, licenses, packaging, docs and regression matrix | Chosen release scope plus SHIP gates; M12 optional for offline release |

**Current completed gates: M01–M04, within their stated scope (M04 is a prototype).** This is not 4/13 of the engineering effort. The simulation/racing gates are substantially broader than the foundation gates.

## Behaviour parity and compatibility proof

- **Baseline:** specify original version/mod, car, track, setup, assists, weather, hardware/input and test procedure. Record what is observed versus inferred.
- **Physics:** compare acceleration bands, braking distance, steady turns, coasting, gear/RPM relationship, suspension response and tire state. Set tolerances before calling a match.
- **Racing:** repeat start, lap crossing, pit service, flag/penalty, retirement and classification scenarios. A green unit test alone does not establish original behaviour.
- **Presentation:** compare named views/assets and known lighting/material cases; owner approval covers those views only.
- **Content:** maintain rows for car/track/season/mod combinations with load result, missing assets, run result and evidence. Include supported and unsupported formats.
- **Persistence/network:** define whether original saves/replays/protocols are compatibility targets or new project formats. Until decided, list them as open questions.
- **Research reuse:** every investigation links its durable note and states the original-game modding benefit. Engine-only approximations do not become original-game modding facts.

## Next work queue

1. **DS11 accepted:** owner short drive worked; review fixes and 169/0 tests pass. Preserve uncommitted edits per owner. See [review](reviews/DS11_REVIEW_2026-10-06.md).
2. **DS12 accepted:** [review](reviews/DS12_REVIEW_2026-10-06.md); 202 configured checks pass and spawn hit reproduced. Prior materials: [handoff](handoffs/DS12_HANDOFF_ground_query_2026-10-06.md), [contract](specs/GROUND_QUERY.md), [research](records/research/GROUND_QUERY_2026-10-06.md). Implement metadata/index/probe before car surface following.
3. **DS13 accepted:** [review](reviews/DS13_REVIEW_2026-10-06.md); owner drive works, 216 checks and configured sample pass. Prior materials: [handoff](handoffs/DS13_HANDOFF_road_follow_2026-10-06.md), [spec](specs/ROAD_FOLLOW.md), [record](records/research/ROAD_FOLLOW_2026-10-06.md). Kinematic following/loss slice; suspension/airborne/barriers remain deferred within M05. Queries/following alone do not complete full contact. [DS11 provenance backfill](records/research/DS11_SOURCES_2026-10-06.md) is not external revalidation.
4. **DS14 wrap-up:** [review](reviews/DS14_REVIEW_2026-10-06.md), [review/audit record](records/research/DS14_REVIEW_2026-10-06.md); owner stop/reset passed, 253 checks pass; release exe lock blocks final rebuild. Publication preparation: records/GITHUB_PUBLICATION.md. Prior materials: [handoff](handoffs/DS14_HANDOFF_barrier_stop_2026-10-06.md), [spec](specs/BARRIER_STOP.md), [research](records/research/BARRIER_STOP_2026-10-06.md). Limited swept-sphere stop; M05 still needs broader contact/airborne scope. **Define validation targets:** first solo-lap track/car, required physics fidelity, original baselines and target machine budgets.
5. **Split M06:** small tire/suspension/drivetrain/brake/aero batches, each with a research record and measurable acceptance. Keep timing/session work separate.

## Decisions and risks to resolve when relevant

| Topic | Current position | Decision trigger |
|---|---|---|
| First playable slice | One sampled offline car/track; prototype then solo lap | Before M05/M07 scope is finalized |
| Original fidelity vs improved behaviour | No parity claim; prototype deviations explicit | Before simulation acceptance targets |
| Supported editions/mods and legacy geometry | Only recorded sample/corpus scope proven | Before expanding content matrix |
| Wheels/FFB, cockpit/audio priority | Needed for a usable driving game; unscheduled | M06/M07 planning |
| Weather and moving content | Not implemented | Track/tire and presentation phases |
| AI/rules reuse | External research available, no implementation credit | M09/M10 specs |
| Multiplayer, original save/replay compatibility | Network deferred; file compatibility undecided | Stable offline foundation |
| Legal/licensing and release scope | Distribution blocked pending review | Before public release |
| Existing technical gaps | MTS 4.01; flat hierarchy; animated instances skipped; renamed mesh lookup heuristic; season/texture edge cases | Their corresponding data/view gates |
| Provenance of earlier fixtures | DS10 corrected; earlier batches not comprehensively re-audited by this roadmap | SHIP-01 audit before distribution |

## Keep this roadmap current

- **After every reviewed batch or decision:** coordinator updates affected capability rows, gate status, measured coverage and date; links review/research evidence.
- **Before acceptance:** active files and DS reports stay In progress. Code review and required owner checks determine status, not file existence.
- **Correct history visibly:** retain source reviews; note superseding evidence. Do not silently replace a dated corpus count with a different denominator.
- **Continuity:** NEXT holds the immediate action; STATUS holds dated history; this file holds overall scope/progress. They must agree.
- **While DS runs:** documentation edits remain uncommitted; combine them into a coordinator commit only after DS stops. Do not alter DS-owned implementation or its active handoff.

Evidence inventory and limitations: [coverage research record](records/research/PROJECT_COVERAGE_2026-10-06.md). Resume point: [NEXT](records/NEXT.md).
