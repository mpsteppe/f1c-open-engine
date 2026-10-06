# DS07 — track viewer from `.SCN`

Role: implementer (Team B). Read CLEAN_ROOM.md and AGENTS.md first. Inputs:
this file, `specs/` (MTS_FORMAT.md v1.2), `crates/*`. Reasoning level:
medium. Bevy stays `=0.19.1`.

Goal: `cargo run -p viewer -- <track>.SCN` shows the whole circuit, textured,
with a fly camera.

## Facts from Team A (clean: game text files + own probes, 2026-10-06)

### `.SCN` (plain text, Latin-1; first line `CUBEASF`)
- Same line rules as `.gen`: `//` comments (whole line or trailing), keys
  case-insensitive, `Instance=NAME` + `{ ... }` blocks that nest.
- `SearchPath=SeasonData\Circuits\Australia\1994_Adelaide` (several lines):
  folders relative to the **game root** = the parent of the nearest ancestor
  folder named `SeasonData` (case-insensitive). Backslashes; may differ in
  case from disk.
- `MASFile=Name.mas` (several lines): each name is looked up in the
  SearchPath folders in order; first folder holding it wins (case-insensitive
  file name). Missing: skip, print `missing MAS: Name.mas`.
- Other top-level blocks (`View=`, `Light=`) have `{ }` too: brace matching
  must not care what opened the block.
- Inside an instance, several `key=value` pairs can share one line, split by
  whitespace: `Moveable=True Pos=(...) Planes=(4)` and
  `MeshFile=TRACK02.mts CollTarget=True HATTarget=True`.

### Which instances to show (viewer rule v1)
Show each `MeshFile` of an instance unless the instance has:
- `Render=False` (invisible collision/timing triggers), or
- `Moveable=True` (sky, clouds, animated props; v1 skips them), or
- `AnimFile=` (animated).
Adelaide 1994: 794 instances, 13 Render=False, 15 Moveable.

### Placement
- Track meshes use the MTS position (G+256) in world coordinates
  (e.g. `TRACK02.mts` at (123.2, 4.7, -108.4)). Same rule as car parts:
  add position, then Z mirror. Ignore instance `Pos=`/`Orient=` (only on
  Moveable instances, skipped).

### Textures
- Same MAS list is the texture search path (order = MASFile order).
- Reuse `TextureIndex`, animated frame rule, magenta-frame skip, Repeat
  addressing.

## Deliverables

1. New crate `crates/formats-scn` (no deps): parse to
   `Scene { search_paths: Vec<String>, mas_files: Vec<String>,
   instances: Vec<SceneInstance> }`,
   `SceneInstance { name, meshes: Vec<String>, render: bool,
   moveable: bool, animated: bool }`. Unit tests on inline text: nesting,
   View/Light blocks, multi-key lines, comments, case.
   (If sharing line helpers with `formats-gen` is cleaner, move them; keep
   both crates' tests green.)
2. `viewer` lib: `resolve_scene_mas(scn_path, &Scene) -> (Vec<PathBuf>,
   Vec<String> missing)`. Temp-dir tests.
3. `main.rs`: argument ending `.scn` = track mode.
   - Load every shown mesh; unsupported MTS (4.01, Z3DM) or missing: skip and
     count.
   - Fly camera: WASD move, Q/E down/up, right mouse drag look, Shift = 5x
     speed, mouse wheel changes speed. Start above the first `TRACK*` mesh
     looking down at it (or the scene centre if none). Far clip >= 3000.
   - One directional light; ambient light so nothing is pitch black.
   - Console: `Track: <scn>, MAS: n found, m missing`,
     `Meshes: shown x, skipped y (hidden/moveable/animated), missing z,
     unsupported w`, `Textures x/y found`.
   - `.veh` and `.mas` modes unchanged.
4. README: track mode + controls.

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
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN"
```

Report: the console lines, load time, frame rate if easy to see. Owner does
the visual check (track surface, walls, buildings in place, textured).

## Stop

- 3 failed fix attempts on one command: stop and report.
- Load slower than 60 s in release: stop and report (no optimisation work).
- Do not commit. Write `handoffs/DS07_REPORT.md` (ADHD format).
