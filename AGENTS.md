# IFCX repository instructions

This repository owns the OpenBIM.rs implementation of IFC5 / IFCX. The
lossless file model, flattening by path, and attribute validation are
implemented. Do not describe the composed tree, imports, or geometry as
implemented without executable evidence here.

## Map

- `crates/openbim-ifcx/` — file model, JSON read/write, flattening (`src/compose/`), attribute validation (`src/validate.rs`); composed tree planned
- `crates/openbim-ifcx-geometry/` — planned geometry and viewer helpers; depends on the core crate, never on a renderer
- `docs/capabilities.md` — authoritative capability table
- `docs/adr/` — architecture decisions; 0002 fixes the crate split
- `scripts/gate.sh` — complete local/CI verification gate
- `CHANGELOG.md` — user-visible changes using Keep a Changelog

## Commands

```bash
./scripts/gate.sh
cargo test --workspace
# opt-in, needs a local buildingSMART/IFC5-development checkout
IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx --test upstream_round_trip
# also needs the imported schema files from ifcx.dev, named by URI's last segment
IFCX_UPSTREAM_DIR=../IFC5-development IFCX_IMPORTS_DIR=../ifcx-imports \
    cargo test --release -p openbim-ifcx --test upstream_validation
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
