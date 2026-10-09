# openbim-ifcx (Python)

Read, write, validate and compose **IFC5 / IFCX** files, and export them as
GLB, from Python. A thin
binding over the [`openbim-ifcx`](https://crates.io/crates/openbim-ifcx)
Rust crate, from the `openbim-ifcx-py` crate in
[openbimrs/ifcx](https://github.com/openbimrs/ifcx): one abi3 wheel for
CPython 3.9 and later.

The [documentation](https://openbimrs.github.io/ifcx/) has a
[Python guide](https://openbimrs.github.io/ifcx/guide/python) and the
[API reference](https://openbimrs.github.io/ifcx/reference/crates/openbim-ifcx-py);
the [viewer](https://openbimrs.github.io/ifcx/viewer/) shows an IFCX file
in the browser.

Targets the `ifcx_alpha` draft of buildingSMART's IFC5. IFCX is still a
moving draft; see the
[capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md).

```sh
pip install openbim-ifcx
```

## Example

```python
from pathlib import Path
from openbim_ifcx import IfcxFile, compose, export_glb

model = Path("model.ifcx").read_bytes()
file = IfcxFile.parse(model)              # str or bytes
print(file.header["id"], file.node_count)

report = file.validate()
for f in report["failures"]:
    print(f["node"], f["attribute"], f["pointer"], f["kind"], f["message"])
Path("copy.ifcx").write_text(file.write())  # unchanged from the input

# Layers weakest first: the last layer's opinions win.
tree = compose([model, Path("overlay.ifcx").read_bytes()])
print(list(tree["children"]))              # the root nodes

# Any glTF viewer opens the result.
Path("model.glb").write_bytes(export_glb([model, Path("overlay.ifcx").read_bytes()]))
```

## API

| Call | Returns |
| --- | --- |
| `IfcxFile.parse(data)` | an `IfcxFile`; `data` is JSON text or UTF-8 bytes |
| `file.write(pretty=False)` | the file as JSON text, lossless |
| `file.to_dict()` | the file as plain dicts and lists |
| `file.header` | the `header` dict |
| `file.node_count` | number of entries in `data` |
| `file.validate()` | a validation report against the file's own `schemas` |
| `compose(layers, imports=None)` | the composed tree as nested dicts |
| `validate(layers, imports=None)` | a validation report over all layers and resolved imports |
| `export_glb(layers, imports=None, *, y_up=True, origin_on_root=True)` | the composed model as binary glTF 2.0 `bytes` |

`layers` is one file or an iterable of files, weakest first. The package
ships type hints (`py.typed`).

**Lossless round trip.** `write()` keeps key order, fields the draft does
not define, `null` deletions and every number exactly as read.

**Composed tree.** `compose` returns the artificial root (path `""`) over
every root node. Each node is `{"path", "attributes", "children"}`, the
shape of upstream's `PostCompositionNode`, with `children` keyed by child
name. Nodes shared through inheritance appear once per place they are used.

**Imports.** Without `imports`, imports are not resolved: the layers'
schemas and data are concatenated in order, as upstream's `Federate` does.
With `imports` (a mapping from the exact import `uri` to a file, possibly
empty), the layers become the imports of a synthetic main layer, as
upstream's `ifcx compose` command builds it, and every import must be
supplied. `integrity` values are checked against the supplied bytes. A
layer overrides the layers it imports, as agreed upstream in
buildingSMART/IFC5-development#144, and a later layer overrides both. Nothing is fetched from the network or the
filesystem.

**GLB export.** `export_glb` writes the render scene of the composed tree:
transformed, instanced meshes, lines and points with their materials, one
glTF node per instance named by its IFCX path. The root node turns IFCX's
Z-up into glTF's Y-up (`y_up=False` keeps Z-up) and carries the scene
origin, so georeferenced models keep their coordinates
(`origin_on_root=False` centres the model on the glTF origin instead).
Geometry values that do not decode are left out.

**Validation report.** `{"valid": bool, "failures": [...]}`, each failure
`{"node", "attribute", "pointer", "kind", "message"}`: the node path, the
attribute id, an RFC 6901 pointer into the value (`""` for the value
itself), a stable `kind` such as `missing-schema`, `type-mismatch`,
`not-an-integer`, `not-an-option`, `missing-key`, `too-few-elements` or
`too-many-elements`, and a human message.

## Errors

Every failure raises `IfcxError` with a stable `code`:

| `code` | Meaning |
| --- | --- |
| `read` | not an IFCX file: invalid JSON or the wrong shape; the message gives line and column |
| `write` | the file could not be written |
| `layer` | imports could not be resolved: a missing file, an import cycle, an `integrity` mismatch |
| `compose` | a reference cycle, or a reference to a node no layer defines |
| `invalid-argument` | an argument of the wrong type, or no layers |
| `glb` | the scene could not be written as GLB (over 4 GiB) |

The codes are shared with the JavaScript binding (`@openbim/ifcx`); a code
is never renamed or reused.

## Threads

Parsing, composing, validating and GLB export release the GIL. An `IfcxFile` is
immutable and can be shared between threads.

## Build from source

```sh
uv venv && . .venv/bin/activate
uv pip install maturin
maturin develop --release     # or: scripts/check-python.sh to build + test
```

## License

MIT, like the rest of `openbimrs/ifcx`.
