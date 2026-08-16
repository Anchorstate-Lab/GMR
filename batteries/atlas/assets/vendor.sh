#!/usr/bin/env bash
# Re-fetch the vendored browser libraries next to this script.
#
# The rendered page is opened from a file:// path with no network, so these are
# committed rather than linked. Bump a version here, run this, commit the diff.
#
# Load order is a dependency chain and the page must keep it:
#   layout-base -> cose-base -> cytoscape -> cytoscape-fcose (self-registers)
set -euo pipefail

cd "$(dirname "$0")"

CYTOSCAPE=3.34.1
LAYOUT_BASE=2.0.1
COSE_BASE=2.2.0
FCOSE=2.2.0

fetch() {
    local url=$1 out=$2
    curl -fsSL --max-time 120 "$url" -o "$out"
    # jsDelivr appends a source map pointing at its own host; nothing can reach it
    # from a file:// page, so drop the line rather than ship a dead reference.
    sed -i '' -e '/^\/\/# sourceMappingURL=/d' "$out" 2>/dev/null ||
        sed -i -e '/^\/\/# sourceMappingURL=/d' "$out"
    printf '%-24s %8s  %s\n' "$out" "$(wc -c <"$out" | tr -d ' ')" "$url"
}

fetch "https://cdn.jsdelivr.net/npm/layout-base@${LAYOUT_BASE}/layout-base.min.js" layout-base.min.js
fetch "https://cdn.jsdelivr.net/npm/cose-base@${COSE_BASE}/cose-base.min.js" cose-base.min.js
fetch "https://cdn.jsdelivr.net/npm/cytoscape@${CYTOSCAPE}/dist/cytoscape.min.js" cytoscape.min.js
fetch "https://cdn.jsdelivr.net/npm/cytoscape-fcose@${FCOSE}/cytoscape-fcose.min.js" cytoscape-fcose.min.js
