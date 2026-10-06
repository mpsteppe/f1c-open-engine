# MTS model format (CUBE_MTS_4.10) — spec v1.2

v1.2 (2026-10-06): geometry header offset 256 = mesh position (car space).

v1.1 (2026-10-06): geometry magic is 14 bytes, not 16 (prototype finding); Z3DM entries added.

Provenance: researchers. Field sizes and meanings come from black-box statistics over
the player's own install (49,283 MTS entries in 685 MAS archives, 2026-10-06).
Material/texture-stage record sizes and the geometry count/offset pairs were
cross-checked against spec-tier knowledge; only sizes and meanings are recorded
here, no code. Status per field: VERIFIED (holds on every 4.10 file), INFERRED,
UNKNOWN (preserve, do not interpret).

All integers little-endian. f32 = IEEE float. Strings: fixed 32-byte,
NUL-terminated, Latin-1.

## Variants

| Variant | Detection | Count | v1 support |
|---|---|---|---|
| 4.10 | file starts with `CUBE_MTS_4.10` + 3 NUL (16 bytes) | 46,236 | YES |
| legacy 4.01 | no leading magic; geometry magic `CUBE_MTS_4.01` | 3,046 | NO: return `Unsupported { version: "4.01" }` |
| Z3DM | file starts with `Z3DM` (misnamed, e.g. CANADA.MAS GARAGENEW.Z3D.MTS) | 1 | NO: return `Unsupported { version: "Z3DM" }` |

## File layout (4.10)

1. Magic: `CUBE_MTS_4.10` + 3 NUL (16 bytes, VERIFIED all zero padding).
2. `material_count` u32.
3. `material_count` material records (variable size).
4. Geometry block at offset G (= end of last material). Starts with 14 bytes
   `CUBE_MTS_4.10` + NUL (VERIFIED 46,236/46,236). Bytes 14-15 vary
   (UNKNOWN; 5,181 files nonzero) — never check them. All geometry offsets below are relative to G.

VERIFIED: walking materials with the rule below lands exactly on the geometry
magic in 46,236 / 46,236 files.

## Material record

| Off | Size | Field | Status |
|---|---|---|---|
| 0 | 32 | name | VERIFIED |
| 32 | 4 | flags u32 | UNKNOWN |
| 36 | 16 | color A: 4 x f32 (r,g,b,a) | INFERRED ambient |
| 52 | 16 | color B | INFERRED diffuse |
| 68 | 16 | color C | INFERRED specular |
| 84 | 16 | color D | INFERRED emissive |
| 100 | 2 | src_blend u16 (Direct3D blend enum: 1 ZERO, 2 ONE, 5 SRCALPHA, 6 INVSRCALPHA) | INFERRED |
| 102 | 2 | dst_blend u16 | INFERRED |
| 104 | 4 | specular power f32 | INFERRED |
| 108 | 4 | `stage_count` u32 | VERIFIED |
| 112 | var | `stage_count` texture stages | VERIFIED |

Material size = 112 + sum of stage sizes.

## Texture stage record

| Off | Size | Field | Status |
|---|---|---|---|
| 0 | 32 | texture file name, no path (e.g. `HUB.BMP`) | VERIFIED |
| 32 | 4 | u32 | UNKNOWN |
| 36 | 4 | u32 | UNKNOWN |
| 40 | 4 | `frames` u32 | VERIFIED |
| 44 | var | only if frames > 1: `n` u32, then n x u32, then 2 x u32 (animation table) | VERIFIED size |
| next | 36 | 9 x u32 stage parameters | UNKNOWN |

Stage size = 80 if frames <= 1, else 80 + 12 + 4*n.

## Geometry header (328 bytes at G)

| Off | Size | Field | Status |
|---|---|---|---|
| 0 | 14 | magic `CUBE_MTS_4.10` + NUL | VERIFIED |
| 14 | 2 | | UNKNOWN (varies) |
| 16 | 4 | u32 | UNKNOWN |
| 20 | 4 | `geom_flags` u32. Bit 0x800000 = 64-byte vertices | VERIFIED (26 files) |
| 24 | 232 | various | UNKNOWN |
| 256 | 12 | `position` 3 x f32 (x, y, z), same axes as vertices. Car parts with local vertices (wheels, spindles, helmet, driver, steering wheel) are placed by adding it. Body meshes have (0,0,0). | INFERRED (Ferrari 1994 wheels at +-0.82/-0.70 x, matching the `.pm` track; helmet y 0.49) |
| 268 | 12 | | UNKNOWN |
| 280 | 4 | `group_count` | VERIFIED |
| 284 | 4 | `group_offset` (records 64 bytes) | VERIFIED |
| 288 | 4 | `face_count` | VERIFIED |
| 292 | 4 | `face_offset` (records 36 bytes) | VERIFIED |
| 296 | 8 | count/offset pair X | UNKNOWN |
| 304 | 8 | count/offset pair Y; count > 0 also means 64-byte vertices | INFERRED |
| 312 | 16 | | UNKNOWN |

Vertex stride = 64 if `geom_flags & 0x800000` or pair-Y count > 0, else 48.
Do NOT derive stride from offset gaps: 244 groups have padding between the
vertex and index arrays.

## Group record (64 bytes)

| Off | Size | Field | Status |
|---|---|---|---|
| 0 | 4 | flags u32 (common: 0x80, 0x81, 0x82, 0x182) | UNKNOWN |
| 4 | 4 | `material_index` into this file's materials | VERIFIED (< material_count everywhere) |
| 8 | 4 | `vertex_count` | VERIFIED |
| 12 | 4 | `vertex_offset` (relative to G) | VERIFIED |
| 16 | 4 | `index_count` | VERIFIED |
| 20 | 4 | `index_offset` (relative to G), u16 indices | VERIFIED |
| 24 | 40 | | UNKNOWN (likely bounds) |

The u16 index list mixes triangle lists and strips with degenerates. v1 ignores
it and renders from face records.

## Face record (36 bytes)

| Off | Size | Field | Status |
|---|---|---|---|
| 0 | 6 | 3 x u16 vertex indices, local to the group | VERIFIED |
| 6 | 2 | u16 | UNKNOWN |
| 8 | 4 | `group_index` | VERIFIED |
| 12 | 24 | 6 x f32 | UNKNOWN (likely plane) |

Every triangle of every group appears in its face set (163,312 groups checked).

## Vertex (48 bytes; the 64-byte variant adds 16 UNKNOWN bytes at the end)

| Off | Size | Field | Status |
|---|---|---|---|
| 0 | 12 | position 3 x f32 | VERIFIED |
| 12 | 12 | normal 3 x f32 (unit length) | VERIFIED |
| 24 | 4 | color u32, Direct3D ARGB | INFERRED |
| 28 | 4 | u32 | UNKNOWN (likely specular color) |
| 32 | 8 | uv0 2 x f32; values leave 0..1, renderer must use wrap (repeat) addressing | VERIFIED (owner visual, prototype) |
| 40 | 8 | uv1 2 x f32 | INFERRED |

Coordinate system: Direct3D left-handed, Y up. Viewers convert by negating Z
only; keep triangle order (the mirror already flips winding). VERIFIED by owner
visual check 2026-10-06 (v1 said reverse winding: wrong, faces showed from behind).
