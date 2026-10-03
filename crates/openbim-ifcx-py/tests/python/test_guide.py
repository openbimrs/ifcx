"""The code of the documentation site's Python guide (docs/guide/python.md),
run against an installed wheel by crates/openbim-ifcx-py/scripts/check-python.sh.

The guide imports the ``#region`` blocks below verbatim, so every snippet it
shows runs in the gate. Edit the code here, not on the page."""

import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path

FIXTURES = Path(__file__).resolve().parents[3] / "openbim-ifcx" / "tests" / "fixtures"
OVERLAY = {
    "header": {"id": "overlay", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
               "author": "guide", "timestamp": "2026-10-03"},
    "imports": [], "schemas": {}, "data": [],
}


class Guide(unittest.TestCase):
    def setUp(self):
        self.work = tempfile.mkdtemp(prefix="ifcx-guide-")
        self.previous = os.getcwd()
        os.chdir(self.work)
        shutil.copy(FIXTURES / "geometry-model.ifcx", "model.ifcx")
        Path("overlay.ifcx").write_text(json.dumps(OVERLAY), encoding="utf-8")

    def tearDown(self):
        os.chdir(self.previous)
        shutil.rmtree(self.work, ignore_errors=True)

    def test_read_validate_write(self):
        # #region read
        from openbim_ifcx import IfcxFile

        with open("model.ifcx", "rb") as f:
            file = IfcxFile.parse(f.read())  # bytes or str
        print(file.header["id"], file.node_count)

        report = file.validate()  # against the file's own schemas
        for failure in report["failures"]:
            print(failure["node"], failure["attribute"], failure["kind"], failure["message"])

        text = file.write(pretty=True)  # content, key order and numbers as read
        # #endregion read
        self.assertTrue(report["valid"])
        self.assertEqual(IfcxFile.parse(text).write(pretty=True), text)

    def test_compose_validate_export(self):
        # #region compose
        from pathlib import Path

        from openbim_ifcx import compose, export_glb, validate

        # Layers weakest first: the last layer's opinions win.
        layers = [Path("model.ifcx").read_bytes(), Path("overlay.ifcx").read_bytes()]

        tree = compose(layers)  # {"path", "attributes", "children"}
        print(list(tree["children"]))  # the root nodes

        report = validate(layers)

        Path("model.glb").write_bytes(export_glb(layers))  # binary glTF 2.0
        # #endregion compose
        self.assertTrue(tree["children"])
        self.assertTrue(report["valid"])
        self.assertEqual(Path("model.glb").read_bytes()[:4], b"glTF")

    def test_imports(self):
        types = {
            "header": {"id": "types", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                       "author": "guide", "timestamp": "2026-10-03"},
            "imports": [], "schemas": {}, "data": [{"path": "panel", "attributes": {}}],
        }
        Path("types.ifcx").write_text(json.dumps(types), encoding="utf-8")
        main = json.dumps({
            "header": {"id": "main", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                       "author": "guide", "timestamp": "2026-10-03"},
            "imports": [{"uri": "https://example.org/model/types.ifcx"}],
            "schemas": {}, "data": [{"path": "wall", "inherits": {"type": "panel"}}],
        })
        from openbim_ifcx import compose
        # #region imports
        # Imports are passed in, keyed by the exact import `uri`; the binding
        # never reads files or the network itself.
        imports = {"https://example.org/model/types.ifcx": Path("types.ifcx").read_bytes()}
        tree = compose([main], imports=imports)
        # #endregion imports
        self.assertIn("wall", tree["children"])


if __name__ == "__main__":
    unittest.main()
