"""Read, write, validate and compose IFC5 / IFCX files.

Python bindings for the ``openbim-ifcx`` Rust crate. An :class:`IfcxFile`
holds one file and writes it back losslessly; :func:`compose` and
:func:`validate` take layers, weakest first, and return plain dicts and
lists; :func:`export_glb` writes them as a binary glTF file. Every failure raises :class:`IfcxError` with a stable ``code``.

>>> from openbim_ifcx import IfcxFile, compose
>>> file = IfcxFile.parse(open("model.ifcx", "rb").read())  # doctest: +SKIP
>>> tree = compose([open("model.ifcx", "rb").read()])  # doctest: +SKIP
"""

from __future__ import annotations

import json
from typing import Any, Dict, Iterable, List, Mapping, Optional, Union

from ._native import IfcxError, NativeFile
from ._native import compose_json as _compose_json
from ._native import export_glb as _export_glb
from ._native import validate_json as _validate_json

__all__ = ["IfcxError", "IfcxFile", "Input", "compose", "export_glb", "validate"]

#: An IFCX file as JSON text or as its UTF-8 bytes.
Input = Union[str, bytes, bytearray, memoryview]


def _bytes(value: Input, what: str) -> bytes:
    if isinstance(value, str):
        return value.encode("utf-8")
    if isinstance(value, (bytes, bytearray, memoryview)):
        return bytes(value)
    error = IfcxError(f"invalid argument: {what} must be str or bytes, not {type(value).__name__}")
    error.code = "invalid-argument"
    raise error


def _layers(layers: Union[Input, Iterable[Input]]) -> List[bytes]:
    if isinstance(layers, (str, bytes, bytearray, memoryview)):
        return [_bytes(layers, "layers")]
    try:
        items = list(layers)
    except TypeError:
        return [_bytes(layers, "layers")]  # type: ignore[arg-type]
    return [_bytes(layer, f"layers[{index}]") for index, layer in enumerate(items)]


def _imports(imports: Optional[Mapping[str, Input]]) -> Optional[Dict[str, bytes]]:
    if imports is None:
        return None
    if not isinstance(imports, Mapping):
        error = IfcxError("invalid argument: imports must be a mapping from uri to file")
        error.code = "invalid-argument"
        raise error
    out = {}
    for uri, file in imports.items():
        if not isinstance(uri, str):
            error = IfcxError("invalid argument: imports keys must be str")
            error.code = "invalid-argument"
            raise error
        out[uri] = _bytes(file, f"imports[{uri!r}]")
    return out


class IfcxFile:
    """One IFCX file.

    Writing it back with :meth:`write` is lossless: key order, fields the
    draft does not define, ``null`` deletions and every number stay as read.
    """

    __slots__ = ("_native",)

    def __init__(self) -> None:
        raise TypeError("use IfcxFile.parse(data)")

    @classmethod
    def parse(cls, data: Input) -> "IfcxFile":
        """Read an IFCX file from its JSON text or UTF-8 bytes.

        Raises :class:`IfcxError` with ``code == "read"`` for invalid JSON
        or JSON that is not an IFCX file; the message gives line and column.
        """
        file = cls.__new__(cls)
        file._native = NativeFile.parse(_bytes(data, "data"))
        return file

    def write(self, pretty: bool = False) -> str:
        """The file as JSON text; ``pretty`` indents it by two spaces."""
        return self._native.write(pretty)

    def to_dict(self) -> Dict[str, Any]:
        """The file as plain dicts and lists, in file order."""
        return json.loads(self._native.write(False))

    @property
    def header(self) -> Dict[str, Any]:
        """The ``header`` object."""
        return json.loads(self._native.header_json())

    @property
    def node_count(self) -> int:
        """Number of entries in ``data``; several may share a path."""
        return self._native.node_count

    def validate(self) -> Dict[str, Any]:
        """Check every attribute against this file's own ``schemas``.

        Returns ``{"valid": bool, "failures": [...]}``; see :func:`validate`.
        Imports are not resolved; use :func:`validate` to include them.
        """
        return json.loads(self._native.validate_json())

    def __repr__(self) -> str:
        return f"<IfcxFile {self.header.get('id')!r}, {self.node_count} nodes>"


def compose(
    layers: Union[Input, Iterable[Input]],
    imports: Optional[Mapping[str, Input]] = None,
) -> Dict[str, Any]:
    """Compose layers, weakest first, into a tree of plain dicts.

    Returns the artificial root (path ``""``) over every root node; each
    node is ``{"path", "attributes", "children"}`` with ``children`` keyed by
    child name. The last layer's opinions win.

    Without ``imports``, imports are not resolved: the layers' schemas and
    data are concatenated in order. With ``imports`` (a mapping from the
    exact import ``uri`` to a file, possibly empty), the layers become the
    imports of a synthetic main layer, as upstream's ``ifcx compose`` builds
    it, and every import must be supplied. In upstream order an import
    overrides the layer that imports it.

    Raises :class:`IfcxError` with ``code`` ``read``, ``layer``, ``compose``
    or ``invalid-argument``.
    """
    return json.loads(_compose_json(_layers(layers), _imports(imports)))


def validate(
    layers: Union[Input, Iterable[Input]],
    imports: Optional[Mapping[str, Input]] = None,
) -> Dict[str, Any]:
    """Check the attributes of the layers against the schemas of every layer.

    Layers and ``imports`` are combined as in :func:`compose`, and nodes
    sharing a path are merged first. Returns ``{"valid": bool, "failures":
    [...]}``, each failure ``{"node", "attribute", "pointer", "kind",
    "message"}``: the node path, the attribute id, an RFC 6901 pointer into
    the value, a stable ``kind`` such as ``missing-schema`` or
    ``type-mismatch``, and a human message.
    """
    return json.loads(_validate_json(_layers(layers), _imports(imports)))


def export_glb(
    layers: Union[Input, Iterable[Input]],
    imports: Optional[Mapping[str, Input]] = None,
    *,
    y_up: bool = True,
    origin_on_root: bool = True,
) -> bytes:
    """Compose layers, weakest first, and write them as a binary glTF 2.0 file.

    The render scene of the composed tree: transformed, instanced meshes,
    lines and points with their materials, one glTF node per instance named
    by its IFCX path. Layers and ``imports`` are combined as in
    :func:`compose`. ``y_up`` rotates IFCX's Z-up axes to glTF's Y-up;
    ``origin_on_root=False`` centres the model on the glTF origin and only
    records the origin in the root's ``extras.ifcxOrigin``. Geometry values
    that do not decode are left out.

    Raises :class:`IfcxError` with ``code`` ``read``, ``layer``, ``compose``,
    ``glb`` or ``invalid-argument``.
    """
    return _export_glb(_layers(layers), _imports(imports), bool(y_up), bool(origin_on_root))
