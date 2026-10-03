#!/usr/bin/env python3
"""Fetch the imports of a buildingSMART/IFC5-development checkout.

usage: fetch-imports.py <upstream dir> <mirror dir> <flat dir>

Collects every `http(s)` import URI of the checkout's `.ifcx` files, and of
the fetched files in turn, and downloads each into

- <mirror dir>/<host>/<path>   (IFCX_IMPORTS_MIRROR, for `upstream_layers`)
- <flat dir>/<last segment>    (IFCX_IMPORTS_DIR, for `upstream_validation`)

A file already in <mirror dir> (for example from a CI cache) is used only if
the server cannot be reached; an HTTP error such as 404 removes it, so a file
that disappears from ifcx.dev shows up as unavailable. Prints one normalised
line per URI to stdout, for the drift baseline, and progress to stderr.
Exit code 0 even if some imports are unavailable: the checks that use the
mirror decide what that means.
"""

import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


def ifcx_files(root: Path):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in (".git", "node_modules"))
        for name in sorted(filenames):
            if name.endswith(".ifcx"):
                yield Path(dirpath) / name


def imports_of(data: bytes):
    try:
        doc = json.loads(data)
    except ValueError:
        return []
    uris = []
    for entry in doc.get("imports") or []:
        uri = entry.get("uri") if isinstance(entry, dict) else None
        if isinstance(uri, str) and uri.startswith(("https://", "http://")):
            uris.append(uri)
    return uris


def fetch(uri: str):
    """Returns (bytes, None), or (None, reason). Retries transient errors."""
    request = urllib.request.Request(uri, headers={"User-Agent": "openbimrs-ifcx-upstream-drift"})
    last = None
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return response.read(), None
        except urllib.error.HTTPError as e:
            if e.code < 500:
                return None, f"HTTP {e.code}"
            last = f"HTTP {e.code}"
        except (urllib.error.URLError, TimeoutError, OSError) as e:
            last = f"unreachable ({getattr(e, 'reason', e)})"
        time.sleep(2 * (attempt + 1))
    return None, last


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    upstream, mirror, flat = (Path(a) for a in sys.argv[1:])
    flat.mkdir(parents=True, exist_ok=True)

    queue = []
    for path in ifcx_files(upstream):
        queue.extend(imports_of(path.read_bytes()))
    seen, lines, flat_owner = set(), [], {}
    while queue:
        uri = queue.pop(0)
        if uri in seen:
            continue
        seen.add(uri)
        parts = urllib.parse.urlsplit(uri)
        local = mirror / parts.netloc / parts.path.lstrip("/")
        data, reason = fetch(uri)
        if data is None and reason.startswith("unreachable") and local.is_file():
            print(f"warning: {uri}: {reason}; using the cached copy", file=sys.stderr)
            data = local.read_bytes()
        if data is None:
            local.unlink(missing_ok=True)
            print(f"warning: {uri}: {reason}", file=sys.stderr)
            lines.append(f"import {uri} unavailable ({reason})")
            continue
        local.parent.mkdir(parents=True, exist_ok=True)
        local.write_bytes(data)
        last = parts.path.rsplit("/", 1)[-1]
        if last in flat_owner and flat_owner[last] != uri:
            print(f"warning: {uri} and {flat_owner[last]} share the name {last}", file=sys.stderr)
        flat_owner.setdefault(last, uri)
        if flat_owner[last] == uri:
            (flat / last).write_bytes(data)
        lines.append(f"import {uri} sha256:{hashlib.sha256(data).hexdigest()[:16]}")
        queue.extend(imports_of(data))

    unavailable = sum("unavailable" in line for line in lines)
    print(f"{len(lines)} URIs, {unavailable} unavailable")
    for line in sorted(lines):
        print(line)


if __name__ == "__main__":
    main()
