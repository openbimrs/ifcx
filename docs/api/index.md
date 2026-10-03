# Rust API

The rustdoc of every crate in the workspace, built from `main` with the
same doc comments the gate checks with `-D warnings`. The released crates
are also on [docs.rs](https://docs.rs/openbim-ifcx).

<!-- RUSTDOC:LIST:BEGIN -->

- [`openbim_ifcx`](/api/rustdoc/openbim_ifcx/index.html){target="_self"} ([`openbim-ifcx`](/reference/crates/openbim-ifcx)): Lossless reader and writer for IFC5 / IFCX files, the layered JSON model.
- [`openbim_ifcx_geometry`](/api/rustdoc/openbim_ifcx_geometry/index.html){target="_self"} ([`openbim-ifcx-geometry`](/reference/crates/openbim-ifcx-geometry)): Renderer-neutral geometry and viewer helpers for IFC5 / IFCX: transforms, meshes, curves, point clouds, presentation, a flat render scene, and GLB export.
- [`openbim_ifcx_binding_core`](/api/rustdoc/openbim_ifcx_binding_core/index.html){target="_self"} ([`openbim-ifcx-binding-core`](/reference/crates/openbim-ifcx-binding-core)): Host-independent core shared by the openbim-ifcx language bindings (WebAssembly, Python).

<!-- RUSTDOC:LIST:END -->

The bindings document their own surface: the
[JavaScript API](/reference/crates/openbim-ifcx-wasm#javascript-api) and the
[Python API](/reference/crates/openbim-ifcx-py#python-api), each generated
from the binding's declarations.
