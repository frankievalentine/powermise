#!/usr/bin/env bash
# Checks an AI setup's compiled fragment against the real mise on PATH, in a
# sandbox: mise must load `conf.d/powermise-<name>.toml` without `-E`, see its
# tools, plan its packages, and dry-run `mise bootstrap --only packages,tools`
# without touching the person's own config. Needs mise 2026.10 or newer.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
selection="${1:-$repo_root/selections/engineering-agents.toml}"
name="$(basename "$selection" .toml)"

if ! command -v mise >/dev/null 2>&1; then
  printf 'mise is required; install mise 2026.10 or newer and rerun this script.\n' >&2
  exit 127
fi

case "$(uname -s)" in
  Darwin) target=darwin ;;
  Linux) target=linux ;;
  *) target=win32 ;;
esac

sandbox="$(mktemp -d)"
cleanup() { rm -rf "$sandbox"; }
trap cleanup EXIT
mkdir -p "$sandbox/home" "$sandbox/config/conf.d"

# The person's own config, which must win for tools it also declares.
printf '[tools]\njq = "1.7"\n' >"$sandbox/config/config.toml"
before="$(shasum -a 256 "$sandbox/config/config.toml" | cut -d ' ' -f 1)"

fragment="$sandbox/config/conf.d/powermise-$name.toml"
(cd "$repo_root" && cargo run --quiet --locked -p powermise-setups --example fragment -- "$selection" "$target") >"$fragment"

# Never MISE_YES: it makes mise auto-trust configs.
mise_sandboxed() {
  env -u MISE_YES HOME="$sandbox/home" MISE_CACHE_DIR="$sandbox/cache" MISE_CONFIG_DIR="$sandbox/config" \
    MISE_DATA_DIR="$sandbox/data" MISE_STATE_DIR="$sandbox/state" NO_COLOR=1 mise "$@"
}

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

mise_sandboxed config ls --json >"$sandbox/config-files.json"
grep -Fq "powermise-$name.toml" "$sandbox/config-files.json" || fail "mise did not load the conf.d fragment."

mise_sandboxed ls --json >"$sandbox/ls.json"
for tool in $(sed -n '/^\[tools\]/,/^\[/{/^[^[#]/s/ *=.*//p;}' "$fragment" | tr -d '"'); do
  grep -Fq "\"$tool\"" "$sandbox/ls.json" || fail "mise ls does not list the fragment's tool \`$tool\`."
done

mise_sandboxed bootstrap plan --json >"$sandbox/plan.json"
for package in $(sed -n '/^\[bootstrap.packages\]/,/^\[/{/^"/s/ *=.*//p;}' "$fragment" | tr -d '"'); do
  grep -Fq "\"$package\"" "$sandbox/plan.json" || fail "mise bootstrap plan does not include \`$package\`."
done

mise_sandboxed bootstrap --dry-run --only packages,tools >"$sandbox/dry-run.txt" 2>&1 ||
  fail "mise bootstrap --dry-run failed: $(cat "$sandbox/dry-run.txt")"

[[ "$(mise_sandboxed current jq)" == 1.7* ]] || fail "the person's own jq version did not win over the fragment."

after="$(shasum -a 256 "$sandbox/config/config.toml" | cut -d ' ' -f 1)"
[[ "$before" == "$after" ]] || fail "the person's mise config changed."

printf 'mise %s loads and plans the %s fragment for %s.\n' "$(mise_sandboxed version --json | sed -n 's/.*"version": *"\([^ "]*\).*/\1/p')" "$name" "$target"
