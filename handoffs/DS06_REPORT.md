# DS06 report — whole-car assembly from `.veh` + `.gen`

**Next action:** owner does the visual check (viewer window is **open** on
`1994_Ferrari28.veh`). Look for: wheels at corners, helmet in cockpit, wings at
the ends, driver present. Claude: review the diff.

## Result: pass

All acceptance commands green. Car mode builds one model from 22 `.gen` meshes.

| Check | Result |
|---|---|
| `cargo fmt --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo test` | pass (formats-gen 18, formats-mts 13, viewer 19, mas 8 + corpus) |
| `1994_Ferrari28.veh` run | **22 meshes, 0 missing** |

## Console output (car mode)

```
Search path: Team.mas, 1994.mas, CDB.MAS, CMAPS.MAS, drivers.mas
Car: 1994_Ferrari28.veh, 22 meshes, 0 missing
SLOT000 GB28VA.MTS found (from Team.mas)
SLOT000 FERBB.MTS found (from Team.mas)
SUSPENSION FERSUSPA.MTS found (from Team.mas)
BODY FERDRIVER.MTS found (from Team.mas)
HELMET 28HELMA.MTS found (from Team.mas)
BACKFIRE FERBACKFIRE.MTS found (from Team.mas)
LFTIRE GTLFA.MTS found (from Team.mas)
LFTIRE FERRLFA.MTS found (from Team.mas)
LFSPINDLE FERSLFA.MTS found (from Team.mas)
RFTIRE GTRFA.MTS found (from Team.mas)
RFTIRE FERRRFA.MTS found (from Team.mas)
RFSPINDLE FERSRFA.MTS found (from Team.mas)
LRTIRE GTLRA.MTS found (from Team.mas)
LRTIRE FERRLRA.MTS found (from Team.mas)
LRSPINDLE FERSLRA.MTS found (from Team.mas)
RRTIRE GTRRA.MTS found (from Team.mas)
RRTIRE FERRRRA.MTS found (from Team.mas)
RRSPINDLE FERSRRA.MTS found (from Team.mas)
FWING GB28FWA.MTS found (from Team.mas)
RWING GB28RWA.MTS found (from Team.mas)
RAINLIGHT FERRNLT.MTS found (from Team.mas)
ARMS BDRIVERARMS.MTS found (from drivers.mas)
```

Matches the handoff's 22-name guide exactly. `BDRIVERARMS.MTS` comes from
`drivers.mas` (not `Team.mas`); everything else from `Team.mas`.

## What changed

- **formats-mts**: `Mts.position: [f32; 3]` read from geometry header G+256
  (spec v1.2). Test `reads_geometry_position_at_256`.
- **formats-gen**: `veh_gen_string()`, `expand_tokens()`, `GenMesh`,
  `car_meshes()` (MAX/NOTSPIN + Render/ShadowObject/LODIn rule, nesting stack,
  per-instance dedupe). 8 new unit tests: every token form (`<digits>`, `<ID>`,
  past-end, unknown), tags, comments, nesting, dedupe, LODIn, Render/Shadow.
- **viewer lib**: `resolve_search_path_for_veh()` + `veh_gen_text()` (share the
  `.gen`/`SearchPath` code with `resolve_search_path`); `MeshIndex` with exact
  match then `-NAME.MTS` fallback; `mts_to_submeshes_at()` adds the placement
  offset before the Z mirror. 8 new unit tests (temp dirs).
- **viewer main.rs**: `.veh` argument = car mode; `.mas` unchanged. Car mode
  merges every mesh's materials with index remap, places each at its MTS
  position, prints the `Car:` + per-mesh lines, camera frames the whole car.
- **viewer README**: car mode section.

## Notes / open points for the reviewer

- `.veh` mode reads the `GenString=` value; `<ID>` is `000` per the handoff.
- Mesh + instance names are upper-cased after token expansion (matches the
  on-disk `GB28VA.MTS` and the handoff guide).
- Found-but-unparseable a mesh: printed as `found`, skipped with an
  `eprintln!`; not counted as missing.
- Placement correctness beyond "0 missing" is the owner's visual call.

## Not committed

Working tree only, as instructed.
