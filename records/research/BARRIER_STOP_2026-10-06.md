# Barrier-stop scope — 2026-10-06

**Next action:** DS14 implements swept proxy queries and records actual geometry/clearance measurements.

- **Question:** can the next prototype stop at candidate barriers without confusing timing geometry with physical walls?
- **Source tier/location:** registered game data SeasonData/Circuits/Australia/1994_Adelaide/1994_Adelaide.SCN. Workspace specs/ROAD_FOLLOW.md, specs/GROUND_QUERY.md, formats-scn/viewer loading and DS13 road follower. No restricted material consulted.
- **Method:** inspect active SCN CollTarget lines and the Xfinish block; inspect fixed-step proposed/accepted follower behavior; design a bounded new contract. No triangle collision measurements yet.
- **Observed:** excluding fully commented lines, 100 lines match CollTarget=True and 694 match CollTarget=False. These are raw active line counts, not selected barriers/triangles.
- **Observed:** Instance=Xfinish has Render=False, MeshFile with CollTarget=True/HATTarget=False, Response=VEHICLE,TIMING. Thus CollTarget=True alone cannot distinguish a visible barrier from this timing-tagged hidden instance.
- **Limits:** no original Response/collision semantics tested; Render=False filtering can omit invisible physical barriers. A WALL02-named mesh is CollTarget=False in this sample, so names are not a valid substitute for the prototype's explicit flags. This does not prove whether it is a physical barrier in the original.
- **Design choices:** only visible explicit-true static top-level geometry; steep normal |Y|<0.5; one radius0.75 m sphere at follow mesh origin + worldY0.75 m; swept contact; retain prior pose/stop/latch. These are prototype choices, not original collision behavior.
- **Oracle:** wall X=5, sphere X0→10, r0.75 first centre X=4.25 gives fraction0.425; analytically derived invented geometry. DS14 tests must verify finite face/edge/vertex behavior as well.
- **Confidence:** high for source text/counts; selected steep-triangle counts, spawn clearance, runtime contact and gameplay usability unknown until DS14. Single sphere can allow nose/wheel penetration; hidden barriers omitted. Full M05 contact remains incomplete.
- **Open Engine use:** prevent supported proxy tunneling and add a bounded stop response after accepted road following.
- **Original modding use:** possible future explicit-flag/geometry diagnostics; untested. No new original solver finding and no game changes authorized.

Reproduce raw active flag counts from registered SCN:

```powershell
$lines=Get-Content -LiteralPath 'C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN'; $active=@($lines | Where-Object { -not $_.TrimStart().StartsWith('//') }); [pscustomobject]@{ True=@($active | Select-String 'CollTarget\s*=\s*True').Count; False=@($active | Select-String 'CollTarget\s*=\s*False').Count }
```

[Spec](../../specs/BARRIER_STOP.md) · [Handoff](../../handoffs/DS14_HANDOFF_barrier_stop_2026-10-06.md) · [DS13 review](../../reviews/DS13_REVIEW_2026-10-06.md). No external F1 research newly consulted.
