#!/usr/bin/env bash
# Tests for scripts/use-local-common.sh, run by CI. Builds a throwaway app
# that pins piddle-common, plus a copy of this checkout under a path with a
# space, a double quote and a backslash, and checks each case against both
# the config file and what Cargo resolves. Needs cargo and network access
# (the pinned commit is fetched from GitHub).
#   scripts/test-use-local-common.sh
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The checkout under an awkward name, and an app beside it that pins the
# v0.1.1 commit (any commit that is on GitHub would do).
common="$work/pc \"x\" \\ dir"
mkdir -p "$common/scripts"
cp "$here/scripts/use-local-common.sh" "$common/scripts/"
cp -r "$here/crates" "$here/Cargo.toml" "$common/"
app="$work/my app"
mkdir -p "$app/src"
cat >"$app/Cargo.toml" <<'EOF'
[package]
name = "fake-app"
version = "0.0.0"
edition = "2024"

[dependencies]
piddle-encode = { git = "https://github.com/scott-hiemstra/piddle-common", rev = "5a28d97804c2dbcd2dd7b25a2e74ed0bc2ca5ac9" }
EOF
: >"$app/src/lib.rs"
config="$app/.cargo/config.toml"
helper="$common/scripts/use-local-common.sh"

passed=0 failed=0
check() { # description, then a command that must succeed
  local what=$1
  shift
  if "$@"; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    echo "FAIL: $what" >&2
  fi
}
run() { "$helper" "$app" "$1" >/dev/null 2>&1; }
not() { ! "$@"; }
# Where Cargo takes piddle-encode from: "path" or "git".
source_of() {
  (cd "$app" && cargo metadata --format-version 1 2>/dev/null) |
    python3 -c 'import json,sys
d = json.load(sys.stdin)
p = [p for p in d["packages"] if p["name"] == "piddle-encode"][0]
print("git" if (p["source"] or "").startswith("git+") else "path")'
}
same() { cmp -s "$config" "$work/before"; }
fresh() {
  rm -rf "$app/.cargo"
  [ $# -gt 0 ] || return 0
  mkdir -p "$app/.cargo"
  printf '%s' "$1" >"$config"
  cp "$config" "$work/before"
}
blocks() { grep -c '^# >>> use-local-common.sh' "$config"; }
lockfile_git() { grep -q 'source = "git+https://github.com/scott-hiemstra/piddle-common' "$app/Cargo.lock"; }

(cd "$app" && cargo generate-lockfile -q) || { echo "cannot resolve the test app (network?)" >&2; exit 1; }

# 1. No config: on patches (under the awkward path), off removes everything.
fresh
check "on without a config" run on
check "patched to the local path" [ "$(source_of)" = path ]
check "off" run off
check "off removes .cargo" [ ! -e "$app/.cargo" ]
check "back on the pinned commit" [ "$(source_of)" = git ]
check "lockfile has the git source again" lockfile_git

# 2. Other settings survive repeated on/off, byte for byte.
fresh $'[build]\njobs = 4\n'
for i in 1 2 3; do run on; run on; done
check "one block after repeated on" [ "$(blocks)" -eq 1 ]
check "settings kept while on" grep -qx 'jobs = 4' "$config"
run off
check "settings kept byte for byte after off" same

# 3. The app's own patch table: refused, unchanged.
fresh $'[patch."https://github.com/scott-hiemstra/piddle-common"]\npiddle-encode = { path = "/x" }\n'
check "own patch table refused" not run on
check "own patch table unchanged" same

# 4. An old-style file, nothing added: taken over, then removed.
legacy=$'# Written by scripts/use-local-common.sh; remove with: scripts/use-local-common.sh off\n[patch."https://github.com/scott-hiemstra/piddle-common"]\npiddle-encode = { path = "/old/crates/piddle-encode" }\n'
fresh "$legacy"
check "pure old-style file taken over" run on
check "old paths gone" not grep -q /old "$config"
run off
check "old-style file removed by off" [ ! -e "$config" ]

# 5. An old-style file with additions: refused by on and off, unchanged.
fresh "$legacy"$'\n[target.x86_64-unknown-linux-gnu]\nlinker = "clang"\n'
check "old-style file with additions refused by on" not run on
check "  unchanged" same
check "old-style file with additions refused by off" not run off
check "  unchanged" same

# 6. Damaged markers: refused, unchanged.
b='# >>> use-local-common.sh (remove with: scripts/use-local-common.sh off)'
e='# <<< use-local-common.sh'
for damaged in "$b"$'\n[build]\njobs = 2\n' \
  $'[build]\njobs = 2\n'"$e"$'\n' \
  "$e"$'\n'"$b"$'\n' \
  "$b"$'\n'"$e"$'\n'"$b"$'\n'"$e"$'\n'; do
  fresh "$damaged"
  check "damaged markers refused by on" not run on
  check "  unchanged" same
  check "damaged markers refused by off" not run off
  check "  unchanged" same
done

# 7. Not an app, and bad usage.
mkdir -p "$work/not an app"
check "a directory that doesn't pin piddle-common is refused" not "$helper" "$work/not an app" on 2>/dev/null
"$helper" "$app" 2>/dev/null
check "missing action is a usage error" [ $? -eq 2 ]
"$helper" "$app" sideways 2>/dev/null
check "unknown action is a usage error" [ $? -eq 2 ]

# 8. CI's layout: siblings named common/ and app/, called from their parent.
mkdir -p "$work/ci"
cp -r "$common" "$work/ci/common"
cp -r "$app" "$work/ci/app"
rm -rf "$work/ci/app/.cargo"
check "CI layout: on" bash -c 'cd "$1" && common/scripts/use-local-common.sh app on >/dev/null' _ "$work/ci"
check "CI layout: patched to common/" grep -qF "$work/ci/common/crates/piddle-encode" "$work/ci/app/.cargo/config.toml"

echo "$passed passed, $failed failed"
[ "$failed" -eq 0 ]
