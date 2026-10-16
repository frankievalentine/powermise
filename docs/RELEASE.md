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
record it here and in [ROADMAP.md](ROADMAP.md) first.

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

## Releasing

1. `scripts/bump-version.sh <version>` sets the version in
   `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/src-tauri/Cargo.toml`
   (and its lock file), and both `package.json` files.
2. Commit, tag `v<version>`, and push the tag.
3. `.github/workflows/release.yml` checks the tag matches the version, builds
   every platform on GitHub's runners, and attaches the installers and
   `SHA256SUMS` to a draft release.
4. Review the draft and publish it. The site's download button and install
   scripts use `/releases/latest/download/<file>`, so they follow the newest
   published release.

## In-app updates

powermise updates itself with Tauri's updater (`src-tauri/src/updater.rs`). It
checks `latest.json` from the newest published release a few seconds after it
opens (and from **Settings › About powermise › Check for Updates**), then offers
**Restart to Update** in the sidebar.

- Each release's build signs the update files with the updater key, and the last
  job writes `latest.json`: the macOS `.app.tar.gz`, each Linux AppImage, and the
  Windows setup `.exe`, each with its `.sig`. The `.deb`, `.rpm`, and `.msi` don't
  update in place; reinstall those (the install script picks up the newest).
- The key pair: the private key is `~/.tauri/powermise-updater.key` (its password
  is in `powermise-updater.key.password`) on the maintainer's Mac, and the GitHub
  secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
  The public key is `plugins.updater.pubkey` in `tauri.conf.json`. Back the private
  key up somewhere safe: without it, installed copies can't verify a new release
  and stop updating.
- This is separate from Apple and Microsoft code signing, which releases don't
  have yet.

## Release artifacts

The desktop app, built with Tauri, under stable file names:

| Platform | Files |
| --- | --- |
| macOS (one universal build for Apple Silicon and Intel) | `powermise-macos-universal.dmg` |
| Linux x86_64 | `powermise-linux-x86_64.AppImage`, `powermise-linux-amd64.deb`, `powermise-linux-x86_64.rpm` |
| Linux arm64 | `powermise-linux-aarch64.AppImage`, `powermise-linux-arm64.deb`, `powermise-linux-aarch64.rpm` |
| Windows x64 | `powermise-windows-x64-setup.exe` (per-user, no administrator prompt), `powermise-windows-x64.msi` |

Builds aren't code-signed yet: macOS gets an ad-hoc signature so it opens on
Apple Silicon, and the first launch of a downloaded installer takes one extra
step (see the site's Getting started page). The install scripts
(`apps/web/public/install.sh` and `install.ps1`, served at
`powermise.pages.dev/install.sh` and `/install.ps1`) verify every download against
`SHA256SUMS`.

The CI matrix (`ci.yml`) builds and tests the library crates on all three
platforms on each commit and checks a compiled setup against real mise on macOS
and Linux. No hosted sync service is used.

## Website

The marketing site and docs (`apps/web`, an Astro and Starlight static site)
are a Cloudflare Pages project, `powermise`, at
[powermise.pages.dev](https://powermise.pages.dev). The site embeds the
browser demo of the app, so building it also builds `apps/desktop`'s demo.

Pages builds and deploys every push through its Git integration. In the Pages
wizard (**Workers & Pages › Create › Pages › Connect to Git**), use:

| Setting | Value |
| --- | --- |
| Production branch | `main` |
| Framework preset | None |
| Root directory | *(leave empty: the repository root)* |
| Build command | `bun install --frozen-lockfile && bun run web:build` |
| Build output directory | `apps/web/dist` |
| Environment variables | `BUN_VERSION` = `1.4.2`, `SKIP_DEPENDENCY_INSTALL` = `true` |

Other branches get preview deployments at `<branch>.powermise.pages.dev`.

To publish a local build instead, run `bun run web:deploy` from the repository
root. It builds the site and uploads it with wrangler (`apps/web/wrangler.jsonc`);
the first run asks you to log in to Cloudflare.

`apps/web/public/_headers` serves the install scripts as plain text with a short
cache, and `src/pages/404.astro` is the page for any address that doesn't exist.

