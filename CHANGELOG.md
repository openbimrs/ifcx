# Changelog

All notable changes to this project are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Repository scaffold: `openbim-ifcx` crate with a status constant only,
  verification gate, pinned CI, and Dependabot for action pins.
- ADR 0001 recording that IFC5 / IFCX is its own family, not an IFC schema
  version.
- ADR 0002 fixing the crate split: one core crate for file model and
  composition, a separate crate for the IFC4 bridge.
- `openbim-ifcx-geometry` scaffold for renderer-neutral geometry, viewer
  helpers, and GLB export.
- Issue templates, labels, Discussions, and the IFCX project board.
- Capability table stating that no IFCX behavior is implemented yet.

### Changed

- Workspace crates live under `crates/`, as in `openbimrs/ifc`.

- License changed from AGPL-3.0 to MIT before any code or release.
- MSRV is 1.88, matching `openbim-ifc` and `openbim-step`.
