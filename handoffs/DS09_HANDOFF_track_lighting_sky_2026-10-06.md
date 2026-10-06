# DS09 — track lighting, fog, sky from `.SCN`

Role: implementer (Team B). Read CLEAN_ROOM.md and AGENTS.md first. Inputs:
this file, `specs/`, `crates/*`. Reasoning level: medium. Bevy `=0.19.1`.

Goal: track mode looks like daytime in the game: sun, ambient, fog, sky.

## Facts from Team A (clean: game text files + own probes, 2026-10-06)

Adelaide 1994 `.SCN` top-level lines:
```
View=mainview
{
  Clear=False
  Color=(255, 252, 213)
  ...
  ClipPlanes=(1.00, 1400.00)
  View=rearview { ... }      // nested; ignore
}
AmbientColor=(138, 138, 140)
FogMode=LINEAR FogIn=(100.00) FogOut=(3000.00) FogDensity=(0.00) FogColor=(235, 253, 247)
Light=FDirect01
{
 Type=Directional Dir=(0.13, -0.82, -0.56) Color=(255, 255, 255)
}
Light=Omni01
{
 Type=StaticOmni Pos=(...) Range=(...) Intensity=(-0.300) Color=(...)
}
```
- Colours: 0..255 per channel. Dir: direction the light travels, game axes
  (Z mirror applies like positions).
- Use the FIRST `Type=Directional` light as the sun. Ignore all other light
  types (StaticOmni are baked; 77 of them at Adelaide).
- `View=mainview` `Color` = background clear colour. `ClipPlanes` far value
  = draw distance (use max(far, 3000) so the sky is not cut).
- Fog: `FogMode=LINEAR` with FogIn/FogOut metres and FogColor. Other modes
  or missing line: no fog.

Sky (instance `skyboxi`, `Moveable=True`, children `clouds`, `cloudsny`,
`cloudrny`):
- `SKYBOXI.MTS`: horizon ring, radius ~593 m, 4 TGA materials with alpha
  (city skyline cut-outs). `CLOUDS.MTS`: dome, radius ~1800 m.
  `cloudsny`/`cloudrny` are other weather variants: do not show.
- Sky meshes follow the camera in X and Z every frame (keep their own Y).
  Draw them unlit, no fog, behind everything (no depth write is fine).
- Find the sky by instance name `skyboxi` (case-insensitive) and child
  `clouds`. Missing: skip silently.

## Deliverables

1. `formats-scn`: parse `AmbientColor`, `FogMode` line (mode, in, out,
   colour), `View=mainview` `Color` + `ClipPlanes`, and `Light=` blocks
   (`Type`, `Dir`, `Color`). Keep existing API working; add fields to
   `Scene` with `Option`s. Tests on inline text (incl. multi-key fog line,
   nested `View=rearview`).
2. `formats-scn`/viewer: keep Moveable instances in the parsed data so the
   viewer can pick the sky ones; viewer still skips other Moveable ones.
3. `main.rs` track mode: sun (Bevy `DirectionalLight`, colour from SCN,
   illuminance tuned so the track is not blown out), ambient from
   `AmbientColor`, `DistanceFog` linear, clear colour, far clip, sky as above.
   Key `L` toggles SCN lighting vs the old default light (for comparison).
   Console: `Sun: dir (x, y, z) colour (r, g, b)`, `Fog: linear in-out`,
   `Sky: ring + dome` or `Sky: none`.
4. README: L key.

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

Report console lines. Owner checks: sky visible with skyline, sunny look,
distant scenery fades into fog colour, no black sky.

## Stop

- 3 failed fix attempts on one command: stop and report.
- Do not commit. Write `handoffs/DS09_REPORT.md` (ADHD format).
