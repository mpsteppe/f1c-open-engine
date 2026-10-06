# DS05 — texture search path from `.veh` / `.gen`, animated texture frames

Role: implementer (Team B). Read CLEAN_ROOM.md and AGENTS.md first. Inputs:
this file, `specs/`, `crates/*`. Reasoning level: medium. Bevy stays `=0.19.1`.

## Facts from Team A (clean: game text files document themselves, 2026-10-06)

- Team folder example: `SeasonData\Vehicles\Ferrari\1994_412T1\` holds
  `Team.mas` and `*.veh` files.
- A `.veh` file has a line `Graphics=1994_Generic_F1.gen` (value may have
  trailing spaces and a `//` comment). Keys are case-insensitive.
- The `.gen` file lives in the vehicles root (`SeasonData\Vehicles\`), the
  nearest ancestor folder named `Vehicles` (case-insensitive).
- `.gen` lines that matter here, in file order:
  `SearchPath=<TEAMDIR>` / `SearchPath=<VEHDIR>` then `MASFile=Name.mas` lines.
  Each `MASFile` belongs to the most recent `SearchPath`. `<TEAMDIR>` = the
  team folder; `<VEHDIR>` = the vehicles root. Example result for the Ferrari:
  `Team.mas` (team), then `1994.mas`, `Cdb.mas`, `Cmaps.mas`, `Drivers.mas`
  (vehicles root). Ignore every other line. Lines may start with tokens like
  `<LOW>`; only plain `SearchPath=`/`MASFile=` lines count. Strip `//` comments.
- Animated textures: a stage named `BBSGR` with frames > 1 is stored as
  `BBSGR00.BMP`, `BBSGR01.BMP`, ... Show frame 0.

## Deliverables

1. New crate `crates/formats-gen` (no deps, MIT OR Apache-2.0):
   - `pub fn veh_graphics(text: &str) -> Option<String>` (value of `Graphics=`).
   - `pub struct SearchEntry { pub dir: GenDir, pub mas: String }`,
     `pub enum GenDir { Team, Vehicles }`,
     `pub fn search_path(text: &str) -> Vec<SearchEntry>`.
   - Input is Latin-1 bytes decoded by caller; parser works on `&str`.
   - Unit tests from inline sample text: comments, tokens before keys,
     spaces, case-insensitive keys, unknown `SearchPath` value skipped
     (entries until next known SearchPath are skipped too).
2. `viewer` lib:
   - `pub fn resolve_search_path(mas_path: &Path) -> Vec<PathBuf>`: if the MAS
     folder has a `.veh` (first one sorted by name) with a `Graphics=` value,
     find the vehicles root, read the `.gen`, map entries to real files
     (case-insensitive filename match in that folder; skip missing). The
     opened MAS stays first. If anything fails, fall back to today's folder
     scan. Never panic.
   - Lookup order per texture name: as given, `.BMP`, `.TGA`; and if the
     stage has frames > 1, try `NAME00.BMP`, `NAME00.TGA` first.
   - Unit tests using temp dirs with tiny fake files (no game data).
3. `main.rs`: use `resolve_search_path`; add console line
   `Search path: Team.mas, 1994.mas, ...` at start.
4. README: one line on the search path.

## Acceptance

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\Team.mas"
```

Report: the search path line, and `Textures x/y found` for models 1, 13
(`412T1-GB28VA.MTS`, was 5/25) and 20. Owner does the visual check.

## Stop

- 3 failed fix attempts on one command: stop and report.
- Do not commit. Write `handoffs/DS05_REPORT.md` (ADHD format).
