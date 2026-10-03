use std::path::PathBuf;

use indexmap::IndexMap;
use openbim_ifcx::{
    validate_attributes, validate_nodes, DataType, FailureKind, IfcxFile, IfcxNode, IfcxSchema,
    JsonType, ValidationFailure,
};
use serde_json::{json, Value};

fn fixture(name: &str) -> IfcxFile {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    IfcxFile::from_json_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn schemas(value: Value) -> IndexMap<String, IfcxSchema> {
    serde_json::from_value(value).unwrap()
}

/// Validates `value` as attribute `t::a` against a schema whose value
/// description is `desc`, returning `(pointer, kind)` for each failure.
fn check(desc: Value, value: Value) -> Vec<(String, FailureKind)> {
    check_with(json!({ "t::a": { "value": desc } }), value)
}

fn check_with(schema_map: Value, value: Value) -> Vec<(String, FailureKind)> {
    let schemas = schemas(schema_map);
    let attributes = IndexMap::from([("t::a".to_owned(), value)]);
    validate_attributes(&schemas, [("n", &attributes)])
        .failures
        .into_iter()
        .map(|f| {
            assert_eq!((f.node.as_str(), f.attribute.as_str()), ("n", "t::a"));
            (f.pointer, f.kind)
        })
        .collect()
}

fn mismatch(expected: DataType, found: JsonType) -> Vec<(String, FailureKind)> {
    vec![(String::new(), FailureKind::TypeMismatch { expected, found })]
}

#[test]
fn boolean() {
    assert_eq!(check(json!({"dataType": "Boolean"}), json!(false)), vec![]);
    assert_eq!(
        check(json!({"dataType": "Boolean"}), json!("true")),
        mismatch(DataType::Boolean, JsonType::String)
    );
}

#[test]
fn string() {
    assert_eq!(check(json!({"dataType": "String"}), json!("")), vec![]);
    assert_eq!(
        check(json!({"dataType": "String"}), json!(1)),
        mismatch(DataType::String, JsonType::Number)
    );
}

#[test]
fn date_time_is_any_string_as_upstream() {
    let desc = json!({"dataType": "DateTime"});
    assert_eq!(check(desc.clone(), json!("2026-10-03T12:00:00Z")), vec![]);
    // Upstream checks only for a string, not the ISO 8601 format.
    assert_eq!(check(desc.clone(), json!("tomorrow")), vec![]);
    assert_eq!(
        check(desc, json!(20261003)),
        mismatch(DataType::DateTime, JsonType::Number)
    );
}

#[test]
fn enumeration() {
    let desc = json!({"dataType": "Enum", "enumRestrictions": {"options": ["A", "B"]}});
    assert_eq!(check(desc.clone(), json!("B")), vec![]);
    assert_eq!(
        check(desc.clone(), json!("C")),
        vec![(
            String::new(),
            FailureKind::NotAnOption {
                value: "C".into(),
                options: vec!["A".into(), "B".into()]
            }
        )]
    );
    assert_eq!(
        check(desc, json!(1)),
        mismatch(DataType::Enum, JsonType::Number)
    );
}

#[test]
fn enum_without_options_is_reported() {
    assert_eq!(
        check(json!({"dataType": "Enum"}), json!("A")),
        vec![(
            String::new(),
            FailureKind::MissingRestrictions {
                data_type: DataType::Enum
            }
        )]
    );
}

#[test]
fn integer_rejects_fractions() {
    let desc = json!({"dataType": "Integer"});
    for ok in [json!(3), json!(-3), json!(3.0), json!(u64::MAX), json!(1e3)] {
        assert_eq!(check(desc.clone(), ok.clone()), vec![], "{ok}");
    }
    let half = json!(3.5);
    assert_eq!(
        check(desc.clone(), half.clone()),
        vec![(
            String::new(),
            FailureKind::NotAnInteger {
                value: half.as_number().unwrap().clone()
            }
        )]
    );
    assert_eq!(
        check(desc, json!("3")),
        mismatch(DataType::Integer, JsonType::String)
    );
}

#[test]
fn real() {
    assert_eq!(check(json!({"dataType": "Real"}), json!(3)), vec![]);
    assert_eq!(check(json!({"dataType": "Real"}), json!(-0.25)), vec![]);
    assert_eq!(
        check(json!({"dataType": "Real"}), json!(null)),
        mismatch(DataType::Real, JsonType::Null)
    );
}

#[test]
fn reference_is_an_unresolved_string() {
    assert_eq!(
        check(json!({"dataType": "Reference"}), json!("no-such-node")),
        vec![]
    );
    assert_eq!(
        check(json!({"dataType": "Reference"}), json!({"ref": "a"})),
        mismatch(DataType::Reference, JsonType::Object)
    );
}

#[test]
fn object_keys_and_optional() {
    let desc = json!({"dataType": "Object", "objectRestrictions": {"values": {
        "x": {"dataType": "Real"},
        "note": {"dataType": "String", "optional": true}
    }}});
    assert_eq!(check(desc.clone(), json!({"x": 1, "extra": [1]})), vec![]);
    assert_eq!(check(desc.clone(), json!({"x": 1, "note": "n"})), vec![]);
    assert_eq!(
        check(desc.clone(), json!({"note": 2})),
        vec![
            (String::new(), FailureKind::MissingKey { key: "x".into() }),
            (
                "/note".into(),
                FailureKind::TypeMismatch {
                    expected: DataType::String,
                    found: JsonType::Number
                }
            ),
        ]
    );
    // `optional` allows absence, not `null`.
    assert_eq!(
        check(desc.clone(), json!({"x": 1, "note": null})),
        vec![(
            "/note".into(),
            FailureKind::TypeMismatch {
                expected: DataType::String,
                found: JsonType::Null
            }
        )]
    );
    assert_eq!(
        check(desc, json!([1])),
        mismatch(DataType::Object, JsonType::Array)
    );
}

#[test]
fn object_without_restrictions_accepts_any_object() {
    assert_eq!(
        check(json!({"dataType": "Object"}), json!({"a": [null]})),
        vec![]
    );
    assert_eq!(
        check(json!({"dataType": "Object"}), json!(null)),
        mismatch(DataType::Object, JsonType::Null)
    );
}

#[test]
fn array_elements_and_bounds() {
    let desc = json!({"dataType": "Array", "arrayRestrictions": {
        "min": 1, "max": 2, "value": {"dataType": "Integer"}
    }});
    assert_eq!(check(desc.clone(), json!([1, 2])), vec![]);
    assert_eq!(
        check(desc.clone(), json!([])),
        vec![(
            String::new(),
            FailureKind::TooFewElements {
                len: 0,
                min: 1.into()
            }
        )]
    );
    assert_eq!(
        check(desc.clone(), json!([1, true, 3])),
        vec![
            (
                String::new(),
                FailureKind::TooManyElements {
                    len: 3,
                    max: 2.into()
                }
            ),
            (
                "/1".into(),
                FailureKind::TypeMismatch {
                    expected: DataType::Integer,
                    found: JsonType::Boolean
                }
            ),
        ]
    );
    assert_eq!(
        check(desc, json!({"0": 1})),
        mismatch(DataType::Array, JsonType::Object)
    );
}

#[test]
fn array_without_restrictions_is_reported() {
    assert_eq!(
        check(json!({"dataType": "Array"}), json!([])),
        vec![(
            String::new(),
            FailureKind::MissingRestrictions {
                data_type: DataType::Array
            }
        )]
    );
}

#[test]
fn nested_arrays_and_objects_report_json_pointers() {
    let desc = json!({"dataType": "Object", "objectRestrictions": {"values": {
        "faces/loops": {"dataType": "Array", "arrayRestrictions": {"value": {
            "dataType": "Object", "objectRestrictions": {"values": {
                "points~": {"dataType": "Array", "arrayRestrictions": {
                    "value": {"dataType": "Array", "arrayRestrictions": {"min": 3, "max": 3, "value": {"dataType": "Real"}}}
                }}
            }}
        }}}
    }}});
    let value = json!({"faces/loops": [
        {"points~": [[0, 0, 0], [1, 0, 0]]},
        {"points~": [[0, 0], [1, 0, "z"]]},
        {}
    ]});
    assert_eq!(
        check(desc, value),
        vec![
            (
                "/faces~1loops/1/points~0/0".into(),
                FailureKind::TooFewElements {
                    len: 2,
                    min: 3.into()
                }
            ),
            (
                "/faces~1loops/1/points~0/1/2".into(),
                FailureKind::TypeMismatch {
                    expected: DataType::Real,
                    found: JsonType::String
                }
            ),
            (
                "/faces~1loops/2".into(),
                FailureKind::MissingKey {
                    key: "points~".into()
                }
            ),
        ]
    );
}

#[test]
fn blob_accepts_any_value() {
    for value in [json!("aGVsbG8="), json!([1, 2]), json!(null)] {
        assert_eq!(check(json!({"dataType": "Blob"}), value), vec![]);
    }
}

#[test]
fn unknown_data_type_is_reported() {
    assert_eq!(
        check(json!({"dataType": "Quaternion"}), json!([0, 0, 0, 1])),
        vec![(
            String::new(),
            FailureKind::UnknownDataType {
                name: "Quaternion".into()
            }
        )]
    );
}

#[test]
fn quantity_kind_is_not_checked() {
    let desc = json!({"dataType": "Real", "quantityKind": "NotAKind"});
    assert_eq!(check(desc, json!(2.5)), vec![]);
}

#[test]
fn inherited_schemas_apply_before_own_type() {
    let schema_map = json!({
        "t::point": {"value": {"dataType": "Object", "objectRestrictions": {"values": {
            "x": {"dataType": "Real"}
        }}}},
        "t::a": {"value": {"dataType": "Object", "inherits": ["t::point"],
            "objectRestrictions": {"values": {"name": {"dataType": "String"}}}}}
    });
    assert_eq!(
        check_with(schema_map.clone(), json!({"x": 1, "name": "p"})),
        vec![]
    );
    assert_eq!(
        check_with(schema_map, json!({"name": "p"})),
        vec![(String::new(), FailureKind::MissingKey { key: "x".into() })]
    );
}

#[test]
fn inherits_inside_nested_descriptions() {
    let schema_map = json!({
        "t::positive": {"value": {"dataType": "Integer"}},
        "t::a": {"value": {"dataType": "Array", "arrayRestrictions": {
            "value": {"dataType": "Real", "inherits": ["t::positive"]}
        }}}
    });
    assert_eq!(
        check_with(schema_map, json!([1, 2.5])),
        vec![(
            "/1".into(),
            FailureKind::NotAnInteger {
                value: json!(2.5).as_number().unwrap().clone()
            }
        )]
    );
}

#[test]
fn unknown_inherited_schema_and_cycles_are_reported() {
    assert_eq!(
        check(
            json!({"dataType": "String", "inherits": ["t::gone"]}),
            json!("s")
        ),
        vec![(
            String::new(),
            FailureKind::UnknownInheritedSchema {
                id: "t::gone".into()
            }
        )]
    );
    let cycle = json!({
        "t::a": {"value": {"dataType": "String", "inherits": ["t::b"]}},
        "t::b": {"value": {"dataType": "String", "inherits": ["t::a"]}}
    });
    assert_eq!(
        check_with(cycle, json!("s")),
        vec![(
            String::new(),
            FailureKind::InheritanceCycle { id: "t::a".into() }
        )]
    );
}

#[test]
fn missing_schema_names_node_and_attribute() {
    let schemas = schemas(json!({}));
    let node: IfcxNode = serde_json::from_value(json!({
        "path": "wall-1", "attributes": {"t::unknown": 1, "__internal_x": 2}
    }))
    .unwrap();
    let report = validate_nodes(&schemas, [&node]);
    assert_eq!(
        report.failures,
        vec![ValidationFailure {
            node: "wall-1".into(),
            attribute: "t::unknown".into(),
            pointer: String::new(),
            kind: FailureKind::MissingSchema,
        }]
    );
    assert_eq!(
        report.failures[0].to_string(),
        "[\"wall-1\"].attributes[\"t::unknown\"]: no schema with this id"
    );
}

#[test]
fn valid_fixture_validates() {
    let file = fixture("valid-attributes.ifcx");
    file.validate().unwrap();
    // Unmerged, the first opinion on `building` is checked on its own too.
    assert!(validate_nodes(&file.schemas, &file.data).is_valid());
}

#[test]
fn later_opinions_replace_earlier_values_before_validation() {
    let file = fixture("invalid-attributes.ifcx");
    let report = file.validate().unwrap_err();
    // `building` has `storeys: 4` and then `4.5`; only the merged 4.5 counts.
    let storeys: Vec<_> = report
        .failures
        .iter()
        .filter(|f| f.attribute == "example::storeys")
        .collect();
    assert_eq!(storeys.len(), 1);
    // Validating raw nodes sees both opinions; the first is fine.
    let raw = validate_nodes(&file.schemas, &file.data);
    assert_eq!(raw.failures.len(), report.failures.len());
}

#[test]
fn invalid_fixture_collects_every_failure() {
    let report = fixture("invalid-attributes.ifcx").validate().unwrap_err();
    let lines: Vec<String> = report.failures.iter().map(|f| f.to_string()).collect();
    assert_eq!(
        lines,
        [
            r#"["building"].attributes["example::height"]: expected Real, found string"#,
            r#"["building"].attributes["example::storeys"]: expected Integer, found 4.5"#,
            r#"["building"].attributes["example::status"]: "PLANNED" is not one of [NEW, EXISTING]"#,
            r#"["building"].attributes["example::colour"]: no schema with this id"#,
            r#"["storey"].attributes["example::outline"] at /1/1: expected Real, found string"#,
            r#"["storey"].attributes["example::outline"] at /2: array has 3 elements, more than max 2"#,
            r#"["storey"].attributes["example::label"]: missing key "text""#,
            r#"["storey"].attributes["example::label"] at /a~1b: expected Boolean, found string"#,
        ]
    );
    assert!(report
        .to_string()
        .starts_with("8 attribute values do not match their schemas\n  [\"building\"]"));
    let as_error: &dyn std::error::Error = &report;
    assert!(as_error.source().is_none());
}
