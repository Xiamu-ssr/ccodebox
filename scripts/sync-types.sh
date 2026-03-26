#!/bin/bash
# Regenerate TypeScript types from Rust backend and sync to frontend.
# Run from repo root: ./scripts/sync-types.sh

set -euo pipefail

echo "[sync-types] Running cargo test to generate bindings..."
cd backend
cargo test --quiet

echo "[sync-types] Copying bindings to frontend..."
cp bindings/*.ts ../frontend/src/lib/generated/

echo "[sync-types] Done. Generated types:"
ls -1 ../frontend/src/lib/generated/*.ts
