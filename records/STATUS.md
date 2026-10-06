# Status

- 2026-10-06: workspace created (moved from F1\development\open_engine on owner
  request). MAS spec v1 written from observation; 75,578/75,578 entries verified
  by Python probe. Finding: F1C models are .MTS (gMotor 1), not GMT.
- 2026-10-06: Rust 1.99.0 stable-msvc + clippy + rustfmt installed (winget Rustlang.Rustup); MSVC BuildTools 2022 present; hello-world builds.
- 2026-10-06: DS01 DONE. `formats-mas` passes fmt/clippy/test; corpus reads
  archives=685 entries=75578. See handoffs/DS01_REPORT.md. Claude review: ACCEPTED (reviews/DS01_REVIEW_2026-10-06.md).
- 2026-10-06: MTS spec v1 (4.10 only): 46,236/46,236 material walks verified; 64-byte vertex flag found; legacy 4.01 (about 3,047) deferred. DS02 handoff ready.
- 2026-10-06: DS02 STOPPED (spec mistake), Claude review upheld. Spec v1.1: geometry
  magic = 14 bytes (13 chars + NUL), `Z3DM` -> Unsupported.
- 2026-10-06: DS02 DONE. `formats-mts` fmt/clippy/test green (12 unit tests);
  corpus mts_ok=46236 unsupported=3047 triangles=12511688 errors=0. See
  handoffs/DS02_REPORT.md. Next: Claude review DS02.
- 2026-10-06: DS02 stop 1: spec bug (geometry magic 14 bytes, not 16). Spec v1.1 fixed. DS02 resume.
- 2026-10-06: DS02 ACCEPTED (reviews/DS02_REVIEW_2026-10-06.md). 46,236 models parse, 12.5M triangles.
- 2026-10-06: Decision (Claude, owner delegated): viewer v1 grey geometry; textures = DS04. Bevy pinned 0.19.1. DS03 handoff ready.
- 2026-10-06: DS03 DONE. `viewer` fmt/clippy/test/build green (14 tests total
  incl. 5 new). Window opened on 2004RH.MAS, first model AHELMA.MTS, 56
  viewable. Viewer left open for owner. See handoffs/DS03_REPORT.md. Next:
  Claude review DS03.
- 2026-10-06: DS03 code ACCEPTED (reviews/DS03_REVIEW_2026-10-06.md). Owner visual check pending.
- 2026-10-06: HUMAN VALIDATION: Matias confirmed viewer shows 2004RH models correctly (grey, no textures). Stage 1 geometry goal met.
- 2026-10-06: DS04 textures handoff ready (folder MAS index resolves 304/~350 lookups on 2004RH).
- 2026-10-06: DS04 DONE. `viewer` fmt/clippy/test/build green (9 lib tests;
  BMP/TGA decode, folder index, sub-mesh split). Viewer ran on 2004RH.MAS:
  first 3 models textures 1/1, 1/1, 3/5 found. See handoffs/DS04_REPORT.md.
  Next: Claude review DS04; owner visual check pending.
- 2026-10-06: DS04 code ACCEPTED (reviews/DS04_REVIEW_2026-10-06.md). Owner visual check pending.
- 2026-10-06: Owner: textures grey. Claude fix: Repeat sampler (UVs outside 0..1). Re-check pending.
- 2026-10-06: Owner saw light/dark grey only. Claude check: 2004RH.MAS = helmets + wheels with genuinely grey/black textures (16x16 WCHELMET sat<=2, METALBLACK). Not a bug. Better test file: SeasonData/Vehicles/Ferrari/1994_412T1/Team.mas (62 models, 37 textures).
- Known limit: folder index is alphabetical after own MAS, so another season's texture (e.g. 1995_cmaps) can win a shared name. Fix later with season-aware search path.
- 2026-10-06: Owner: textures visible only from underneath. Cause: spec v1 winding rule wrong (Z mirror already flips winding). Claude fix: keep triangle order; spec updated.
- 2026-10-06: HUMAN VALIDATION: Matias confirmed textured Ferrari body correct (screenshot, 5/25 textures). DS05 handoff ready (option 1, Claude chose per owner delegation).
- 2026-10-06: DS05 DONE. `crates/formats-gen` (new, 10 tests) + viewer
  `resolve_search_path`; fmt/clippy/test green. Ferrari Team.mas: search path
  `Team.mas, 1994.mas, CDB.MAS, CMAPS.MAS, drivers.mas`; 412T1-GB28VA.MTS
  25/25 (was 5/25), all 62 models x/x. See handoffs/DS05_REPORT.md. Viewer left
  open for owner visual check. Next: Claude review DS05.
- 2026-10-06: Added coordinator continuity rule (AGENTS.md) + records/NEXT.md + CLAUDE.md.
- 2026-10-06: DS05 reviewed, accepted (reviews/DS05_REVIEW_2026-10-06.md). Waiting owner visual check.
- 2026-10-06: Owner visual ok DS05. MTS spec v1.2 (position at G+256). DS06 handoff written (car assembly).
- 2026-10-06: DS06 reviewed, accepted pending owner visual (car mode, 22/22 meshes).
- 2026-10-06: Owner visual DS06: car assembled. Claude fixes: magenta placeholder frame skip, skip AnimFile instances (arms).
- 2026-10-06: Owner visual ok: whole Ferrari car (DS06 + fixes). Milestone car viewer done.
- 2026-10-06: Claude chose next milestone: track viewer. DS07 handoff written.
- 2026-10-06: DS07 reviewed, accepted pending owner visual (track viewer).
- 2026-10-06: Owner visual DS07: track geometry ok. Claude fixes: JPEG data in .BMP, alpha-test cutouts.
- 2026-10-06: Owner visual ok track (Adelaide). DS08 handoff written (car on track).
- 2026-10-06: DS08 reviewed, accepted pending owner visual + yaw sign.
- 2026-10-06: Owner visual DS08: car placement + yaw +1 correct. Claude fixes: magenta colour key, hide BACKFIRE.
- 2026-10-06: Owner visual ok: car on track. Milestone done.
- 2026-10-06: Claude chose lighting/sky milestone. DS09 handoff written.
- 2026-10-06 DS09 reviewed: accepted (lighting/fog/sky); owner visual pending.
- 2026-10-06 DS09 owner visual ok. Milestone track look done.
- 2026-10-06 DS10 handoff written (physics file parsing).
- 2026-10-06 DS10 reviewed: CHANGES REQUIRED (copied game fixtures, spaced PM tuples, malformed indexes; corpus/temp test gaps). Reviewer fmt/clippy/workspace tests pass; corpus 155/0. Fix handoff ready; implementation remains uncommitted.
- 2026-10-06 DS10 fixes reviewed: ACCEPTED (8a36928; reviews/DS10_FIX_REVIEW_2026-10-06.md). Coordinator replaced remaining copied PM numbers. fmt/clippy/workspace tests pass; HDV 39 tests, corpus 155/0; fresh Ferrari loader summary matches with no missing links. Next: DS11 Drive behaviour spec.
- 2026-10-06 DS11 Drive v1 spec and handoff ready: opt-in flat-ground RWD prototype, parsed torque/gears/brakes/radii/geometry, fixed 120 Hz, manual controls, chase camera. Original-engine research distilled with explicit prototype departures; numerical oracle calculated. Next: DS11 implementation, no commit while DS runs.
- 2026-10-06 Owner requests durable research records reusable for original-game modding. Added evidence/provenance, negative-result, confidence and dual-use requirements to AGENTS.md/CLEAN_ROOM.md/CLAUDE.md. DS11 running; instruction and continuity edits saved uncommitted until DS stops.
- 2026-10-06 Project-wide ROADMAP.md added: dated accepted coverage, subsystem inventory, 13 milestone gates, compatibility/parity checks and research reuse. Added research index/coverage record and roadmap maintenance rules. DS11 remains in progress; no commit while DS runs.
- 2026-10-06 DS11 completion reported: fmt/clippy/tests/release build pass (166 passed, 0 failed), numerical oracle and Ferrari28 headless validation pass per handoffs/DS11_REPORT.md. Owner short-drive check and independent coordinator review pending; implementation remains unaccepted. Owner requested no commit; all edits preserved uncommitted.
- 2026-10-06 DS11 owner short drive worked. Coordinator review ACCEPTED within flat-ground prototype scope after fixing interpolated torque-band validation and non-finite wheel coordinates, and strengthening the brake test. Final fmt/clippy/workspace tests/release build pass (169/0). Review: reviews/DS11_REVIEW_2026-10-06.md; findings: records/research/DS11_REVIEW_2026-10-06.md. No commit per owner; HEAD 089f09f.
- 2026-10-06 DS12 spec/handoff READY: per-mesh SCN flags, indexed nearby-height queries and opt-in ground-probe HUD, preserving DS11 flat motion. Adelaide data observation: 822 HAT lines, 54 true/768 false; records/research/GROUND_QUERY_2026-10-06.md. DS11 provenance backfilled without revalidating external experiments. No commit per owner; DS12 not yet known running.
- 2026-10-06 DS12 completion reported: fmt/clippy/workspace tests/release build pass (202/0); sampled probe 52 meshes/27755 triangles, height 4.663 m versus 4.709 m reference, TRACK02C.mts, 371 candidates. Report handoffs/DS12_REPORT.md; research records/research/DS12_GROUND_QUERY_2026-10-06.md. Owner probe check and independent review pending; not accepted. No commit per owner; HEAD 089f09f.
- 2026-10-06 DS12 owner screenshots: away-from-spawn view reports no nearby surface; subsequent reset/grid view shows ground ~4.66 m, ref ~4.71 m, difference ~-0.05 m, TRACK02C.mts, 371 candidates, paused at 0 km/h. Reset/spawn HUD sample matches DS report; T/L checklist and independent code review remain unconfirmed. No commit.
- 2026-10-06 DS12 independently reviewed ACCEPTED within diagnostic scope: configured workspace 202/0, fmt/clippy/release build pass; spawn sample reproduced. Review reviews/DS12_REVIEW_2026-10-06.md. Corrected HAT count explanation: two commented true lines, 52 active. DS13 ROAD_FOLLOW spec/handoff READY (per-step tangent pose, previous-height reference, latched loss); records/research/ROAD_FOLLOW_2026-10-06.md. No commit per owner; HEAD 089f09f.
- 2026-10-06 DS13 completion reported: 216/0 tests, fmt/clippy/release build pass; 5 s real sample 600 accepted/no loss, heights 4.561–4.663 m, max jump 0.0004 m. Owner/code review pending; not accepted. Reported L-vector typo is not present in current ROAD_FOLLOW.md, which already matches F cross U; verification noted. No commit; HEAD 089f09f.
- 2026-10-06 DS13 owner drive works; review ACCEPTED within kinematic scope (reviews/DS13_REVIEW_2026-10-06.md), independent216/0/fmt/clippy/build and configured600-step sample pass. DS14 swept visible-barrier sphere stop spec/handoff READY; records/research/BARRIER_STOP_2026-10-06.md. Active SCN Coll lines100 true/694 false; hidden timing target excluded by prototype design. No commit; HEAD089f09f.
- 2026-10-06 DS14 completion reported: fmt/clippy/workspace tests/release build pass (252/0 with F1C_GAME_DIR); Adelaide grid0 barrier 95 selected/699 excluded, 48091 retained/72356 rejected (1 degenerate/72355 shallow), clear spawn; diagnostic face sweep TRACK02D.mts t=0.6250; 600-step straight no contact (negative). Broad-phase XZ-axis bug found by the diagnostic and fixed with regression test. Report handoffs/DS14_REPORT.md; measurements records/research/DS14_BARRIER_STOP_2026-10-06.md. Owner barrier drive and independent review pending; not accepted. No commit; HEAD089f09f.
- 2026-10-06 Owner requests phase wrap-up/GitHub upload after DS14 and additionally decompiled C. Publication plan records/GITHUB_PUBLICATION.md; destination asked; no remote/DS14 report yet. Decompile rights/separation and existing README legal/provenance gate unresolved. No commit/upload performed.
- 2026-10-06 Owner DS14 barrier-stop/R PASS. Coordinator review corrected unresolved-candidate earliest-hit handling (new regression); fmt/clippy253/0/configured runtime pass. Release rebuild blocked by open viewer.exe Windows lock; acceptance pending rebuild. AIW copied fixture replaced; contribution/templates/Windows CI draft prepared. GitHub destination/license/legal/decompile rights/full audit unresolved; no commit/upload. Review reviews/DS14_REVIEW_2026-10-06.md; research records/research/DS14_REVIEW_2026-10-06.md.
- 2026-10-06 Owner reiterates GitHub upload including decompiled corpus. GitHub plugin confirmed not installed in Codex, suggested installation/connection; no upload. License files prepared from existing dual-license manifests, excluding game/decompile material. Viewer close/targeted stop failed access denied; final release build remains locked. records/GITHUB_PUBLICATION.md updated; no commit.

- 2026-10-06: GitHub plugin installation verified; connected as mpsteppe. Existing public f1-challenge-decompiled tree has 14,264 C files (metadata only). Engine repository creation needs browser sign-in; final viewer rebuild still locked by pid28128. No upload/commit. See records/GITHUB_PUBLICATION.md.

- 2026-10-06: Created public mpsteppe/f1c-open-engine, main README commit a51d0a0. Browser and connector verification pass. Engine upload/audit/rebuild pending; corpus remains separate.
