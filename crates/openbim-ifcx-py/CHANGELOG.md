# Changelog -- openbim-ifcx-py

All notable changes to the `openbim-ifcx-py` crate are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows Semantic Versioning independently of its siblings:
a release here does not imply a release of any other crate in this
repository.

## [Unreleased]

### Changed

- **Breaking:** with imports resolved, a layer now overrides the layers it
  imports, as agreed upstream in buildingSMART/IFC5-development#144 (was:
  an import overrode the layer importing it). `compose`, `validate` and
  GLB export give different results when an import carries `data` or
  redefines a schema that a layer also sets. Layers given without imports,
  and imports that carry only schemas, compose as before; a later layer
  still overrides an earlier one and everything it imports (#36).

## [0.1.2] - 2026-10-03

### Changed

- Built on `openbim-ifcx` 0.1.1 and `openbim-ifcx-geometry` 0.1.1:
  validation with imports checks every layer against the schemas of all
  layers, flattening and composition no longer copy attribute values, and
  the fuzzing fixes apply (deep schema inheritance, oversized coordinates,
  exponential scene walks, malformed PCD headers).

- The README, which is the PyPI page, links the documentation site
  (<https://openbimrs.github.io/ifcx/>) and the viewer at its new address,
  <https://openbimrs.github.io/ifcx/viewer/> (#64).

## [0.1.1] - 2026-10-03

### Fixed

- The source distribution carries `LICENSE` at its root, where its metadata
  names it. PyPI rejected the 0.1.0 sdist for the missing file, so 0.1.0 is
  available as wheels only.

## [0.1.0] - 2026-10-03

First version, for PyPI as `openbim-ifcx` (`publish = false` on crates.io).

### Added

- Python bindings for `openbim-ifcx`, built with pyo3 and maturin as one
  abi3 wheel for CPython 3.9+. `IfcxFile.parse` reads a file from `str` or
  `bytes`; `write` writes it back losslessly; `to_dict`, `header`,
  `node_count` and `validate` expose it as plain dicts. `compose(layers,
  imports=None)` returns the composed tree and `validate(layers,
  imports=None)` a structured report over all layers, with imports resolved
  from memory when given. `export_glb(layers, imports=None, *, y_up=True,
  origin_on_root=True)` writes the composed model as binary glTF 2.0.
  Parsing, composing, validating and GLB export release the GIL.
- Every failure raises `IfcxError` with a stable `code`.
- `scripts/check-python.sh` builds the wheel, installs it into a throwaway
  uv venv and runs the Python suite against it.

[Unreleased]: https://github.com/openbimrs/ifcx/compare/openbim-ifcx-py-v0.1.2...HEAD
[0.1.2]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-py-v0.1.2
[0.1.1]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-py-v0.1.1
[0.1.0]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-py-v0.1.0
