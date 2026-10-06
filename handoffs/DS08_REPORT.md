# DS08 REPORT — car on track (grid slot from `.aiw`)

**Next action:** Matias does the visual check. Look at the running viewer: is the
Ferrari on the road, wheels on the surface, nose pointing along the track? If it
points backwards, press **R** (prints `Yaw sign: -1`).

## Result

- **PASS** all four acceptance commands.
- **No commit** (as instructed).
- Viewer **left open** for the owner check (`viewer.exe`, PID 25440).

## Console output (release, Adelaide + Ferrari)

```
Track: 1994_Adelaide.SCN, MAS: 7 found, 0 missing
Meshes: shown 766, skipped 28 (hidden/moveable/animated), missing 0, unsupported 0
Car: 1994_Ferrari28.veh on grid 0 at (309.345, 4.709, -334.392), yaw 2.498
Yaw sign: +1  (press R to toggle)
Textures 3116/3146 found
```

- **Grid 0** pos/ori match Team A's facts exactly.
- Car textures add **87** found (3029 -> 3116); **30** unresolved, grey fallback.
- Window ran 35 s with no stall.

## What was built

- New crate **`formats-aiw` (no deps)**: `GridSlot { index, pos, ori }`,
  `grid_slots(&str) -> Vec<GridSlot>`. `[GRID]` only, other sections ignored.
- **`viewer::place_car`**: un-mirror to game axes, `Rz*Rx*Ry` (Y first), translate
  by `Pos`, lift lowest vertex to `Pos.y`, mirror again. Normals rotate only.
- **`viewer::car_forward`**: placed forward direction (`-z` game, mirrored).
- **`main.rs`**: `--car <veh>` / `--grid N` in track mode; camera 8 m behind,
  3 m above; **R** flips the inferred yaw sign at run time.
- **`viewer/README.md`**: example command.

## Verified by unit tests

- `formats-aiw` **7 tests**: sample, spacing, missing Ori -> zero, other sections
  ignored, case-insensitive keys/section, early Pos/Ori dropped.
- `viewer` **5 new tests**: identity placement, yaw around Y, lift to `Pos.y`,
  forward default, yaw-sign flip.

- `cargo fmt --check` — pass.
- `cargo clippy --all-targets -- -D warnings` — pass.
- `cargo test` — pass (workspace).
- `cargo run --release -p viewer -- <scn> --car <veh>` — pass (output above).

## Rule details (as implemented)

1. Car built exactly as DS06 (car space, then the standard Z mirror).
2. `place_car` recovers game axes, applies `Rz*Rx*Ry`, translates by `Pos`.
3. Lift: after placement, `lift = Pos.y - min_y` over all car vertices.
4. Mirror applied last (positions and normals).
5. **Yaw sign inferred**; `R` toggles and prints `Yaw sign: +1/-1`.

## Fallback

- Missing `.aiw` or slot: prints `No grid N in <aiw>; placing car at the DS07
  camera start` and places the car there with yaw 0.

## Environment note (not a code issue)

- A leftover `viewer.exe` (PID 31576, DS07) locked `target\release\viewer.exe`.
  Killed it; release build then passed in 6 s.

## Open questions

- **Yaw sign is unverified** — needs the owner's eye, hence the `R` key.
- Grid `Ori.x`/`Ori.z` are near zero; applied but their sign is untested.
