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
//! merged. [`validate_flat`] checks the output of [`flatten`](crate::flatten)
//! or [`flatten_owned`](crate::flatten_owned). [`IfcxFile::validate`] merges
//! the attributes of nodes sharing a path, later opinions winning, as
//! flattening does, before validating them; it does not copy any value.
//! [`validate_attributes`] takes any `(path, attributes)` pairs, and
//! [`validate_nodes`] raw nodes as they are.
//!
//! # Imports
//!
//! A file's own `schemas` often do not describe its attributes: most
//! upstream examples take them from imported files.
//! [`LayerStack::validate`](crate::layers::LayerStack::validate) checks a
//! layer stack built with its imports: the schemas of every layer, merged as
//! `federate` does, against the attributes of every layer, merged per path as
//! flattening does.

use std::error::Error;
use std::fmt;

use indexmap::IndexMap;
use serde_json::{Number, Value};

use crate::compose::FlatNode;
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
    check_nodes(
        schemas,
        nodes.into_iter().map(|(path, attributes)| {
            (
                path,
                attributes.iter().map(|(id, value)| (id.as_str(), value)),
            )
        }),
    )
}

/// Checks flattened nodes, as [`flatten`](crate::flatten) or
/// [`flatten_owned`](crate::flatten_owned) return them, against `schemas`.
/// This is what upstream validates: one merged node per path, in the
/// flattened order.
///
/// ```
/// use openbim_ifcx::{flatten_owned, validate_flat, IfcxFile};
///
/// let file = IfcxFile::from_json_str(r#"{
///     "header": {"id": "demo", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
///                "author": "someone", "timestamp": "2026-10-03"},
///     "imports": [],
///     "schemas": {"x::height": {"value": {"dataType": "Real"}}},
///     "data": [{"path": "w", "attributes": {"x::height": "tall"}},
///              {"path": "w", "attributes": {"x::height": 3.0}}]
/// }"#)?;
/// // Only the value that wins is checked.
/// assert!(validate_flat(&file.schemas, &flatten_owned(file.data.clone())).is_valid());
/// # Ok::<(), openbim_ifcx::ReadError>(())
/// ```
pub fn validate_flat(
    schemas: &IndexMap<String, IfcxSchema>,
    nodes: &IndexMap<String, FlatNode>,
) -> ValidationReport {
    check_nodes(
        schemas,
        nodes.iter().map(|(path, node)| {
            (
                path,
                node.attributes
                    .iter()
                    .map(|(id, value)| (id.as_str(), &**value)),
            )
        }),
    )
}

/// The shared loop: `(path, attributes)` pairs whose attributes are
/// `(id, value)` pairs.
fn check_nodes<'v, P, A, I>(schemas: &IndexMap<String, IfcxSchema>, nodes: I) -> ValidationReport
where
    P: AsRef<str>,
    A: IntoIterator<Item = (&'v str, &'v Value)>,
    I: IntoIterator<Item = (P, A)>,
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
    /// reported once per path, in order of each path's first node. The
    /// result equals [`validate_flat`] over [`flatten`](crate::flatten) of
    /// `data`, but no value is copied.
    ///
    /// Imports are not resolved, so attributes whose schemas live only in an
    /// imported file are reported as [`FailureKind::MissingSchema`]. To
    /// validate a file with its imports, build its
    /// [`LayerStack`](crate::layers::LayerStack) and call
    /// [`LayerStack::validate`](crate::layers::LayerStack::validate).
    pub fn validate(&self) -> Result<(), ValidationReport> {
        check_merged(&self.schemas, &self.data).into_result()
    }
}

/// Merges the attributes of `nodes` per path, later values replacing earlier
/// ones in place, as flattening does, without copying any value.
fn merge_attributes<'a>(
    nodes: impl IntoIterator<Item = &'a IfcxNode>,
) -> IndexMap<&'a str, IndexMap<&'a str, &'a Value>> {
    let mut merged: IndexMap<&str, IndexMap<&str, &Value>> = IndexMap::new();
    for node in nodes {
        let entry = merged.entry(node.path.as_str()).or_default();
        for (id, value) in node.attributes.iter().flatten() {
            entry.insert(id.as_str(), value);
        }
    }
    merged
}

fn check_merged<'a>(
    schemas: &IndexMap<String, IfcxSchema>,
    nodes: impl IntoIterator<Item = &'a IfcxNode>,
) -> ValidationReport {
    let merged = merge_attributes(nodes);
    check_nodes(
        schemas,
        merged
            .iter()
            .map(|(path, attributes)| (*path, attributes.iter().map(|(id, value)| (*id, *value)))),
    )
}

/// Validates `files` as one federated file: schemas merged in order (a
/// repeated id keeps its first position and takes the last value), and the
/// data of every file merged per path in order.
pub(crate) fn validate_files<'a>(
    files: impl IntoIterator<Item = &'a IfcxFile>,
) -> ValidationReport {
    let files: Vec<&IfcxFile> = files.into_iter().collect();
    let mut schemas: IndexMap<String, IfcxSchema> = IndexMap::new();
    for file in &files {
        for (id, schema) in &file.schemas {
            schemas.insert(id.clone(), schema.clone());
        }
    }
    check_merged(&schemas, files.iter().flat_map(|file| &file.data))
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
