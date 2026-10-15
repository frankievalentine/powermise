#!/usr/bin/env bash
# Captures mise --json output fixtures for powermise-mise's contract tests.
#
#   scripts/capture-mise-fixtures.sh <mise-binary> <output-dir>
#
# Everything runs in a throwaway sandbox (its own HOME and mise data, config,
# cache, and state directories), so the machine's own mise setup is never
# read or changed. Installs one small tool (jq) into the sandbox. Paths in
# the output are rewritten to /Users/demo/... so fixtures carry no real paths.
set -euo pipefail

MISE="${1:?usage: $0 <mise-binary> <output-dir>}"
OUT="${2:?usage: $0 <mise-binary> <output-dir>}"
MISE="$(cd "$(dirname "$MISE")" && pwd)/$(basename "$MISE")"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"

REAL_HOME="$HOME"
# The real path: macOS temp dirs resolve through /private, and mise reports
# real paths, so sanitizing must match that form.
SANDBOX="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$SANDBOX"' EXIT
mkdir -p "$SANDBOX"/{home,data,config,cache,state,proj,untrusted}
export HOME="$SANDBOX/home" MISE_DATA_DIR="$SANDBOX/data" MISE_CONFIG_DIR="$SANDBOX/config"
export MISE_CACHE_DIR="$SANDBOX/cache" MISE_STATE_DIR="$SANDBOX/state" NO_COLOR=1
# Never MISE_YES: it makes mise silently trust untrusted configs.
unset MISE_ENV MISE_CONFIG_FILE MISE_GLOBAL_CONFIG_FILE MISE_YES

sanitize() {
  sed -e "s#$SANDBOX/home#/Users/demo#g" \
    -e "s#$SANDBOX/data#/Users/demo/.local/share/mise#g" \
    -e "s#$SANDBOX/config#/Users/demo/.config/mise#g" \
    -e "s#$SANDBOX/cache#/Users/demo/Library/Caches/mise#g" \
    -e "s#$SANDBOX/state#/Users/demo/.local/state/mise#g" \
    -e "s#$SANDBOX/proj#/Users/demo/code/app#g" \
    -e "s#$SANDBOX/untrusted#/Users/demo/code/infra#g" \
    -e "s#$(dirname "$SANDBOX")/[^\"]*#/Users/demo/tmp#g" \
    -e "s#$REAL_HOME#/Users/demo#g"
}
capture() { local name="$1"; shift; "$@" </dev/null 2>/dev/null | sanitize >"$OUT/$name"; }

# A global tool, plus a project that pins newer/missing versions, has
# env (one redacted), and tasks (one with a usage spec).
"$MISE" use -g jq@1.7.1 >/dev/null 2>&1
cat >"$SANDBOX/proj/mise.toml" <<'TOML'
[tools]
jq = "1.8"
shellcheck = "0.10"

[env]
APP_ENV = "dev"
SECRET_TOKEN = { value = "abc", redact = true }

[tasks.build]
description = "Build it"
run = "echo build"

[tasks.deploy]
description = "Deploy"
usage = '''
arg "<env>" help="Target" {
  choices "staging" "prod"
}
flag "--dry-run" help="Preview only"
'''
run = "echo deploy {{usage.env}}"
TOML
(cd "$SANDBOX/proj" && "$MISE" trust >/dev/null 2>&1)
printf '[tools]\njq = "1.7.1"\n[hooks]\nenter = "echo hi"\n' >"$SANDBOX/untrusted/mise.toml"

"$MISE" version --json </dev/null 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); print(json.dumps({"version": d["version"], "latest": d.get("latest")}, indent=2))' >"$OUT/version.json"

cd "$SANDBOX/home"
capture ls-global.json "$MISE" ls --json
capture outdated-global.json "$MISE" outdated --json
capture outdated-bump-global.json "$MISE" outdated --bump --json
capture ls-remote-jq.json "$MISE" ls-remote --json jq
"$MISE" registry --json </dev/null 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); print(json.dumps([e for e in d if e["short"] in ("jq","node","python","ripgrep","terraform","go")], indent=2))' >"$OUT/registry-sample.json"
"$MISE" doctor --json </dev/null 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); d["paths"]=d.get("paths",[])[:2]; d["env_vars"]={k:v for k,v in d.get("env_vars",{}).items() if k!="MISE_YES"}; print(json.dumps(d, indent=2))' | sanitize >"$OUT/doctor.json"

cd "$SANDBOX/proj"
capture ls-project.json "$MISE" ls --json
capture outdated-project.json "$MISE" outdated --json
capture config-ls.json "$MISE" config ls --json
capture tasks-ls.json "$MISE" tasks ls --json
capture task-info-deploy.json "$MISE" tasks info --json deploy
capture tool-jq.json "$MISE" tool --json jq
"$MISE" env --json-extended </dev/null 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); d.pop("PATH",None); print(json.dumps(d, indent=2))' | sanitize >"$OUT/env-extended.json"
capture env-redacted.json "$MISE" env --json --redacted
capture trust-show-trusted.txt "$MISE" trust --show
capture lock-dry-run.json "$MISE" lock --dry-run --json

cd "$SANDBOX/untrusted"
capture trust-show-untrusted.txt "$MISE" trust --show
"$MISE" ls --json </dev/null 2>&1 >/dev/null | sanitize >"$OUT/untrusted-error.txt" || true

# Settings (one set in the global config), an unused version for prune,
# and the GitHub token status with no token anywhere.
cd "$SANDBOX/home"
"$MISE" settings set jobs 4 >/dev/null 2>&1
"$MISE" install jq@1.6 >/dev/null 2>&1
"$MISE" settings ls --all --json-extended </dev/null 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); keep={k: d[k] for k in ("jobs","experimental","always_keep_download","github","node") if k in d}; print(json.dumps(keep, indent=2))' | sanitize >"$OUT/settings-all.json"
capture ls-prunable.json "$MISE" ls --prunable --json
env -u GITHUB_TOKEN -u GH_TOKEN -u MISE_GITHUB_TOKEN -u GITHUB_API_TOKEN "$MISE" token github </dev/null 2>/dev/null | sanitize >"$OUT/token-github-none.txt"

version="$("$MISE" version --json </dev/null 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["version"].split()[0])')"

# Per-machine [vars] (mise 2026.10.6+).
if [[ "$(printf '%s\n2026.10.6\n' "$version" | sort -V | head -n1)" == "2026.10.6" ]]; then
  printf '\n[vars]\nregion = "us-east-1"\n\n[vars.api_url]\ndefault = "http://localhost"\n' >>"$SANDBOX/proj/mise.toml"
  (cd "$SANDBOX/proj" && "$MISE" trust >/dev/null 2>&1)
  (cd "$SANDBOX/proj" && "$MISE" vars ls --no-header </dev/null 2>/dev/null) | sanitize >"$OUT/vars-ls.txt"
fi

# AI Setups: a conf.d fragment, as powermise writes one (mise 2026.10+).
if [[ "$(printf '%s\n2026.10.0\n' "$version" | sort -V | head -n1)" == "2026.10.0" ]]; then
  mkdir -p "$SANDBOX/config/conf.d"
  printf '[tools]\njq = "1.7.1"\n\n[bootstrap.packages]\n"brew:hello" = "latest"\n' >"$SANDBOX/config/conf.d/powermise-demo.toml"
  cd "$SANDBOX/home"
  capture bootstrap-plan.json "$MISE" bootstrap plan --json
  capture bootstrap-status.json "$MISE" bootstrap status --json
fi

# Fixtures must never carry the capturing machine's real paths.
if grep -rqs "$SANDBOX\|/private/tmp\|/var/folders\|$REAL_HOME" "$OUT"; then
  echo "error: unsanitized paths left in $OUT" >&2
  exit 1
fi
echo "captured $(ls "$OUT" | wc -l | tr -d ' ') fixtures from $("$MISE" --version) into $OUT"
