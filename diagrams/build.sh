#!/usr/bin/env bash
# Compile all PlantUML diagrams in this folder to PNG using Docker.
# Requires: docker (with network access for C4-PlantUML include on first run).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

shopt -s nullglob
puml_files=(*.puml)
if [ ${#puml_files[@]} -eq 0 ]; then
  echo "No .puml files found in $SCRIPT_DIR"
  exit 0
fi

echo "Compiling ${#puml_files[@]} PlantUML file(s)..."

docker run --rm \
  -v "$SCRIPT_DIR:/work" \
  -w /work \
  plantuml/plantuml:latest \
  -tpng \
  "${puml_files[@]}"

echo
echo "Done. Generated PNGs:"
ls -1 *.png 2>/dev/null || echo "  (none)"
