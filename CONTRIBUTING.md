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

The command must exit successfully. Update README, rustdoc, capabilities, and
CHANGELOG together when a user-visible contract changes.

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
CI to pass on the tagged commit, and publishes to crates.io in the
`crates.io` environment. A version that is already live is skipped, so a
re-run finishes a partly failed release.

crates.io trusts the workflow through trusted publishing: in each crate's
crates.io settings, add a trusted publisher with repository
`openbimrs/ifcx`, workflow `release.yml`, and environment `crates.io`. A new
crate can only be configured after its first version exists, so the first
release is published by hand after `cargo login`:

```bash
python3 scripts/release-crate.py <crate> --publish --local
```

That tags and pushes as above, then runs `cargo publish` from a detached
worktree at the tag. The release workflow that the tag starts finds the
version live and succeeds without a token. Until a crate has a trusted
publisher, CI falls back to the `crates.io` environment's
`CARGO_REGISTRY_TOKEN` secret if one is set.
