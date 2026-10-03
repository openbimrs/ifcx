//! `IfcxFile::validate` with schemas that inherit each other (chains,
//! diamonds, cycles, unknown ids) and nested object and array restrictions,
//! against values of every JSON type. IFCX JSON input is validated as is.
#![no_main]

use arbitrary::{Result, Unstructured};
use libfuzzer_sys::fuzz_target;
use openbim_ifcx::IfcxFile;
use openbim_ifcx_fuzz::file;
use serde_json::{json, Map, Value};

const IDS: usize = 10;
const TYPES: [&str; 12] = [
    "Boolean",
    "String",
    "DateTime",
    "Reference",
    "Enum",
    "Real",
    "Integer",
    "Object",
    "Array",
    "Blob",
    "Relation",
    "Unknown",
];

fn id(u: &mut Unstructured<'_>) -> Result<String> {
    Ok(format!("x::s{}", u.choose_index(IDS)?))
}

fn description(u: &mut Unstructured<'_>, depth: u8) -> Result<Value> {
    let data_type = *u.choose(&TYPES)?;
    let mut desc = json!({ "dataType": data_type });
    if u.ratio(1, 2)? {
        let inherits: Result<Vec<String>> = (0..u.int_in_range(1..=3u8)?).map(|_| id(u)).collect();
        desc["inherits"] = json!(inherits?);
    }
    if u.ratio(1, 4)? {
        desc["optional"] = json!(u.arbitrary::<bool>()?);
    }
    match data_type {
        "Enum" if u.ratio(4, 5)? => {
            desc["enumRestrictions"] = json!({ "options": ["a", "b"] });
        }
        "Object" if depth > 0 && u.ratio(4, 5)? => {
            let mut values = Map::new();
            for key in ["a", "b", "c/~"].iter().take(u.int_in_range(0..=3usize)?) {
                values.insert((*key).into(), description(u, depth - 1)?);
            }
            desc["objectRestrictions"] = json!({ "values": values });
        }
        "Array" if depth > 0 && u.ratio(4, 5)? => {
            let mut restrictions = json!({ "value": description(u, depth - 1)? });
            if u.ratio(1, 2)? {
                restrictions["min"] = json!(u.int_in_range(0..=3u8)?);
            }
            if u.ratio(1, 2)? {
                restrictions["max"] = json!(f64::from(u.int_in_range(0..=6u8)?) / 2.0);
            }
            desc["arrayRestrictions"] = restrictions;
        }
        _ => {}
    }
    Ok(desc)
}

fn value(u: &mut Unstructured<'_>, depth: u8) -> Result<Value> {
    Ok(match u.choose_index(if depth == 0 { 6 } else { 8 })? {
        0 => Value::Null,
        1 => json!(u.arbitrary::<bool>()?),
        2 => json!(u.int_in_range(-3..=3i64)?),
        3 => json!(f64::from(u.arbitrary::<i8>()?) / 4.0),
        4 => json!(*u.choose(&["a", "b", "c", ""])?),
        5 => json!(u.arbitrary::<u64>()?),
        6 => {
            let n = u.int_in_range(0..=4u8)?;
            Value::Array((0..n).map(|_| value(u, depth - 1)).collect::<Result<_>>()?)
        }
        _ => {
            let mut object = Map::new();
            for key in ["a", "b", "c/~", "d"] {
                if u.ratio(1, 2)? {
                    object.insert(key.into(), value(u, depth - 1)?);
                }
            }
            Value::Object(object)
        }
    })
}

fn structured(u: &mut Unstructured<'_>) -> Result<IfcxFile> {
    let mut schemas = Map::new();
    for _ in 0..u.int_in_range(0..=IDS)? {
        schemas.insert(id(u)?, json!({ "value": description(u, 3)? }));
    }
    let mut nodes = Vec::new();
    for _ in 0..u.int_in_range(0..=4u8)? {
        let mut attributes = Map::new();
        for _ in 0..u.int_in_range(0..=4u8)? {
            let key = if u.ratio(1, 10)? {
                "__internal".to_owned()
            } else {
                id(u)?
            };
            attributes.insert(key, value(u, 3)?);
        }
        nodes.push(json!({ "path": format!("n{}", u.choose_index(3)?), "attributes": attributes }));
    }
    Ok(file(nodes, schemas))
}

fuzz_target!(|data: &[u8]| {
    let file = if data.first() == Some(&b'{') {
        match IfcxFile::from_json_slice(data) {
            Ok(file) => file,
            Err(_) => return,
        }
    } else {
        match structured(&mut Unstructured::new(data)) {
            Ok(file) => file,
            Err(_) => return,
        }
    };
    if let Err(report) = file.validate() {
        assert!(!report.is_valid());
        let _ = report.to_string();
    }
});
