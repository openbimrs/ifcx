//! Layer stacks: import order, federation, and typed failures.

use openbim_ifcx::layers::{
    federate, federate_owned, LayerError, LayerStack, LayerStackBuilder, MemoryResolver,
};
use openbim_ifcx::{flatten, flatten_owned, validate_flat, FailureKind, IfcxFile};
use serde_json::{json, Value};

/// A small layer with one `demo::value` opinion on path `root`.
fn layer(id: &str, imports: &[&str], value: &str) -> String {
    let imports: Vec<Value> = imports.iter().map(|uri| json!({ "uri": uri })).collect();
    json!({
        "header": {"id": id, "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                   "author": "openbimrs contributors", "timestamp": "2026-10-03"},
        "imports": imports,
        "schemas": {"demo::value": {"value": {"dataType": "String"}}},
        "data": [{"path": "root", "attributes": {"demo::value": value}}]
    })
    .to_string()
}

fn resolver(layers: &[(&str, &[&str])]) -> MemoryResolver {
    let mut resolver = MemoryResolver::new();
    for (id, imports) in layers {
        resolver.insert(*id, layer(id, imports, id));
    }
    resolver
}

fn keys(stack: &LayerStack) -> Vec<&str> {
    stack.keys().collect()
}

/// The flattened value of attribute `name` at `path` in a federated file.
fn attribute(file: &IfcxFile, path: &str, name: &str) -> Value {
    (*flatten(&file.data)[path].attributes[name]).clone()
}

/// The `demo::value` that wins at `root` when `main` is federated.
fn winner(layers: &[(&str, &[&str])]) -> Value {
    let stack = LayerStackBuilder::new(resolver(layers))
        .build("main")
        .unwrap();
    attribute(&stack.federate(), "root", "demo::value")
}

/// A `main` layer importing `imports`, with no schemas and no data.
fn empty_main(imports: &[&str]) -> String {
    let imports: Vec<Value> = imports.iter().map(|uri| json!({ "uri": uri })).collect();
    json!({
        "header": {"id": "main", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                   "author": "openbimrs contributors", "timestamp": "2026-10-03"},
        "imports": imports, "schemas": {}, "data": []
    })
    .to_string()
}

#[test]
fn every_layer_follows_its_imports_and_the_main_layer_comes_last() {
    let mut builder = LayerStackBuilder::new(resolver(&[
        ("main", &["a", "b"]),
        ("a", &["c"]),
        ("b", &[]),
        ("c", &[]),
    ]));
    let stack = builder.build("main").unwrap();
    assert_eq!(keys(&stack), ["c", "a", "b", "main"]);
    assert_eq!(stack.main().key(), "main");
    assert_eq!(stack.layers()[1].file().header.id, "a");

    let mut builder =
        LayerStackBuilder::new(resolver(&[("main", &["a"]), ("a", &["b"]), ("b", &[])]));
    assert_eq!(keys(&builder.build("main").unwrap()), ["b", "a", "main"]);
}

#[test]
fn federation_takes_the_main_layers_header_and_every_layers_data() {
    let mut builder =
        LayerStackBuilder::new(resolver(&[("main", &["a", "b"]), ("a", &[]), ("b", &[])]));
    let federated = builder.build("main").unwrap().federate();
    assert_eq!(federated.header.id, "main");
    assert!(federated.imports.is_empty());
    assert_eq!(federated.data.len(), 3);
    assert_eq!(attribute(&federated, "root", "demo::value"), "main");
}

#[test]
fn a_layer_overrides_the_layers_it_imports() {
    // buildingSMART/IFC5-development#144: imported data comes first.
    assert_eq!(winner(&[("main", &["a"]), ("a", &[])]), "main");
    assert_eq!(
        winner(&[("main", &["a"]), ("a", &["b"]), ("b", &[])]),
        "main"
    );
}

#[test]
fn an_import_overrides_the_layers_it_imports() {
    // `main` sets nothing, so `a` wins over its own import `b`.
    let mut resolver = resolver(&[("a", &["b"]), ("b", &[])]);
    resolver.insert("main", empty_main(&["a"]));
    let stack = LayerStackBuilder::new(resolver).build("main").unwrap();
    assert_eq!(keys(&stack), ["b", "a", "main"]);
    let federated = stack.federate();
    assert_eq!(federated.header.id, "main");
    assert_eq!(attribute(&federated, "root", "demo::value"), "a");
}

#[test]
fn a_later_sibling_import_overrides_an_earlier_one() {
    let mut resolver = resolver(&[("a", &[]), ("b", &[])]);
    resolver.insert("main", empty_main(&["a", "b"]));
    let stack = LayerStackBuilder::new(resolver.clone())
        .build("main")
        .unwrap();
    assert_eq!(keys(&stack), ["a", "b", "main"]);
    assert_eq!(attribute(&stack.federate(), "root", "demo::value"), "b");

    resolver.insert("main", empty_main(&["b", "a"]));
    let stack = LayerStackBuilder::new(resolver).build("main").unwrap();
    assert_eq!(keys(&stack), ["b", "a", "main"]);
    assert_eq!(attribute(&stack.federate(), "root", "demo::value"), "a");
}

#[test]
fn a_diamond_loads_its_shared_import_once_before_both_importers() {
    let mut resolver = resolver(&[("a", &["c"]), ("b", &["c"]), ("c", &[])]);
    resolver.insert("main", empty_main(&["a", "b"]));
    let stack = LayerStackBuilder::new(resolver).build("main").unwrap();
    assert_eq!(keys(&stack), ["c", "a", "b", "main"]);
    let federated = stack.federate();
    assert_eq!(federated.data.len(), 3);
    assert_eq!(attribute(&federated, "root", "demo::value"), "b");
}

#[test]
fn a_layer_imported_twice_loads_once_where_first_reached() {
    // `a` imports `b` too, so `b` is placed under `a`, before it: a layer
    // always overrides what it imports. `main`'s later import of `b` does
    // not move it.
    let mut builder = LayerStackBuilder::new(resolver(&[
        ("main", &["a", "b"]),
        ("a", &["b", "c"]),
        ("b", &["c"]),
        ("c", &[]),
    ]));
    let stack = builder.build("main").unwrap();
    assert_eq!(keys(&stack), ["c", "b", "a", "main"]);
}

#[test]
fn cycles_allowed_skip_the_closing_import_and_keep_the_main_layer_last() {
    // The import graph of upstream's `layer-stack-test.ts` order tests, where
    // `file2` imports `file1` back. The import that closes the cycle names a
    // layer still being loaded; it is skipped, so that layer overrides the
    // importer closing the cycle.
    let layers: &[(&str, &[&str])] = &[
        ("file1", &["file2", "file3", "file4"]),
        ("file2", &["file4", "file3", "file1"]),
        ("file3", &[]),
        ("file4", &[]),
    ];
    let mut builder = LayerStackBuilder::new(resolver(layers)).allow_cycles(true);
    let stack = builder.build("file1").unwrap();
    assert_eq!(keys(&stack), ["file4", "file3", "file2", "file1"]);
    assert_eq!(stack.main().key(), "file1");
    assert_eq!(
        keys(&builder.build("file2").unwrap()),
        ["file4", "file3", "file1", "file2"]
    );

    let mut builder =
        LayerStackBuilder::new(resolver(&[("main", &["a"]), ("a", &["main"])])).allow_cycles(true);
    let stack = builder.build("main").unwrap();
    assert_eq!(keys(&stack), ["a", "main"]);
    assert_eq!(attribute(&stack.federate(), "root", "demo::value"), "main");
    let mut builder = LayerStackBuilder::new(resolver(&[("main", &["main"])])).allow_cycles(true);
    assert_eq!(keys(&builder.build("main").unwrap()), ["main"]);

    let err = LayerStackBuilder::new(resolver(layers))
        .build("file1")
        .unwrap_err();
    assert!(
        matches!(&err, LayerError::Cycle { chain } if chain == &["file1", "file2", "file1"]),
        "{err}"
    );
}

#[test]
fn a_long_import_chain_does_not_recurse() {
    const DEPTH: usize = 20_000;
    let mut resolver = MemoryResolver::new();
    for i in 0..DEPTH {
        let next = format!("l{}", i + 1);
        let imports: &[&str] = if i + 1 < DEPTH { &[&next] } else { &[] };
        resolver.insert(format!("l{i}"), layer(&format!("l{i}"), imports, "x"));
    }
    let stack = LayerStackBuilder::new(resolver).build("l0").unwrap();
    assert_eq!(stack.layers().len(), DEPTH);
    assert_eq!(stack.main().key(), "l0");
    assert_eq!(stack.layers()[0].key(), format!("l{}", DEPTH - 1));
}

#[test]
fn federate_matches_upstream_for_files_in_given_order() {
    let a = IfcxFile::from_json_str(&layer("a", &[], "a")).unwrap();
    let b = IfcxFile::from_json_str(&layer("b", &[], "b")).unwrap();
    assert_eq!(
        attribute(&federate([&a, &b]).unwrap(), "root", "demo::value"),
        "b"
    );
    assert_eq!(
        attribute(&federate([&b, &a]).unwrap(), "root", "demo::value"),
        "a"
    );
    // The last file is the strongest and gives the header.
    assert_eq!(federate([&a, &b]).unwrap().header.id, "b");
    assert_eq!(federate_owned([b, a]).unwrap().header.id, "a");
    assert!(federate([]).is_none());
}

#[test]
fn build_all_stacks_named_layers_like_a_synthetic_main_layer() {
    let layers: &[(&str, &[&str])] = &[("a", &["c"]), ("b", &["a"]), ("c", &[])];
    // Each named layer follows its imports; `b`'s import of `a` does not move
    // `a`, which is already placed, and naming `a` again does not either.
    let stack = LayerStackBuilder::new(resolver(layers))
        .build_all(["a", "b", "a"])
        .unwrap();
    assert_eq!(keys(&stack), ["c", "a", "b"]);
    assert_eq!(stack.main().key(), "b");
    let federated = stack.federate();
    assert_eq!(federated.header.id, "b");
    assert_eq!(attribute(&federated, "root", "demo::value"), "b");

    // The last named layer wins, as below a synthetic main layer.
    let plain: &[(&str, &[&str])] = &[("x", &[]), ("y", &[]), ("z", &[])];
    for order in [["x", "y", "z"], ["z", "y", "x"], ["y", "z", "x"]] {
        let stack = LayerStackBuilder::new(resolver(plain))
            .build_all(order)
            .unwrap();
        assert_eq!(keys(&stack), order);
        assert_eq!(
            attribute(&stack.federate(), "root", "demo::value"),
            order[2]
        );
    }
    // A named layer that an earlier one imports is placed under that one,
    // which overrides it.
    let stack = LayerStackBuilder::new(resolver(layers))
        .build_all(["b", "c"])
        .unwrap();
    assert_eq!(keys(&stack), ["c", "a", "b"]);

    let err = LayerStackBuilder::new(resolver(layers))
        .build_all(["b", "gone"])
        .unwrap_err();
    assert!(
        matches!(&err, LayerError::Missing { uri, importer: None } if uri == "gone"),
        "{err}"
    );
    let err = LayerStackBuilder::new(resolver(layers))
        .build_all(Vec::<String>::new())
        .unwrap_err();
    assert!(matches!(err, LayerError::Missing { importer: None, .. }));
}

#[test]
fn owned_federation_and_flattening_equal_the_borrowed_ones() {
    let stack = LayerStackBuilder::new(resolver(&[
        ("main", &["a", "b"]),
        ("a", &["c"]),
        ("b", &[]),
        ("c", &[]),
    ]))
    .build("main")
    .unwrap();
    let federated = stack.federate();
    let files: Vec<IfcxFile> = stack.layers().iter().map(|l| l.file().clone()).collect();
    assert_eq!(federate_owned(files).unwrap(), federated);
    assert!(federate_owned(Vec::new()).is_none());
    let flat = flatten(&federated.data);
    let owned = stack.into_federated();
    assert_eq!(owned, federated);
    assert_eq!(flatten_owned(owned.data), flat);
}

/// A layer without data whose `schemas` describe `demo::height` as `Real`.
fn schema_layer(id: &str) -> String {
    json!({
        "header": {"id": id, "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                   "author": "openbimrs contributors", "timestamp": "2026-10-03"},
        "imports": [], "data": [],
        "schemas": {"demo::height": {"value": {"dataType": "Real"}}}
    })
    .to_string()
}

/// A layer setting `demo::height` on `wall` to `height`, importing `imports`,
/// with no schemas of its own.
fn height_layer(id: &str, imports: &[&str], height: Value) -> String {
    let imports: Vec<Value> = imports.iter().map(|uri| json!({ "uri": uri })).collect();
    json!({
        "header": {"id": id, "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                   "author": "openbimrs contributors", "timestamp": "2026-10-03"},
        "imports": imports, "schemas": {},
        "data": [{"path": "wall", "attributes": {"demo::height": height}}]
    })
    .to_string()
}

#[test]
fn a_stack_validates_against_schemas_only_its_imports_define() {
    let mut resolver = MemoryResolver::new();
    resolver.insert("main", height_layer("main", &["schemas"], json!(2.5)));
    resolver.insert("schemas", schema_layer("schemas"));
    let stack = LayerStackBuilder::new(resolver).build("main").unwrap();

    let alone = stack.main().file().validate().unwrap_err();
    assert_eq!(alone.failures.len(), 1);
    assert_eq!(alone.failures[0].kind, FailureKind::MissingSchema);
    assert_eq!(stack.validate(), Ok(()));
    // The same as validating the federated file or its flattened nodes.
    let federated = stack.federate();
    assert_eq!(federated.validate(), Ok(()));
    assert!(validate_flat(&federated.schemas, &flatten(&federated.data)).is_valid());
}

#[test]
fn a_stack_checks_only_the_value_that_wins_for_a_path() {
    // `fix` writes a string, `main`, which imports it, overrides it with a
    // number, and `schemas` says the value is `Real`.
    let mut resolver = MemoryResolver::new();
    resolver.insert(
        "main",
        height_layer("main", &["fix", "schemas"], json!(3.0)),
    );
    resolver.insert("fix", height_layer("fix", &[], json!("tall")));
    resolver.insert("schemas", schema_layer("schemas"));
    let stack = LayerStackBuilder::new(resolver.clone())
        .build("main")
        .unwrap();
    assert_eq!(stack.validate(), Ok(()));

    // Reversed: the main layer's string wins and is reported once, at its
    // path.
    resolver.insert(
        "main",
        height_layer("main", &["fix", "schemas"], json!("tall")),
    );
    resolver.insert("fix", height_layer("fix", &[], json!(3.0)));
    let stack = LayerStackBuilder::new(resolver).build("main").unwrap();
    let report = stack.validate().unwrap_err();
    assert_eq!(report.failures.len(), 1, "{report}");
    assert_eq!(report.failures[0].node, "wall");
    assert!(matches!(
        report.failures[0].kind,
        FailureKind::TypeMismatch { .. }
    ));
    assert_eq!(stack.federate().validate(), Err(report.clone()));
    assert_eq!(
        validate_flat(&stack.federate().schemas, &flatten(&stack.federate().data)),
        report
    );
}

#[test]
fn self_import_is_a_cycle() {
    let err = LayerStackBuilder::new(resolver(&[("main", &["main"])]))
        .build("main")
        .unwrap_err();
    assert!(matches!(&err, LayerError::Cycle { chain } if chain == &["main", "main"]));
}

#[test]
fn missing_layers_name_the_importer() {
    let err = LayerStackBuilder::new(resolver(&[("main", &["a"]), ("a", &["gone"])]))
        .build("main")
        .unwrap_err();
    assert!(
        matches!(&err, LayerError::Missing { uri, importer } if uri == "gone" && importer.as_deref() == Some("a")),
        "{err}"
    );
    let err = LayerStackBuilder::new(MemoryResolver::new())
        .build("main")
        .unwrap_err();
    assert!(matches!(err, LayerError::Missing { importer: None, .. }));
}

#[test]
fn invalid_layers_are_read_errors() {
    let mut resolver = resolver(&[("main", &["a"])]);
    resolver.insert("a", "{}");
    let err = LayerStackBuilder::new(resolver).build("main").unwrap_err();
    assert!(
        matches!(&err, LayerError::Read { key, .. } if key == "a"),
        "{err}"
    );
}

#[test]
fn integrity_applies_to_every_import_of_a_layer() {
    let mut resolver = resolver(&[("b", &[])]);
    let bad = r#"{"uri": "b", "integrity": "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}"#;
    // `a` imports `b` again after `main` loaded it, with a wrong digest.
    resolver.insert("main", layer("main", &["a", "b"], "main"));
    resolver.insert(
        "a",
        layer("a", &[], "a").replace(r#""imports":[]"#, &format!(r#""imports":[{bad}]"#)),
    );
    let err = LayerStackBuilder::new(resolver).build("main").unwrap_err();
    assert!(
        matches!(&err, LayerError::Integrity { uri, importer, .. } if uri == "b" && importer == "a"),
        "{err}"
    );
}

#[cfg(not(feature = "integrity"))]
#[test]
fn integrity_values_fail_closed_without_the_feature() {
    use openbim_ifcx::layers::IntegrityError;

    let mut resolver = resolver(&[("b", &[])]);
    resolver.insert(
        "main",
        layer("main", &[], "main").replace(
            r#""imports":[]"#,
            r#""imports":[{"uri":"b","integrity":"sha256-AAAA"}]"#,
        ),
    );
    let err = LayerStackBuilder::new(resolver).build("main").unwrap_err();
    assert!(matches!(
        err,
        LayerError::Integrity {
            source: IntegrityError::Unavailable { .. },
            ..
        }
    ));
}

#[cfg(feature = "fs")]
mod fs {
    use std::path::{Path, PathBuf};

    use openbim_ifcx::layers::{FsError, FsResolver, LayerError, LayerStackBuilder};

    fn fixture(name: &str) -> String {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "tests",
            "fixtures",
            "layers",
            name,
        ]
        .iter()
        .collect();
        path.into_os_string().into_string().unwrap()
    }

    fn file_name(key: &str) -> &str {
        Path::new(key).file_name().unwrap().to_str().unwrap()
    }

    #[cfg(feature = "integrity")]
    #[test]
    fn three_file_chain_loads_in_order_and_federates() {
        let stack = LayerStackBuilder::new(FsResolver::new())
            .build(&fixture("chain/main.ifcx"))
            .unwrap();
        let names: Vec<_> = stack.keys().map(file_name).collect();
        assert_eq!(names, ["base.ifcx", "mid.ifcx", "main.ifcx"]);
        assert!(stack.layers()[0].key().ends_with("sub/base.ifcx"));

        let federated = stack.federate();
        assert_eq!(federated.header, stack.main().file().header);
        assert_eq!(
            federated.schemas.keys().collect::<Vec<_>>(),
            [
                "demo::fire",
                "demo::height",
                "demo::thickness",
                "demo::name"
            ]
        );
        // Position from `base`, value from `main`, the last layer defining it.
        assert_eq!(
            federated.schemas["demo::fire"].value.data_type.as_str(),
            "String"
        );
        let paths: Vec<_> = federated.data.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(paths, ["wall", "storey", "wall", "storey", "wall", "site"]);

        let nodes = openbim_ifcx::flatten(&federated.data);
        let wall = &nodes["wall"].attributes;
        assert_eq!(*wall["demo::fire"], "EI30", "the main layer wins");
        assert_eq!(*wall["demo::name"], "Wall 1");
        assert_eq!(*wall["demo::thickness"], 0.2);
        assert_eq!(*wall["demo::height"], 3.0);
        // `base` defines `Door`, and `mid`, which imports it, deletes it.
        assert_eq!(nodes["storey"].children["Door"], None);
        assert_eq!(nodes["storey"].children["Wall"].as_deref(), Some("wall"));
    }

    #[cfg(feature = "integrity")]
    #[test]
    fn a_chain_validates_with_its_imports_and_build_all_takes_paths() {
        let main = fixture("chain/main.ifcx");
        let stack = LayerStackBuilder::new(FsResolver::new())
            .build(&main)
            .unwrap();
        assert_eq!(stack.validate(), Ok(()));

        // Naming `base` before `main` places it first; `mid` follows it
        // without loading it again, and `main` comes last and wins.
        let stack = LayerStackBuilder::new(FsResolver::new())
            .build_all([fixture("chain/sub/base.ifcx"), main.clone()])
            .unwrap();
        let names: Vec<_> = stack.keys().map(file_name).collect();
        assert_eq!(names, ["base.ifcx", "mid.ifcx", "main.ifcx"]);
        assert_eq!(stack.validate(), Ok(()));

        // Naming `base` after `main` cannot move it: `mid` imports it, so it
        // was placed before `mid`, and `main` stays the strongest.
        let stack = LayerStackBuilder::new(FsResolver::new())
            .build_all([main, fixture("chain/sub/base.ifcx")])
            .unwrap();
        let names: Vec<_> = stack.keys().map(file_name).collect();
        assert_eq!(names, ["base.ifcx", "mid.ifcx", "main.ifcx"]);
    }

    #[test]
    fn cycle_is_rejected() {
        let err = LayerStackBuilder::new(FsResolver::new())
            .build(&fixture("cycle/a.ifcx"))
            .unwrap_err();
        let LayerError::Cycle { chain } = &err else {
            panic!("{err}");
        };
        let names: Vec<_> = chain.iter().map(|k| file_name(k)).collect();
        assert_eq!(names, ["a.ifcx", "b.ifcx", "a.ifcx"]);

        let stack = LayerStackBuilder::new(FsResolver::new())
            .allow_cycles(true)
            .build(&fixture("cycle/a.ifcx"))
            .unwrap();
        let names: Vec<_> = stack.keys().map(file_name).collect();
        assert_eq!(names, ["b.ifcx", "a.ifcx"]);
    }

    #[test]
    fn missing_layer_is_reported() {
        let err = LayerStackBuilder::new(FsResolver::new())
            .build(&fixture("missing/main.ifcx"))
            .unwrap_err();
        assert!(
            matches!(&err, LayerError::Missing { uri, importer: Some(importer) }
                if uri == "absent.ifcx" && importer.ends_with("main.ifcx")),
            "{err}"
        );
    }

    #[cfg(feature = "integrity")]
    #[test]
    fn integrity_mismatch_is_rejected() {
        use openbim_ifcx::layers::IntegrityError;

        let err = LayerStackBuilder::new(FsResolver::new())
            .build(&fixture("integrity-mismatch/main.ifcx"))
            .unwrap_err();
        let LayerError::Integrity { uri, source, .. } = &err else {
            panic!("{err}");
        };
        assert_eq!(uri, "../chain/sub/base.ifcx");
        let IntegrityError::Mismatch { actual, .. } = source else {
            panic!("{source}");
        };
        // The digest `mid.ifcx` records for the same file.
        assert_eq!(
            actual,
            "sha256-banE308tp+Ekaz+sBmsEBvCEfEgy80oUURH2xS80g+8="
        );
    }

    #[test]
    fn unmapped_schemes_are_unsupported_and_prefixes_map_to_directories() {
        let mut resolver = FsResolver::new();
        let mut builder = LayerStackBuilder::new(&mut resolver);
        let err = builder.build("https://example.org/a.ifcx").unwrap_err();
        assert!(matches!(
            err,
            LayerError::Resolver {
                source: FsError::UnsupportedUri(_),
                ..
            }
        ));

        let dir = fixture("chain");
        let mut builder =
            LayerStackBuilder::new(FsResolver::new().map_prefix("https://example.org/", dir));
        let stack = builder.build("https://example.org/sub/base.ifcx").unwrap();
        assert_eq!(file_name(stack.main().key()), "base.ifcx");
    }
}
