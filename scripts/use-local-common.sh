#!/usr/bin/env bash
# Build an app (piddle, kpiddle) against this piddle-common checkout instead
# of the commit it pins, to change shared code and the app together without
# tagging or pushing. Each app's scripts/use-local-common.sh calls this, so
# there is one copy of the logic; piddle-common's CI uses it too, and
# scripts/test-use-local-common.sh tests it.
#   scripts/use-local-common.sh APP_DIR on    build APP_DIR against this checkout
#   scripts/use-local-common.sh APP_DIR off   back to the pinned commit
#
# "on" adds a marked [patch] block for this checkout's crates to
# APP_DIR/.cargo/config.toml (gitignored there); "off" removes that block.
# Anything else in the file is left alone, the file is only deleted when
# nothing else is in it, and it is replaced atomically. Anything this script
# can't take apart safely (damaged markers, a patch table of the app's own,
# an old-style file with additions) is refused and left unchanged.
#
# Cargo.lock: while patched, Cargo records these crates without their git
# source. "off" re-resolves once against the pinned commit, which puts the
# source back; it doesn't restore an earlier copy of the file, so check
# `git diff Cargo.lock` before committing it. Cargo still fetches the pinned
# commit (it must be on GitHub) even while patched.
set -euo pipefail
common=$(cd "$(dirname "$0")/.." && pwd)
usage() {
  echo "usage: $0 APP_DIR on|off" >&2
  exit 2
}
[ $# -eq 2 ] || usage
case "$2" in on | off) ;; *) usage ;; esac
root=$(cd "$1" && pwd)
if ! grep -qF 'github.com/scott-hiemstra/piddle-common' "$root/Cargo.toml" 2>/dev/null; then
  echo "error: $root doesn't pin piddle-common in its Cargo.toml" >&2
  exit 1
fi
config="$root/.cargo/config.toml"
begin="# >>> use-local-common.sh (remove with: scripts/use-local-common.sh off)"
end="# <<< use-local-common.sh"
table='[patch."https://github.com/scott-hiemstra/piddle-common"]'
legacy='# Written by scripts/use-local-common.sh'

fail() {
  echo "error: $*" >&2
  echo "$config is unchanged." >&2
  exit 1
}

# A TOML basic string: backslashes and double quotes escaped.
toml_str() {
  local s=${1//\\/\\\\}
  printf '"%s"' "${s//\"/\\\"}"
}

# Our block must be absent, or exactly one begin marker followed later by
# one end marker; otherwise we can't tell what is ours.
check_markers() {
  [ -f "$config" ] || return 0
  local nb ne first_b first_e
  nb=$(grep -cxF "$begin" "$config" || true)
  ne=$(grep -cxF "$end" "$config" || true)
  [ "$nb" -eq 0 ] && [ "$ne" -eq 0 ] && return 0
  if [ "$nb" -ne 1 ] || [ "$ne" -ne 1 ]; then
    fail "its use-local-common.sh markers are damaged ($nb start, $ne end); fix them by hand"
  fi
  first_b=$(grep -nxF "$begin" "$config" | cut -d: -f1)
  first_e=$(grep -nxF "$end" "$config" | cut -d: -f1)
  [ "$first_b" -lt "$first_e" ] || fail "its use-local-common.sh end marker comes before the start; fix it by hand"
}

# A config written by the old version of this script (no markers, the whole
# file was generated) can be taken over only if it still holds nothing but
# what that version wrote: its header, the patch table and the crates' path
# lines. Anything added since might be lost, so then refuse and leave it.
check_legacy() {
  [ -f "$config" ] && head -1 "$config" | grep -qF "$legacy" || return 0
  local extra
  extra=$(grep -vxE "${legacy}.*|\\[patch\.\"https://github\.com/scott-hiemstra/piddle-common\"\\]|piddle-[a-z]+ = \\{ path = \".*\" \\}|[[:space:]]*" "$config" || true)
  if [ -n "$extra" ]; then
    echo "error: $config was written by an older version of this script and has other settings added since:" >&2
    printf '%s\n' "$extra" | sed 's/^/  /' >&2
    echo "Remove its first lines (the header, the $table table and its piddle-* path lines) by hand, then run this again." >&2
    exit 1
  fi
}

# The config without our block (empty if there is no config).
others() {
  [ -f "$config" ] || return 0
  # A pure old-style file (see check_legacy) is all ours.
  if head -1 "$config" | grep -qF "$legacy"; then
    return 0
  fi
  awk -v b="$begin" -v e="$end" '$0 == b { skip = 1; next } $0 == e { skip = 0; next } !skip' "$config"
}

# Write $1 as the config through a temporary file and a rename, or remove
# the file (and an empty .cargo) if $1 has nothing but blank lines.
write_config() {
  if [ -z "$(printf '%s' "$1" | tr -d '[:space:]')" ]; then
    rm -f "$config"
    rmdir "$root/.cargo" 2>/dev/null || true
    return
  fi
  mkdir -p "$root/.cargo"
  local tmp
  tmp=$(mktemp "$root/.cargo/.config.toml.XXXXXX")
  printf '%s\n' "$1" >"$tmp"
  [ -f "$config" ] && chmod --reference="$config" "$tmp" 2>/dev/null || chmod 644 "$tmp"
  mv -f "$tmp" "$config"
}

check_markers
check_legacy
case "$2" in
  on)
    rest=$(others)
    # A second table with the same name would make the file invalid TOML.
    if grep -qF "$table" <<<"$rest"; then
      fail "it already has its own $table table; merge the paths into it by hand"
    fi
    block=$(
      echo "$begin"
      echo "$table"
      for c in "$common"/crates/*/; do
        c=$(basename "$c")
        echo "$c = { path = $(toml_str "$common/crates/$c") }"
      done
      echo "$end"
    )
    if [ -n "$(printf '%s' "$rest" | tr -d '[:space:]')" ]; then
      write_config "$rest"$'\n\n'"$block"
    else
      write_config "$block"
    fi
    echo "building against $common"
    ;;
  off)
    write_config "$(others)"
    # Put the crates' git source back into Cargo.lock (see the top).
    (cd "$root" && cargo metadata --format-version 1 >/dev/null)
    echo "building against the pinned piddle-common commit; check git diff Cargo.lock before committing it"
    ;;
esac
