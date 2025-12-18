#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "Building layered-nlp-demo-wasm..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack is not installed. Install with: cargo install wasm-pack"
    exit 1
fi

# Build WASM with wasm-pack targeting web
wasm-pack build layered-nlp-demo-wasm \
    --release \
    --target web \
    --out-name layered_nlp_demo \
    --out-dir ../web/pkg

echo "WASM build complete! Output in web/pkg/"
echo ""
echo "To test locally:"
echo "  cd web && python3 -m http.server 8080"
echo "  Then open http://localhost:8080/contract-viewer.html"
