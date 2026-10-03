use std::path::PathBuf;

use openbim_ifcx::{DataType, IfcxFile, ReadErrorKind};
use serde_json::Value;

const FIXTURES: [&str; 4] = [
    "minimal.ifcx",
    "wall-with-type.ifcx",
    "deletions.ifcx",
    "unknown-fields.ifcx",
];

fn fixture(name: &str) -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn read(name: &str) -> IfcxFile {
    IfcxFile::from_json_str(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn every_fixture_round_trips_to_equal_json() {
    for name in FIXTURES {
        let original: Value = serde_json::from_str(&fixture(name)).unwrap();
        let written = read(name).to_json_string().unwrap();
        let reread: Value = serde_json::from_str(&written).unwrap();
        assert_eq!(reread, original, "{name}");
    }
}

#[test]
fn map_keys_keep_their_order() {
    // Value equality ignores order, so compare key sequences directly.
    let original: Value = serde_json::from_str(&fixture("wall-with-type.ifcx")).unwrap();
    let written: Value =
        serde_json::from_str(&read("wall-with-type.ifcx").to_json_string().unwrap()).unwrap();
    let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    for pointer in [
        "/schemas",
        "/data/1/attributes",
        "/schemas/example::wall::colour/value/objectRestrictions/values",
    ] {
        assert_eq!(
            keys(written.pointer(pointer).unwrap()),
            keys(original.pointer(pointer).unwrap()),
            "{pointer}"
        );
    }
}

#[test]
fn known_fields_are_written_in_schema_order() {
    // `deletions.ifcx` lists `inherits` before `children`; the writer uses
    // the order of `ifcx.tsp`.
    let written = read("deletions.ifcx").to_json_string().unwrap();
    let node = &written[written.find("000000000002").unwrap()..];
    assert!(node.find("\"children\"").unwrap() < node.find("\"inherits\"").unwrap());
}

#[test]
fn write_then_read_is_stable() {
    for name in FIXTURES {
        let file = read(name);
        let again = IfcxFile::from_json_str(&file.to_json_string_pretty().unwrap()).unwrap();
        assert_eq!(again, file, "{name}");
    }
}

#[test]
fn null_children_and_inherits_survive() {
    let file = read("deletions.ifcx");
    let root = &file.data[0];
    assert_eq!(root.children.as_ref().unwrap()["Wall"], None);

    let wall = &file.data[1];
    assert_eq!(wall.inherits.as_ref().unwrap()["WallType"], None);
    assert!(wall.children.as_ref().unwrap().is_empty());
    assert!(wall.attributes.as_ref().unwrap().is_empty());

    let written: Value = serde_json::from_str(&file.to_json_string().unwrap()).unwrap();
    assert_eq!(written["data"][0]["children"]["Wall"], Value::Null);
    assert_eq!(written["data"][1]["inherits"]["WallType"], Value::Null);
}

#[test]
fn absent_and_empty_maps_stay_distinct() {
    let file = read("wall-with-type.ifcx");
    let root = &file.data[0];
    assert!(root.inherits.is_none());
    assert!(root.attributes.is_none());

    let written: Value = serde_json::from_str(&file.to_json_string().unwrap()).unwrap();
    let root = written["data"][0].as_object().unwrap();
    assert!(!root.contains_key("inherits"));
    assert!(!root.contains_key("attributes"));
}

#[test]
fn nodes_sharing_a_path_are_kept_in_file_order() {
    let file = read("wall-with-type.ifcx");
    let wall = "f3a1c2d4-0000-4000-8000-000000000002";
    let heights: Vec<_> = file
        .data
        .iter()
        .filter(|n| n.path == wall)
        .map(|n| n.attributes.as_ref().unwrap()["example::wall::height"].clone())
        .collect();
    assert_eq!(heights, [2.75, 3.0]);
}

#[test]
fn header_imports_and_schemas_are_typed() {
    let file = read("wall-with-type.ifcx");
    assert_eq!(file.header.ifcx_version, "ifcx_alpha");
    assert_eq!(file.imports.len(), 2);
    assert_eq!(file.imports[0].integrity, None);
    assert!(file.imports[1]
        .integrity
        .as_deref()
        .unwrap()
        .starts_with("sha256-"));

    let types: Vec<_> = file
        .schemas
        .values()
        .map(|s| s.value.data_type.clone())
        .collect();
    assert_eq!(
        types,
        [
            DataType::Real,
            DataType::Boolean,
            DataType::Integer,
            DataType::String,
            DataType::DateTime,
            DataType::Enum,
            DataType::Reference,
            DataType::Blob,
            DataType::Array,
            DataType::Object,
        ]
    );

    let height = &file.schemas["example::wall::height"];
    assert_eq!(height.value.quantity_kind.as_deref(), Some("Length"));

    let profile = file.schemas["example::wall::profile"]
        .value
        .array_restrictions
        .as_ref()
        .unwrap();
    assert_eq!(profile.min.as_ref().and_then(|n| n.as_u64()), Some(2));
    let point = profile.value.array_restrictions.as_ref().unwrap();
    assert_eq!(point.value.data_type, DataType::Real);

    let colour = &file.schemas["example::wall::colour"].value;
    assert_eq!(
        colour.inherits.as_deref(),
        Some(&["example::wall::base".to_owned()][..])
    );
    let values = &colour.object_restrictions.as_ref().unwrap().values;
    assert_eq!(values.keys().collect::<Vec<_>>(), ["r", "g", "b"]);
    assert_eq!(values["b"].optional, Some(false));
}

#[test]
fn unknown_fields_and_types_are_kept() {
    let file = read("unknown-fields.ifcx");
    assert_eq!(file.header.ifcx_version, "ifcx_beta");
    assert!(file.extra.contains_key("signature"));
    assert_eq!(file.header.extra["tool"]["version"], 1);
    assert_eq!(file.imports[0].extra["optional"], true);

    let schema = &file.schemas["example::future"];
    assert_eq!(schema.extra["status"], "draft");
    assert_eq!(schema.value.data_type, DataType::Other("Quaternion".into()));
    assert!(schema.value.extra.contains_key("precision"));
    let array = schema.value.array_restrictions.as_ref().unwrap();
    assert_eq!(array.extra["stride"], 1);
    assert_eq!(array.value.extra["unit"], "1");

    let node = &file.data[0];
    assert!(node.extra.contains_key("comment"));
    assert!(node.extra.contains_key("linearreference"));
}

#[test]
fn numbers_survive_exactly() {
    let file = read("unknown-fields.ifcx");
    let written = file.to_json_string().unwrap();
    assert!(written.contains("9007199254740993"), "{written}");
    assert!(written.contains("0.7071067811865476"), "{written}");
    assert!(written.contains("0.7071067811865475"), "{written}");
    assert!(written.contains("1234.5678901234567"), "{written}");
}

#[test]
fn syntax_errors_carry_a_location() {
    let err = IfcxFile::from_json_str("{\n  \"header\": {,\n}").unwrap_err();
    assert_eq!(err.kind(), ReadErrorKind::Syntax);
    assert_eq!((err.line(), err.column()), (2, 14));
}

#[test]
fn shape_errors_carry_a_location() {
    let input = fixture("minimal.ifcx").replace("\"data\": []", "\"data\": [{\"path\": 7}]");
    let err = IfcxFile::from_json_str(&input).unwrap_err();
    assert_eq!(err.kind(), ReadErrorKind::Data);
    assert_eq!(err.line(), 11);
    assert!(err.to_string().contains("invalid type"), "{err}");
}

#[test]
fn missing_required_fields_are_reported() {
    let input = fixture("minimal.ifcx").replace("\"imports\": [],", "");
    let err = IfcxFile::from_json_str(&input).unwrap_err();
    assert_eq!(err.kind(), ReadErrorKind::Data);
    assert!(err.to_string().contains("imports"), "{err}");
}

#[test]
fn truncated_input_is_eof() {
    let text = fixture("minimal.ifcx");
    let err = IfcxFile::from_json_str(&text[..text.len() / 2]).unwrap_err();
    assert_eq!(err.kind(), ReadErrorKind::Eof);
}

#[test]
fn reader_and_slice_agree_with_str() {
    let text = fixture("wall-with-type.ifcx");
    let from_str = IfcxFile::from_json_str(&text).unwrap();
    assert_eq!(
        IfcxFile::from_json_slice(text.as_bytes()).unwrap(),
        from_str
    );
    assert_eq!(
        IfcxFile::from_json_reader(text.as_bytes()).unwrap(),
        from_str
    );

    let mut out = Vec::new();
    from_str.to_json_writer(&mut out).unwrap();
    assert_eq!(out, from_str.to_json_string().unwrap().into_bytes());
}
