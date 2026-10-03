//! Checking attribute values against a file's `schemas`.
//!
//! The rules follow upstream's `Validate` in
//! `src/ifcx-core/schema/schema-validation.ts` of the `ifcx_alpha` draft, with
//! two differences: every failure is collected into a [`ValidationReport`]
//! instead of stopping at the first, and a few cases upstream leaves open are
//! decided here.
//!
//! # Rules
//!
//! Every attribute id on a node must name a schema; ids starting with
//! `__internal` are skipped, as upstream does. The value is then checked
//! against the schema's [`IfcxValueDescription`]:
//!
//! | `dataType` | Accepted value |
//! | --- | --- |
//! | `Boolean` | JSON `true` or `false` |
//! | `String`, `Reference` | any JSON string; a reference is not resolved |
//! | `DateTime` | any JSON string; the ISO 8601 format is not checked, as upstream |
//! | `Enum` | a string equal to one of `enumRestrictions.options` |
//! | `Real` | any JSON number |
//! | `Integer` | a JSON number without a fractional part (`3` and `3.0`, not `3.5`) |
//! | `Object` | a JSON object with every non-`optional` key of `objectRestrictions.values`, each checked recursively; other keys are allowed |
//! | `Array` | a JSON array whose length is within `arrayRestrictions.min` and `max`, each element checked against `arrayRestrictions.value` |
//! | `Blob` | any value, unchecked |
//! | anything else | nothing; reported as [`FailureKind::UnknownDataType`] |
//!
//! A value description with `inherits` is first checked against the value
//! description of every inherited schema, then against its own `dataType`.
//! `optional` only lets an object key be absent; a JSON `null` is never
//! accepted where a value is present. `quantityKind` is not checked.
//!
//! # Where this is stricter or more defined than upstream
//!
//! - `Integer` rejects numbers with a fractional part; upstream accepts any
//!   number for both `Integer` and `Real`.
//! - `Array` checks `min` and `max`; upstream ignores them.
//! - `Object` requires a JSON object; upstream's `typeof value === "object"`
//!   also lets arrays and `null` through.
//! - `Blob` accepts any value. The draft does not say how a blob is encoded,
//!   and upstream rejects every blob as an unexpected data type.
//! - An `Enum` without `enumRestrictions` and an inheritance cycle are
//!   reported; upstream crashes or recurses without end.
//! - `quantityKind` is an annotation for numbers. Neither upstream nor this
//!   module checks it against the value or against the list of known kinds.
//!
//! # Flattened and raw nodes
//!
//! Upstream validates flattened nodes, after all opinions on a path have been
//! merged. [`validate_attributes`] takes any `(path, attributes)` pairs, so it
//! works on flattened nodes as well as on raw ones. [`IfcxFile::validate`]
//! merges the attributes of nodes sharing a path, later opinions winning, as
//! upstream's flattening does, before validating them.

use std::error::Error;
use std::fmt;

use indexmap::IndexMap;
use serde_json::{Number, Value};

use crate::model::{DataType, IfcxFile, IfcxNode, IfcxSchema, IfcxValueDescription};

/// Attribute ids with this prefix are not validated, as upstream.
pub const INTERNAL_ATTRIBUTE_PREFIX: &str = "__internal";

/// The JSON type of a value, for failure messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonType {
    Null,
    Boolean,
    Number,
    String,
    Array,
    Object,
}

impl JsonType {
    pub fn of(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(_) => Self::Boolean,
            Value::Number(_) => Self::Number,
            Value::String(_) => Self::String,
            Value::Array(_) => Self::Array,
            Value::Object(_) => Self::Object,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Number => "number",
            Self::String => "string",
            Self::Array => "array",
            Self::Object => "object",
        }
    }
}

impl fmt::Display for JsonType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What is wrong with one attribute value.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FailureKind {
    /// The attribute id names no schema.
    MissingSchema,
    /// A value description inherits from a schema id that does not exist.
    UnknownInheritedSchema { id: String },
    /// Following `inherits` leads back to a schema already being checked.
    InheritanceCycle { id: String },
    /// The value has the wrong JSON type for the `dataType`.
    TypeMismatch { expected: DataType, found: JsonType },
    /// An `Integer` value has a fractional part.
    NotAnInteger { value: Number },
    /// An `Enum` value is not one of the options.
    NotAnOption { value: String, options: Vec<String> },
    /// An object lacks a key that is not `optional`.
    MissingKey { key: String },
    /// An array has fewer elements than `arrayRestrictions.min`.
    TooFewElements { len: usize, min: Number },
    /// An array has more elements than `arrayRestrictions.max`.
    TooManyElements { len: usize, max: Number },
    /// An `Enum` or `Array` description lacks the restrictions it needs, so
    /// the value cannot be checked.
    MissingRestrictions { data_type: DataType },
    /// The `dataType` is not one this crate knows, so the value cannot be
    /// checked.
    UnknownDataType { name: String },
}

impl fmt::Display for FailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSchema => f.write_str("no schema with this id"),
            Self::UnknownInheritedSchema { id } => {
                write!(f, "inherits unknown schema \"{id}\"")
            }
            Self::InheritanceCycle { id } => {
                write!(f, "schema \"{id}\" inherits from itself")
            }
            Self::TypeMismatch { expected, found } => {
                write!(f, "expected {}, found {found}", expected.as_str())
            }
            Self::NotAnInteger { value } => write!(f, "expected Integer, found {value}"),
            Self::NotAnOption { value, options } => {
                write!(f, "\"{value}\" is not one of [{}]", options.join(", "))
            }
            Self::MissingKey { key } => write!(f, "missing key \"{key}\""),
            Self::TooFewElements { len, min } => {
                write!(f, "array has {len} elements, fewer than min {min}")
            }
            Self::TooManyElements { len, max } => {
                write!(f, "array has {len} elements, more than max {max}")
            }
            Self::MissingRestrictions { data_type } => {
                let field = match data_type {
                    DataType::Enum => "enumRestrictions",
                    DataType::Array => "arrayRestrictions",
                    _ => "restrictions",
                };
                write!(f, "{} schema has no {field}", data_type.as_str())
            }
            Self::UnknownDataType { name } => write!(f, "unknown dataType \"{name}\""),
        }
    }
}

/// One attribute value that does not match its schema.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationFailure {
    /// Path of the node carrying the attribute.
    pub node: String,
    /// Attribute id, which is also the schema id.
    pub attribute: String,
    /// RFC 6901 JSON pointer into the attribute value; empty for the value
    /// itself, `/points/0` for the first element of its `points` key.
    pub pointer: String,
    pub kind: FailureKind,
}

impl fmt::Display for ValidationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[\"{}\"].attributes[\"{}\"]", self.node, self.attribute)?;
        if !self.pointer.is_empty() {
            write!(f, " at {}", self.pointer)?;
        }
        write!(f, ": {}", self.kind)
    }
}

impl Error for ValidationFailure {}

/// Every failure found in one validation run, in node and attribute order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValidationReport {
    pub failures: Vec<ValidationFailure>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.failures.is_empty()
    }

    /// `Ok` when nothing failed, otherwise the report as the error.
    pub fn into_result(self) -> Result<(), ValidationReport> {
        if self.is_valid() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.failures.len() {
            0 => return f.write_str("all attributes match their schemas"),
            1 => f.write_str("1 attribute value does not match its schema")?,
            n => write!(f, "{n} attribute values do not match their schemas")?,
        }
        for failure in &self.failures {
            write!(f, "\n  {failure}")?;
        }
        Ok(())
    }
}

impl Error for ValidationReport {}

/// Checks the attributes of `nodes`, given as `(path, attributes)` pairs,
/// against `schemas` and collects every failure.
///
/// The pairs may come from raw file nodes or from flattened ones; see the
/// [module documentation](self) for the rules.
///
/// ```
/// use indexmap::IndexMap;
/// use openbim_ifcx::{validate_attributes, FailureKind, IfcxSchema};
/// use serde_json::json;
///
/// let schemas: IndexMap<String, IfcxSchema> = serde_json::from_value(json!({
///     "example::height": {"value": {"dataType": "Real"}}
/// })).unwrap();
/// let attributes: IndexMap<String, serde_json::Value> = serde_json::from_value(json!({
///     "example::height": "tall",
///     "example::colour": "red"
/// })).unwrap();
///
/// let report = validate_attributes(&schemas, [("wall", &attributes)]);
/// assert_eq!(report.failures.len(), 2);
/// assert_eq!(report.failures[1].kind, FailureKind::MissingSchema);
/// ```
pub fn validate_attributes<'a, P, I>(
    schemas: &IndexMap<String, IfcxSchema>,
    nodes: I,
) -> ValidationReport
where
    P: AsRef<str> + 'a,
    I: IntoIterator<Item = (P, &'a IndexMap<String, Value>)>,
{
    let mut report = ValidationReport::default();
    for (path, attributes) in nodes {
        let path = path.as_ref();
        for (id, value) in attributes {
            if id.starts_with(INTERNAL_ATTRIBUTE_PREFIX) {
                continue;
            }
            let mut checker = Checker {
                schemas,
                node: path,
                attribute: id,
                pointer: String::new(),
                inheriting: Vec::new(),
                failures: &mut report.failures,
            };
            match schemas.get_key_value(id) {
                Some((key, schema)) => {
                    checker.inheriting.push(key);
                    checker.check(&schema.value, value);
                }
                None => checker.fail(FailureKind::MissingSchema),
            }
        }
    }
    report
}

/// Checks the attributes of each node as it is, without merging nodes that
/// share a path. Nodes without attributes are skipped.
pub fn validate_nodes<'a, I>(schemas: &IndexMap<String, IfcxSchema>, nodes: I) -> ValidationReport
where
    I: IntoIterator<Item = &'a IfcxNode>,
{
    validate_attributes(
        schemas,
        nodes
            .into_iter()
            .filter_map(|node| Some((node.path.as_str(), node.attributes.as_ref()?))),
    )
}

impl IfcxFile {
    /// Checks every attribute in `data` against this file's `schemas`.
    ///
    /// Nodes sharing a path are merged first, later attribute values
    /// replacing earlier ones, as upstream's flattening does; a failure is
    /// reported once per path, in order of each path's first node. Imports
    /// are not resolved, so attributes whose schemas live only in an imported
    /// file are reported as [`FailureKind::MissingSchema`].
    pub fn validate(&self) -> Result<(), ValidationReport> {
        let mut merged: IndexMap<&str, IndexMap<String, Value>> = IndexMap::new();
        for node in &self.data {
            let entry = merged.entry(node.path.as_str()).or_default();
            if let Some(attributes) = &node.attributes {
                for (id, value) in attributes {
                    entry.insert(id.clone(), value.clone());
                }
            }
        }
        validate_attributes(&self.schemas, merged.iter().map(|(p, a)| (*p, a))).into_result()
    }
}

struct Checker<'s, 'r> {
    schemas: &'s IndexMap<String, IfcxSchema>,
    node: &'r str,
    attribute: &'r str,
    pointer: String,
    /// Schema ids whose value description is being checked through
    /// `inherits`, to stop cycles.
    inheriting: Vec<&'s str>,
    failures: &'r mut Vec<ValidationFailure>,
}

impl<'s> Checker<'s, '_> {
    fn fail(&mut self, kind: FailureKind) {
        self.failures.push(ValidationFailure {
            node: self.node.to_owned(),
            attribute: self.attribute.to_owned(),
            pointer: self.pointer.clone(),
            kind,
        });
    }

    fn mismatch(&mut self, expected: &DataType, value: &Value) {
        self.fail(FailureKind::TypeMismatch {
            expected: expected.clone(),
            found: JsonType::of(value),
        });
    }

    /// Checks `value` at the current pointer, then restores the pointer.
    fn check_at(&mut self, token: &str, desc: &'s IfcxValueDescription, value: &Value) {
        let len = self.pointer.len();
        self.pointer.push('/');
        for c in token.chars() {
            match c {
                '~' => self.pointer.push_str("~0"),
                '/' => self.pointer.push_str("~1"),
                c => self.pointer.push(c),
            }
        }
        self.check(desc, value);
        self.pointer.truncate(len);
    }

    fn check(&mut self, desc: &'s IfcxValueDescription, value: &Value) {
        for id in desc.inherits.iter().flatten() {
            let Some((key, schema)) = self.schemas.get_key_value(id) else {
                self.fail(FailureKind::UnknownInheritedSchema { id: id.clone() });
                continue;
            };
            if self.inheriting.contains(&key.as_str()) {
                self.fail(FailureKind::InheritanceCycle { id: id.clone() });
                continue;
            }
            self.inheriting.push(key);
            self.check(&schema.value, value);
            self.inheriting.pop();
        }

        let data_type = &desc.data_type;
        match data_type {
            DataType::Boolean => {
                if !value.is_boolean() {
                    self.mismatch(data_type, value);
                }
            }
            DataType::String | DataType::DateTime | DataType::Reference => {
                if !value.is_string() {
                    self.mismatch(data_type, value);
                }
            }
            DataType::Real => {
                if !value.is_number() {
                    self.mismatch(data_type, value);
                }
            }
            DataType::Integer => match value {
                Value::Number(n) => {
                    let integral = n.is_i64()
                        || n.is_u64()
                        || n.as_f64()
                            .is_some_and(|f| f.is_finite() && f.fract() == 0.0);
                    if !integral {
                        self.fail(FailureKind::NotAnInteger { value: n.clone() });
                    }
                }
                _ => self.mismatch(data_type, value),
            },
            DataType::Enum => {
                let Value::String(s) = value else {
                    return self.mismatch(data_type, value);
                };
                let Some(restrictions) = &desc.enum_restrictions else {
                    return self.fail(FailureKind::MissingRestrictions {
                        data_type: data_type.clone(),
                    });
                };
                if !restrictions.options.contains(s) {
                    self.fail(FailureKind::NotAnOption {
                        value: s.clone(),
                        options: restrictions.options.clone(),
                    });
                }
            }
            DataType::Object => {
                let Value::Object(object) = value else {
                    return self.mismatch(data_type, value);
                };
                let Some(restrictions) = &desc.object_restrictions else {
                    return;
                };
                for (key, key_desc) in &restrictions.values {
                    match object.get(key) {
                        Some(v) => self.check_at(key, key_desc, v),
                        None if key_desc.optional == Some(true) => {}
                        None => self.fail(FailureKind::MissingKey { key: key.clone() }),
                    }
                }
            }
            DataType::Array => {
                let Value::Array(elements) = value else {
                    return self.mismatch(data_type, value);
                };
                let Some(restrictions) = &desc.array_restrictions else {
                    return self.fail(FailureKind::MissingRestrictions {
                        data_type: data_type.clone(),
                    });
                };
                let len = elements.len();
                if let Some(min) = &restrictions.min {
                    if (len as f64) < min.as_f64().unwrap_or(f64::NEG_INFINITY) {
                        self.fail(FailureKind::TooFewElements {
                            len,
                            min: min.clone(),
                        });
                    }
                }
                if let Some(max) = &restrictions.max {
                    if (len as f64) > max.as_f64().unwrap_or(f64::INFINITY) {
                        self.fail(FailureKind::TooManyElements {
                            len,
                            max: max.clone(),
                        });
                    }
                }
                for (i, element) in elements.iter().enumerate() {
                    self.check_at(&i.to_string(), &restrictions.value, element);
                }
            }
            DataType::Blob => {}
            DataType::Other(name) => self.fail(FailureKind::UnknownDataType { name: name.clone() }),
        }
    }
}
