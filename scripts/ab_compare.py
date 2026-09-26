#!/usr/bin/env python3
"""A/B exact-output regression: run one project with two CLI binaries and
compare the final gridfiles variable by variable, plus the `refine_*` lines.

Usage:
    scripts/ab_compare.py BASE_BIN NEW_BIN PROJECT.yaml WORKDIR [CASE_NAME]

Each binary runs `--project project.yaml` in WORKDIR/CASE/{base,new}. The
summary line is printed as JSON and appended to WORKDIR/results.jsonl:
`grid_diffs` lists every differing variable or global attribute (empty means
identical); `refine_lines_equal` compares the sorted `refine_*` stdout lines.
Needs netCDF4 and numpy. See docs/architecture_layering_audit_2026-09-25.md
section 6 for how it is used.
"""
import json
import os
import re
import subprocess
import sys
import time

import netCDF4
import numpy as np

# Counters a newer binary prints that an older one does not; ignored on both
# sides so they do not read as a difference.
IGNORED_REFINE_PREFIXES = ("refine_hfield_dropped_",)


def run(binary, directory, project):
    os.makedirs(directory, exist_ok=True)
    with open(project) as source, open(os.path.join(directory, "project.yaml"), "w") as target:
        target.write(source.read())
    started = time.time()
    process = subprocess.run(
        [binary, "--project", "project.yaml"], cwd=directory, capture_output=True, text=True
    )
    output = process.stdout + process.stderr
    with open(os.path.join(directory, "run.log"), "w") as log:
        log.write(output)
    grid = re.search(r"^project_final_gridfile=(.*)$", output, re.M)
    refine = sorted(
        line
        for line in output.splitlines()
        if line.startswith("refine_")
        and "gridfile=" not in line
        and not line.startswith(IGNORED_REFINE_PREFIXES)
    )
    return {
        "rc": process.returncode,
        "secs": round(time.time() - started, 1),
        "grid": grid.group(1) if grid else None,
        "refine": refine,
        "tail": output.strip().splitlines()[-1][:200] if output.strip() else "",
    }


def netcdf_differences(left, right):
    a, b = netCDF4.Dataset(left), netCDF4.Dataset(right)
    differences = []
    if set(a.variables) != set(b.variables):
        differences.append(f"variable sets differ: {sorted(set(a.variables) ^ set(b.variables))}")
    for name in sorted(set(a.variables) & set(b.variables)):
        x, y = a.variables[name][:], b.variables[name][:]
        if x.shape != y.shape or not np.array_equal(np.ma.getdata(x), np.ma.getdata(y)):
            differences.append(f"var {name}")
    left_attrs = {key: a.getncattr(key) for key in a.ncattrs()}
    right_attrs = {key: b.getncattr(key) for key in b.ncattrs()}
    for key in sorted(set(left_attrs) | set(right_attrs)):
        x, y = left_attrs.get(key), right_attrs.get(key)
        if x is None or y is None or not np.array_equal(np.asarray(x), np.asarray(y)):
            differences.append(f"attr {key}")
    return differences


def main():
    if len(sys.argv) not in (5, 6):
        sys.exit(__doc__)
    base_bin, new_bin, project, workdir = map(os.path.abspath, sys.argv[1:5])
    case = sys.argv[5] if len(sys.argv) == 6 else os.path.splitext(os.path.basename(project))[0]
    base = run(base_bin, os.path.join(workdir, case, "base"), project)
    new = run(new_bin, os.path.join(workdir, case, "new"), project)
    summary = {
        "case": case,
        "base_rc": base["rc"],
        "new_rc": new["rc"],
        "base_s": base["secs"],
        "new_s": new["secs"],
        "refine_lines_equal": base["refine"] == new["refine"],
    }
    if base["grid"] and new["grid"]:
        summary["grid_diffs"] = netcdf_differences(base["grid"], new["grid"])
    if base["rc"] or new["rc"]:
        summary["base_tail"], summary["new_tail"] = base["tail"], new["tail"]
    line = json.dumps(summary)
    print(line)
    with open(os.path.join(workdir, "results.jsonl"), "a") as results:
        results.write(line + "\n")


if __name__ == "__main__":
    main()
