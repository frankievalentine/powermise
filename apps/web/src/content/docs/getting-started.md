---
title: Getting started
description: Install powermise, let it set up mise, and take a first look around.
---

powermise is an unofficial companion to [mise](https://mise.jdx.dev) — it
isn't made by the mise team. It runs on macOS, Windows, and Linux.

## 1. Install powermise

The quickest way is the install script, which downloads the latest release,
checks it against the release's checksums, and installs it:

```sh
# macOS and Linux
curl -fsSL https://powermise.pages.dev/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://powermise.pages.dev/install.ps1 | iex
```

On macOS it puts **powermise.app** in Applications. On Linux it installs the
`.deb` or `.rpm` through your package manager (which asks for your password),
or the AppImage in `~/.local/bin` where neither fits. On Windows it installs
for your account only, with no administrator prompt.

Or download an installer from the [latest release](https://github.com/frankievalentine/powermise/releases/latest):

| System | File |
| --- | --- |
| macOS (Apple Silicon and Intel) | `powermise-macos-universal.dmg` |
| Windows | `powermise-windows-x64-setup.exe` (or the `.msi`) |
| Linux x64 | `powermise-linux-x86_64.AppImage`, `-amd64.deb`, or `-x86_64.rpm` |
| Linux ARM | `powermise-linux-aarch64.AppImage`, `-arm64.deb`, or `-aarch64.rpm` |

### Opening it the first time

Releases aren't code-signed yet, so your system asks before opening an app it
can't verify. The install script avoids this on macOS; a downloaded installer
needs one extra step:

- **macOS** — open powermise once, then go to **System Settings › Privacy &
  Security** and click **Open Anyway**. Or, in Terminal:
  `xattr -dr com.apple.quarantine /Applications/powermise.app`
- **Windows** — if SmartScreen says it protected your PC, click **More info**,
  then **Run anyway**.
- **Linux** — AppImages need FUSE 2 (`libfuse2`, or `libfuse2t64` on Ubuntu
  24.04). Make the file executable first: `chmod +x powermise-linux-*.AppImage`.

## 2. mise

powermise runs [mise](https://mise.jdx.dev) for everything it does. If you
don't have it, the Welcome screen installs it for you — mise's own installer on
macOS and Linux, winget or Scoop on Windows — and shows the one line that turns
it on in your terminal. powermise works with mise **2026.7.18 or newer**, and is
designed around **2026.10 or newer**, the version a fresh install gets.

## 3. Look around

After Welcome, powermise opens on **Home**, with a checklist that ticks itself off:

1. Turn mise on in your terminal — Home shows the one line to add.
2. Open **Discover**, pick a tool, and click **Install**. powermise shows the
   exact `mise use -g …` command first, then mise's own output while it runs.
3. Add a project folder to see the tools and tasks it uses.
4. If another version manager installed some versions, **Bring over versions
   you already have** opens **Import**. It also brings over Homebrew packages
   and command-line tools you installed another way, with a dry run before
   anything changes.

**My Tools** shows what's installed and what has an update; **Health** lists
anything that needs attention, with the fix.

The [app tour](/app-tour/) walks through each screen.

## Try it without installing

The [live demo](/#demo) on the homepage runs the real app in your browser with
pretend data. Nothing it does touches your computer.

## Build from source

You'll need [Bun](https://bun.sh) and [Rust](https://rustup.rs). On macOS, also
the Xcode command line tools (`xcode-select --install`); on Linux, the
[Tauri system dependencies](https://tauri.app/start/prerequisites/).

```sh
git clone https://github.com/frankievalentine/powermise.git
cd powermise
bun install
bun run desktop:build
```

The finished app is in `apps/desktop/src-tauri/target/release/bundle/`. To run
it straight from the source instead:

```sh
bun run desktop:dev
```
