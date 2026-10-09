"""Suite for the Python bindings, run against an installed wheel by
crates/openbim-ifcx-py/scripts/check-python.sh.

Proves the binding works from Python: read and write back the hand-written
fixtures, validate, compose layers with and without imports, and check every
refusal raises IfcxError with a stable code. The behaviour itself is tested
in openbim-ifcx-binding-core."""

import json
import struct
import threading
import unittest
from pathlib import Path

import openbim_ifcx
from openbim_ifcx import IfcxError, IfcxFile, compose, export_glb, validate

FIXTURES = Path(__file__).resolve().parents[3] / "openbim-ifcx" / "tests" / "fixtures"


def text(name):
    return (FIXTURES / name).read_text(encoding="utf-8")


def data(name):
    return (FIXTURES / name).read_bytes()


def overlay(imports=()):
    """A layer that renames the roof of geometry-model.ifcx."""
    return json.dumps({
        "header": {"id": "overlay", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                   "author": "test", "timestamp": "2026-10-03"},
        "imports": [{"uri": uri} for uri in imports],
        "schemas": {},
        "data": [{"path": "roof", "attributes": {"example::class": "Roof"}}],
    })


def roof_class(tree):
    return tree["children"]["pavilion"]["children"]["Storey"]["children"]["Roof"]["attributes"]["example::class"]


class Installed(unittest.TestCase):
    def test_the_wheel_is_imported_not_the_sources(self):
        package = Path(openbim_ifcx.__file__).resolve().parent
        self.assertNotEqual(package, Path(__file__).resolve().parents[2] / "python" / "openbim_ifcx")
        self.assertTrue(any(part in ("site-packages", "dist-packages") for part in package.parts), package)
        self.assertTrue((Path(openbim_ifcx.__file__).parent / "py.typed").exists())


class Files(unittest.TestCase):
    def test_a_file_reads_from_str_or_bytes_and_writes_back_unchanged(self):
        for name in ("minimal.ifcx", "wall-with-type.ifcx", "unknown-fields.ifcx", "geometry-model.ifcx"):
            from_text = IfcxFile.parse(text(name))
            from_bytes = IfcxFile.parse(data(name))
            self.assertEqual(from_text.write(), from_bytes.write(), name)
            self.assertEqual(IfcxFile.parse(from_text.write(pretty=True)).write(), from_text.write())
        written = IfcxFile.parse(text("unknown-fields.ifcx")).write(pretty=True)
        self.assertEqual(json.loads(written), json.loads(text("unknown-fields.ifcx")))

    def test_header_node_count_and_plain_dicts(self):
        file = IfcxFile.parse(text("geometry-model.ifcx"))
        self.assertEqual(file.header["ifcxVersion"], "ifcx_alpha")
        self.assertEqual(file.node_count, 9)
        self.assertEqual(file.to_dict(), json.loads(text("geometry-model.ifcx")))
        self.assertIn("9 nodes", repr(file))
        with self.assertRaises(TypeError):
            IfcxFile()

    def test_validation_is_a_structured_report(self):
        self.assertEqual(IfcxFile.parse(text("valid-attributes.ifcx")).validate(),
                         {"valid": True, "failures": []})
        report = IfcxFile.parse(text("invalid-attributes.ifcx")).validate()
        self.assertFalse(report["valid"])
        self.assertGreater(len(report["failures"]), 3)
        for failure in report["failures"]:
            self.assertEqual(list(failure), ["node", "attribute", "pointer", "kind", "message"])
            self.assertNotEqual(failure["kind"], "other", failure["message"])
        self.assertIn("missing-schema", {f["kind"] for f in report["failures"]})

    def test_a_file_can_be_used_from_another_thread(self):
        file = IfcxFile.parse(text("minimal.ifcx"))
        seen = []
        thread = threading.Thread(target=lambda: seen.append(file.write()))
        thread.start()
        thread.join()
        self.assertEqual(seen, [file.write()])


class Composition(unittest.TestCase):
    def test_compose_returns_the_tree_upstream_composes(self):
        tree = compose([text("occurrence-type.ifcx")])
        self.assertEqual(tree, json.loads(text("occurrence-type.composed.json")))
        self.assertEqual(compose(data("occurrence-type.ifcx")), tree, "a single layer")
        self.assertEqual(tree["path"], "")

    def test_the_last_layer_wins(self):
        model = text("geometry-model.ifcx")
        self.assertEqual(roof_class(compose([model])), "Slab")
        self.assertEqual(roof_class(compose([model, overlay()])), "Roof")
        self.assertEqual(roof_class(compose((overlay(), model))), "Slab")
        self.assertEqual(validate([model, overlay()]), {"valid": True, "failures": []})

    def test_imports_resolve_from_memory(self):
        imports = {"model.ifcx": data("geometry-model.ifcx")}
        # A layer overrides the layers it imports (buildingSMART/IFC5-development#144).
        self.assertEqual(roof_class(compose([overlay(["model.ifcx"])], imports=imports)), "Roof")
        self.assertEqual(roof_class(compose([overlay(["model.ifcx"]), overlay()], imports)), "Roof")
        self.assertTrue(validate([overlay(["model.ifcx"])], imports)["valid"])
        chain = {"mid.ifcx": data("layers/chain/mid.ifcx"),
                 "sub/base.ifcx": text("layers/chain/sub/base.ifcx")}
        self.assertTrue(validate([text("layers/chain/main.ifcx")], chain)["valid"])


def glb_json(glb):
    """The JSON chunk of a GLB file, after checking its container."""
    magic, version, length = struct.unpack_from("<4sII", glb, 0)
    assert (magic, version, length) == (b"glTF", 2, len(glb)), (magic, version, length)
    chunk_length, chunk_type = struct.unpack_from("<I4s", glb, 12)
    assert chunk_type == b"JSON"
    return json.loads(glb[20:20 + chunk_length])


class Glb(unittest.TestCase):
    def test_composed_layers_export_as_glb(self):
        model = text("geometry-model.ifcx")
        glb = export_glb(model)
        self.assertIsInstance(glb, bytes)
        nodes = glb_json(glb)["nodes"]
        self.assertIn("pavilion/Storey/Column 1/Body", [node.get("name") for node in nodes])
        self.assertIn("rotation", nodes[0], "Y-up by default")

        local = glb_json(export_glb([data("geometry-model.ifcx")], y_up=False, origin_on_root=False))
        self.assertNotIn("rotation", local["nodes"][0])
        self.assertNotIn("translation", local["nodes"][0])

        layered = glb_json(export_glb([overlay(["model.ifcx"]), overlay()], {"model.ifcx": model}))
        self.assertEqual(len(layered["nodes"]), len(nodes))
        with self.assertRaises(IfcxError) as caught:
            export_glb([text("layer-edit.ifcx")])
        self.assertEqual(caught.exception.code, "compose")


class Refusals(unittest.TestCase):
    def assertCode(self, code, call, *args, **kwargs):
        with self.assertRaises(IfcxError) as caught:
            call(*args, **kwargs)
        self.assertEqual(caught.exception.code, code, str(caught.exception))

    def test_every_failure_is_an_ifcx_error_with_a_stable_code(self):
        self.assertCode("read", IfcxFile.parse, "not json")
        self.assertCode("read", IfcxFile.parse, b"{}")
        self.assertCode("invalid-argument", IfcxFile.parse, 42)
        self.assertCode("invalid-argument", compose, [])
        self.assertCode("invalid-argument", compose, [text("minimal.ifcx"), 7])
        self.assertCode("invalid-argument", compose, 7)
        self.assertCode("invalid-argument", compose, [text("minimal.ifcx")], imports=["x"])
        self.assertCode("invalid-argument", compose, [text("minimal.ifcx")], imports={1: "x"})
        self.assertCode("read", compose, [text("minimal.ifcx"), "[]"])
        self.assertCode("compose", compose, [text("layer-edit.ifcx")])
        self.assertCode("layer", compose, [text("layers/chain/main.ifcx")], imports={})
        self.assertCode("layer", compose, [text("layers/chain/mid.ifcx")],
                        imports={"sub/base.ifcx": text("minimal.ifcx")})
        self.assertTrue(issubclass(IfcxError, Exception))


class ReadmeExample(unittest.TestCase):
    def test_read_validate_compose(self):
        model, overlay_text = text("geometry-model.ifcx"), overlay()
        # Mirrors the README example.
        file = IfcxFile.parse(model)  # str or bytes
        report = file.validate()  # {"valid": ..., "failures": [...]}
        out = file.write()  # JSON text, unchanged from the input

        # Layers weakest first: the overlay's opinions win.
        tree = compose([model, overlay_text])
        storey = tree["children"]["pavilion"]["children"]["Storey"]
        self.assertTrue(report["valid"])
        self.assertEqual(out, IfcxFile.parse(out).write())
        self.assertEqual(storey["children"]["Roof"]["attributes"]["example::class"], "Roof")


if __name__ == "__main__":
    unittest.main()
