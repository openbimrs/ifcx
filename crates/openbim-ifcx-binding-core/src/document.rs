//! One IFCX file, read and written losslessly.

use openbim_ifcx::IfcxFile;

use crate::error::BindingError;
use crate::report;

/// One IFCX file held by a host.
///
/// The host keeps this handle rather than a plain object, so writing it back
/// is lossless: key order, unknown fields, `null` deletions and every number
/// stay exactly as read, which a JavaScript object would not guarantee.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    file: IfcxFile,
}

impl Document {
    /// Reads an IFCX file from its UTF-8 JSON bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, BindingError> {
        IfcxFile::from_json_slice(bytes)
            .map(|file| Self { file })
            .map_err(|e| BindingError::Read(e.to_string()))
    }

    /// The file as JSON text: compact, or indented by two spaces.
    pub fn write(&self, pretty: bool) -> Result<String, BindingError> {
        let written = if pretty {
            self.file.to_json_string_pretty()
        } else {
            self.file.to_json_string()
        };
        written.map_err(|e| BindingError::Write(e.to_string()))
    }

    /// The `header` object as JSON text.
    pub fn header_json(&self) -> Result<String, BindingError> {
        serde_json::to_string(&self.file.header).map_err(|e| BindingError::Write(e.to_string()))
    }

    /// Number of entries in `data`. Several may share a path.
    pub fn node_count(&self) -> usize {
        self.file.data.len()
    }

    /// Checks every attribute against this file's own `schemas` and returns
    /// the report as JSON text (see [`LayerSet::validate_json`] for the
    /// shape). Imports are not resolved, so an attribute whose schema lives
    /// only in an imported file is reported as `missing-schema`.
    ///
    /// [`LayerSet::validate_json`]: crate::LayerSet::validate_json
    pub fn validate_json(&self) -> String {
        report::to_json(&self.file.validate().err().unwrap_or_default())
    }

    /// The file this document holds.
    pub fn file(&self) -> &IfcxFile {
        &self.file
    }

    /// Takes the file out of the document.
    pub fn into_file(self) -> IfcxFile {
        self.file
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::Document;
    use crate::fixtures::fixture;

    #[test]
    fn every_fixture_round_trips_through_write() {
        for bytes in [
            fixture!("minimal.ifcx"),
            fixture!("wall-with-type.ifcx"),
            fixture!("unknown-fields.ifcx"),
            fixture!("geometry-model.ifcx"),
        ] {
            let document = Document::parse(bytes).unwrap();
            let again = Document::parse(document.write(false).unwrap().as_bytes()).unwrap();
            assert_eq!(again, document);
            let pretty = Document::parse(document.write(true).unwrap().as_bytes()).unwrap();
            assert_eq!(pretty, document);
        }
    }

    #[test]
    fn unknown_fields_and_exact_numbers_survive() {
        let bytes = fixture!("unknown-fields.ifcx");
        let written = Document::parse(bytes).unwrap().write(false).unwrap();
        let original: Value = serde_json::from_slice(bytes).unwrap();
        let written: Value = serde_json::from_str(&written).unwrap();
        assert_eq!(written, original);
    }

    #[test]
    fn header_is_plain_json() {
        let document = Document::parse(fixture!("minimal.ifcx")).unwrap();
        let header: Value = serde_json::from_str(&document.header_json().unwrap()).unwrap();
        assert_eq!(header["ifcxVersion"], "ifcx_alpha");
        assert_eq!(document.node_count(), 0);
    }

    #[test]
    fn invalid_input_is_a_read_error_with_a_location() {
        let error = Document::parse(b"{\n  \"header\": 1\n}").unwrap_err();
        assert_eq!(error.code(), "read");
        assert!(error.to_string().contains("line 2"), "{error}");
        assert_eq!(Document::parse(b"not json").unwrap_err().code(), "read");
        assert_eq!(Document::parse(&[0xff, 0xfe]).unwrap_err().code(), "read");
    }

    #[test]
    fn validation_reports_every_failure_as_data() {
        let valid = Document::parse(fixture!("valid-attributes.ifcx")).unwrap();
        let report: Value = serde_json::from_str(&valid.validate_json()).unwrap();
        assert_eq!(report["valid"], true);
        assert_eq!(report["failures"].as_array().unwrap().len(), 0);

        let invalid = Document::parse(fixture!("invalid-attributes.ifcx")).unwrap();
        let report: Value = serde_json::from_str(&invalid.validate_json()).unwrap();
        assert_eq!(report["valid"], false);
        let failures = report["failures"].as_array().unwrap();
        let expected = invalid.file().validate().unwrap_err().failures;
        assert_eq!(failures.len(), expected.len());
        for (json, failure) in failures.iter().zip(&expected) {
            assert_eq!(json["node"], failure.node.as_str());
            assert_eq!(json["attribute"], failure.attribute.as_str());
            assert_eq!(json["pointer"], failure.pointer.as_str());
            assert_eq!(json["message"], failure.kind.to_string());
            assert_ne!(json["kind"], "other", "{failure}");
        }
    }
}
