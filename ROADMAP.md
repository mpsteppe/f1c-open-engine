# Roadmap

**Help next:** open a small issue in one area below, with a reproduction or proposed acceptance check.

## Current capabilities

| Area | Current scope | Useful next contributions |
|---|---|---|
| File loading | MAS, MTS, GEN, SCN, AIW and linked vehicle configuration readers | More invented edge-case fixtures, error reporting and format compatibility |
| Rendering | Textured cars/tracks, lighting, grid placement, chase camera | Material correctness, performance, camera controls and visual checks |
| Driving | Headless fixed-step planar motion, input queue and drivetrain configuration | Measured dynamics, better configuration validation and controls |
| Road queries | Indexed triangle height queries and constrained surface following | Content coverage, query performance and robust surface selection |
| Barrier contact | Swept sphere stopping against selected steep surfaces | Better collision shapes and well-defined contact response |
| Contributor tools | Cargo workspace tests and Windows CI | Documentation, portable paths and diagnostics |

Current driving and contact behavior are prototypes. Geometry/configuration compatibility and physics fidelity are separate goals.

## Building toward a playable replacement

1. Improve parser compatibility and reliable loading diagnostics.
2. Establish measured vehicle-dynamics contracts: tires, suspension, braking, clutch and drivetrain.
3. Develop track surfaces and collision response with reproducible tests.
4. Add lap timing, sessions, race rules, opponents and pit behavior.
5. Improve menus, input devices, accessibility, replay and packaging.
6. Explore multiplayer after a stable local simulation contract exists.

Large features need a discussed design and testable milestones. There is no whole-game completion percentage or promised release date.

## Evidence standard

A parser passing tests, a usable prototype and a measured original-game match are different results. Describe exact versions, scenarios, tolerances and limitations. Use invented fixtures; optional checks against user-owned installations must say whether they actually ran.
