#!/usr/bin/env bash
# releaser-strip.sh - try to run `strip --strip-all` on the provided argument

set -u

if [ $# -lt 1 ]; then
    echo "Usage: $(basename "$0") <file> [...]"
    exit 0
fi

STRIP_CMD=$(command -v strip || true)
if [ -z "$STRIP_CMD" ]; then
    echo "Error: 'strip' not found in PATH"
    exit 0
fi

for file in "$@"; do
    if [ -f "$file" ]; then
        echo "Stripping binary: $file"
        "$STRIP_CMD" --strip-all "$file" || {
            echo "Warning: Failed to strip $file"
        }
    else
        echo "Warning: $file is not a valid file"
    fi
done
exit 0
