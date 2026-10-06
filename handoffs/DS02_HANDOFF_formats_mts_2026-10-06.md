# DS02 — `formats-mts` crate (MTS model reader)

Role: implementer (Team B). Read CLEAN_ROOM.md and AGENTS.md first. Inputs:
this file, `specs/MTS_FORMAT.md`, `crates/formats-mas` (use it in tests).
Do NOT open paths outside this workspace except game data via env var in tests.
Reasoning level: medium.

## Deliverables

1. Add `crates/formats-mts` to workspace members. Edition 2021, no
   dependencies. Dev-dependency: `formats-mas = { path = "../formats-mas" }`.
   License `MIT OR Apache-2.0`.
2. Public API (exact):

```rust
#[derive(Debug, Clone)] pub struct Mts { pub materials: Vec<Material>, pub groups: Vec<Group>, pub vertex_stride: u32 }
#[derive(Debug, Clone)] pub struct Material { pub name: String, pub colors: [[f32; 4]; 4],
    pub src_blend: u16, pub dst_blend: u16, pub specular_power: f32, pub stages: Vec<TextureStage> }
#[derive(Debug, Clone)] pub struct TextureStage { pub texture: String, pub frames: u32 }
#[derive(Debug, Clone)] pub struct Group { pub flags: u32, pub material_index: u32,
    pub vertices: Vec<Vertex>, pub triangles: Vec<[u16; 3]> }
#[derive(Debug, Clone, Copy)] pub struct Vertex { pub position: [f32; 3], pub normal: [f32; 3],
    pub color: u32, pub uv0: [f32; 2], pub uv1: [f32; 2] }
#[derive(Debug)] pub enum MtsError { BadMagic, Unsupported { version: String },
    Truncated { what: &'static str }, OutOfBounds { what: &'static str },
    BadIndex { group: usize }, BadReference { what: &'static str } }
// implement Display and std::error::Error for MtsError
pub fn parse(bytes: &[u8]) -> Result<Mts, MtsError>;
```

3. Behaviour (from spec):
   - No leading 4.10 magic: if the bytes contain `CUBE_MTS_4.01` anywhere,
     return `Unsupported { version: "4.01" }`, else `BadMagic`.
   - Walk materials by the size rule; the geometry magic must follow, else
     `BadMagic`.
   - Vertex stride per spec (flag bit or pair-Y count). Never from offset gaps.
   - Triangles per group = face records whose group_index == group, in file
     order. Indices must be < vertex_count (`BadIndex`); group_index must be
     < group_count and material_index < material count (`BadReference`).
   - Caps: material_count <= 4096, stage_count <= 16, n <= 4096. Every read
     bounds-checked with checked arithmetic. No panics on any input.
4. Unit tests with synthetic files built in memory by a test helper: one
   material with one stage + one 2-triangle group; animated stage (frames 3,
   n 3); 64-byte vertex flag; 4.01 detection; bad magic; truncated material;
   face index out of range; group_index out of range.
5. Integration test `tests/game_corpus.rs`: skip if `F1C_GAME_DIR` unset.
   Else for every MAS, every `.MTS` entry: parse. Count ok / unsupported /
   errors. Assert errors == 0. Print `mts_ok=N unsupported=M triangles=T`.
   Expected: mts_ok=46236, unsupported about 3047 (report exact).

## Acceptance

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
$env:F1C_GAME_DIR="C:\F1Research\F1 Challenge V10"; cargo test --release -- --nocapture
```

## Stop

- 3 failed fix attempts on one command: stop and report.
- If the corpus has errors that look like a spec mistake: stop, list 5 example
  archive + entry names with the error. Do not change the spec.
- Do not commit. Write `handoffs/DS02_REPORT.md` (ADHD format: result first,
  bullets, last 10 lines per command, deviations).
