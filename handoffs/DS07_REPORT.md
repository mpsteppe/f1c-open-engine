# DS07 REPORT — track viewer from `.SCN`

**Next action:** Claude reviews this report + the diff; then Matias does the
visual check (track surface, walls, buildings in place, textured).

## Result

- **PASS** all four acceptance commands. Adelaide 1994 loads and renders.
- **No commit** (as instructed).

## Console output (release, 1994_Adelaide.SCN)

```
Track: 1994_Adelaide.SCN, MAS: 7 found, 0 missing
Meshes: shown 766, skipped 28 (hidden/moveable/animated), missing 0, unsupported 0
Textures 3029/3059 found
```

- **Load time:** ~1 s (window created <1 s after start; well under the 60 s stop).
- **Frame rate:** window ran 35 s with no visible stall; not measured.

## What was built

- New crate `formats-scn` (no deps): `parse(&str) -> Scene`.
- `viewer::resolve_scene_mas(scn_path, &Scene) -> (Vec<PathBuf>, Vec<String>)`.
- `main.rs` track mode: `.scn` argument, fly camera, one light + ambient.
- `viewer/README.md`: track mode + controls.

## Verified by unit tests

- `formats-scn` 7 tests: nesting, View/Light brace matching, multi-key lines,
  comments, case, Render/Moveable/AnimFile flags.
- `viewer` 3 new tests: game-root search, case-insensitive path + file match,
  first-folder-wins, missing list.

- `cargo fmt --check` — pass.
- `cargo clippy --all-targets -- -D warnings` — pass.
- `cargo test` — pass (all crates).
- `cargo run --release -p viewer -- <scn>` — pass (output above).

## Notes / decisions

- **Brace matching:** a generic depth stack; only `Instance=` frames collect
  meshes/flags, so `View=`/`Light=` blocks are ignored safely.
- **Game root:** parent of nearest `SeasonData` ancestor; `SearchPath=` resolved
  **component-by-component, case-insensitive**; `MASFile=` matched
  case-insensitively per folder, first folder wins. CDB resolved to
  `SeasonData\Vehicles\CDB.MAS`.
- **Track meshes** placed by MTS position (G+256) then Z mirror; instance
  `Pos=`/`Orient=` ignored (only on Moveable, skipped).
- **Camera:** starts above the first `TRACK*` mesh looking at it; height from
  whole-scene radius. Far clip 12000. Ambient 350 so nothing is black.
- **Shadows off** in track mode only (perf); car/mas views unchanged.
- **Texture count** lookup made O(shipped materials) instead of O(n²).

## Environment note (not a code issue)

- A leftover `viewer.exe` (PID 7660) was locking `target\debug\viewer.exe` and
  blocked the first build. Killed it; build then passed.

## Open questions

- Nested `Moveable` parent with a non-moveable child: flags are per instance in
  this parser; Adelaide repeats `Moveable=True` on the children, so counts match.
- 30 of 3059 textures unresolved (same as car-mode ratio); grey fallback.
