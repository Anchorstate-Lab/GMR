#!/usr/bin/env sh
set -eu

# Usage: create_platform_npm.sh <tarball> <npm_package_name> <version> <outdir>
# Example: create_platform_npm.sh artifacts/gmr-x86_64-unknown-linux-gnu.tar.gz @zongming_he/gmr-linux-x64 0.1.0 out

tarball=$1
pkgname=$2
version=$3
outdir=${4:-out}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$outdir"

tar -xzf "$tarball" -C "$tmp"

pkgdir="$tmp/pkg"
mkdir -p "$pkgdir/bin"
cp -r "$tmp/bin/"* "$pkgdir/bin/"

cat > "$pkgdir/package.json" <<JSON
{
  "name": "$pkgname",
  "version": "$version",
  "bin": { "gmr": "bin/gmr" },
  "files": ["bin/gmr"],
  "license": "MIT"
}
JSON

cd "$pkgdir"
npm pack --pack-destination "$PWD/../$outdir"

echo "Packed $pkgname @$version into $outdir"
