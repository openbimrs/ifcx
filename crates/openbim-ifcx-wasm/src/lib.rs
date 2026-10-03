//! WebAssembly bindings for `openbim-ifcx`.
//!
//! Reads, writes, validates and composes IFC5 / IFCX files from JavaScript.
//! An `IfcxFile` holds one file and writes it back losslessly; `compose` and
//! `validate` take a list of layers, weakest first, and return plain
//! objects; `exportGlb` writes the composed model as binary glTF.
//!
//! Every operation lives in `openbim-ifcx-binding-core`, shared with the
//! Python binding. This crate converts JS arguments and results only, and is
//! empty outside `wasm32`.
//!
//! A test of anything that does not need a JS host belongs in
//! `openbim-ifcx-binding-core`, where `cargo test` runs it natively; the Node
//! suite here covers only the JS conversion itself.

#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod api;
#[cfg(target_arch = "wasm32")]
mod convert;
#[cfg(target_arch = "wasm32")]
mod error;

#[cfg(target_arch = "wasm32")]
pub use api::IfcxFile;
