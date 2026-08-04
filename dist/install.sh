#!/bin/sh
# GMR installer. The tarball is the primitive here; npm is one wrapper over it,
# not the only door — a Go or Python team should not need node to use this.
#
#   curl -fsSL https://raw.githubusercontent.com/OWNER/REPO/main/dist/install.sh | sh
#
# Honours GMR_VERSION (default: latest) and GMR_PREFIX (default: ~/.local).
set -eu

repo=${GMR_REPO:-Zongming-He/gmr}
version=${GMR_VERSION:-latest}
prefix=${GMR_PREFIX:-$HOME/.local}

case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux)  os=unknown-linux-gnu ;;
    *)      echo "gmr: no prebuilt binary for $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
    arm64|aarch64) arch=aarch64 ;;
    x86_64|amd64)  arch=x86_64 ;;
    *)             echo "gmr: no prebuilt binary for $(uname -m)" >&2; exit 1 ;;
esac
target=$arch-$os

if [ "$version" = latest ]; then
    url=https://github.com/$repo/releases/latest/download/gmr-$target.tar.gz
else
    url=https://github.com/$repo/releases/download/$version/gmr-$target.tar.gz
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "gmr: fetching $target"
curl -fsSL "$url" -o "$work/gmr.tar.gz" \
    || { echo "gmr: cannot download $url" >&2; exit 1; }
tar -xzf "$work/gmr.tar.gz" -C "$work"

# The probes travel with the binary and are found relative to it, so bin/ and
# probes/ must stay siblings. Installing only the binary would leave every
# `about:` unresolvable.
mkdir -p "$prefix/bin" "$prefix/libexec/gmr"
rm -rf "$prefix/libexec/gmr/probes"
cp -R "$work/probes" "$prefix/libexec/gmr/probes"
cp "$work/bin/gmr" "$prefix/libexec/gmr/gmr"
chmod +x "$prefix/libexec/gmr/gmr"
ln -sf "$prefix/libexec/gmr/gmr" "$prefix/bin/gmr"

echo "gmr: installed to $prefix/bin/gmr"
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) echo "gmr: add $prefix/bin to your PATH" ;;
esac
"$prefix/bin/gmr" --version 2>/dev/null || true
