# DS06 — whole-car assembly from `.veh` + `.gen`

Role: implementer (Team B). Read CLEAN_ROOM.md and AGENTS.md first. Inputs:
this file, `specs/` (MTS_FORMAT.md is now v1.2), `crates/*`. Reasoning level:
medium. Bevy stays `=0.19.1`.

Goal: `cargo run -p viewer -- <file>.veh` shows the whole car (body, wheels,
suspension, driver, helmet, wings) in one view, textured.

## Facts from Team A (clean: game text files + own probes, 2026-10-06)

### `.veh`
- `GenString=GB28FERBV` (value may have spaces and `//` comment, key
  case-insensitive). Same parsing rules as `Graphics=`.

### `.gen` instances (same file as DS05)
- Block form: `Instance=NAME` then `{` ... `}`. Blocks nest (e.g. `ARMS`
  inside `DUMMY1`). Treat every instance flat; nesting only matters for
  matching braces.
- Lines may start with zero or more tags in angle brackets, then spaces, then
  the content: `<NOTSPIN><DASHHIGH>  MeshFile=...`.
  Tags: `SPIN NOTSPIN LOW MED HIGH MAX DASHLOW DASHHIGH`.
- Lines starting with `//` (after optional spaces) are comments. Strip
  trailing `//` comments too.
- Mesh line: `MeshFile=<name> key=value key=value ...` tokens split by
  whitespace. Keys used: `Render` (True/False), `ShadowObject` (True/False),
  `LODIn=(f)`, `LODOut=(f)`. Defaults: Render=True, ShadowObject=False,
  LODIn=0. Other keys: ignore.
- Name tokens inside the mesh name: `<digits>` = those 1-based character
  positions of GenString, concatenated (`<1234>` -> `GB28`, `<567>` -> `FER`,
  `<34>` -> `28`, `<8>` -> `B`, `<1>` -> `G`). A position past the end gives
  nothing. `<ID>` -> `000`. Unknown token -> keep text unchanged.

### Which mesh lines to show (viewer rule, "MAX detail, race view")
Keep a line when ALL hold:
- every tag on it is in {`MAX`, `NOTSPIN`} (no tag = keep);
- `Render` is not False; `ShadowObject` is not True;
- `LODIn` == 0.
Then drop exact duplicate names inside the same instance.

Expected for `1994_Ferrari28.veh` (`GB28FERBV`), 22 meshes:
`GB28VA`, `FERBB`, `FERSUSPA`, `FERDRIVER`, `28HELMA`, `FERBACKFIRE`,
`GTLFA` + `FERRLFA`, `FERSLFA`, `GTRFA` + `FERRRFA`, `FERSRFA`,
`GTLRA` + `FERRLRA`, `FERSLRA`, `GTRRA` + `FERRRRA`, `FERSRRA`,
`GB28FWA`, `GB28RWA`, `FERRNLT`, `BDRIVERARMS` (all `.MTS`).
(Count is a guide; trust the rule, report the real list.)

### Finding a mesh file
- Search the DS05 search path archives in order, case-insensitive.
- Fallback (this install ships renamed files): if `NAME.MTS` is missing,
  accept the first entry whose name ends with `-NAME.MTS`
  (e.g. `GB28VA.MTS` -> `412T1-GB28VA.MTS`).
- Not found: skip, print `missing: NAME.MTS`.

### Placement
- MTS geometry header offset 256 (relative to geometry start G): position
  3 x f32. Add it to every vertex of that mesh before the Z mirror. Body
  meshes have 0; wheels/helmet/driver have real values.

## Deliverables

1. `formats-mts`: expose `pub position: [f32; 3]` on `Mts` (read from G+256).
   Test with a built sample.
2. `formats-gen`: `pub fn veh_gen_string(text) -> Option<String>`,
   `pub fn expand_tokens(name, gen_string) -> String`,
   `pub struct GenMesh { pub instance: String, pub mesh: String }`,
   `pub fn car_meshes(gen_text, gen_string) -> Vec<GenMesh>` (rule above).
   Unit tests on inline samples: tags, comments, nesting, dedupe, LODIn,
   Render/ShadowObject, every token form.
3. `viewer` lib: `pub fn resolve_search_path_for_veh(veh: &Path) -> Vec<PathBuf>`
   (share code with DS05; the team MAS = `<TEAMDIR>` entries in the veh
   folder). Mesh lookup with the `-NAME` fallback in `TextureIndex`-like index
   or a sibling `MeshIndex`. Temp-dir tests.
4. `main.rs`: argument ending `.veh` = car mode: load all meshes, place,
   show as one model (camera frames the whole car). `.mas` argument keeps the
   old one-model mode unchanged. Console: `Car: <veh name>, N meshes, M
   missing` then one line per mesh `INSTANCE NAME found|missing (from X.mas)`.
5. README: car mode line.

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
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh"
```

Report: the console mesh list, missing count. Owner does the visual check
(wheels at corners, helmet in cockpit, wings at ends).

## Stop

- 3 failed fix attempts on one command: stop and report.
- Do not commit. Write `handoffs/DS06_REPORT.md` (ADHD format).
