# IFCX repository instructions

This repository owns the OpenBIM.rs implementation of IFC5 / IFCX. It is a
scaffold. Do not describe any IFCX reading, writing, or composition as
implemented without executable evidence in this repository.

## Map

- `openbim-ifcx/` — the planned model and composition crate (status only today)
- `openbim-ifcx-geometry/` — planned geometry and viewer helpers; depends on the core crate, never on a renderer
- `docs/capabilities.md` — authoritative capability table
- `docs/adr/` — architecture decisions; 0002 fixes the crate split
- `scripts/gate.sh` — complete local/CI verification gate
- `CHANGELOG.md` — user-visible changes using Keep a Changelog

## Commands

```bash
./scripts/gate.sh
cargo test --workspace
```

Trust command exit codes. Never summarize a Cargo pipeline in a way that hides
the Cargo process status.

## Boundaries

- IFCX may depend on released `openbim-step`, `openbim-core`, and `openbim-ifc`
  crates, for example for an IFC4 to IFCX bridge.
- `openbimrs/ifc` must never depend on IFCX. If both need something, move it
  down into `core` or `step`.
- IFCX is not a `SchemaVersion` of `openbim-ifc`. Do not add one there.
- Do not vendor buildingSMART schemas or sample files without verified
  redistribution rights. Record the IFCX draft revision a capability targets.
- Release-critical package metadata and cross-repository dependency versions
  are explicit in crate manifests.

## Documentation discipline

IFCX is still a moving draft. Keep the capability table honest and name the
draft revision for each claim. Update README, rustdoc, capabilities, and
CHANGELOG together for user-visible changes.
