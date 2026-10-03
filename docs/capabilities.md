# Capabilities

Status vocabulary: **reserved** (named, no behavior), **implemented** (code
exists with tests), **conformance-tested** (checked against buildingSMART
samples for a named draft revision).

| Capability | Status | IFCX draft | Notes |
| --- | --- | --- | --- |
| Parse an IFCX JSON file | reserved | — | |
| Compose layers into a resolved model | reserved | — | |
| Write IFCX | reserved | — | |
| IFC4 to IFCX bridge | reserved | — | Would depend on `openbim-ifc`; never the reverse |

## Not here

- IFC2X3, IFC4, and IFC4X3 in STEP or XML: [`openbimrs/ifc`](https://github.com/openbimrs/ifc).
- buildingSMART's draft ifcJSON encoding of the IFC4 data model: not planned
  anywhere in OpenBIM.rs (openbimrs/ifc#122 was closed).
