# Release Policy

How Powermise is versioned, what it supports, and how it is updated.

## Minimum supported mise version

Powermise integrates with the separately installed `mise` executable (see the
Architecture Decision in the build plan). It depends on a small, stable surface:

- `mise --env <environment> bootstrap --dry-run`
- `mise --env <environment> bootstrap status --json`
- `mise --env <environment> bootstrap --yes`
- `mise --env <environment> exec -- <command>`

The desktop app supports **mise 2026.7.18** and newer
(`MiseVersion::MINIMUM` in `powermise-mise`, the release its `--json` contract
fixtures were captured from) and recommends **2026.10** (`MiseVersion::RECOMMENDED`).
Features that need newer mise are gated by version rather than raising the floor. The support matrix in `setup.md`
reflects what has been clean-machine verified.

A future Powermise release must:

- declare the minimum mise version it was validated against in this document
  and the build plan;
- keep using only the supported surface above, so a mise upgrade on the user's
  machine does not require a Powermise upgrade;
- fail early with a clear message if the installed mise is below the declared
  minimum (the version probe backing `doctor`).

## Versioning

Powermise follows semantic versioning. The CLI surface — subcommands, flags,
and the schema-stable JSON envelope (`{"command","schema_version":1,"payload"}`)
— is the contract. A change that breaks JSON field names or removes a flag is a
major version. New commands, flags, and additive JSON fields are minor.

The generated-file format is versioned separately (headers + the persisted
manifest carry their own schema version) so regenerated output stays
attributable and comparable across upgrades.

## Update policy

- Powermise does not auto-update and never modifies your global `mise`
  configuration.
- `mise` itself is updated independently of Powermise; because the integration
  boundary is stable, a mise upgrade should be transparent.
- When Powermise gains a feature that needs a newer mise capability, the minimum
  version is raised here first, then the support matrix is updated.

## Release artifacts

The release output is the single static binary `powermise`, built from
`cargo build --release`, for each supported target:

- macOS (Apple Silicon)
- Linux (x86_64)
- Windows (`win32`, x86_64)

The CI matrix (`ci.yml`) builds and tests every target on each commit; release
binaries are produced from those same workflows. No hosted sync service is used
in the initial release.
