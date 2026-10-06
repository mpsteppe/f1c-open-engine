# DS04 report — viewer textures

**Result: DONE.** All acceptance commands pass. Viewer launched on 2004RH.MAS
without errors. Owner does the visual check.

## Next action

- Owner: run the command below and confirm textures appear and **T** toggles
  them.

```
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\2004RH.MAS"
```

## Textures-found line (first 3 models)

```
AHELMA.MTS: Textures 1/1 found
AHELMB.MTS: Textures 1/1 found
BBRRLFA.MTS: Textures 3/5 found
```

Counts are per model, over distinct used materials (not raw stage lookups).

## Deliverables

- `crates/viewer/Cargo.toml`: added `image 0.25` (`default-features = false`,
  features `bmp`, `tga`).
- `crates/viewer/src/lib.rs`:
  - `SubMesh { material_index, positions, normals, uvs, indices }`.
  - `mts_to_submeshes` — one sub-mesh per group; same Z flip and winding rule.
  - `bounds(&[SubMesh])` — spans all sub-meshes.
  - `TextureIndex::build(paths)` / `find(name)` — first archive wins; lookup
    tries name, `.BMP`, `.TGA`, case-insensitive; unreadable archives skipped.
  - `decode_rgba(bytes, name)` via the `image` crate.
- `crates/viewer/src/main.rs`:
  - Builds `TextureIndex` from the opened MAS + every other `*.mas` in its
    folder (own archive first). Caches archives and decoded textures by name.
  - One Bevy entity per non-empty sub-mesh with a `StandardMaterial`.
  - `src_blend == 5` -> `AlphaMode::Blend`, else opaque; missing/undecodable
    textures fall back to grey.
  - **T** toggles textures; on-screen line `Textures f/t found  T: toggle`.
- `crates/viewer/README.md`: controls + folder-index rule.

## Acceptance (last lines)

```
cargo fmt --all --check                  -> FMT_EXIT=0
cargo clippy --all-targets -- -D warnings -> CLIPPY_EXIT=0
cargo test                               -> TEST_EXIT=0 (viewer 9 tests)
cargo build -p viewer                    -> BUILD_EXIT=0
```

## Launch evidence

Exe launched, 10 s soak, stopped. stdout:

```
viewer: 56 viewable model(s); starting at AHELMA.MTS
AHELMA.MTS: Textures 1/1 found
AHELMB.MTS: Textures 1/1 found
BBRRLFA.MTS: Textures 3/5 found
```

stderr tail: window created on RTX 5080 / Vulkan, no errors.

## Deviations

- **`MeshData` / `mts_to_mesh` removed** (replaced by `SubMesh` sub-meshes), per
  handoff. `bounds` now takes `&[SubMesh]`.
- **TGA detection**: `image::load_from_memory` cannot detect TGA (no signature),
  so `decode_rgba` retries the bytes as TGA when the automatic guess fails.
  Unit test `decodes_tga` covers it.
- **Texture counts are find-based**, deduped by material index over non-empty
  sub-meshes; a texture that resolves but fails to decode still counts as
  "found" and draws grey.
- **Startup stdout report** of the first 3 models added (same text as the
  on-screen line) so the numbers could be captured; not a UI change.
- `#[allow(clippy::too_many_arguments)]` on `apply_state` for its Bevy params.
- Did not commit.

## Files changed (uncommitted)

- `crates/viewer/Cargo.toml`, `src/lib.rs`, `src/main.rs`, `README.md`.
