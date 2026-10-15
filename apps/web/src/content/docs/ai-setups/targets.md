---
title: Platforms
description: How AI Setups resolve across macOS, Linux, Windows, and WSL.
---

A setup compiles for one platform at a time. **Apply** always uses the
computer you're on; the setup editor's **Preview for** menu shows what the
same setup would install anywhere else.

| Platform | Notes |
| --- | --- |
| macOS (Apple Silicon) | mise tools, Homebrew packages, and Mac apps from Homebrew Cask — all installed by mise, no Homebrew needed. git and curl come with macOS. |
| macOS (Intel) | The same, except mise can't install Homebrew packages (formulas) on Intel; the preview lists anything left out. Mac apps still install. |
| Linux | mise tools, then Homebrew packages (installed by mise into `/home/linuxbrew/.linuxbrew`), then apt. |
| Windows | mise tools, plus winget or Scoop packages. |
| WSL | Treated as Linux: a setup applied inside WSL installs Linux tools there. Desktop apps belong on the Windows side. |

## How providers are chosen

Each tool in the catalog lists the ways it can be installed and the platforms
each one supports. powermise picks exactly one per tool, in this order:

1. **A mise tool** — goes in the file's `[tools]` with a pinned version.
2. **Comes with the system** — nothing to install (git and curl on macOS).
3. **Homebrew formula**, then **Homebrew cask** (Mac apps), then **apt**, then
   **winget**, then **Scoop** — goes in `[bootstrap.packages]`, which
   `mise bootstrap` installs. mise installs Homebrew packages and casks itself,
   so Homebrew doesn't need to be installed. They can't be pinned, so they're
   `latest`.

apt comes after Homebrew because every apt install needs an administrator
password. On a machine that already has the package, apt does nothing.

If a tool has no provider for the platform, the editor reports it before
anything is saved, and harnesses that don't support the platform can't be
picked for it.

### Linux and WSL: the one-time Homebrew setup

mise puts Homebrew packages in `/home/linuxbrew/.linuxbrew`, and creating that
folder needs `sudo` once. The app can't type a password, so when a setup needs
Homebrew and the folder isn't ready, **Apply** becomes **Apply in Terminal**:
the first install runs in your terminal app, where `sudo` can ask. After that,
installs run in the app. To do it yourself:

```sh
sudo mkdir -p /home/linuxbrew/.linuxbrew && sudo chown -R "$USER" /home/linuxbrew
```

On Linux the Apply sheet also offers **Run in Terminal**, for apt packages that
aren't installed yet.

## Support status

| Platform | Status |
| --- | --- |
| macOS | Compiled fragments verified against real mise 2026.10 in CI; a clean-machine apply is still an open acceptance gate. |
| Linux | Verified against real mise in CI; not yet clean-machine tested. |
| Windows | Compiles and previews; no real Windows installs are claimed until clean-machine evidence exists. |
| WSL | Compiles as its own target; not yet clean-machine tested. |
