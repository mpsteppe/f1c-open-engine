# Road-following prototype scope — 2026-10-06

**Next action:** DS13 implements ROAD_FOLLOW.md; its report must measure both successful steps and surface loss.

- **Question:** how can the next prototype follow sampled road height without continuing under slopes, teleporting onto distant layers or pretending to implement suspension?
- **Sources/tier:** workspace contract/code and DS12 runtime measurements, specs/GROUND_QUERY.md, specs/DRIVE.md, crates/ground-query/src/lib.rs, crates/physics-drive/src/sim.rs, crates/viewer/src/main.rs, records/research/DS12_GROUND_QUERY_2026-10-06.md. Owner screenshots show flat car placement away from spawn and a matching reset hit. No restricted evidence consulted.
- **Method:** inspect headless stepping/queries and rendering, independently rerun configured-game workspace tests and ground_probe, inspect registered Adelaide SCN comment lines. This unit plans a prototype; it measures no original-game dynamics.
- **Reproduced sample:** 52 selected, 27,755 retained triangles; rear X/Z 310.208/-335.542 m, surface 4.663 m versus grid 4.709 m, difference -0.046 m, TRACK02C.mts, 371 candidates. Query reference in DS12 remains fixed grid Y.
- **Correction:** raw 54 true HAT lines include two commented MeshFile lines: TRACK07LOGO.mts and TRACK08LOGO.mts. Active count is 52. This replaces the prior unsupported before-MeshFile/outside-instance explanation. Reproduce by filtering true-flag lines whose TrimStart begins with // in registered SCN.
- **Deduction/limits:** fixed-Y diagnostic queries can lose nearby coverage as road height changes; screenshots alone do not prove the precise reason for the no-hit at the unrecorded moving position. Updating query reference is necessary for the planned follower, but does not establish road identity.
- **Design:** per-fixed-step height and tangent-plane pose, last-height reference, 2 m lookup band, 0.25 m accepted vertical jump, atomic pose/planar step, latched loss/reset. All are project choices. Horizontal forces stay DS11; no gravity, tire/suspension or barrier solver.
- **Invented oracle:** Y=1+0.5X, at X=1/Z=1 gives 1.5 m and normal (-0.447213595499958,0.894427190999916,0); yaw0/rear local Z1.5 gives origin (1,1.5,-0.5). Obtained analytically from the plane and coordinate contract, not game data; DS13 tests reproduce to 1e-9.
- **Confidence:** high for rerun DS12 sample and commented-line correction. DS13 motion/loss is not yet implemented/validated. Closely stacked surfaces, uneven wheel contact and sampling steep/rapid height changes remain limitations.
- **Open Engine use:** move visibly with candidate road height/tilt while stopping on lost support; foundation for later contact dynamics.
- **Original-game modding use:** no new physics finding. Possible future geometry/layer diagnostics are untested; chosen thresholds do not imply original limits.

Reproduce sampled geometry:

```powershell
$env:F1C_GAME_DIR='C:\F1Research\F1 Challenge V10'; cargo test -p viewer --test ground_probe -- --nocapture
```

[Spec](../../specs/ROAD_FOLLOW.md) · [Handoff](../../handoffs/DS13_HANDOFF_road_follow_2026-10-06.md) · [DS12 review](../../reviews/DS12_REVIEW_2026-10-06.md). No related external F1 research newly consulted.
