# OpenBIM.rs IFCX

Canonical repository: <https://github.com/openbimrs/ifcx>
Integration repository: <https://github.com/openbimrs/openbim>

Read `AGENTS.md` before changing the repository. Keep the workspace
independently buildable; the parent integration repository consumes families
only as released crates.io versions.

## Verification

Run `./scripts/gate.sh`. It is the authoritative local and CI gate and decides
success from command exit codes.
