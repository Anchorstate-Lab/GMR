#!/usr/bin/env sh
set -eu

# Usage: create_platform_npm.sh <tarball> <npm_package_name> <version> <outdir>
# Example: create_platform_npm.sh artifacts/gmr-x86_64-unknown-linux-gnu.tar.gz @anchorstate-lab/gmr-linux-x64 0.1.0 out

tarball=$1
pkgname=$2
version=$3
outdir=${4:-out}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$outdir"
outdir=$(cd "$outdir" && pwd)

tar -xzf "$tarball" -C "$tmp"

pkgdir="$tmp/pkg"
mkdir -p "$pkgdir/bin"
cp -r "$tmp/bin/"* "$pkgdir/bin/"

# The addon is optional in the packer and required in practice: a platform
# package without it installs a working CLI and an SDK that cannot load.
files='"bin/gmr"'
if [ -f "$tmp/gmr.node" ]; then
    cp "$tmp/gmr.node" "$pkgdir/gmr.node"
    files='"bin/gmr", "gmr.node"'
fi

cat > "$pkgdir/package.json" <<JSON
{
  "name": "$pkgname",
  "version": "$version",
  "bin": { "gmr": "bin/gmr" },
  "files": [$files],
  "license": "MIT"
}
JSON

cd "$pkgdir"
npm pack --pack-destination "$outdir"

echo "Packed $pkgname @$version into $outdir"
