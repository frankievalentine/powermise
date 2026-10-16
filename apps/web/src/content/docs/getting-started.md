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

powermise isn't code-signed yet: Apple and Microsoft can't vouch for who made it,
so your system asks before opening it the first time. That's one extra step, once.
The install script skips it on macOS, and on Linux there's nothing to approve.

**macOS.** Opening a downloaded copy shows *“powermise” Not Opened* (or *Apple
could not verify…*). Click **Done**, then either:

- open **System Settings › Privacy & Security**, scroll to **Security**, click
  **Open Anyway** next to *“powermise” was blocked*, and confirm with your
  password; or
- run this in Terminal, then open powermise as usual:

  ```sh
  xattr -dr com.apple.quarantine /Applications/powermise.app
  ```

**Windows.** If **Windows protected your PC** appears, click **More info**, then
**Run anyway**. The install script doesn't trigger this.

**Linux.** The `.deb` and `.rpm` install like any local package — nothing to
approve (the install script uses them). An AppImage needs two things first:

- make it executable — `chmod +x powermise-linux-*.AppImage`, or tick **Allow
  executing file as program** in its Properties;
- FUSE 2 — `sudo apt install libfuse2` (`libfuse2t64` on Ubuntu 24.04 and
  later), `sudo dnf install fuse-libs`, or `sudo pacman -S fuse2`.

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
pretend data. Nothing it does touches your computer. On a phone the homepage
shows screenshots of each step instead; open it on a larger screen to use the
app.

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
