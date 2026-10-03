# 0002 — Split crates by dependency weight, not by concept

- **Status:** Accepted
- **Date:** 2026-10-03
- **Deciders:** openbimrs contributors
- **Supersedes:** —

## Context

The first target is the `ifcx_alpha` draft as defined by `schema/ifcx.tsp` in
`buildingSMART/IFC5-development` (last upstream commit 2026-09-08). An IFCX
file has four parts:

- `header`: dataset id, `ifcxVersion`, `dataVersion`, author, timestamp
- `imports`: other IFCX files by URI, with optional integrity
- `schemas`: attribute value descriptions keyed by namespaced name
- `data`: a flat list of nodes, each with a `path`, `children`, `inherits`,
  and namespaced `attributes`

Layers compose by path, and later opinions override earlier ones. The draft is
alpha and its own schema notes that modularity and extensibility are open.

Parsing, composition, and attribute checking change together whenever the
draft changes. An IFC4 bridge would pull in `openbim-ifc` and its
dependencies, which a plain IFCX reader should not pay for.

## Decision

- `openbim-ifcx` holds the file model, lossless JSON read/write, layer
  composition into a resolved node tree, and checking attributes against the
  file's own `schemas`, as modules of one crate. It depends on serde and
  serde_json only.
- The reader preserves unknown fields so files from newer drafts round-trip.
- Imports are resolved through a caller-supplied resolver trait. The core
  crate performs no network or filesystem access of its own.
- `openbim-ifcx-ifc4` holds the IFC4 to IFCX bridge and is the only crate
  that depends on `openbim-ifc`. It is created once the core model is stable.
- Interpretation of `usd::usdgeom::*` and other domain attributes stays out of
  the core crate until a consumer needs it.

Every capability names the draft revision it targets.

## Alternatives considered

| Option | Why not |
| --- | --- |
| Separate parse, compose, and schema crates now | Each draft bump would need coordinated releases of crates that always change together |
| Bridge inside the core crate behind a feature | Feature unification would let one consumer's bridge pull `openbim-ifc` into another's build |
| HTTP import fetching in the core crate | Forces a network stack on every consumer, including WASM and offline tools |

## Consequences

- A plain IFCX reader compiles serde plus one crate.
- Splitting composition out later stays possible if it gains a heavy
  dependency or a second consumer.
- The upstream repository publishes no license, so its examples are not
  vendored. Tests use hand-written samples. An optional check may run against
  a local upstream checkout without committing its files.
