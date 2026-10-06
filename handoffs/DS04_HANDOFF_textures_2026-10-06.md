# DS04 — viewer textures

Role: implementer (Team B). Read CLEAN_ROOM.md and AGENTS.md first. Inputs:
this file, `specs/`, `crates/*`. Reasoning level: medium. Bevy stays `=0.19.1`;
check the real API in the local registry source, not memory.

## Facts from Team A (measured on 2004RH.MAS, 2026-10-06)

- Texture names come from `Material.stages[0].texture`.
- Only 160 of about 350 stage lookups resolve inside the model's own MAS.
  Indexing **every MAS in the same folder** resolves 304. The rest live in
  other folders; show those grey.
- Some names have no extension (e.g. `GREYRIM`): try the name as given, then
  `.BMP`, then `.TGA`. Matching is case-insensitive.
- Texture files are BMP or TGA (MAS types 0x12 / 0x14).
- UVs: use `uv0` unchanged (Direct3D and wgpu both put the origin top-left).

## Deliverables

1. `crates/viewer/src/lib.rs` (pure, no Bevy types):
   - Replace the single merged mesh with one sub-mesh per group:
     `pub struct SubMesh { pub material_index: u32, pub positions, normals, uvs: Vec<[f32; 2]>, indices: Vec<u32> }`
     and `pub fn mts_to_submeshes(&Mts) -> Vec<SubMesh>` (same Z flip and
     winding rule as now). Keep `bounds` working over all sub-meshes.
   - `pub struct TextureIndex` built from a list of MAS paths:
     `fn build(paths: &[PathBuf]) -> TextureIndex` (skips unreadable archives),
     `fn find(&self, name: &str) -> Option<(PathBuf, String)>` with the lookup
     rule above. First archive in the list wins on duplicates; put the model's
     own MAS first.
   - `pub fn decode_rgba(bytes: &[u8], name: &str) -> Result<(u32, u32, Vec<u8>), String>`
     using the `image` crate, `default-features = false, features = ["bmp", "tga"]`.
2. Unit tests: sub-mesh split by group with uvs; lookup order (as given,
   `.BMP`, `.TGA`, case-insensitive); first archive wins; decode a tiny BMP
   and TGA generated in the test with `image` (no game files).
3. `main.rs`:
   - Build `TextureIndex` from the opened MAS + every other `*.mas` in its
     folder (case-insensitive extension). Cache decoded textures by name.
   - One Bevy entity per sub-mesh with a `StandardMaterial` using the texture;
     missing or undecodable texture -> grey material.
   - Material `src_blend == 5` (SRCALPHA) -> `AlphaMode::Blend`, else opaque.
   - `T` key toggles textures on/off.
   - Add one on-screen line: `Textures 12/14 found  T: toggle`.
4. Update `crates/viewer/README.md` (controls, the folder-index rule).

## Acceptance

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build -p viewer
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\2004RH.MAS"
```

Report the textures-found line for the first 3 models. Owner does the visual
check; do not claim it looks right.

## Stop

- 3 failed fix attempts on one command: stop and report.
- Do not commit. Write `handoffs/DS04_REPORT.md` (ADHD format).
