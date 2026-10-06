# DS01 — `formats-mas` crate (MAS archive reader)

Role: implementer (Team B). Read CLEAN_ROOM.md first. Allowed inputs: this
file, `specs/MAS_FORMAT.md`, files you create. Do NOT open any path outside
`C:\Projects\F1C_OpenEngine` except game data read by tests via
the env var below. Do not use `tools/f1src.py knowledge`.

## Deliverables

1. Cargo workspace at repo root:
   `Cargo.toml` with `[workspace] members = ["crates/formats-mas"]`, `resolver = "2"`.
2. `crates/formats-mas/` library, edition 2021, deps: `flate2` only
   (dev-deps allowed: none). License field: `MIT OR Apache-2.0`.
3. Public API (exact):

```rust
pub struct MasArchive { /* owns file bytes */ }
pub struct MasEntry { pub name: String, pub kind: u32, pub offset: u32,
    pub uncompressed_size: u32, pub compressed_size: u32, pub unknown: u32 }
#[derive(Debug)] pub enum MasError { Io(std::io::Error), BadMagic,
    Truncated { what: &'static str }, OutOfBounds { entry: String },
    Decompress { entry: String }, SizeMismatch { entry: String, expected: u32, got: usize } }
impl std::fmt::Display for MasError {..} impl std::error::Error for MasError {}

impl MasArchive {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, MasError>;
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, MasError>;
    pub fn entries(&self) -> &[MasEntry];
    pub fn find(&self, name: &str) -> Option<&MasEntry>; // ASCII case-insensitive
    pub fn read(&self, entry: &MasEntry) -> Result<Vec<u8>, MasError>;
}
```

4. Behaviour (from spec): magic check; parse count; directory at 24, 256 bytes
   each; name = bytes 20..256 up to first NUL, Latin-1 decoded (each byte ->
   char). `data_base = 24 + 256*count`. Validate every entry's
   `data_base+offset+compressed_size <= len` at open (OutOfBounds). `read`:
   stored if sizes equal, else zlib (flate2 ZlibDecoder); result length must
   equal uncompressed_size (SizeMismatch). Never validate header `data_size`.
   No panics on any input; use checked arithmetic.

5. Unit tests (no game data): build archives in-memory with a test helper
   (one stored entry, one zlib entry), test: find case-insensitive, read both,
   bad magic, truncated directory, out-of-bounds offset, corrupt zlib,
   size mismatch, zero entries.

6. Integration test `tests/game_corpus.rs`: if env `F1C_GAME_DIR` unset ->
   print "skipped" and pass. Else walk dir recursively, open every `*.mas`
   (case-insensitive ext), read every entry; assert zero errors; print counts
   `archives=N entries=M`. Expected on owner PC: archives=685 entries=75578.

## Acceptance commands

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
$env:F1C_GAME_DIR="C:\F1Research\F1 Challenge V10"; cargo test --release -- --nocapture
```

## Stop conditions

- Stop and report if any acceptance command fails after 3 fix attempts.
- Stop after deliverables; do not start other crates. Do not commit.
- Write `handoffs/DS01_REPORT.md`: files created, command outputs (last 10
  lines each), deviations from this handoff.
