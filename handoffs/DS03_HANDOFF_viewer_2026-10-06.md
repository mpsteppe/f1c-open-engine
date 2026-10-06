# DS03 — `viewer` app (grey shaded MTS models in Bevy)

Role: implementer (Team B). Read CLEAN_ROOM.md and AGENTS.md first (UI text
must follow the ADHD-friendly rule). Inputs: this file, `specs/`,
`crates/formats-mas`, `crates/formats-mts`. Reasoning level: medium.

## Important: Bevy version

Use `bevy = "=0.19.1"` exactly. Its API changed a lot across releases and may
differ from what you remember. Before writing Bevy code, check the real API in
the downloaded source (`%USERPROFILE%\.cargo\registry\src\*\bevy_*-0.19.1\`)
or `cargo doc -p bevy --no-deps`, and the examples in `bevy-0.19.1\examples\`
(`3d\3d_scene.rs`, `3d\generate_custom_mesh.rs`, `ui\text.rs`). No other
Bevy plugins or camera crates.

## Deliverables

1. New member `crates/viewer` (binary + lib), license `MIT OR Apache-2.0`.
   Deps: `bevy =0.19.1`, `formats-mas`, `formats-mts` (path deps).
   Add to workspace `Cargo.toml`:
   `[profile.dev.package."*"] opt-level = 3` (fast Bevy in dev builds).
2. `src/lib.rs` — pure, no Bevy types:

```rust
pub struct MeshData { pub positions: Vec<[f32; 3]>, pub normals: Vec<[f32; 3]>, pub indices: Vec<u32> }
pub struct Bounds { pub min: [f32; 3], pub max: [f32; 3] }
/// All groups of one model merged. Convert left-handed to right-handed:
/// negate Z of positions and normals, and reverse each triangle's winding
/// ([a,b,c] -> [a,c,b]). Indices offset per group.
pub fn mts_to_mesh(mts: &formats_mts::Mts) -> MeshData;
pub fn bounds(mesh: &MeshData) -> Option<Bounds>; // None if empty
/// MTS entry names in the archive, sorted case-insensitively.
pub fn model_names(archive: &formats_mas::MasArchive) -> Vec<String>;
```

   Unit tests: Z negation, winding reversal, index offset across 2 groups,
   bounds, empty model.
3. `src/main.rs` — Bevy app:
   - CLI: `viewer <MAS path> [model name]`. No args: print one-line usage
     and exit code 2. Bad path: print `Cannot open <path>: <error>. Check the
     path.` and exit 1.
   - Shows one model at a time, grey `StandardMaterial`, one directional
     light + ambient light, dark background.
   - Skip models that return `Unsupported` or are empty; keep a list of
     viewable ones. Start at the model given, else the first.
   - Camera auto-frames the model bounds on every model change.
   - Controls: left-drag = orbit, wheel = zoom, `Right`/`Left` = next /
     previous model, `W` = wireframe toggle optional (skip if not trivial),
     `Esc` = quit.
   - On-screen text, top-left, short lines:
     `Model 3/57: BBRRLFA.MTS`
     `Triangles 1,234`
     `Left/Right: change model  Drag: orbit  Wheel: zoom`
4. README section in `crates/viewer/README.md` (ADHD format): one run
   command, controls list.

## Acceptance

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build -p viewer
```

Then launch once and leave it to the owner:

```
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\2004RH.MAS"
```

Report whether the window opened and the first model name shown. The owner
does the visual check; do not claim it looks right.

## Stop

- 3 failed fix attempts on one command: stop and report.
- Do not commit. Write `handoffs/DS03_REPORT.md` (ADHD format: result first,
  bullets, last 10 lines per command, deviations).
