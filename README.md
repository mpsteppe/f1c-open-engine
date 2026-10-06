# F1C Open Engine

Help build an open-source engine compatible with F1 Challenge '99–'02 game data.

This is an independent community project written in Rust and Bevy. It loads files from a user's own game installation and includes no game assets or original game source. It is an early prototype, not a complete replacement for the game.

## What works

- Read MAS archives, MTS geometry, GEN/SCN scenes, AIW grid positions and vehicle configuration files.
- Display textured cars and tracks with lighting and a chase camera.
- Drive a configured car with fixed-step motion, gear changes, pause and reset.
- Query nearby ground, follow road height and tilt, and stop at selected barriers.

Driving, road following and barrier contact are simplified prototypes. Suspension, tire dynamics, racing rules, opponents, multiplayer and original-game physics fidelity remain open work.

## Build and test

Start on Windows with Rust stable and MSVC C++ build tools.

```powershell
cargo test --locked --workspace
```

```powershell
cargo build --locked --release -p viewer
```

Tests use invented fixtures by default. Optional installation tests use F1C_GAME_DIR and skip when it is unset.

For viewer commands and controls, see [the viewer guide](crates/viewer/README.md). Replace example paths with your own installation.

## Help build it

Read [CONTRIBUTING.md](CONTRIBUTING.md), browse [ROADMAP.md](ROADMAP.md), then open an issue describing one small change. Contributions through pull requests are welcome: format compatibility, tests, rendering, physics research, tools, documentation and accessibility.

[Specifications](specs/) describe current behavior and format contracts. They document constraints and assumptions, not a claim of complete original-game fidelity.

## Layout

| Directory | Purpose |
|---|---|
| crates/ | Rust parsers, spatial queries, simulation and viewer |
| specs/ | Format and behavior specifications |
| .github/ | Automated checks and issue/pull-request templates |

## License and independence

Original engine code is available under **MIT OR Apache-2.0**. See [LICENSING.md](LICENSING.md). Game files and third-party material retain their own rights.

The project is not affiliated with or endorsed by the game's original developers or publishers. Independent legal clearance has not been established. Follow [CLEAN_ROOM.md](CLEAN_ROOM.md): do not submit game assets, copied code or decompiled source as engine implementation material.
