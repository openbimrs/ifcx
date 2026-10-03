#!/usr/bin/env python3
"""Release a single crate without touching its siblings.

Ported from openbimrs/ifc. Every crate in this workspace is versioned and
released on its own, and `^0.2.0` already accepts 0.2.1, so dependents pick
a compatible release up without being rebuilt.

This tool answers the one question that decides the blast radius:

    is this bump compatible, or breaking?

A compatible bump (patch/minor on 0.x, i.e. 0.2.0 -> 0.2.1) publishes the
crate alone. A breaking bump (0.2.x -> 0.3.0) invalidates every dependents'
requirement, so each dependent must have its requirement edited and be
republished in turn -- the tool lists exactly which, and refuses to publish
until they are dealt with.

Usage:
    scripts/release-crate.py <crate>                   # current/published versions
    scripts/release-crate.py <crate> --set 0.2.1       # dry run: what would this cost?
    scripts/release-crate.py <crate> --set 0.2.1 --apply  # bump manifests+changelog
                                                       # (incl. the npm/PyPI manifest)
    scripts/release-crate.py <crate> --publish         # tag; CI publishes
    scripts/release-crate.py <crate> --publish --local # publish from here
    scripts/release-crate.py <tag> --plan              # registries for a tag
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    return json.loads(out)


def dependents(target: str) -> list[str]:
    """Workspace crates depending on `target`, by any dependency kind.

    Dev- and build-dependencies count: cargo resolves them when packaging,
    so a requirement they carry can block a publish just as a normal one can.
    """
    found = []
    for pkg in metadata()["packages"]:
        for dep in pkg["dependencies"]:
            if dep["name"] == target:
                found.append(pkg["name"])
                break
    return sorted(found)


def requirement_of(dependent: str, target: str) -> str | None:
    """The version requirement `dependent` places on `target`."""
    for pkg in metadata()["packages"]:
        if pkg["name"] != dependent:
            continue
        for dep in pkg["dependencies"]:
            if dep["name"] == target:
                return dep["req"]
    return None


def parse_version(text: str) -> tuple[int, int, int]:
    parts = text.split("-")[0].split(".")
    nums = [int(p) if p.isdigit() else 0 for p in parts]
    while len(nums) < 3:
        nums.append(0)
    return tuple(nums[:3])


def is_breaking(old: str, new: str) -> bool:
    """Cargo compatibility for 0.x: the MINOR field is the breaking axis.

    Below 1.0, `^0.2.0` means `>=0.2.0, <0.3.0`. So 0.2.x is compatible and
    0.3.0 is not. At or above 1.0 the major field takes that role.
    """
    o, n = parse_version(old), parse_version(new)
    if o[0] != n[0]:
        return True
    if o[0] == 0:
        if o[1] != n[1]:
            return True
        # 0.0.x is the degenerate case: `^0.0.1` means `>=0.0.1, <0.0.2`,
        # so even a patch bump is breaking.
        if o[1] == 0:
            return o[2] != n[2]
        return False
    return False


def current_version(crate: str) -> str:
    for pkg in metadata()["packages"]:
        if pkg["name"] == crate:
            return pkg["version"]
    raise SystemExit(f"not a workspace member: {crate}")


def crate_dir(crate: str) -> Path:
    """The directory holding `crate`'s Cargo.toml, as cargo reports it."""
    for pkg in metadata()["packages"]:
        if pkg["name"] == crate:
            return Path(pkg["manifest_path"]).parent
    raise SystemExit(f"not a workspace member: {crate}")


def published_versions(crate: str) -> list[str]:
    """Versions live on crates.io, via the sparse index.

    The index is used rather than the API because the API rejects unusual
    user agents with 403, which is indistinguishable from "crate absent".
    """
    name = crate.lower()
    if len(name) <= 2:
        path = f"{len(name)}/{name}"
    elif len(name) == 3:
        path = f"3/{name[0]}/{name}"
    else:
        path = f"{name[:2]}/{name[2:4]}/{name}"
    url = f"https://index.crates.io/{path}"
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            body = response.read().decode()
    except Exception:
        return []
    out = []
    for line in body.splitlines():
        if line.strip():
            out.append(json.loads(line)["vers"])
    return out


def impact(crate: str, new: str | None) -> int:
    old = current_version(crate)
    deps = dependents(crate)
    print(f"crate            {crate}")
    print(f"current version  {old}")
    live = published_versions(crate)
    latest = live[-1] if live else "(none)"
    print(f"published        {latest}")
    print(f"dependents       {len(deps)}" + (f"  {deps}" if deps else ""))
    if new is None:
        return 0
    breaking = is_breaking(old, new)
    kind = "BREAKING" if breaking else "compatible"
    print(f"proposed         {new}   ({kind})")
    print()
    if not breaking:
        print(f"publish cost: 1 crate ({crate}).")
        for d in deps:
            req = requirement_of(d, crate)
            print(f"  {d} requires {req} -- satisfied by {new}, no action needed")
        return 0
    print(f"publish cost: {1 + len(deps)} crates.")
    print("each dependent needs its requirement edited and a release of its own:")
    for d in deps:
        req = requirement_of(d, crate)
        print(f"  {d} requires {req} -- REJECTS {new}, must be updated")
    return 0




def today() -> str:
    import datetime
    return datetime.date.today().isoformat()


def lift_workspace_requirement(crate: str, new: str) -> None:
    """Set the root `[workspace.dependencies]` requirement on `crate` to `new`.

    Members inherit with `crate.workspace = true`, so the root entry is the
    only requirement most dependents carry. Editing only member manifests
    matches nothing there and leaves the breaking bump unrequired.
    """
    root = ROOT / "Cargo.toml"
    text = root.read_text(encoding="utf-8")
    updated, count = re.subn(
        r'(?m)^(%s\s*=\s*\{[^}\n]*version\s*=\s*)"[^"]+"' % re.escape(crate),
        r'\g<1>"%s"' % new, text, count=1)
    if count == 1 and updated != text:
        root.write_text(updated, encoding="utf-8")
        print(f"  updated workspace requirement on {crate} to {new}")


def apply_bump(crate: str, new: str) -> None:
    """Write the new version into the manifest and open a changelog section."""
    manifest = crate_dir(crate) / "Cargo.toml"
    text = manifest.read_text(encoding="utf-8")
    if re.search(r"(?m)^version\.workspace\s*=\s*true", text):
        raise SystemExit(
            f"{crate} inherits version.workspace: it cannot be released")
    old = current_version(crate)
    updated, count = re.subn(
        r"(?m)^version\s*=\s*\"%s\"" % re.escape(old),
        'version = "%s"' % new, text, count=1)
    if count != 1:
        raise SystemExit(f"could not rewrite version in {manifest}")
    registry_manifest = registry_manifest_bump(crate, old, new)
    manifest.write_text(updated, encoding="utf-8")
    if registry_manifest is not None:
        path, text = registry_manifest
        path.write_text(text, encoding="utf-8")
        print(f"  updated {path.relative_to(ROOT)} to {new}")

    # Sibling manifests requiring this crate need their requirement lifted
    # only when the bump is breaking; a compatible one is already accepted.
    if is_breaking(old, new):
        lift_workspace_requirement(crate, new)
        for dep in dependents(crate):
            dm = crate_dir(dep) / "Cargo.toml"
            dt = dm.read_text(encoding="utf-8")
            nt = re.sub(
                r"(%s\s*=\s*\{[^}]*version\s*=\s*)\"[^\"]+\"" % re.escape(crate),
                r'\1"%s"' % new, dt)
            nt = re.sub(
                r"(?m)^(%s\s*=\s*)\"[^\"]+\"$" % re.escape(crate),
                r'\1"%s"' % new, nt)
            if nt != dt:
                dm.write_text(nt, encoding="utf-8")
                print(f"  updated requirement in {dep}")

    # CI builds with --locked, so the lock file must carry the new version.
    subprocess.run(["cargo", "update", "--workspace", "--quiet"],
        cwd=ROOT, check=True)

    changelog = crate_dir(crate) / "CHANGELOG.md"
    ct = changelog.read_text(encoding="utf-8")
    anchor = "## [Unreleased]\n"
    if anchor not in ct:
        raise SystemExit(f"no [Unreleased] section in {changelog}")
    head, _, tail = ct.partition(anchor)
    ct = head + anchor + "\n## [%s] - %s\n" % (new, today()) + tail
    ct = ct.replace(
        "[Unreleased]: https://github.com/openbimrs/ifcx/compare/%s-v%s...HEAD"
        % (crate, old),
        "[Unreleased]: https://github.com/openbimrs/ifcx/compare/%s-v%s...HEAD\n"
        "[%s]: https://github.com/openbimrs/ifcx/releases/tag/%s-v%s"
        % (crate, new, new, crate, new))
    changelog.write_text(ct, encoding="utf-8")
    print(f"bumped {crate}: {old} -> {new}")



# Registries a crate's tag releases to, beyond crates.io. The version in each
# manifest must equal the crate's, so one tag names one release everywhere.
EXTRA_REGISTRIES = {
    "openbim-ifcx-wasm": ("npm", "crates/openbim-ifcx-wasm/npm/package.json"),
    "openbim-ifcx-py": ("pypi", "crates/openbim-ifcx-py/pyproject.toml"),
}
TAG = re.compile(r"^(?P<crate>[a-z0-9][a-z0-9-]*)-v(?P<version>\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?)$")


def registry_manifest_bump(crate: str, old: str, new: str) -> tuple[Path, str] | None:
    """The npm/PyPI manifest of `crate` rewritten from `old` to `new`.

    `plan` refuses a tag whose registry manifest disagrees with Cargo.toml,
    so a bump that left it behind produced a release that could not be
    tagged. Only the version line is rewritten, so formatting survives.
    Returned rather than written, so a refusal here leaves every file as it
    was.
    """
    if crate not in EXTRA_REGISTRIES:
        return None
    _, relative = EXTRA_REGISTRIES[crate]
    path = ROOT / relative
    declared = manifest_version(path)
    if declared != old:
        raise SystemExit(
            f"{relative} says {declared} but Cargo.toml says {old}; "
            "bring them in step before bumping")
    text = path.read_text(encoding="utf-8")
    if path.suffix == ".json":
        pattern = r'("version"\s*:\s*)"%s"' % re.escape(old)
    else:
        pattern = r'(?m)^(version = )"%s"' % re.escape(old)
    updated, count = re.subn(pattern, r'\g<1>"%s"' % new, text, count=1)
    if count != 1:
        raise SystemExit(f"could not rewrite version in {relative}")
    return path, updated


def manifest_version(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    if path.suffix == ".json":
        return json.loads(text)["version"]
    found = re.search(r'^version = "([^"]+)"', text, re.MULTILINE)
    if not found:
        raise SystemExit(f"no version in {path}")
    return found.group(1)


def plan(tag: str) -> int:
    """Resolve a release tag to its registries, as GitHub Actions outputs.

    Refuses a tag that disagrees with any manifest: publishing a version
    the tree does not declare is how registries end up out of step.
    """
    match = TAG.match(tag)
    if not match:
        print(f"not a release tag: {tag}", file=sys.stderr)
        return 1
    crate, version = match["crate"], match["version"]
    package = next((p for p in metadata()["packages"] if p["name"] == crate), None)
    if package is None:
        print(f"{tag}: {crate} is not a workspace member", file=sys.stderr)
        return 1
    if package["version"] != version:
        print(f"{tag}: Cargo.toml says {package['version']}", file=sys.stderr)
        return 1
    targets = {"crates_io": package.get("publish") != [], "npm": False, "pypi": False}
    if crate in EXTRA_REGISTRIES:
        registry, manifest = EXTRA_REGISTRIES[crate]
        declared = manifest_version(ROOT / manifest)
        if declared != version:
            print(f"{tag}: {manifest} says {declared}", file=sys.stderr)
            return 1
        targets[registry] = True
    if not any(targets.values()):
        print(f"{tag}: {crate} publishes nowhere", file=sys.stderr)
        return 1
    print(f"crate={crate}")
    print(f"version={version}")
    for key, value in targets.items():
        print(f"{key}={'true' if value else 'false'}")
    return 0


def publish_here(crate: str) -> int:
    """Publish the checked-out tree to crates.io, for the Release workflow.

    No tagging and no worktree: CI has already checked out the tag. A version
    that is already live is skipped, so a re-run after a partial failure
    finishes the release instead of failing on the part that succeeded.
    """
    version = current_version(crate)
    if version in published_versions(crate):
        print(f"{crate} {version} is already live; nothing to do")
        return 0
    result = subprocess.run(["cargo", "publish", "-p", crate, "--locked"], cwd=ROOT)
    if result.returncode == 0:
        print(f"published {crate} {version}")
    return result.returncode


def publish(crate: str, local: bool) -> int:
    """Verify and tag one crate, then let the release workflow publish it.

    Pushing `<crate>-v<version>` triggers `.github/workflows/release.yml`,
    which gates the tagged commit and publishes to every registry the crate
    targets (crates.io, and npm or PyPI for the bindings). `local` is the
    fallback when CI cannot publish: it runs `cargo publish` here instead,
    from a detached worktree at the tag so the VCS SHA embedded in the
    .crate is deterministic and the working tree stays free for other work.
    """
    version = current_version(crate)
    if local and version in published_versions(crate):
        print(f"{crate} {version} is already live; nothing to do")
        return 0
    dirty = subprocess.run(["git", "status", "--porcelain"],
        cwd=ROOT, capture_output=True, text=True).stdout.strip()
    if dirty:
        print("working tree is dirty; commit before publishing", file=sys.stderr)
        return 1
    tag = f"{crate}-v{version}"
    existing = subprocess.run(["git", "tag", "-l", tag],
        cwd=ROOT, capture_output=True, text=True).stdout.strip()
    if not existing:
        subprocess.run(["git", "tag", "-a", tag, "-m", f"{crate} {version}"],
            cwd=ROOT, check=True)
        print(f"tagged {tag}")
    subprocess.run(["git", "push", "origin", tag], cwd=ROOT, check=True)
    if not local:
        print(f"pushed {tag}; the Release workflow publishes it:")
        print("  https://github.com/openbimrs/ifcx/actions/workflows/release.yml")
        return 0
    worktree = Path("/tmp") / f"pub-{crate}-{version}"
    subprocess.run(["git", "worktree", "remove", str(worktree), "--force"],
        cwd=ROOT, capture_output=True)
    subprocess.run(["git", "worktree", "add", "--detach", str(worktree), tag],
        cwd=ROOT, check=True)
    try:
        result = subprocess.run(["cargo", "publish", "-p", crate, "--locked"],
            cwd=worktree)
        if result.returncode != 0:
            return result.returncode
    finally:
        subprocess.run(["git", "worktree", "remove", str(worktree), "--force"],
            cwd=ROOT, capture_output=True)
    print(f"published {crate} {version}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("crate")
    parser.add_argument("--set", dest="new_version",
        help="version to bump to; reports impact, writes only with --apply")
    parser.add_argument("--apply", action="store_true",
        help="with --set, write the manifest and changelog changes")
    parser.add_argument("--publish", action="store_true",
        help="tag the crate at its committed version and push the tag; "
             "the Release workflow publishes it")
    parser.add_argument("--local", action="store_true",
        help="with --publish, run cargo publish here instead of in CI")
    parser.add_argument("--plan", action="store_true",
        help="treat CRATE as a release tag and print its registry targets")
    parser.add_argument("--publish-here", action="store_true",
        help="publish the checked-out tree to crates.io, skipping a live "
             "version (used by the Release workflow)")
    args = parser.parse_args()

    if args.plan:
        return plan(args.crate)
    if args.publish_here:
        return publish_here(args.crate)
    if args.publish:
        return publish(args.crate, args.local)

    code = impact(args.crate, args.new_version)
    if code != 0:
        return code
    if args.new_version:
        print()
        if args.apply:
            apply_bump(args.crate, args.new_version)
            print()
            print("next: review the changelog entry, commit, then")
            print(f"      python3 scripts/release-crate.py {args.crate} --publish")
        else:
            print("dry run: pass --apply to write the bump")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
