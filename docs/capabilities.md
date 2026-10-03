# Capabilities

Status vocabulary: **reserved** (named, no behavior), **implemented** (code
exists with tests), **conformance-tested** (checked against buildingSMART
samples for a named draft revision).

Target draft: `ifcx_alpha` (`schema/ifcx.tsp` in buildingSMART/IFC5-development).

| Capability | Status | IFCX draft | Crate | Notes |
| --- | --- | --- | --- | --- |
| Read and write an IFCX file losslessly | reserved | — | `openbim-ifcx` | |
| Compose layers into a resolved node tree | reserved | — | `openbim-ifcx` | |
| Check attributes against the file's `schemas` | reserved | — | `openbim-ifcx` | |
| Resolve imports | reserved | — | `openbim-ifcx` | Through a caller-supplied resolver; no built-in network access |
| World transforms through the node hierarchy | reserved | — | `openbim-ifcx-geometry` | `usd::xformop::transform` |
| Triangle meshes | reserved | — | `openbim-ifcx-geometry` | `usd::usdgeom::mesh` |
| Polylines | reserved | — | `openbim-ifcx-geometry` | `usd::usdgeom::basiscurves` |
| Point clouds | reserved | — | `openbim-ifcx-geometry` | `points::array::*`, `points::base64::*`, `pcd::base64` |
| Visibility, colour, opacity, materials | reserved | — | `openbim-ifcx-geometry` | `usd::usdgeom::visibility`, `bsi::ifc::presentation::*`, `usd::usdshade::*`, `gltf::material::*` |
| Flat render scene for viewers | reserved | — | `openbim-ifcx-geometry` | |
| GLB export | reserved | — | `openbim-ifcx-geometry` | |
| IFC4 to IFCX bridge | reserved | — | `openbim-ifcx-ifc4` (not created) | Would depend on `openbim-ifc`; never the reverse |

## Not here

- IFC2X3, IFC4, and IFC4X3 in STEP or XML: [`openbimrs/ifc`](https://github.com/openbimrs/ifc).
- buildingSMART's draft ifcJSON encoding of the IFC4 data model: not planned
  anywhere in OpenBIM.rs (openbimrs/ifc#122 was closed).
