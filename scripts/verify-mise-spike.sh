#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture_root="$repo_root/tests/fixtures/mise"
selection="powermise-engineering-agents"

if ! command -v mise >/dev/null 2>&1; then
  printf 'mise is required for the compatibility spike; install a supported mise release and rerun this script.\n' >&2
  exit 127
fi

sandbox="$(mktemp -d)"
cleanup() { rm -rf "$sandbox"; }
trap cleanup EXIT

cp -R "$fixture_root/config" "$sandbox/config"
mkdir -p "$sandbox/home"
baseline_before="$(shasum -a 256 "$sandbox/config/config.toml" | cut -d ' ' -f 1)"

mise_in_fixture() {
  HOME="$sandbox/home" MISE_CACHE_DIR="$sandbox/cache" MISE_CONFIG_DIR="$sandbox/config" MISE_DATA_DIR="$sandbox/data" mise --env "$selection" "$@"
}

mise_in_fixture config ls --json >"$sandbox/config-files.json"
mise_in_fixture current bun >"$sandbox/current-bun.txt"
mise_in_fixture bootstrap --dry-run --only tools >"$sandbox/dry-run.txt"
mise_in_fixture bootstrap --dry-run --only packages >"$sandbox/package-dry-run.txt"
mise_in_fixture bootstrap --dry-run --only dotfiles >"$sandbox/dotfile-dry-run.txt"
mise_in_fixture bootstrap --dry-run --only repos >"$sandbox/repo-dry-run.txt"
mise_in_fixture bootstrap --yes --only dotfiles >"$sandbox/dotfile-apply.txt"
mise_in_fixture bootstrap --yes --only dotfiles >"$sandbox/dotfile-repeat-apply.txt"
mise_in_fixture bootstrap status --json >"$sandbox/status.json"

baseline_after="$(shasum -a 256 "$sandbox/config/config.toml" | cut -d ' ' -f 1)"
if [[ "$baseline_before" != "$baseline_after" ]]; then
  printf 'global mise baseline changed during a dry-run.\n' >&2
  exit 1
fi

if ! cmp -s "$sandbox/config/gitconfig" "$sandbox/home/.gitconfig"; then
  printf 'dotfile apply did not converge to the configured source.\\n' >&2
  exit 1
fi

if ! grep -Fq "config.$selection.toml" "$sandbox/config-files.json"; then
  printf 'selection environment file was not discovered.\n' >&2
  exit 1
fi

if ! grep -Eq '^1\.3(\.|$)' "$sandbox/current-bun.txt"; then
  printf 'selection environment tool was not resolved.\n' >&2
  exit 1
fi

printf 'mise compatibility fixture passed for %s.\n' "$selection"
