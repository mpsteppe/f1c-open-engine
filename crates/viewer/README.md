# viewer

Shows one textured MTS model from an F1C MAS archive in a Bevy window.

## Run

```
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\2004RH.MAS"
```

Optional: add a model name to open a specific one, e.g. `... 2004RH.MAS BBRRLFA.MTS`.

Car mode: pass a team `.veh` file to assemble and show the whole car (body,
wheels, suspension, driver, helmet, wings) in one textured view.

```
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh"
```

Track mode: pass a circuit `.SCN` file to show the whole track, textured, with
a fly camera. The `SearchPath=`/`MASFile=` lines decide which archives are
loaded; hidden, moveable and animated instances are skipped (the `skyboxi` ring
and its `clouds` dome are kept and follow the camera). Sun, ambient colour,
linear fog, clear colour and draw distance come from the `.SCN`.

```
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN"
```

Car on track: add `--car <veh>` to place the assembled car on a grid slot of the
sibling `.aiw` (same folder, same stem). `--grid N` picks the slot (default 0).
The camera starts 8 m behind and 3 m above the car.

```
cargo run -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0
```

The yaw sign is inferred; press **R** to flip it and print `Yaw sign: +1/-1`.

Drive mode (prototype prototype): add `--drive` to the car-on-track command. The car
is validated against its physics files, then becomes a flat-ground car you can
accelerate, brake, steer and shift. The window opens only after validation
passes; a missing grid, unreadable car or unsupported drivetrain stops with a
message naming the field to fix.

```
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive
```

- **Limit:** the ground is an infinite flat plane at the selected grid's Y.
  Roads, walls, slopes and other objects are visual only; the car can clip into
  or float above slopes, and there are no barriers, tire slip, suspension or
  aero. A short grid-area drive is the intended test, not a full lap or a match
  for the original handling.

### Drive controls

- **W** gas, **S** brake, **A**/**D** steer left/right.
- **E** shift up, **Q** shift down; **N** is below first, no reverse.
- **R** reset to the grid, **P** pause.
- **T** textures, **L** lighting, **Esc** quit.
- HUD shows speed (km/h), gear, RPM, pedals, pause state and the prototype label.

### Ground probe (prototype prototype)

Add `--ground-probe` (needs the track `.SCN`, `--car` and `--drive`) to load the
track's explicit `HATTarget=True` surfaces and query the nearby road height under
the rear axle. Invalid combinations are rejected before the window opens.

```
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --ground-probe
```

- **Ground probe only** — the car stays on the flat plane. This mode does not
  follow roads and cannot prove a safe full lap.
- Preflight prints selected/excluded/unsupported mesh counts and retained/
  rejected triangle counts, and fails with a clear message if no surface is
  within 2 m of the spawn rear axle.
- The HUD shows the hit height, the grid-to-surface difference, the source mesh
  and the tested candidate count, or `No nearby surface` when driving out of
  coverage. Reset restores the grid reference; pause keeps showing the current
  result; texture/lighting toggles keep the index.

### Road follow (prototype prototype)

Add `--road-follow` (needs the track `.SCN`, `--car` and `--drive`) to make the car
follow the height and tangent plane of the nearby `HATTarget` surface under its
rear axle. This is kinematic surface following, not wheel contact, suspension or
original handling parity.

```
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --road-follow
```

- **Road-follow prototype — no suspension or collisions.** The car's height and
  tilt follow the surface; horizontal prototype motion is unchanged.
- Preflight selects the same surfaces as `--ground-probe`, requires a hit within
  2 m of the spawn rear axle and prints the source, height, normal and grid
  difference once.
- Each 120 Hz step proposes the prototype step, queries the surface under the
  proposed rear axle using the **last accepted height** as reference, and accepts
  it only when the height changes by at most 0.25 m. A missing hit, a query
  error or a larger jump rejects the step, stops the car and latches
  `Surface lost — R to reset`.
- **R** restores the spawn surface and clears the loss; **P**, **T** and **L**
  behave as in flat drive. Reset waits for held controls to be released.
- The HUD shows the surface height, source mesh, candidate count and
  following/lost state on a dark panel. No full-lap acceptance claim.

### Barrier stop (prototype prototype)

Add `--barrier-stop` (needs the track `.SCN`, `--car`, `--drive` and
`--road-follow`) to stop a one-sphere car proxy at selected steep visible
`CollTarget=True` geometry. The proxy is a single `0.75 m` sphere centred
`0.75 m` above the road-follow mesh origin; each 120 Hz step sweeps it from the
last accepted centre to the proposed centre.

```
cargo run --release -p viewer -- "C:\F1Research\F1 Challenge V10\SeasonData\Circuits\Australia\1994_Adelaide\1994_Adelaide.SCN" --car "C:\F1Research\F1 Challenge V10\SeasonData\Vehicles\Ferrari\1994_412T1\1994_Ferrari28.veh" --grid 0 --drive --road-follow --barrier-stop
```

- **Barrier-stop prototype — sphere proxy, no damage.** Only visible
  (`Render=True`), top-level, static, non-sky explicit `CollTarget=True` meshes
  select; a HAT-only mesh never selects and hidden (`Render=False`) occurrences
  are excluded, so some real invisible barriers are omitted. Only faces steeper
  than 60 degrees are retained.
- Preflight builds the barrier set, prints selected/excluded/unsupported and
  retained/degenerate/shallow counts, verifies the stationary spawn proxy is
  clear, and runs a constructed sweep to a loaded retained triangle's face.
- On contact the whole proposal is discarded: the car keeps its last pose and
  gear, stops, and latches `Barrier stopped — R to reset`. No t-based
  repositioning, sliding, bounce or damage. `R` restores the spawn and clears
  the latch; `P`/focus/`T`/`L` do not clear it.
- One central sphere can allow nose/wheel penetration; this is a conservative
  prototype, not original collision behaviour.

## Controls

- **Left/Right** arrows: next / previous model.
- **T**: toggle textures on / off.
- **Left-drag**: orbit the camera.
- **Wheel**: zoom in / out.
- **Esc**: quit.

### Track mode (fly camera)

- **W A S D**: move forward / left / back / right.
- **Q / E**: move down / up.
- **Right-drag**: look around.
- **Shift**: 5x speed.
- **Wheel**: change speed.
- **L**: toggle SCN lighting vs the old default light.
- **Esc**: quit.

- Top-left text shows the model name, triangle count and how many textures
  resolved (`Textures found/total`).

Models that are unsupported or empty are skipped automatically.

## Textures

Texture names come from each material's first stage. Lookup tries the name as
given, then `.BMP`, then `.TGA`, case-insensitively; animated stages try
`NAME00.BMP` / `NAME00.TGA` (frame 0) first. The index is the team's texture
search path: the `.veh` `Graphics=` file names the `.gen` whose
`SearchPath=`/`MASFile=` entries are resolved in the vehicles root. Missing or
undecodable textures fall back to grey.
