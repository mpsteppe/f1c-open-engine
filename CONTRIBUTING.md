# Contributing

**Start:** choose one item from [ROADMAP.md](ROADMAP.md) and open an issue with a concrete proposal.

- **Scope:** one focused change per pull request. Explain the problem, resulting behavior, checks and remaining limits.
- **Source:** implement from public documentation, clean specifications and reproducible observations. Follow [CLEAN_ROOM.md](CLEAN_ROOM.md).
- **Fixtures:** invent example names and values. Read real installation data only at runtime; never commit or attach it.
- **Research:** describe the question, source, reproducible method, units, confidence and limitations. Distinguish observed behavior from prototype design decisions.
- **Compatibility:** report the relevant format/version and a minimal invented fixture or reproduction command.
- **Review:** passing tests does not prove original-game fidelity. Add measurements and scope for fidelity claims.
- **Community:** be respectful. Avoid private details and credentials in issues, logs and screenshots.

Run these checks before submitting:

```powershell
cargo fmt --all -- --check
```

```powershell
cargo clippy --locked --workspace --all-targets -- -D warnings
```

```powershell
cargo test --locked --workspace
```

Viewer changes also need:

```powershell
cargo build --locked --release -p viewer
```

Optional installation tests use F1C_GAME_DIR. State when those tests were skipped. Visual changes should include reproducible steps and screenshots without game files attached.
