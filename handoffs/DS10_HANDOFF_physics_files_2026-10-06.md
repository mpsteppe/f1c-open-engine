# DS10 handoff — parse car physics files (HDV, engine INI, gears, TBC, PM)

**Goal:** new crate `formats-hdv` that reads every physics text file a car uses.
Parse only; no physics simulation yet (that is DS11 "Drive").

## Facts from Team A (clean: game text files document themselves, 2026-10-06)

- Chain: `.veh` `HDVehicle=1994_ferrari.hdv` (case-insensitive name; same folder
  as `.veh`).
- HDV is INI-like: `[SECTION]` lines, `Key=Value`, `//` comments anywhere.
  Sections seen: GENERAL, FRONTWING, REARWING, BODYAERO, DIFFUSER, SUSPENSION,
  CONTROLS, ENGINE, DRIVELINE, FRONTLEFT, FRONTRIGHT, REARLEFT, REARRIGHT.
- Values: number, quoted string, bare word (`REAR`, `1994_f1susp.pm`), or
  tuple `(a, b, c)` with spaces allowed.
- `XRange=(min, step, count)` + `XSetting=n` means value = `min + step*n`.
  Provide a helper for this.
- HDV header: SI units, RPM, degrees. Axes: +x left, +y up, +z rear.
- Linked files (resolve case-insensitive; search `.veh` folder, then
  `SeasonData\Vehicles`):
  - `[ENGINE] Normal=Ferrari_043` -> `Ferrari_043.ini` (team folder).
    Repeated key `RPMTorque=(rpm, min_torque, max_torque)`: keep ALL, in order.
    Also `RevLimitRange/Setting`, `EngineInertia`, `IdleRPMLogic`, etc.
  - `[DRIVELINE] GearFile=1994_gear_ratios.ini` -> `SeasonData\Vehicles`.
    `GearNSetting` / `FinalDriveSetting` / `ReverseSetting` index into its list.
  - `[GENERAL] TireBrand=1994_Goodyear` -> `1994_Goodyear.tbc`;
    `TireCompoundSetting` = compound index.
  - `[SUSPENSION] PhysicalModelFile=1994_f1susp.pm` -> `SeasonData\Vehicles`.
- Inspect gear INI, TBC and PM yourself via `python tools/f1src.py`; document
  their structure in `specs/PHYSICS_FILES.md` (format only, observed, with
  file names as provenance).

## Tasks

1. Crate `crates/formats-hdv`: generic parser (sections, ordered entries,
   repeated keys kept, tuples) + typed accessors for: mass, inertia, CG height,
   wheel drive, gear ratios (resolved), final drive, torque curve, rev limit,
   steer lock, brake torque per wheel, tire compound name, PM file name.
2. `Car physics` loader: from `.veh` path, load HDV + all 4 linked files;
   report missing files, never panic.
3. Viewer `--car` mode: print one summary block (mass, gears, peak torque@rpm,
   rev limit, tire compound, missing files).
4. Tests: unit tests on inline small strings (no game data pasted). Corpus test
   reads all `.hdv` under game install at run time, skips when absent; report
   parsed / failed counts.
5. `specs/PHYSICS_FILES.md` as above.

## Acceptance

- `cargo fmt --check`, clippy `-D warnings`, `cargo test` green.
- Ferrari28 summary printed; corpus: 0 HDV failures (or list why).
- Report `handoffs/DS10_REPORT.md` (ADHD format). No commit.
