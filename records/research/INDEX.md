# Research index

**Next action:** coordinator reviews DS14 barrier stop; owner drives with `--barrier-stop`.

| Question / topic | Date | Result | Record / related specs |
|---|---|---|---|
| What project/game coverage is accepted? | 2026-10-06 | Dated foundation inventory; superseded DS11 state is in ROADMAP/review | [Coverage](PROJECT_COVERAGE_2026-10-06.md) |
| Does DS11 meet the flat-ground contract? | 2026-10-06 | Accepted after torque-band/finite-wheel fixes; 169 checks pass | [Review findings](DS11_REVIEW_2026-10-06.md), [DRIVE](../../specs/DRIVE.md) |
| Where did DS11 assumptions originate? | 2026-10-06 | Existing ledger backfilled; external experiments not rerun | [Source ledger](DS11_SOURCES_2026-10-06.md), [DRIVE](../../specs/DRIVE.md); restricted source locations inside note |
| What data can road-height queries use? | 2026-10-06 | Adelaide has 54 explicit-true HAT lines; parser currently discards flags; conservative DS12 design | [Ground query scope](GROUND_QUERY_2026-10-06.md), [spec](../../specs/GROUND_QUERY.md) |
| Does the DS12 probe load real surfaces and hit at spawn? | 2026-10-06 | Adelaide grid 0: 52 selected meshes, 27,755 retained triangles, spawn hit 4.663 m (ref 4.709 m), source TRACK02C.mts | [Ground query measurements](DS12_GROUND_QUERY_2026-10-06.md), [spec](../../specs/GROUND_QUERY.md) |
| How can a prototype follow road height safely? | 2026-10-06 | DS12 independently verified; last-height reference and latched-loss design ready | [Road-follow scope](ROAD_FOLLOW_2026-10-06.md), [spec](../../specs/ROAD_FOLLOW.md) |
| Does DS13 follow sampled road height per step? | 2026-10-06 | Adelaide grid 0: 600/600 straight steps accepted, height range 0.102 m, max jump 0.0004 m; loss/timing headless checks pass; spec L typo corrected via F×U | [Road-follow measurements](DS13_ROAD_FOLLOW_2026-10-06.md), [spec](../../specs/ROAD_FOLLOW.md) |
| How can prototype barriers avoid timing geometry? | 2026-10-06 | Active Coll flags 100 true/694 false; hidden Xfinish timing-tagged; swept visible-steep proxy design ready | [Barrier scope](BARRIER_STOP_2026-10-06.md), [spec](../../specs/BARRIER_STOP.md) |
| Does the DS14 swept proxy build, clear spawn and query loaded barriers? | 2026-10-06 | Adelaide grid 0: 95 selected/699 excluded; 48,091 retained/72,356 rejected (1 degenerate, 72,355 shallow); spawn clear; diagnostic face sweep `t=0.6250` on TRACK02D.mts; 600-step straight made no contact; broad-phase XZ-axis bug fixed | [Barrier-stop measurements](DS14_BARRIER_STOP_2026-10-06.md), [spec](../../specs/BARRIER_STOP.md), [report](../../handoffs/DS14_REPORT.md) |
| Does DS14 sweep resolve earliest contact safely; what blocks publication? | 2026-10-06 | Uncertain-candidate fix and 253/0; runtime pass; release exe locked; AIW fixture corrected | [Review/audit](DS14_REVIEW_2026-10-06.md), [spec](../../specs/BARRIER_STOP.md) |
