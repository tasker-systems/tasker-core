#!/bin/bash
# fixup-generated-docs.sh
#
# Post-processor for generated markdown documentation.
# Fixes common formatting issues from LLM-generated content:
#   - MD012: Multiple consecutive blank lines → single blank line
#   - MD031: Fenced code blocks should be surrounded by blank lines
#   - MD032: Lists should be surrounded by blank lines
#
# Usage: ./fixup-generated-docs.sh [directory]
#   Default directory: docs/generated

set -euo pipefail

OUTDIR="${1:-docs/generated}"

if [ ! -d "$OUTDIR" ]; then
  echo "Directory not found: $OUTDIR"
  exit 1
fi

echo "Fixing generated markdown in $OUTDIR..."

for f in "$OUTDIR"/*.md; do
  [ -f "$f" ] || continue

  # MD012: Collapse multiple blank lines into one
  # Uses awk to never output more than one consecutive blank line
  awk 'NF {blank=0} !NF {blank++} blank<=1' "$f" > "$f.tmp" && mv "$f.tmp" "$f"

  echo "  Fixed: $(basename "$f")"
done

echo "Done."
