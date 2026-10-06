# DS02 stop 1 review

**Verdict: DeepSeek correct. Spec error was Claude's.** Stop rule worked.

- **Cause:** spec said geometry magic = 16 bytes; Claude's own probe compared 13.
- **Claude re-check:** byte 13 is NUL in 46,236/46,236 geometry blocks; bytes 14-15 vary.
- **Spec v1.1:** geometry magic = 14 bytes (13 chars + NUL). Z3DM entry -> `Unsupported { version: "Z3DM" }`.
- **DS instruction:** use 14-byte check (not 13). Re-run corpus. Expected
  `mts_ok=46236`, unsupported 3,047 (3,046 4.01 + 1 Z3DM), errors 0. Then report.

Lesson: spec numbers must be copied from the probe code, not retyped.
