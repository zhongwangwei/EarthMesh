#!/usr/bin/env bash
set -euo pipefail

ulimit -s unlimited
make "$@" 2>&1 | tee logmake
wait
echo "make finish"
