#!/usr/bin/env bash
# Compatibility no-op for old compiler-switching workflows.
# EarthMesh now has one supported build path: Rust/Cargo via `make`.
set -euo pipefail

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    echo "Usage: make [BUILD_PROFILE=release|debug]"
    echo "Compiler switching is obsolete; use Rust/Cargo through the root Makefile."
    exit 0
fi

echo "Compiler switching is obsolete; EarthMesh now builds with Rust/Cargo."
echo "Run: make"
exit 0
