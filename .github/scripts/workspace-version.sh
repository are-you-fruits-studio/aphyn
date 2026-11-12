#!/usr/bin/env bash
#
# Read or change the workspace version: `version` under `[workspace.package]` of
# the root Cargo.toml, which every crate inherits via `version.workspace = true`.
#
# Usage:
#   workspace-version.sh get                          print the version
#   workspace-version.sh set <version>                set it (a leading "v" is stripped)
#   workspace-version.sh bump <major|minor|patch>     bump it and print the new version

set -euo pipefail

manifest="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/Cargo.toml"
semver='^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.+-]+)?$'

get_version() {
  local version
  version=$(awk '
    /^[[:space:]]*\[/ { in_section = ($0 ~ /^[[:space:]]*\[workspace\.package\][[:space:]]*$/) }
    in_section && /^[[:space:]]*version[[:space:]]*=/ {
      sub(/^[^"]*"/, ""); sub(/".*$/, ""); print; exit
    }
  ' "$manifest")
  version="${version//$'\r'/}"
  if [[ -z "$version" ]]; then
    echo "No version found under [workspace.package] in $manifest" >&2
    exit 1
  fi
  echo "$version"
}

set_version() {
  local version="${1#v}"
  version="${version//$'\r'/}"
  if [[ ! "$version" =~ $semver ]]; then
    echo "Not a valid crate version: '$version'" >&2
    exit 1
  fi
  local tmp="$manifest.tmp"
  awk -v version="$version" '
    /^[[:space:]]*\[/ { in_section = ($0 ~ /^[[:space:]]*\[workspace\.package\][[:space:]]*$/) }
    in_section && !done && /^[[:space:]]*version[[:space:]]*=/ {
      print "version = \"" version "\""
      done = 1
      next
    }
    { print }
    END { if (!done) exit 3 }
  ' "$manifest" > "$tmp" || {
    rm -f "$tmp"
    echo "No version line found under [workspace.package] in $manifest" >&2
    exit 1
  }
  mv "$tmp" "$manifest"
}

case "${1:-}" in
  get)
    get_version
    ;;
  set)
    [[ $# -eq 2 ]] || { echo "Usage: $0 set <version>" >&2; exit 2; }
    set_version "$2"
    ;;
  bump)
    [[ $# -eq 2 ]] || { echo "Usage: $0 bump <major|minor|patch>" >&2; exit 2; }
    current=$(get_version)
    if [[ ! "$current" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
      echo "Cannot bump '$current': expected a plain X.Y.Z version" >&2
      exit 1
    fi
    major=${BASH_REMATCH[1]} minor=${BASH_REMATCH[2]} patch=${BASH_REMATCH[3]}
    case "$2" in
      major) major=$((major + 1)); minor=0; patch=0 ;;
      minor) minor=$((minor + 1)); patch=0 ;;
      patch) patch=$((patch + 1)) ;;
      *) echo "Unknown bump type: '$2'" >&2; exit 2 ;;
    esac
    set_version "$major.$minor.$patch"
    echo "$major.$minor.$patch"
    ;;
  *)
    echo "Usage: $0 get | set <version> | bump <major|minor|patch>" >&2
    exit 2
    ;;
esac
