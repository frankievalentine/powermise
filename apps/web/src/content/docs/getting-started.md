---
title: Getting started
description: Install mise, build the powermise desktop app, and take a first look around.
---

powermise is pre-release, so for now you build it from source. Installers for
macOS, Windows, and Linux are on the [roadmap](https://github.com/frankievalentine/powermise/blob/main/ROADMAP.md).

## 1. Install mise

powermise works with mise **2026.7.18 or newer**, and is designed around
**2026.10 or newer** — the version a fresh install gets. If you don't have mise
yet:

```sh
curl https://mise.run | sh
```

Then hook it into your shell so terminals pick up the tools it installs — for
zsh:

```sh
echo 'eval "$(mise activate zsh)"' >> ~/.zshrc
```

See the [mise getting started guide](https://mise.jdx.dev/getting-started.html)
for other shells and install methods.

## 2. Build powermise

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

## 3. Look around

powermise opens on **Installed**, showing what mise already manages. From there:

1. Open **Discover**, search for a tool, and pick a version.
2. Click **Install**. powermise shows the exact `mise use -g …` command first,
   then mise's own output while it runs.
3. Check **Updates** for anything with a newer version.
4. Visit **Health** to confirm mise is set up in your shell.

The [app tour](/app-tour/) walks through each screen.

## Try it without installing

The [live demo](/#demo) on the homepage runs the real app in your browser with
pretend data. Nothing it does touches your computer.
