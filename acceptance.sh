#!/bin/sh
set -eu

root=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
bundle=$work/bundle
prefix=$work/prefix
repo=$work/repo

fail() {
    echo
    echo "Acceptance failed: $1"
    [ $# -gt 1 ] && { echo "--- actual output ---"; echo "$2"; }
    exit 1
}

step() { echo; echo "── $1"; }

step "build the tarball (release side; needs cargo)"
cargo build --quiet --release -p coding-anchor
mkdir -p "$bundle/bin"
cp "$root/target/release/gmr" "$bundle/bin/gmr"

[ -e "$bundle/probes" ] && fail "tarball contains probes/ — this is the repo's bootstrap data"

step "install the way dist/install.sh does"
mkdir -p "$prefix/bin"
cp "$bundle/bin/gmr" "$prefix/bin/gmr"

gmr="$prefix/bin/gmr"

step "a stranger's TypeScript repo"
mkdir -p "$repo/src"
cat > "$repo/src/auth.ts" <<'EOF'
export function createSession(userId: string, ttl: number): Session {
  return { userId, ttl };
}
export const verify = (token: string) => token.length > 0;
