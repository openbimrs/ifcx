# openbim-ifcx

OpenBIM.rs crate for IFC5 / IFCX. It reads and writes single `ifcx_alpha`
files losslessly and checks attribute values against the file's `schemas`,
collecting every failure with its node path, attribute id, and JSON pointer
(`IfcxFile::validate`, `validate_attributes`). Layer composition and imports
are not implemented yet. See the
[repository capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md).

Licensed under MIT.
