#!/usr/bin/env python3
"""Fail if the source tree or a built artifact carries buildingSMART material.

buildingSMART/IFC5-development publishes no license, so none of its files,
examples or texts may be committed here or published on the docs site
(AGENTS.md, Boundaries). Ported from openbimrs/ifc's check-leakage.py.

Usage:
    scripts/check-leakage.py                       # the tracked source tree
    scripts/check-leakage.py DIR...                # built artifacts, e.g. the site
    IFCX_UPSTREAM_DIR=../IFC5-development scripts/check-leakage.py DIR...

Always checked:
- no PDF, XSD or TypeSpec (`.tsp`) file, and no `references/`, `.upstream/`
  or `.ifcx-mirror/` directory;
- every IFCX file (JSON with `header.ifcxVersion`) is this repository's own:
  `header.author` is "openbimrs contributors" and `header.id` starts with
  "openbimrs/".

With an upstream checkout (`IFCX_UPSTREAM_DIR` or `--upstream DIR`), also:
- no file is byte-identical to an upstream file;
- no node path of an upstream example (their UUIDs) occurs anywhere;
- no line of upstream prose (README, FAQ, schema text) occurs anywhere.
"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import os
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_SUFFIXES = {".pdf", ".xsd", ".tsp"}
FORBIDDEN_DIRS = {"references", ".upstream", ".ifcx-mirror"}
OWN_AUTHOR = "openbimrs contributors"
OWN_ID_PREFIX = "openbimrs/"
UUID = re.compile(rb"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
# Upstream files that are not buildingSMART content: tooling configuration
# and lockfiles whose bytes can legitimately coincide.
UPSTREAM_SKIP_DIRS = {".git", "node_modules"}
UPSTREAM_SKIP_NAMES = {"package-lock.json", ".gitignore", "LICENSE"}
PROSE_SUFFIXES = {".md", ".tsp"}
PROSE_MIN_LENGTH = 70

problems: list[str] = []


def fail(message: str) -> None:
    problems.append(message)


def check_name(name: str, origin: str) -> None:
    path = PurePosixPath(name.replace("\\", "/"))
    if path.suffix.lower() in FORBIDDEN_SUFFIXES:
        fail(f"{origin}: {name}: {path.suffix} files are not published here")
    for part in path.parts[:-1]:
        if part.lower() in FORBIDDEN_DIRS:
            fail(f"{origin}: {name}: {part}/ holds local reference material only")


def check_ifcx(name: str, payload: bytes, origin: str) -> None:
    """An IFCX file must be this repository's own."""
    if b"ifcxVersion" not in payload[:4096]:
        return
    try:
        header = json.loads(payload).get("header", {})
    except (ValueError, AttributeError):
        return
    if not isinstance(header, dict) or "ifcxVersion" not in header:
        return
    author, ident = header.get("author"), str(header.get("id", ""))
    if author != OWN_AUTHOR or not ident.startswith(OWN_ID_PREFIX):
        fail(
            f"{origin}: {name}: IFCX file by {author!r} (id {ident!r}); only "
            f"hand-written files by {OWN_AUTHOR!r} with an id under "
            f"{OWN_ID_PREFIX!r} may be committed or published"
        )


def check_bytes(name: str, payload: bytes, origin: str) -> None:
    if payload.lstrip()[:5] == b"%PDF-":
        fail(f"{origin}: {name}: PDF content")
    if PurePosixPath(name).suffix.lower() in {".ifcx", ".json"}:
        check_ifcx(name, payload, origin)


class Upstream:
    """What an upstream checkout would leak: file hashes, example node
    paths, and prose lines."""

    def __init__(self, root: Path) -> None:
        self.hashes: dict[str, str] = {}
        self.uuids: set[bytes] = set()
        self.prose: list[str] = []
        files = 0
        for item in root.rglob("*"):
            relative = item.relative_to(root)
            if not item.is_file() or set(relative.parts) & UPSTREAM_SKIP_DIRS:
                continue
            if item.name in UPSTREAM_SKIP_NAMES or item.stat().st_size < 64:
                continue
            files += 1
            payload = item.read_bytes()
            self.hashes.setdefault(hashlib.sha256(payload).hexdigest(), relative.as_posix())
            if item.suffix == ".ifcx":
                self.uuids.update(m.group(0).lower() for m in UUID.finditer(payload))
            if item.suffix in PROSE_SUFFIXES:
                for line in payload.decode("utf-8", "replace").splitlines():
                    line = line.strip().lstrip("#*->|` ").strip()
                    if len(line) >= PROSE_MIN_LENGTH and line.count(" ") >= 8:
                        self.prose.append(line)
        if files == 0:
            raise SystemExit(f"leakage: {root} holds no files; is it an IFC5-development checkout?")
        print(
            f"leakage: upstream {root}: {len(self.hashes)} files, {len(self.uuids)} node "
            f"paths, {len(self.prose)} prose lines"
        )

    def check(self, name: str, payload: bytes, origin: str) -> None:
        digest = hashlib.sha256(payload).hexdigest()
        if digest in self.hashes:
            fail(f"{origin}: {name}: identical to upstream {self.hashes[digest]}")
        found = {m.group(0).lower() for m in UUID.finditer(payload)} & self.uuids
        if found:
            fail(f"{origin}: {name}: contains {len(found)} node path(s) of upstream examples, e.g. {sorted(found)[0].decode()}")
        if self.prose:
            text = html.unescape(payload.decode("utf-8", "ignore"))
            for line in self.prose:
                if line in text:
                    fail(f"{origin}: {name}: contains upstream text {line[:60]!r}...")
                    break


def tracked_files() -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    )
    return [line for line in result.stdout.splitlines() if line]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("artifacts", nargs="*", type=Path,
                        help="built directories to check; none checks the source tree")
    parser.add_argument("--upstream", type=Path, default=os.environ.get("IFCX_UPSTREAM_DIR") or None,
                        help="an IFC5-development checkout (default: $IFCX_UPSTREAM_DIR)")
    args = parser.parse_args()
    upstream = Upstream(args.upstream.resolve()) if args.upstream else None

    if not args.artifacts:
        names = tracked_files()
        for name in names:
            check_name(name, "source")
            path = ROOT / name
            if path.is_file():
                payload = path.read_bytes()
                check_bytes(name, payload, "source")
                if upstream:
                    upstream.check(name, payload, "source")
        checked = f"{len(names)} source files"
    else:
        count = 0
        for artifact in args.artifacts:
            if not artifact.is_dir():
                raise SystemExit(f"leakage: {artifact} is not a directory")
            for item in sorted(artifact.rglob("*")):
                if not item.is_file():
                    continue
                count += 1
                name = item.relative_to(artifact).as_posix()
                payload = item.read_bytes()
                check_name(name, str(artifact))
                check_bytes(name, payload, str(artifact))
                if upstream:
                    upstream.check(name, payload, str(artifact))
        checked = f"{count} files in {len(args.artifacts)} artifact(s)"

    if problems:
        print("\n".join(f"leakage: {p}" for p in problems), file=sys.stderr)
        return 1
    print(f"leakage: PASS ({checked}{', compared with upstream' if upstream else ''})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
