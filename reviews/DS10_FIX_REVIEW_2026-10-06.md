# DS10 fix review — accepted

**Next action:** Coordinator researches and writes the DS11 Drive behaviour spec before assigning implementation.

- **Verdict: ACCEPT.** Supersedes the changes-required verdict in DS10_REVIEW_2026-10-06.md.
- **Fixture cleanup:** DS replaced engine, HDV and tire examples. Review found PM masses, inertia and bar coordinates still copied after renaming. Coordinator replaced those remaining blocks and expected values with invented numbers in pm.rs/car.rs before acceptance.
- **Parser fixes:** spaced PM body tuples survive; NaN, infinity and fractional indexes return None. Reviewer probe independently confirms both fixes.
- **Checks: PASS.** Reviewer reran fmt, workspace/all-target clippy with warnings denied, and workspace tests after the final fixture edits. HDV library: 39 passed / 0 failed.
- **Corpus: PASS.** Installed root: seen=155 parsed=155 failed=0. Missing-root and empty-root runs correctly fail. Unset root skips. Coverage is section headers only.
- **Real-car check: PASS.** Fresh headless CarPhysics::load run on Ferrari28 resolves HDV plus engine/gears/tires/suspension, reports no missing files, and reproduces mass 590 kg, six gears, final drive 6.462, torque 400 Nm at 12000 RPM, rev limit 15900 RPM, Soft Compound index 1. Viewer summary wiring was inspected; no new visual check needed for console-only output.
- **Continuity:** DS10 is parsing only. PM constraints are counted; tire curves/blocks are not decoded. DS11 must specify simulation behaviour and any extra parsed fields before implementation.
- **Limits:** descending gear sort is an observed-order assumption. On 64-bit targets the index overflow check is sound; on 32-bit it conservatively excludes usize::MAX. That impractical list index does not block this Windows 64-bit batch.
- **Evidence:** scratch probes remain under %TEMP%\f1c_openengine\ds10_review. No game assets are committed; former copied fixture markers are absent.