# DS10 fix handoff — review blockers

**Next action:** DeepSeek fixes DS10, runs checks, updates handoffs/DS10_REPORT.md. No commit.

- **Read:** reviews/DS10_REVIEW_2026-10-06.md, CLEAN_ROOM.md, specs/PHYSICS_FILES.md. Stay within this workspace; no further game-file inspection is needed for these fixes.
- **Fixtures:** replace all game-derived fixture blocks with small invented names and values across formats-hdv tests. Update expected results. Remove copied game comments. Keep real Ferrari acceptance values only in the observed spec/report; real-data tests read files at run time.
- **PM:** parse multiple key/value pairs while preserving spaces within parenthesized values. Prove body mass, inertia, position and orientation survive spaced tuples using invented data.
- **Indexes:** reject NaN, infinity, negative, fractional and out-of-range numbers; accept integer spellings such as 4 and 4.0. Test boundaries without accepting overflow through a saturating cast.
- **Corpus:** unset F1C_GAME_DIR still skips. An explicitly configured unreadable root or zero HDVs must fail clearly. Surface traversal read errors instead of silently treating them as successful coverage.
- **Temp:** separate fixture directories per test under %TEMP%\f1c_openengine; prevent simultaneous fixture writes. Do not hard-delete outputs.
- **Checks:** fmt, clippy for workspace/all targets with warnings denied, cargo test --workspace, and HDV corpus with the installed game root. Record exact counts and failures.
- **Report:** acknowledge corrected fixture provenance; distinguish header-only corpus coverage from typed/linked-file checks. Preserve Ferrari summary as previous-run evidence unless rerun.