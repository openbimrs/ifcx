# openbim-ifcx

OpenBIM.rs crate for IFC5 / IFCX. It reads and writes single `ifcx_alpha`
files losslessly, flattens layered nodes by path (the first half of
composition), and checks attribute values against the file's `schemas`,
collecting every failure with its node path, attribute id, and JSON pointer
(`IfcxFile::validate`, `validate_attributes`). Building the composed tree and
imports are not implemented yet. See the
[repository capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md).

Licensed under MIT.
