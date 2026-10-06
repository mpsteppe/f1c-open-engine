# DS01 report — `formats-mas` crate

Status: **PASS**. All acceptance commands green. Corpus matches expected
counts exactly (archives=685, entries=75578). Not committed.

## Files created

- `Cargo.toml` — workspace, `members = ["crates/formats-mas"]`, `resolver = "2"`.
- `crates/formats-mas/Cargo.toml` — edition 2021, `flate2` only, license `MIT OR Apache-2.0`.
- `crates/formats-mas/src/lib.rs` — `MasArchive`, `MasEntry`, `MasError`, 8 unit tests.
- `crates/formats-mas/tests/game_corpus.rs` — corpus walk, skips when env unset.

## Acceptance command results

`cargo fmt --check` — clean (no output).

`cargo clippy --all-targets -- -D warnings` — last lines:
```
    Checking formats-mas v0.1.0 (C:\Projects\F1C_OpenEngine\crates\formats-mas)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.15s
```

`cargo test` — last lines:
```
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 1 test
test reads_every_archive_in_game_dir ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`$env:F1C_GAME_DIR="C:\F1Research\F1 Challenge V10"; cargo test --release -- --nocapture` — last lines:
```
running 1 test
archives=685 entries=75578
   Doc-tests formats_mas
test reads_every_archive_in_game_dir ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.92s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Deviations from handoff

- **Magic check stricter**: validates the full 16-byte `CUBEMAS4.10` + 5 NUL
  constant (spec verified 685/685). Handoff said "magic check" only.
- **Decompress memory guard**: `read` caps zlib output at `uncompressed_size + 1`
  bytes before the size check, so a corrupt stream cannot allocate unbounded memory.
- **`MasError` derives `Debug`; `MasEntry` derives `Debug, Clone`** (handoff showed
  `#[derive(Debug)]` on the enum; extras added for test ergonomics).
- **`open` for files under 16 bytes returns `BadMagic`**, not `Truncated`, since a
  magic cannot be present.
- No other deviations; `data_size` header never read or validated. No panics on any
  input; all size math uses `u64` or `checked_*`.
