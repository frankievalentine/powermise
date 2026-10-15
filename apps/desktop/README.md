# powermise Desktop

A Tauri v2 app for browsing the harness catalog, building setups, and applying
them — for people who'd rather not live in a terminal.

## Run it

```sh
bun install
bun run desktop:dev      # from the repo root
```

`bun run --cwd apps/desktop dev` serves the UI alone in a browser at
<http://localhost:1420>. Outside Tauri it reads the real catalog TOML files but
fakes everything that touches the machine (`src/dev/browserPreview.ts`), which
is handy for design work.

## How it fits with the CLI

- `src-tauri/src/lib.rs` calls the same `powermise-lifecycle` functions the CLI
  runs, against the same selection files, ledger, and mise config directory.
  A "setup" in the app is a selection in `~/.config/powermise/selections/`.
- Every action shows the equivalent `powermise …` command, so anything done
  here can be repeated or undone from a terminal.
- The crate is excluded from the Cargo workspace so Tauri's dependency tree
  stays out of CLI builds and CI.
- Apps launched from Finder don't inherit the shell `PATH`; the backend adds
  the usual mise/Homebrew locations before looking for `mise`.

## Design

One brand on every platform, with native edges:

- **Shared:** Geist and Geist Mono (bundled, so layouts match everywhere), the
  coral accent from the website, warm neutrals, platform-neutral controls with
  visible focus rings and 32px targets, and restrained motion (springs without
  overshoot, reduced-motion fallbacks).
- **Per platform** (`html[data-platform]`, set before first render):
  - **macOS:** overlay title bar with traffic lights over a vibrancy sidebar,
    ⌘ shortcuts, primary dialog button last.
  - **Windows:** our own title bar with caption buttons over Mica
    (`tauri.windows.conf.json`), Ctrl shortcuts, primary dialog button first,
    thin scrollbars.
  - **Linux:** the system's window frame and solid surfaces, Ctrl shortcuts,
    GNOME button order, thin scrollbars.

Window settings live in `src-tauri/tauri.conf.json` (the Linux default) plus
`tauri.macos.conf.json` and `tauri.windows.conf.json`, which Tauri merges per
platform.

In a browser, `?platform=win32` (or the **Preview as** switcher at the bottom
right) shows another platform's chrome and conventions.
