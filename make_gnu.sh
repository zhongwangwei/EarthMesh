#!/usr/bin/env bash
# Compatibility wrapper retained for old workflows.
# EarthMesh now builds through Rust/Cargo, so this delegates to the root Makefile.
set -euo pipefail

ulimit -s unlimited
echo "Building EarthMesh with Rust/Cargo..."
make clean
make 2>&1 | tee logmake_rust

if [ -x mkgrd.x ]; then
    echo ""
    echo "=========================================="
    echo "Build successful!"
    echo "Executable: mkgrd.x"
    echo "=========================================="
else
    echo ""
    echo "=========================================="
    echo "Build failed!"
    echo "Check logmake_rust for details"
    echo "=========================================="
    exit 1
fi
