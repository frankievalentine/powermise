# Powermise Desktop

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

Native macOS chrome (overlay title bar, sidebar vibrancy) with styling guided
by Emil Kowalski's apple-design skill: system type with Apple's tracking curve,
springs without overshoot, press feedback on pointer-down, and
reduced-motion / reduced-transparency / increased-contrast fallbacks.
