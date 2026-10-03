//! Serde types for one IFCX file, following `schema/ifcx.tsp` of the
//! `ifcx_alpha` draft.
//!
//! Every struct keeps fields it does not know in `extra`, and optional fields
//! stay `None` when absent, so a file read and written again keeps its
//! content. Maps keep their key order.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

/// Fields this crate does not model, kept in input order for round trip.
pub type Extra = Map<String, Value>;

/// One IFCX file: a single layer of a dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfcxFile {
    pub header: IfcxHeader,
    pub imports: Vec<ImportNode>,
    /// Attribute value descriptions, keyed by namespaced attribute id.
    pub schemas: IndexMap<String, IfcxSchema>,
    /// Nodes in file order. Several nodes may share a path; layering merges
    /// them later.
    pub data: Vec<IfcxNode>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfcxHeader {
    /// Dataset identifier, a name or a path.
    pub id: String,
    /// Draft revision of the format, `ifcx_alpha` for every known file.
    #[serde(rename = "ifcxVersion")]
    pub ifcx_version: String,
    #[serde(rename = "dataVersion")]
    pub data_version: String,
    pub author: String,
    /// ISO 8601 date or date-time, kept as written.
    pub timestamp: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A reference to another IFCX file that this layer builds on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportNode {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// One node opinion. `children` and `inherits` map a local name to a
/// referenced path; a `None` value deletes that entry when layers merge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfcxNode {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<IndexMap<String, Option<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits: Option<IndexMap<String, Option<String>>>,
    /// Attribute values keyed by namespaced attribute id, as raw JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<IndexMap<String, Value>>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Describes the values allowed for one attribute id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfcxSchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    pub value: IfcxValueDescription,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfcxValueDescription {
    #[serde(rename = "dataType")]
    pub data_type: DataType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits: Option<Vec<String>>,
    /// Quantity kind name, kept as a string so new kinds still read.
    #[serde(
        rename = "quantityKind",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub quantity_kind: Option<String>,
    #[serde(
        rename = "enumRestrictions",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub enum_restrictions: Option<EnumRestrictions>,
    #[serde(
        rename = "arrayRestrictions",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub array_restrictions: Option<Box<ArrayRestrictions>>,
    #[serde(
        rename = "objectRestrictions",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub object_restrictions: Option<ObjectRestrictions>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumRestrictions {
    pub options: Vec<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrayRestrictions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<Number>,
    pub value: IfcxValueDescription,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectRestrictions {
    pub values: IndexMap<String, IfcxValueDescription>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// The `dataType` of a value description. Names this crate does not know
/// read as [`DataType::Other`] and are written back unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum DataType {
    Real,
    Boolean,
    Integer,
    String,
    DateTime,
    Enum,
    Array,
    Object,
    Reference,
    Blob,
    Other(String),
}

impl DataType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Real => "Real",
            Self::Boolean => "Boolean",
            Self::Integer => "Integer",
            Self::String => "String",
            Self::DateTime => "DateTime",
            Self::Enum => "Enum",
            Self::Array => "Array",
            Self::Object => "Object",
            Self::Reference => "Reference",
            Self::Blob => "Blob",
            Self::Other(name) => name,
        }
    }
}

impl From<String> for DataType {
    fn from(name: String) -> Self {
        match name.as_str() {
            "Real" => Self::Real,
            "Boolean" => Self::Boolean,
            "Integer" => Self::Integer,
            "String" => Self::String,
            "DateTime" => Self::DateTime,
            "Enum" => Self::Enum,
            "Array" => Self::Array,
            "Object" => Self::Object,
            "Reference" => Self::Reference,
            "Blob" => Self::Blob,
            _ => Self::Other(name),
        }
    }
}

impl From<DataType> for String {
    fn from(data_type: DataType) -> Self {
        match data_type {
            DataType::Other(name) => name,
            known => known.as_str().to_owned(),
        }
    }
}
