# Getting started: Rust

Two crates: [`openbim-ifcx`](/reference/crates/openbim-ifcx) reads, writes,
composes and validates, and
[`openbim-ifcx-geometry`](/reference/crates/openbim-ifcx-geometry) turns a
composed tree into a render scene and GLB.

```sh
cargo add openbim-ifcx openbim-ifcx-geometry
```

Every snippet on this page is a region of
[`crates/openbim-ifcx-geometry/tests/guide.rs`](https://github.com/openbimrs/ifcx/blob/main/crates/openbim-ifcx-geometry/tests/guide.rs),
which the gate compiles and runs, so it works as shown. Errors are
propagated with `?` into a `Box<dyn Error>`.

## Read and write a file

<<< ../../crates/openbim-ifcx-geometry/tests/guide.rs#read{rust}

`IfcxFile` holds `header`, `imports`, `schemas` and `data`, the four parts
of an IFCX file, and keeps fields the draft does not define. A malformed
file is a `ReadError` with line and column.

## Validate attributes

<<< ../../crates/openbim-ifcx-geometry/tests/guide.rs#validate{rust}

Each failure names the node path, the attribute, an RFC 6901 pointer into
the value and a `FailureKind`. `validate_flat` and `validate_attributes`
check flattened nodes or single values.

## Compose layers

<<< ../../crates/openbim-ifcx-geometry/tests/guide.rs#compose{rust}

To compose several files, concatenate their `data` weakest first before
flattening: later opinions win. `flatten_owned` moves attribute values
instead of copying them, which is several times faster than `flatten` on
large models. A reference cycle or a reference to a node no layer defines
is a `ComposeError`.

## Resolve imports

<<< ../../crates/openbim-ifcx-geometry/tests/guide.rs#imports{rust}

This needs the `fs` feature (`cargo add openbim-ifcx --features fs`). The
stack loads in upstream's order, in which an import overrides the layer
that imports it; [Capabilities](/capabilities#layer-order-and-import-priority)
explains why. `build_all` stacks several files the way upstream's
`ifcx compose` does.

## Export GLB

<<< ../../crates/openbim-ifcx-geometry/tests/guide.rs#glb{rust}

Buffers are relative to their own centre and instance matrices to a render
origin, so georeferenced models keep sub-millimetre precision in `f32`.
From the command line, the `ifcx2glb` example does the same:

```sh
cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- model.ifcx model.glb
```

## Next

- The [`openbim-ifcx`](/reference/crates/openbim-ifcx) and
  [`openbim-ifcx-geometry`](/reference/crates/openbim-ifcx-geometry)
  reference pages list the public API, with links into the
  [rustdoc](/api/).
- [Capabilities](/capabilities) records what is implemented and the
  evidence for it.
