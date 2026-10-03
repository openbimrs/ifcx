# openbim-ifcx

OpenBIM.rs crate for IFC5 / IFCX. It reads and writes `ifcx_alpha` files
losslessly, flattens layered nodes by path (the first half of composition),
and checks attribute values against the file's `schemas`, collecting every
failure with its node path, attribute id, and JSON pointer
(`IfcxFile::validate`, `validate_attributes`). It resolves `imports` into an
ordered layer stack through a caller-supplied resolver and federates schemas
and data in upstream order; in that order an imported layer overrides the
layer that imports it. Building the composed tree is not implemented yet.

Features:

- `integrity` (default): check SRI-style `sha256-<base64>` (also `sha384`,
  `sha512`) `integrity` values on imports, using `sha2` and `base64`. Without
  it, imports that carry `integrity` are rejected.
- `fs`: `FsResolver`, which loads imports from local files. The crate never
  uses the network.

See the
[repository capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md).

Licensed under MIT.
