# Release Policy

How powermise is versioned, what it supports, and how it is updated.

## mise versions

powermise drives the separately installed `mise` executable and depends only
on its documented commands and `--json` output. The contract is pinned by
fixtures captured from real mise (`crates/powermise-mise/tests/fixtures/<version>/`,
via `scripts/capture-mise-fixtures.sh`) and tested against every listed version.

- **Minimum: 2026.7.18** (`MiseVersion::MINIMUM`). Below it, the app says mise
  is too old and offers an update.
- **Recommended: 2026.10** (`MiseVersion::RECOMMENDED`). Features that need
  newer mise are gated by version rather than raising the floor. AI Setups are
  one: they rely on `conf.d` loading and `mise bootstrap --only`.

When a feature needs a newer mise, gate it on the version and capture a new
fixture set; raise the minimum only when gating stops being practical, and
record it here and in ROADMAP.md first.

powermise never runs mise with `MISE_YES` (it makes mise trust configs on its
own); `MiseClient` removes it from every invocation.

## Versioning

powermise follows semantic versioning for the desktop app. There is no
powermise CLI; the stable formats are the files powermise writes:

- the selection file (`schema_version = 1`), and
- the generated `conf.d/powermise-<name>.toml`, which is plain mise config with
  a header naming the setup and hashes of the selection and catalog it came from.

A change that makes existing selection files unreadable is a major version.

## Update policy

- powermise never edits your own mise config. AI Setups write only their own
  `conf.d/powermise-<name>.toml` files.
- mise is updated independently (Health offers `mise self-update`).

## Release artifacts

The desktop app, built with Tauri (`bun run desktop:build`), for:

- macOS (Apple Silicon and Intel)
- Linux (x86_64)
- Windows (x86_64)

The CI matrix (`ci.yml`) builds and tests the library crates on all three on
each commit and checks a compiled setup against real mise on macOS and Linux.
No hosted sync service is used.
