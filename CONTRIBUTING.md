# Contributing

**Next action:** choose a small roadmap task and open an issue before changing behavior.

- **Start:** read README.md, ROADMAP.md, CLEAN_ROOM.md and the relevant spec. Current driving/contact systems are prototypes; passing tests do not establish original-game parity.
- **Clean room:** implement from clean behavior/format specs. Do not read or submit decompiled source, addresses, restricted research or copied game assets as engine implementation material. Researchers should provide distilled, reproducible behavior notes through the coordinator.
- **Fixtures:** invent names and numbers. Real game data is read only at runtime from your own installation; never attach it to issues or commit it.
- **Scope:** one issue and one focused pull request. State the problem, behavior change, tests and remaining limits. Discuss changes to simulation constants/contracts first.
- **Checks:** run fmt, clippy with warnings denied and workspace tests. Viewer changes also require a release build. Optional game tests use F1C_GAME_DIR and may skip when unset; distinguish skipped data checks from validated content.
- **Evidence:** record investigation results/failures in records/research and INDEX.md with source tier/location, command, units, confidence and engine/modding uses. No restricted raw content.
- **Review:** maintainers review provenance, behavior and checks before merge. Visual changes need a named owner/sample check; launching a window is not approval.
- **Conduct:** be respectful, provide reproducible reports and avoid personal information/secrets. Publication/license gates in README apply until resolved.

Run individually from the repository root:

```powershell
cargo fmt --all -- --check
```

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

```powershell
cargo test --workspace
```

```powershell
cargo build --release -p viewer
```

Windows development needs Rust with rustfmt/clippy and MSVC C++ build tools. Use a temporary output directory for extracted game data. Set F1C_GAME_DIR only for optional tests against a locally owned installation.
