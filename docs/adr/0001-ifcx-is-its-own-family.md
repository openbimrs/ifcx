# 0001 — IFC5 / IFCX is its own standard family

- **Status:** Accepted
- **Date:** 2026-10-03
- **Deciders:** openbimrs contributors
- **Supersedes:** —

## Context

IFC2X3, IFC4, and IFC4X3 are EXPRESS schemas serialized as STEP or XML.
`openbimrs/ifc` models them as an untyped entity graph keyed by a
`SchemaVersion`.

IFC5 / IFCX is not another EXPRESS release. It is a JSON model built from
layers of components that compose in a USD-like way. The unit of data, the
identity model, and the composition rules all differ from the IFC4 entity
graph. Forcing it into a `SchemaVersion` would make the ifc model carry two
unrelated data models behind one enum.

## Decision

IFCX lives in `openbimrs/ifcx` as a separate standard family.

- `openbimrs/ifc` keeps rejecting the `IFC5` schema token with a typed
  `UnsupportedSchema` error and points readers here.
- `ifcx` may depend on `step`, `core`, and `ifc`. Nothing in `ifc` may depend
  on `ifcx`.
- buildingSMART's draft ifcJSON (a JSON encoding of the IFC4 data model) is
  not pursued. openbimrs/ifc#122 was closed as not planned.

## Alternatives considered

| Option | Why not |
| --- | --- |
| Add `IFC5` to ifc's `SchemaVersion` | Different data model; would leak composition semantics into the EXPRESS entity graph |
| Sibling crate inside the ifc workspace | Ties IFCX releases and its draft churn to the ifc release cycle |
| Do nothing and leave IFC5 as an implicit gap | The gap was visible only as a negative test case |

## Consequences

- IFCX can follow its draft revisions without churning the ifc crates.
- An IFC4 to IFCX bridge, if built, lives here and depends on `openbim-ifc`.
- Tracking: openbimrs/ifcx#1.
