#!/usr/bin/env bash
# Generate a static copy of the LSF JSON Schema from the running server into the
# mdBook source tree. Requires `jq` and a built `lumina-server`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT_DIR="$ROOT/docs/src/generated"
mkdir -p "$OUT_DIR"

echo "Starting lumina-server…"
cargo run -q -p lumina-server &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT

# Wait for the server to accept connections.
for _ in $(seq 1 30); do
  if curl -fsS localhost:3000/health >/dev/null 2>&1; then break; fi
  sleep 1
done

echo "Fetching /schema → $OUT_DIR/schema.json"
curl -fsS localhost:3000/schema | jq '.' > "$OUT_DIR/schema.json"

echo "Fetching /objects → $OUT_DIR/objects.json"
curl -fsS localhost:3000/objects | jq '.' > "$OUT_DIR/objects.json"

echo "Done."
