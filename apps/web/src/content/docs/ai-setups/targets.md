---
title: Platforms
description: How AI Setups resolve across macOS, Linux, Windows, and WSL.
---

A setup compiles for one platform at a time. **Apply** always uses the
computer you're on; the setup editor's **Preview for** menu shows what the
same setup would install anywhere else.

| Platform | Notes |
| --- | --- |
| macOS (Apple Silicon & Intel) | mise tools, plus Homebrew packages and casks for the rest. |
| Linux | mise tools, plus apt or Homebrew packages. |
| Windows | mise tools, plus winget or Scoop packages. |
| WSL | Treated as Linux, separate from Windows: a setup applied inside WSL installs Linux tools there. |

## How providers are chosen

Each tool in the catalog lists the ways it can be installed and the platforms
each one supports. powermise picks exactly one per tool, in this order:

1. **A mise tool** — goes in the file's `[tools]` with a pinned version.
2. **apt**, then **Homebrew** formula, then **Homebrew cask**, then **winget**,
   then **Scoop** — goes in `[bootstrap.packages]`, which `mise bootstrap`
   installs with that package manager. Homebrew can't install a pinned
   version, so its packages are `latest`.

If a tool has no provider for the platform, the editor reports it before
anything is saved, and harnesses that don't support the platform can't be
picked for it.

## Support status

| Platform | Status |
| --- | --- |
| macOS | Compiled fragments verified against real mise 2026.10 in CI; a clean-machine apply is still an open acceptance gate. |
| Linux | Verified against real mise in CI; not yet clean-machine tested. |
| Windows | Compiles and previews; no real Windows installs are claimed until clean-machine evidence exists. |
| WSL | Compiles as its own target; not yet clean-machine tested. |
