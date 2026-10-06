# DS03 report — `viewer` app

**Result: DONE.** All acceptance commands pass. Window opened; first model
`AHELMA.MTS` (56 viewable models in 2004RH.MAS).

## Deliverables

- New member `crates/viewer` (binary + lib), `MIT OR Apache-2.0`, deps
  `bevy =0.19.1`, `formats-mas`, `formats-mts` (path).
- Workspace `Cargo.toml`: added `[profile.dev.package."*"] opt-level = 3`.
- `src/lib.rs`: `MeshData`, `Bounds`, `mts_to_mesh`, `bounds`, `model_names`.
  5 unit tests: Z negation, winding reversal, index offset across 2 groups,
  bounds, empty model.
- `src/main.rs`: CLI + Bevy app (grey material, directional + ambient light,
  dark background, auto-frame camera, orbit/zoom/model-change/Esc).
- `crates/viewer/README.md`: one run command, controls list.

## Acceptance (last lines)

```
cargo fmt --all --check        -> FMT_EXIT=0
cargo clippy --all-targets -- -D warnings -> CLIPPY_EXIT=0
cargo test                     -> TEST_EXIT=0 (formats 12+1, viewer 5 pass)
cargo build -p viewer          -> BUILD_EXIT=0
```

## Launch evidence

Direct exe, 8 s soak, then stopped:

```
stdout: viewer: 56 viewable model(s); starting at AHELMA.MTS
stderr: bevy_winit::system: Creating new window F1C Viewer
```

Then relaunched detached for the owner (PID 28028, Responding=True); left open.
Owner does the visual check.

## Deviations

- **`W` wireframe skipped**: handoff marked it optional ("skip if not trivial").
- **`AmbientLight` attached to the camera entity** instead of spawned alone.
  Alone it triggers Bevy's `#[require(Camera)]` and logs a spurious
  "camera has no render graph" warning; behaviour is the same.
- **Added one startup `println!`** ("N viewable model(s); starting at NAME")
  so the first model could be reported. Not a UI change.
- `model_names` filters entries by `.mts` suffix (ASCII case-insensitive),
  then sorts case-insensitively. Archive entry `type` is opaque per spec.
- Did not commit.
