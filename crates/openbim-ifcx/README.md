# openbim-ifcx

OpenBIM.rs crate for IFC5 / IFCX. It reads and writes `ifcx_alpha` files
losslessly, composes layered nodes into a resolved tree as upstream does
(`flatten`, then `compose`), and checks attribute values against the file's `schemas`, collecting every
failure with its node path, attribute id, and JSON pointer
(`IfcxFile::validate`, `validate_flat`, `validate_attributes`). It resolves
`imports` into an ordered layer stack through a caller-supplied resolver
(`LayerStackBuilder::build` for one main layer, `build_all` for several
layers stacked as upstream's `ifcx compose` does), validates the stack
against the schemas of all its layers (`LayerStack::validate`), and
federates schemas and data in upstream order; in that order an imported
layer overrides the layer that imports it. `flatten_owned` and
`LayerStack::into_federated` move nodes and attribute values instead of
copying them, which makes flattening large models several times faster
than `flatten` over borrowed nodes. The composed tree shares sub-trees between instances
instead of copying them, and reference cycles and unknown references are
typed errors.

Features:

- `integrity` (default): check SRI-style `sha256-<base64>` (also `sha384`,
  `sha512`) `integrity` values on imports, using `sha2` and `base64`. Without
  it, imports that carry `integrity` are rejected.
- `fs`: `FsResolver`, which loads imports from local files. The crate never
  uses the network.

See the
[repository capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md),
the [documentation](https://openbimrs.github.io/ifcx/) with a
[Rust guide](https://openbimrs.github.io/ifcx/guide/rust), and the
[viewer](https://openbimrs.github.io/ifcx/viewer/), which shows an IFCX file
in the browser.

Licensed under MIT.
