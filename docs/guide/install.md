# Install

Every package is versioned and released on its own. The table is
generated from the crate manifests and changelogs, so it always shows the
latest release recorded on `main`.

<!-- INSTALL:TABLE:BEGIN -->

| Language | Package | Latest release | Install | Requires | Reference |
| --- | --- | --- | --- | --- | --- |
| Rust | [`openbim-ifcx`](https://crates.io/crates/openbim-ifcx) | 0.1.0 (2026-10-03) | `cargo add openbim-ifcx` | Rust `1.88.0` | [`openbim-ifcx`](/reference/crates/openbim-ifcx) |
| Rust | [`openbim-ifcx-geometry`](https://crates.io/crates/openbim-ifcx-geometry) | 0.1.0 (2026-10-03) | `cargo add openbim-ifcx-geometry` | Rust `1.88.0` | [`openbim-ifcx-geometry`](/reference/crates/openbim-ifcx-geometry) |
| Python | [`openbim-ifcx`](https://pypi.org/project/openbim-ifcx/) | 0.1.1 (2026-10-03) | `pip install openbim-ifcx` | Python `>=3.9` | [`openbim-ifcx-py`](/reference/crates/openbim-ifcx-py) |
| JavaScript / TypeScript | [`@openbim/ifcx`](https://www.npmjs.com/package/@openbim/ifcx) | 0.2.0 (2026-10-03) | `npm install @openbim/ifcx` | Node `>=18`, or a current browser | [`openbim-ifcx-wasm`](/reference/crates/openbim-ifcx-wasm) |

<!-- INSTALL:TABLE:END -->

## Rust

```sh
cargo add openbim-ifcx                  # read, write, compose, validate
cargo add openbim-ifcx-geometry         # render scene and GLB export
cargo add openbim-ifcx --features fs    # also: load imports from local files
```

`openbim-ifcx` has two features: `integrity` (default) checks the SHA-2
`integrity` value of an import, and `fs` adds `FsResolver`, which loads
imports from disk. The crates never use the network. See the
[`openbim-ifcx` reference](/reference/crates/openbim-ifcx#cargo-features).

## JavaScript and TypeScript

```sh
npm install @openbim/ifcx
```

One package for Node 18 and later, bundlers and plain browser pages, with
TypeScript declarations. In Node, `require` or `import` it; a bundler gets
the bundler build; a page without a bundler (or Vite) imports
`@openbim/ifcx/web` and awaits `init()` once. The
[JavaScript guide](/guide/javascript) shows each.

## Python

```sh
pip install openbim-ifcx
```

One abi3 wheel for CPython 3.9 and later on Linux (x86_64, aarch64), macOS
(universal2) and Windows (x64), and an sdist that builds with Rust and
`maturin`. The package ships type hints.

## From source

```sh
git clone https://github.com/openbimrs/ifcx
cd ifcx
cargo build --release
crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh   # npm package in crates/openbim-ifcx-wasm/pkg
crates/openbim-ifcx-py/scripts/check-python.sh      # wheel, installed into a throwaway venv and tested
```

The binding scripts need the tools listed in
[Contributing](/project/contributing#verification).
