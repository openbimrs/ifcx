# Getting started: Python

[`openbim-ifcx`](/reference/crates/openbim-ifcx-py) on PyPI is a thin
binding over the Rust crates: one abi3 wheel for CPython 3.9 and later.

```sh
pip install openbim-ifcx
```

Every snippet on this page is a region of
[`crates/openbim-ifcx-py/tests/python/test_guide.py`](https://github.com/openbimrs/ifcx/blob/main/crates/openbim-ifcx-py/tests/python/test_guide.py),
which the gate runs against the built wheel, so it works as shown.

## Read, validate and write

<<< ../../crates/openbim-ifcx-py/tests/python/test_guide.py#read{python}

Results are plain dicts and lists. A validation failure is
`{"node", "attribute", "pointer", "kind", "message"}`, with a stable `kind`
such as `missing-schema` or `type-mismatch`.

## Compose layers and export GLB

<<< ../../crates/openbim-ifcx-py/tests/python/test_guide.py#compose{python}

`export_glb` takes `y_up=False` to keep IFCX's Z-up axes and
`origin_on_root=False` to centre the model on the glTF origin.

## Imports

<<< ../../crates/openbim-ifcx-py/tests/python/test_guide.py#imports{python}

With `imports` (possibly empty) the layers become the imports of a
synthetic main layer, as upstream's `ifcx compose` builds it, and every
import must be supplied; `integrity` values are checked against the bytes.

## Errors

Every failure raises `IfcxError` with a stable `code`: `read`, `write`,
`layer`, `compose`, `invalid-argument` or `glb`. The
[reference](/reference/crates/openbim-ifcx-py) lists the whole API,
generated from the package source.
