# App icon sources

The website's dithered chef hat (`apps/web/src/components/landing/ditheredChefHat.ts`,
captured from its WebGL canvas) on the app's dark tile, at 1024px:

- `icon-macos.png` — the tile inset on Apple's icon grid, for `icon.icns`.
- `icon.png` — the tile filling the canvas, for everything else.

Regenerate with `bunx tauri icon <source> -o <dir>` (from `apps/desktop`), then
copy `icon.icns` from the macOS run and the other files from the full run.
