# Changelog -- openbim-ifcx-binding-core

All notable changes to the `openbim-ifcx-binding-core` crate are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows Semantic Versioning independently of its siblings:
a release here does not imply a release of any other crate in this
repository.

## [Unreleased]

Not released on its own (`publish = false`): it ships inside the npm and
PyPI packages, whose changelogs name the releases.

### Changed

- `LayerSet` with imports stacks its layers with
  `LayerStackBuilder::build_all` instead of building its own synthetic main
  layer; the order and the reports are unchanged. Federation and
  composition move the parsed layers instead of copying them
  (`federate_owned`, `LayerStack::into_federated`, `flatten_owned`) (#42,
  #43).

### Added

- The host-independent half of the language bindings: `Document` (read,
  lossless write, header, validation against the file's own schemas),
  `LayerSet` (layers weakest first, optional in-memory imports under a
  synthetic main layer, composition into a `{path, attributes, children}`
  tree, federated validation, GLB export through `openbim-ifcx-geometry`), validation reports with stable failure
  `kind` codes, and `BindingError` with the stable codes `read`, `write`,
  `layer`, `compose`, `invalid-argument` and `glb`.
