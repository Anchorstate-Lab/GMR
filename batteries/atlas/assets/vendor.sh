#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

CYTOSCAPE=3.34.1
LAYOUT_BASE=2.0.1
COSE_BASE=2.2.0
FCOSE=2.2.0

fetch() {
    local url=$1 out=$2
    curl -fsSL --max-time 120 "$url" -o "$out"
    sed -i '' -e '/^\/\/# sourceMappingURL=/d' "$out" 2>/dev/null ||
        sed -i -e '/^\/\/# sourceMappingURL=/d' "$out"
    printf '%-24s %8s  %s\n' "$out" "$(wc -c <"$out" | tr -d ' ')" "$url"
}

fetch "https://cdn.jsdelivr.net/npm/layout-base@${LAYOUT_BASE}/layout-base.min.js" layout-base.min.js
fetch "https://cdn.jsdelivr.net/npm/cose-base@${COSE_BASE}/cose-base.min.js" cose-base.min.js
fetch "https://cdn.jsdelivr.net/npm/cytoscape@${CYTOSCAPE}/dist/cytoscape.min.js" cytoscape.min.js
fetch "https://cdn.jsdelivr.net/npm/cytoscape-fcose@${FCOSE}/cytoscape-fcose.min.js" cytoscape-fcose.min.js
