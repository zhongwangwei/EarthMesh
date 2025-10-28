#!/bin/bash
#
# Build script for GNU Fortran compiler (gfortran)
# Usage: ./make_gnu.sh
#

ulimit -s unlimited

# Use GNU Makeoptions
if [ -f Makeoptions.gnu ]; then
    echo "Using GNU Fortran compiler configuration..."
    cp Makeoptions Makeoptions.intel.bak 2>/dev/null
    cp Makeoptions.gnu Makeoptions
else
    echo "Error: Makeoptions.gnu not found!"
    exit 1
fi

# Clean previous build
echo "Cleaning previous build..."
make clean

# Compile
echo "Compiling with gfortran..."
make 2>&1 | tee logmake_gnu

# Check if compilation was successful
if [ -f mkgrd.x ]; then
    echo ""
    echo "=========================================="
    echo "Compilation successful!"
    echo "Executable: mkgrd.x"
    echo "=========================================="
    exit 0
else
    echo ""
    echo "=========================================="
    echo "Compilation failed!"
    echo "Check logmake_gnu for details"
    echo "=========================================="
    exit 1
fi
