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
