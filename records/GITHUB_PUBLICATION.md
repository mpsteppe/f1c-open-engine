# GitHub publication plan

**Next action:** finish/review DS14 and owner barrier check, then prepare an audited contributor-ready snapshot for the requested GitHub destination.

- **Owner request:** after DS14, wrap up this prototype phase and upload to GitHub so others can contribute. This authorizes publication preparation and the eventual upload within the agreed scope. Earlier no-commit instruction remains in force while DS14 is active; wrap-up commits become authorized after it finishes.
- **Waiting on:** DS14 report/review/owner check; GitHub account or repository destination (asked); code license choice and existing README legal/provenance gate.
- **Engine package:** reviewed Rust crates, clean specs/research, accurate roadmap, portable configuration/quick start, CONTRIBUTING, issue/PR templates and CI. Clarify implemented prototypes versus original-game parity.
- **Audit:** current tree plus any history being uploaded, copied game fixtures/assets, secrets, restricted evidence and personal paths. Preserve local history. If old history is unsuitable, create a separate audited export snapshot rather than deleting local history or force-pushing it.
- **Decompiler request:** owner additionally requested decompiled C. Recommend a separate research repository/access boundary; do not silently include restricted source in the engine package. Rights/permission to publicly distribute that material are not established. Private hosting does not establish distribution permission. Do not copy/read external decompile files for publication before resolving this scope.
- **Clean-room contributors:** engine implementers work from clean specs; restricted-source researchers use the separate evidence boundary. Uploading restricted evidence beside source changes the current separation model and requires an explicit revised policy.
- **Reviewable delivery:** prepare audit findings, exact proposed file set, license/contribution rules and checks before resolving any remaining required publication gate. Do not treat a public repository or license as proof that original game material is licensed for distribution.
- **Current limits:** no remote configured, gh not found on PATH in this check; DS14 report present and owner stop/reset passed; final rebuilt release pending executable lock. No GitHub upload or commit performed.

## Prepared locally

- CONTRIBUTING.md, .github issue/PR templates and Windows CI draft added; GitHub workflow not executed.
- DS14 uncertain-candidate sweep correction/regression passes; 253/0 and configured barrier sample pass. Release rebuild waits for open viewer to close.
- AIW game-derived fixture numbers replaced; history retains earlier copied fixtures. Fresh audited export recommended; current-tree/history audit not complete.
- Pending: GitHub destination/authentication, root license files/owner confirmation of existing MIT OR Apache-2.0 manifests, public-source legal/provenance gate, separate decompile publication rights/scope. No decompile copied; no upload/commit yet.

## Connection check — 2026-10-06

- GitHub plugin discovery confirmed installed=false in this Codex chat; suggested GitHub installation/connection. ChatGPT connection has not provided callable GitHub access here. No upload performed.
- Root LICENSE-MIT/LICENSE-APACHE and LICENSING.md now mirror existing MIT OR Apache-2.0 crate declarations, limited to original engine material. No license applied to decompiled game content.
- Viewer pid28128 (Adelaide window) accepted a normal close request but remained running; targeted Stop-Process failed with access denied. Release build still locked; user must close this viewer.
- Owner reiterated upload including decompiled C. Request recorded; exact corpus/destination and its redistribution rights/revised clean-room scope are unresolved. No external C corpus read/copied yet.

## Installed connection verified — 2026-10-06

- **Result:** GitHub connector authenticated as mpsteppe; repository listing and metadata reads pass.
- **Corpus:** public mpsteppe/f1-challenge-decompiled already exists. Recursive main tree 985572cc1d341f64358a5ef2f5a394d262d755b3 contains 14,273 entries and 14,264 .c files; truncated=false. Verified filenames/metadata only, not C contents. This verifies presence, not completeness against the local corpus or redistribution rights.
- **Engine destination:** proposed separate mpsteppe/f1c-open-engine; not created or uploaded. Connector exposes Git object writes but no repository-creation tool. Opened https://github.com/new in Codex browser; it redirects to sign-in. User asked to sign in there, or create the public repository with an initial README independently.
- **Checks:** tested viewer pid28128 still running; final release rebuild remains locked. No new commit/upload during this connection check.
- **Resume:** create/verify engine repository, finish snapshot audit and final rebuild, then upload the audited tree using the connected GitHub tools. Preserve local history and keep C outside the engine tree.

## Engine repository created — 2026-10-06

- Public destination: https://github.com/mpsteppe/f1c-open-engine; initialized main with README only, commit a51d0a02057c0e01ee36a4d6a9291ebd50c9e843.
- Creation verified in browser and authenticated repository metadata. Engine source, licenses, specs and CI not uploaded yet.
- Next: finish snapshot audit/final release rebuild and upload original engine tree separately from existing decompiled corpus.
