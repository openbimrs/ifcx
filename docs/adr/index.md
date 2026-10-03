# Architecture decision records

An ADR records a decision that constrains future work: the context that
forced it, the choice made, and the consequences accepted. An accepted
record is not edited; a reversal is a new record that supersedes it.

Add `docs/adr/NNNN-slug.md` with a `# NNNN — Title` heading and a
`- **Status:**` line, then run `cargo run -p xtask -- docs`: the index
below and the sidebar follow from the file.

<!-- ADR:INDEX:BEGIN -->

| # | Title | Status |
| ---: | --- | --- |
| [0001](/adr/0001-ifcx-is-its-own-family) | IFC5 / IFCX is its own standard family | Accepted |
| [0002](/adr/0002-crate-split-by-dependency-weight) | Split crates by dependency weight, not by concept | Accepted |

<!-- ADR:INDEX:END -->
