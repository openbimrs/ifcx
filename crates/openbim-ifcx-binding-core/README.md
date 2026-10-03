# openbim-ifcx-binding-core

The host-independent core shared by the `openbim-ifcx` language bindings:
the operations, the JSON shapes handed to the hosts, and the stable error
codes, written once and wrapped by the WebAssembly (`@openbim/ifcx` on npm)
and Python (`openbim-ifcx` on PyPI) bindings. It adds no IFCX behaviour of
its own.

This crate is internal to the bindings and ships only inside them
(`publish = false`); Rust applications use
[`openbim-ifcx`](https://crates.io/crates/openbim-ifcx) directly.

- `Document`: one file, read and written back losslessly, validated against
  its own schemas.
- `LayerSet`: layers weakest first and, optionally, the files their imports
  name (resolved through `MemoryResolver` under a synthetic main layer, as
  upstream's `ifcx compose` does); composed into a `{path, attributes,
  children}` tree, validated as one federated file, or exported as GLB
  through `openbim-ifcx-geometry`.
- `BindingError`: `read`, `write`, `layer`, `compose`, `invalid-argument`, `glb`.
  A code may be added, never renamed or reused; a unit test pins the set.

Results cross to the hosts as JSON text, which they parse into plain
objects (`JSON.parse`, `json.loads`). Tests of the behaviour live here and
run with `cargo test`; the hosts' suites cover only their conversions.

Licensed under MIT.
