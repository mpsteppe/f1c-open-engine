# Road-height query scope — 2026-10-06

**Next action:** DS12 implements specs/GROUND_QUERY.md; sampled height measurements come from its runtime test.

- **Question:** what data can the next clean-room batch use for nearby road-height queries, without guessing full contact physics?
- **Source tier/location:** data-tier sources.toml game, SeasonData/Circuits/Australia/1994_Adelaide/1994_Adelaide.SCN. Workspace code: crates/formats-scn/src/lib.rs; crates/viewer/src/main.rs run_track; crates/viewer/src/lib.rs mts_to_submeshes_at. Prior distilled format observation: handoffs/DS07_HANDOFF_track_viewer_2026-10-06.md.
- **Method:** read registered SCN only, count HATTarget lines using case-insensitive PowerShell regex, inspect workspace parser/loading. No restricted sources, extracted assets or external web sources used.
- **Observed:** 822 lines contain HATTarget; 54 match HATTarget=True, 768 match HATTarget=False. Counts are lines, not unique instances, triangles or validated roads. MeshFile can carry CollTarget and HATTarget on the same line.
- **Static deduction:** current parser keeps mesh names but drops those flags; current loader skips Render=False before reading geometry. Reusing rendered meshes alone would omit hidden candidate geometry and mix surface/decorative content.
- **Rejected shortcut:** selecting all visible triangles or TRACK-name meshes is not supported surface classification. A generic collision dependency cannot supply missing data semantics.
- **Prototype choices:** explicit-true flags only; static top-level geometry; nearby height chosen by reference Y; 2 m diagnostic band; car stays flat. These are not findings about original-engine HAT defaults, layering, slope limits or dynamics.
- **Confidence/limits:** high for line counts and workspace behavior. No original-engine semantic verification, road-height/grid measurement, triangle counts or full-contact model yet. Selected moving/nested/animated geometry is intentionally unsupported.
- **Open Engine use:** create testable indexed surface queries before slope/airborne dynamics; preserve hidden geometry metadata.
- **Original-game modding use:** possible diagnostic of a mod's explicit flags/geometry gaps; untested until DS12 measurements. No claim about how the original engine reacts, and no game/mod edits authorized.

Reproduce source counts from this workspace:

```powershell
$lines = Get-Content -LiteralPath 'C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN'; $hits = @($lines | Select-String 'HATTarget'); [pscustomobject]@{ Lines=$hits.Count; True=@($hits | Where-Object { $_.Line -match 'HATTarget\s*=\s*True' }).Count; False=@($hits | Where-Object { $_.Line -match 'HATTarget\s*=\s*False' }).Count }
```

[Spec](../../specs/GROUND_QUERY.md) · [Handoff](../../handoffs/DS12_HANDOFF_ground_query_2026-10-06.md). No related external F1 research newly consulted.
