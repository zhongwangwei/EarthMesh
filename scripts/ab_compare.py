#!/usr/bin/env python3
"""A/B exact-output regression: run one project with two CLI binaries and
compare the final gridfiles variable by variable, plus the `refine_*` lines.

Usage:
    scripts/ab_compare.py BASE_BIN NEW_BIN PROJECT.yaml WORKDIR [CASE_NAME] [--all-artifacts] [--reuse-base]

Each binary runs `--project project.yaml` in WORKDIR/CASE/{base,new}. The
summary line is printed as JSON and appended to WORKDIR/results.jsonl:
`grid_diffs` lists every differing variable or global attribute (empty means
identical); `refine_lines_equal` compares the sorted `refine_*` stdout lines.
With --all-artifacts every file under each run directory is compared as well
(`artifact_diffs`): NetCDF by variable, JSON after dropping timing fields and
normalising the run-directory path and nested scratch-directory names, anything
else byte for byte after the same normalisation. With --reuse-base the base
run is kept from an earlier call with the same base binary (its record is
WORKDIR/CASE/base/base_run.json) and only the new binary runs, from a fresh
directory; NEW_BIN `-` only records the base run. Needs netCDF4 and numpy. See docs/architecture_layering_audit_2026-09-25.md
section 6 for how it is used.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time

import netCDF4
import numpy as np

# Counters a newer binary prints that an older one does not; ignored on both
# sides so they do not read as a difference.
IGNORED_REFINE_PREFIXES = ("refine_hfield_dropped_",)


def run(binary, directory, project):
    if os.path.isdir(directory):
        shutil.rmtree(directory)
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


# Scratch directories a run nests inside its run root carry their own pid and
# timestamp, e.g. `project.nml.earthmesh-lowered-11069-1790495697509485000-1`.
SCRATCH_NAME = re.compile(r"earthmesh-(run|lowered)-\d+-\d+-\d+")


# Any run directory, not only this side's own: a base kept from another
# working directory (--reuse-base after a copy) names its original one.
ANY_RUN_ROOT = re.compile(r"/[^\s\"']*?/project\.yaml\.earthmesh-run-[^/\s\"']*")


def normalised_text(text, root):
    text = ANY_RUN_ROOT.sub("<RUN>", text.replace(root, "<RUN>"))
    return SCRATCH_NAME.sub(r"earthmesh-\1-N", text)


VOLATILE_JSON_KEYS = {"certification_elapsed_ms", "elapsed_ms", "elapsed_seconds", "wall_seconds"}


def run_root(directory):
    """The single `project.yaml.earthmesh-run-*` directory a project run leaves."""
    roots = [name for name in os.listdir(directory) if name.startswith("project.yaml.earthmesh-run-")]
    return os.path.join(directory, roots[0]) if len(roots) == 1 else None


def normalised_json(value, root):
    if isinstance(value, dict):
        kept = {k: normalised_json(v, root) for k, v in value.items() if k not in VOLATILE_JSON_KEYS}
        # A manifest lists absolute run paths, so its byte size tracks the
        # run directory's name length, not the content.
        if isinstance(kept.get("artifact_bytes"), dict):
            kept["artifact_bytes"].pop("manifest", None)
        return kept
    if isinstance(value, list):
        return [normalised_json(v, root) for v in value]
    if isinstance(value, str):
        return normalised_text(value, root)
    return value


def artifact_differences(base_dir, new_dir):
    base_root, new_root = run_root(base_dir), run_root(new_dir)
    if not base_root or not new_root:
        return ["run directory not found"]
    def files(root):
        out = set()
        for dirpath, _, names in os.walk(root):
            for name in names:
                out.add(os.path.relpath(os.path.join(dirpath, name), root))
        return out
    base_files, new_files = files(base_root), files(new_root)
    differences = [f"only in base: {p}" for p in sorted(base_files - new_files)]
    differences += [f"only in new: {p}" for p in sorted(new_files - base_files)]
    for rel in sorted(base_files & new_files):
        a, b = os.path.join(base_root, rel), os.path.join(new_root, rel)
        if rel.endswith((".nc", ".nc4")):
            diffs = netcdf_differences(a, b)
            if diffs:
                differences.append(f"{rel}: {diffs[:5]}")
        elif rel.endswith(".json"):
            try:
                x = normalised_json(json.load(open(a)), base_root)
                y = normalised_json(json.load(open(b)), new_root)
            except ValueError:
                x, y = open(a, "rb").read(), open(b, "rb").read()
            if x != y:
                differences.append(f"{rel}: json differs")
        elif rel.endswith((".log", ".nml")) or os.path.basename(rel) == "run.log":
            continue
        else:
            x = normalised_text(open(a, "rb").read().decode("latin-1"), base_root)
            y = normalised_text(open(b, "rb").read().decode("latin-1"), new_root)
            if x != y:
                differences.append(f"{rel}: bytes differ")
    return differences


def base_run(base_bin, directory, project, reuse):
    """The base binary's run, or the one recorded for it when reusing."""
    record = os.path.join(directory, "base_run.json")
    if reuse and os.path.exists(record):
        recorded = json.load(open(record))
        if recorded.get("binary") == base_bin and recorded.get("project") == open(project).read():
            return recorded["run"]
    result = run(base_bin, directory, project)
    with open(record, "w") as out:
        json.dump({"binary": base_bin, "project": open(project).read(), "run": result}, out)
    return result


def main():
    flags = {"--all-artifacts", "--reuse-base"}
    args = [a for a in sys.argv[1:] if a not in flags]
    all_artifacts = "--all-artifacts" in sys.argv[1:]
    reuse = "--reuse-base" in sys.argv[1:]
    if len(args) not in (4, 5):
        sys.exit(__doc__)
    base_bin, project, workdir = (os.path.abspath(args[i]) for i in (0, 2, 3))
    case = args[4] if len(args) == 5 else os.path.splitext(os.path.basename(project))[0]
    base = base_run(base_bin, os.path.join(workdir, case, "base"), project, reuse)
    if args[1] == "-":
        print(json.dumps({"case": case, "base_rc": base["rc"], "base_s": base["secs"]}))
        return
    new = run(os.path.abspath(args[1]), os.path.join(workdir, case, "new"), project)
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
    if all_artifacts:
        summary["artifact_diffs"] = artifact_differences(
            os.path.join(workdir, case, "base"), os.path.join(workdir, case, "new")
        )
    if base["rc"] or new["rc"]:
        summary["base_tail"], summary["new_tail"] = base["tail"], new["tail"]
    line = json.dumps(summary)
    print(line)
    with open(os.path.join(workdir, "results.jsonl"), "a") as results:
        results.write(line + "\n")


if __name__ == "__main__":
    main()
