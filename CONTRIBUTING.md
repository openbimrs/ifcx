# Contributing

## Before changing code

1. Read `AGENTS.md`.
2. Keep IFCX independent of `openbimrs/ifc` internals, and never make `ifc`
   depend on this repository.
3. Name the IFCX draft revision that a capability targets.
4. Do not commit buildingSMART schemas or samples without verified
   redistribution rights.

## Verification

Run the complete gate from the repository root:

```bash
./scripts/gate.sh
```

The command must exit successfully. It runs two sections, which CI runs as
parallel jobs: `rust` (format, tests, clippy, rustdoc, packaging) and
`bindings` (the npm package built with the pinned `wasm-bindgen` CLI and
tested under Node; the Python wheel built with `maturin`, installed into a
`uv` venv, and tested). Run one with `./scripts/gate.sh rust`. The bindings
need `wasm-bindgen-cli` 0.2.128, Node, `uv` and `maturin` on `PATH`; a
missing tool fails the gate and names what to install. `IFCX_SKIP_JS=1` or
`IFCX_SKIP_PYTHON=1` skips a suite locally with a warning; CI never sets
them. Update README, rustdoc, capabilities, and
CHANGELOG together when a user-visible contract changes.

## Upstream drift

IFCX is an alpha draft. The evidence in `docs/capabilities.md` was recorded
against a pinned buildingSMART/IFC5-development commit, and
`.github/workflows/upstream-drift.yml` re-checks it against upstream's
default branch every Monday at 04:23 UTC. It runs
`scripts/upstream-drift.sh`, which fetches the `ifcx.dev` imports into an
offline mirror and runs every opt-in upstream check: round trip,
validation, composition, layer stacks, geometry decode, render scenes,
composition parity with upstream's TypeScript, and the Khronos glTF
validator. Each check's summary line (counts and failures, without
timings) is compared with `scripts/upstream-drift/baseline.txt`.

When it finds drift, a failed check or a changed summary line, it opens
an issue labelled `upstream-drift`, a sub-issue of
[#40](https://github.com/openbimrs/ifcx/issues/40). If one is already
open it updates that issue and comments, so there is never more than one.
The issue names the upstream commit and the failed checks, and links:

- the run and its `Upstream checks` job log, where each check is a
  collapsible group;
- the `upstream-drift-report` artifact, with `logs/<check>.log` per check,
  `report.md`, `summary.txt`, and `baseline.diff`.

Start from `baseline.diff` and the failed check's log, then reproduce
locally against the same commit:

```bash
git -C ../IFC5-development fetch && git -C ../IFC5-development checkout <commit>
IFCX_UPSTREAM_DIR=../IFC5-development ./scripts/upstream-drift.sh
```

Then either fix the crate, if upstream changed the draft or found a bug,
or accept the new results: update the counts and the named revision in
`docs/capabilities.md`, run `./scripts/upstream-drift.sh --update-baseline`,
and commit both. The first scheduled or dispatched run against the default
branch that passes closes the issue. Known gaps are part of the baseline;
for example, `ifcx.dev` does not serve `ifc-mat/prop@v1.0.0.ifcx`
([upstream #124](https://github.com/buildingSMART/IFC5-development/issues/124)),
so 4 examples have an unresolved import. If upstream fixes that, the
baseline differs and the issue says so.

The run summary also lists the upstream branches (draft work such as
`post-alpha` happens there) that moved, appeared, or disappeared since the
last recorded run. A moved branch alone opens no issue. The record is the
`upstream-drift-state` artifact (branch heads and the checked revision,
kept 90 days) of the latest non-dry run on `main`.

Run it by hand from Actions -> Upstream drift -> Run workflow, or:

```bash
gh workflow run upstream-drift.yml -R openbimrs/ifcx --ref main
gh workflow run upstream-drift.yml -R openbimrs/ifcx --ref main \
  -f upstream_ref=post-alpha -f dry_run=true
```

`upstream_ref` checks a branch, tag, or full commit SHA instead of the
default branch. `dry_run` only writes the run summary: no issue is opened,
updated, or closed, and no state is recorded.

## Fuzzing

`fuzz/` holds [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets
for the reader, composition, validation, and the geometry decoders. It is
its own Cargo workspace with its own `Cargo.lock`, outside the root
workspace and the MSRV gate, because libFuzzer needs a nightly toolchain;
`fuzz/rust-toolchain.toml` pins the date.

| Target | Exercises |
| --- | --- |
| `read` | `IfcxFile::from_json_slice` on arbitrary bytes; a file that reads must write and read back equal; then validate, flatten, and compose |
| `compose` | `flatten` + `compose` over IFCX JSON or structured node lists with colliding paths, cycles, diamonds, and `head/child` edits |
| `validate` | `IfcxFile::validate` with schemas that inherit each other (chains, diamonds, cycles) and nested restrictions |
| `pcd` | `PointCloud::from_pcd` on arbitrary bytes (`ascii`, `binary`, `binary_compressed`), checked against `from_pcd_base64` |
| `pcd_lzf` | the `binary_compressed` LZF decoder and column-major layout behind a fixed valid header |
| `points_base64` | `points::base64` strings and `points::array` JSON |
| `geometry` | mesh, curve, transform, presentation, and point-array decoders on one JSON value |
| `scene_glb` | `RenderScene` and `to_glb` on composed files; checks the GLB structure, alignment, index ranges, and that the JSON holds no `null` |

Install cargo-fuzz once (`cargo install cargo-fuzz`), then run a target for
a bounded time:

```bash
fuzz/run.sh --list              # target names
fuzz/run.sh pcd 600             # ten minutes
fuzz/run.sh compose 60 -jobs=4  # extra libFuzzer flags after the seconds
```

`fuzz/run.sh` builds with the pinned nightly, debug assertions (so integer
overflow panics), and ASan; reads seeds from `fuzz/seeds/<target>` and, for
targets taking IFCX files, from the hand-written fixtures in
`crates/*/tests/fixtures`; writes new inputs to `fuzz/corpus/<target>`; and
limits memory to 2 GiB and each input to 10 s, so unbounded allocation and
super-linear blow-ups are findings too. Plain
`cargo +nightly-2026-09-25 fuzz run <target>` works as well.

A crash leaves its input in `fuzz/artifacts/<target>/`. Fix it in the
library crate with a regression test in that crate's normal test suite
(never only in `fuzz/`), and add the minimized input
(`cargo fuzz tmin <target> <file>`) to `fuzz/seeds/<target>/` when it is a
useful seed. Seeds must be original work: never add buildingSMART samples.

CI runs every target in `.github/workflows/fuzz.yml`: one minute each on
pull requests that touch the fuzzed crates or `fuzz/`, ten minutes each
every Monday (keeping the corpus between weekly runs), and on demand with
`gh workflow run fuzz.yml -R openbimrs/ifcx -f seconds=1800`. That workflow
is not part of the required gate. When bumping the nightly date, change
`fuzz/rust-toolchain.toml` and the workflow together.

## Releasing a crate

Every crate is versioned and released on its own; releasing one does not
imply releasing another. Each crate keeps its changelog beside its
`Cargo.toml`.

Ask what a release would cost, then write the bump:

```bash
python3 scripts/release-crate.py openbim-ifcx --set 0.1.1          # dry run
python3 scripts/release-crate.py openbim-ifcx --set 0.1.1 --apply  # manifest + changelog
```

A compatible bump (`0.1.0` to `0.1.1`) releases the crate alone. A breaking
one (`0.1.x` to `0.2.0`) also lifts the requirement in each dependent, which
then needs its own release. Fill in the changelog, run `./scripts/gate.sh`,
and merge the bump to `main`.

Then tag and push:

```bash
python3 scripts/release-crate.py openbim-ifcx --publish
```

This refuses a dirty tree and pushes `openbim-ifcx-v0.1.1`. The tag runs
`.github/workflows/release.yml`, which checks the tag is on `main`, waits for
CI to pass on the tagged commit, and publishes to every registry the crate
targets. The gate runs once, in CI; the release does not run it again:

| Crate | Registries |
| --- | --- |
| any crate without `publish = false` (`openbim-ifcx`, `openbim-ifcx-geometry`) | crates.io |
| `openbim-ifcx-wasm` | npm only (`@openbim/ifcx`) |
| `openbim-ifcx-py` | PyPI only (`openbim-ifcx`): Linux x86_64 and aarch64, macOS universal2 and Windows x64 abi3 wheels plus an sdist |
| `openbim-ifcx-binding-core` | nowhere (`publish = false`) |

The version in `crates/openbim-ifcx-wasm/npm/package.json` or
`crates/openbim-ifcx-py/pyproject.toml` must equal the crate's. `--set
--apply` bumps it together with `Cargo.toml`, and refuses if the two were
already out of step. The workflow refuses a tag that disagrees with any
manifest. Every publish step skips a version that is already live, so a
re-run finishes a partly failed release.

### Rehearsing a release

Actions -> Release -> Run workflow on `main`, with a tag name such as
`openbim-ifcx-py-v0.1.0`, runs everything except the uploads: it plans the
tag against the manifests on `main`, waits for CI, builds and tests the npm
package and runs `npm publish --dry-run` (or `npm pack --dry-run` for a
version already live), and builds and tests every wheel and the sdist. The
`crates.io` and PyPI publish jobs only run for a pushed tag.

```bash
gh workflow run release.yml -R openbimrs/ifcx --ref main -f tag=openbim-ifcx-wasm-v0.1.0
```

### Environments and trusted publishing

Each publish job runs in the environment named after its registry:
`crates.io`, `npmjs.com` or `pypi.org`. Only `main` (for rehearsals) and
release tags matching `*v*.*.*` may deploy to them. A deployment links to
the version it published on that registry.

All three registries use trusted publishing (OIDC): the job holds
`id-token: write` and exchanges a GitHub token for a short-lived registry
token, so no long-lived npm or PyPI token exists. Each registry trusts
exactly the repository `openbimrs/ifcx`, the workflow file `release.yml`
and its own environment; renaming either breaks publishing until the
registry settings follow.

- **crates.io** is configured per crate: in the crate's settings, add a
  trusted publisher with repository `openbimrs/ifcx`, workflow
  `release.yml`, and environment `crates.io`. A new crate can only be
  configured after its first version exists, so the first release is
  published by hand after `cargo login`:

  ```bash
  python3 scripts/release-crate.py <crate> --publish --local
  ```

  That tags and pushes as above, then runs `cargo publish` from a detached
  worktree at the tag. The release workflow that the tag starts finds the
  version live and succeeds without a token. Until a crate has a trusted
  publisher, CI falls back to the `crates.io` environment's
  `CARGO_REGISTRY_TOKEN` secret if one is set.
- **npm** (`@openbim/ifcx`): a trusted publisher is set in the package's
  settings on npmjs.com (Settings -> Trusted publishing -> GitHub Actions:
  organization `openbimrs`, repository `ifcx`, workflow filename
  `release.yml`, environment `npmjs.com`), or with `npm trust github`. Both
  need the package to exist, so **the first version of a new package is
  published by hand** with an account that may publish to the `@openbim`
  scope; every later version comes from `release.yml`. Build the package
  exactly as CI does, then publish it:

  ```bash
  git checkout main && git pull
  crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh    # builds pkg/, runs the Node suite and the browser check
  cd crates/openbim-ifcx-wasm/pkg
  npm login
  npm publish --access public
  cd -
  python3 scripts/release-crate.py openbim-ifcx-wasm --publish   # tags openbim-ifcx-wasm-v0.1.0
  ```

  The tag's workflow run then finds the version live and skips the upload.
  Trusted publishing needs npm 11.5.1 or later (the workflow installs it;
  Node 22 ships npm 10) and publishes with provenance automatically. A
  hand-published first version has no provenance.
- **PyPI** (`openbim-ifcx`): PyPI accepts a *pending* trusted publisher for
  a project that does not exist yet, so the first release can come from
  the workflow. Before pushing the first tag, add one under Your account ->
  Publishing -> Add a new pending publisher -> GitHub: PyPI project name
  `openbim-ifcx`, owner `openbimrs`, repository `ifcx`, workflow
  `release.yml`, environment `pypi.org`. The first upload creates the
  project and turns the pending publisher into a normal one. A pending
  publisher does not reserve the name: if someone else registers
  `openbim-ifcx` first, it is invalidated.

If CI cannot publish a crate to crates.io, `--publish --local` runs `cargo
publish` from a detached worktree at the tag instead, so the embedded VCS
hash stays deterministic. A crate already live at its committed version is
a no-op, so a rate-limited run can be repeated safely.
