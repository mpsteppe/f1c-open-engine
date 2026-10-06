# F1C car physics files (observed)

Format notes for the text files a car uses for physics. Everything here is read
from the player's game folder at run time and never committed. Parsing only;
the values are not simulated yet (that is DS11).

Provenance: observed 2026-10-06 from the files named under each heading in the
clean reference install. No decompile was used.

## Chain

1. `.veh` names `HDVehicle=1994_ferrari.hdv` (name is case-insensitive; the file
   sits in the same team folder as the `.veh`).
   Provenance: `SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh`.
2. The `.hdv` names four linked files: engine, gear ratios, tire brand,
   suspension model.

Linked names are resolved case-insensitively in the team folder first, then in
the nearest ancestor folder named `Vehicles` (`SeasonData\Vehicles`).

## Common INI shape (`.hdv`, engine `.ini`, gear `.ini`, `.tbc`)

- `[SECTION]` starts a section; it runs to the next header.
- `Key=Value`; the first `=` splits the pair, both sides trimmed.
- `//` starts a comment to end of line, anywhere.
- Keys and section names are case-insensitive.
- Repeated keys are allowed and stay in file order.
- Values: number, bare word (`REAR`, `1994_f1susp.pm`), quoted string
  (`"Ferrari"`), or tuple `(a, b, c)` with spaces allowed.
- `XRange=(min, step, count)` plus `XSetting=n` means `value = min + step * n`.
- Units: SI, engine speed RPM, angles degrees. Axes +x left, +y up, +z rear.
  Provenance: `.hdv` header comments.

## `.hdv`

Sections seen: GENERAL, FRONTWING, REARWING, BODYAERO, DIFFUSER, SUSPENSION,
CONTROLS, ENGINE, DRIVELINE, FRONTLEFT, FRONTRIGHT, REARLEFT, REARRIGHT.

Keys the engine needs:

| Section | Key | Meaning |
|---|---|---|
| GENERAL | `Mass` | kg, all mass except fuel |
| GENERAL | `Inertia` | `(x, y, z)` kg m^2, except fuel |
| GENERAL | `CGHeight` | m above the reference plane |
| GENERAL | `TireBrand` | `.tbc` name without extension, before the setting |
| GENERAL | `TireCompoundSetting` | compound index inside the brand |
| SUSPENSION | `PhysicalModelFile` | `.pm` file name |
| ENGINE | `Normal` | unrestricted engine `.ini` name, no extension |
| DRIVELINE | `WheelDrive` | `REAR`, `FOUR` or `FRONT` |
| DRIVELINE | `GearFile` | gear ratios `.ini` name |
| DRIVELINE | `FinalDriveSetting` | index into the final-drive list |
| DRIVELINE | `ReverseSetting` | index into the gear list |
| DRIVELINE | `ForwardGears` | number of forward gears |
| DRIVELINE | `GearNSetting` | index into the gear list, N = 1..ForwardGears |
| CONTROLS | `SteerLockRange`/`SteerLockSetting` | steering lock in degrees |
| FRONTLEFT..REARRIGHT | `BrakeTorque` | max brake torque at zero wear |

Provenance: `SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari.hdv`.

## Engine `.ini`

No sections; a flat list of `Key=Value` lines.

- Repeated `RPMTorque=(rpm, min_torque, max_torque)`: keep all, in file order.
  `min_torque` is the drag side (negative), `max_torque` the full-throttle side.
  Peak torque is the largest `max_torque`; for Ferrari 043 that is
  `400.0 Nm @ 12000 RPM`.
- `EngineInertia` (0.06620), `IdleRPMLogic=(low, high)`.
- `RevLimitRange=(15400, 100, 6)` + `RevLimitSetting=5` -> `15400 + 100*5 =
  15900 RPM`.

Provenance: `SeasonData\Vehicles\Ferrari\1994_412T1\Ferrari_043.ini`,
`SeasonData\Vehicles\94renaultv10.ini`.

## Gear ratios `.ini`

Two sections, each with repeated tooth-count pairs; a ratio is `driven / drive`.

- `[GEAR_RATIOS]`: 74 `ratio=(drive, driven)` lines in `1994_gear_ratios.ini`.
- `[FINAL_DRIVE]`: one `bevel=(drive, driven)` plus `ratio=` lines.

The file says each list is **sorted after reading**; the lists are already in
descending-ratio order. `GearNSetting`, `ReverseSetting` and
`FinalDriveSetting` are indexes into those lists.

Final drive = bevel ratio times the selected final-drive ratio. Worked example:
`bevel=(30,42)` and `(13,56)` -> `42/30 * 56/13 = 6.031`, matching the file's
own `// 6.031` comment. Ferrari 043: `FinalDriveSetting=1` and the list
`(13,70), (13,60), ...` give `42/30 * 60/13 = 6.462`.

Provenance: `SeasonData\Vehicles\1994_gear_ratios.ini`,
`SeasonData\Vehicles\gear_ratios.ini`.

## Tire brand `.tbc`

- `[SLIPCURVE]`: a `Name`, a `Step`, then `Data:` rows of bare numbers. Ignored.
- `[COMPOUND]`: repeated, one per compound, in file order. Each has
  `Name="Hard Compound"`, `WetWeather=1`, then `Front:` and `Rear:` value
  blocks (free-text labels, not headings). `TireCompoundSetting` indexes this
  list; Ferrari 043 index 1 -> `Soft Compound`.

Provenance: `SeasonData\Vehicles\1994_Goodyear.tbc`.

## Suspension `.pm`

A line may hold several `key=value` pairs separated by spaces, so it is
tokenised per line rather than by the common INI rule.

- `[BODY]`: `name=`, `mass=(...)`, `inertia=(x,y,z)`, `pos=(x,y,z)`,
  `ori=(x,y,z)`.
- `[BAR]`, `[JOINT]`, `[HINGE]`, `[JOINT&HINGE]`: constraints. Each section is
  one constraint; its line holds `posbody=`, `negbody=`, `pos=`, `neg=`,
  `axis=`. `//` comments are honoured.

Provenance: `SeasonData\Vehicles\1994_f1susp.pm`.

## Corpus

155 `.hdv` files under the game install parse with a `[GENERAL]` and
`[DRIVELINE]` section; 0 failures (2026-10-06).
