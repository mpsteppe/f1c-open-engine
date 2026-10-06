# DS08 — car on track (grid slot from `.aiw`)

Role: implementer (Team B). Read CLEAN_ROOM.md and AGENTS.md first. Inputs:
this file, `specs/`, `crates/*`. Reasoning level: medium. Bevy `=0.19.1`.

Goal: `viewer -- <track>.SCN --car <car>.veh [--grid N]` shows the track with
the assembled car sitting on grid slot N (default 0).

## Facts from Team A (clean: game text files, 2026-10-06)

- The `.aiw` file sits next to the `.SCN`, same stem
  (`1994_Adelaide.SCN` -> `1994_Adelaide.aiw`, case-insensitive). INI-like
  text, Latin-1.
- Section `[GRID]` holds repeated triples:
  `GridIndex=0` / `Pos=(309.345,4.709,-334.392)` / `Ori=(-0.011,2.498,-0.001)`.
  Pos = metres, same axes as MTS vertices (before Z mirror). Ori = 3 angles
  in radians around X, Y, Z; Y (yaw) is the one that matters. Section ends
  at the next `[...]` line. Other sections: ignore.
- Car local axes: +x left, +y up, +z rear (hdv comment). Car meshes from DS06
  already sit in car space.
- Pos.y is the road surface height. Lift the car so its lowest vertex
  (all shown car meshes, after placement) touches Pos.y.

## Placement rule
1. Build the car exactly as DS06 (car space, game axes).
2. Rotate around Y by Ori.y, then by X (Ori.x) and Z (Ori.z) — small; Y
   first. Use the rotation that turns a game-axes point p into
   `R_y(yaw) * p` with the standard right-hand formula; then translate by
   Pos (+ lift).
3. Apply the same Z mirror as all other meshes last.
4. Yaw sign is INFERRED. Add key `R` that toggles yaw sign at run time and
   print `Yaw sign: +1/-1`. Owner tells which is right (car points along the
   track, toward the first corner/away from the grid slots behind it).

## Deliverables

1. New tiny crate `crates/formats-aiw` (no deps): `pub struct GridSlot { index:
   u32, pos: [f32;3], ori: [f32;3] }`, `pub fn grid_slots(text) -> Vec<GridSlot>`.
   Tests: inline sample, spacing, missing Ori (default 0), other sections
   ignored, case-insensitive keys/section name.
2. `main.rs`: `--car` and `--grid` args in track mode. Camera starts 8 m
   behind and 3 m above the car looking at it (fly camera as DS07).
   Console: `Car: <veh> on grid N at (x, y, z), yaw a`. Missing `.aiw` or
   slot: print and place at the camera start of DS07 with yaw 0.
3. README: example command.

## Acceptance

```
cargo fmt --check
```
```
cargo clippy --all-targets -- -D warnings
```
```
cargo test
```
```
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh"
```

Report console lines. Owner checks: car on the road, wheels on surface, points
along the track (else press R).

## Stop

- 3 failed fix attempts on one command: stop and report.
- Do not commit. Write `handoffs/DS08_REPORT.md` (ADHD format).
