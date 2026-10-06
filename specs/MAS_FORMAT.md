# MAS archive format (CUBEMAS4.10) — spec v1

Provenance: black-box observation of the player's own installed files
code used. Status: FACT for fields marked verified; others UNKNOWN.

All integers little-endian u32.

| Offset | Size | Field | Status |
|---|---|---|---|
| 0 | 16 | Magic `CUBEMAS4.10` + 5 zero bytes | verified 685/685 |
| 16 | 4 | `entry_count` | verified |
| 20 | 4 | `data_size` — equals file_len - data_base in 574/685; differs in 111. Do NOT validate against it | unknown meaning |
| 24 | 256 × entry_count | Directory | verified |
| data_base = 24 + 256·entry_count | | Payload area | verified |

Directory entry (256 bytes):

| Offset | Size | Field | Status |
|---|---|---|---|
| 0 | 4 | `type` — tracks extension: 0x11 MTS, 0x12 BMP, 0x14 TGA, 0x16 JPG, 0x1F ANM/MAG | observed; treat as opaque |
| 4 | 4 | `offset` relative to data_base | verified |
| 8 | 4 | `uncompressed_size` | verified |
| 12 | 4 | `compressed_size` | verified |
| 16 | 4 | `unknown` — not Adler-32/CRC-32 of payload | unknown; preserve |
| 20 | 236 | Name, NUL-terminated, Latin-1, no path | verified |

Payload: if compressed_size == uncompressed_size, stored raw; else zlib stream
(RFC 1950). All 75,578 entries decompressed to exactly uncompressed_size.

Reader requirements: bounds-check every offset/size against file length; reject
bad magic; names compared case-insensitively; duplicate names possible across
archives (search-path precedence is a separate future spec).
