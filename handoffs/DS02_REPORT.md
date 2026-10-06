# DS02 report — `formats-mts` (MTS model reader)

**DONE. All acceptance commands green. Corpus 100% parsed, 0 errors. Not committed.**
Spec v1.1 applied (14-byte geometry magic, `Z3DM` → `Unsupported`).

## Result

- `cargo fmt --check`: clean.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test`: green — 8 MAS + **12** MTS unit tests.
- Corpus (release): **`mts_ok=46236 unsupported=3047 triangles=12511688`**, 0 errors.
- Matches expected exactly: 46,236 ok; 3,047 unsupported (3,046 legacy 4.01 +
  1 `Z3DM`); errors 0.

## What changed since stop 1 (spec v1.1)

- Geometry magic check: **16 → 14 bytes** (`CUBE_MTS_4.10` + NUL). Bytes 14–15
  are payload and never checked.
- New `Z3DM` leading magic → `Unsupported { version: "Z3DM" }`.
- New unit tests: `detects_misnamed_z3dm`, `accepts_nonzero_geometry_padding`.
- Leading 4.10 magic stays 16 bytes (13 chars + 3 NUL), per spec.

## Files (uncommitted)

- `crates/formats-mts/Cargo.toml`, `src/lib.rs`, `tests/game_corpus.rs`.
- `Cargo.toml` (workspace member). `Cargo.lock`. `records/STATUS.md`.

## Acceptance command outputs (last 10 lines)

`cargo fmt --check`
```
(no output) FMT_CHECK=0
```

`cargo clippy --all-targets -- -D warnings`
```
    Checking formats-mts v0.1.0 (crates\formats-mts)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.17s
CLIPPY_EXIT=0
```

`cargo test`
```
test tests::detects_misnamed_z3dm ... ok
test tests::accepts_nonzero_geometry_padding ... ok
test tests::detects_legacy_4_01 ... ok
test tests::rejects_group_index_out_of_range ... ok
test tests::parses_64_byte_vertex_flag ... ok
test tests::parses_one_material_one_group ... ok
test tests::rejects_face_index_out_of_range ... ok
test tests::parses_animated_stage ... ok
test tests::rejects_material_index_out_of_range ... ok
test tests::rejects_truncated_material ... ok

test result: ok. 12 passed; 0 failed
TEST_EXIT=0
```

`$env:F1C_GAME_DIR="C:\F1Research\F1 Challenge V10"; cargo test --release -- --nocapture`
```
running 1 test
mts_ok=46236 unsupported=3047 triangles=12511688
test parses_every_mts_in_game_dir ... ok

test result: ok. 1 passed; 0 failed
CORPUS_EXIT=0
```

## Deviations

- None from spec v1.1 or the review instruction.
- No game data or decompile content committed; corpus reads game files at run
  time and skips when `F1C_GAME_DIR` is unset.
