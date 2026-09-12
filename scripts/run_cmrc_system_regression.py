#!/usr/bin/env python3
"""Bounded CMRC system-regression runner."""

import argparse
import datetime as dt
import glob
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import time

GIB = 1024 ** 3
SAFE_NAME = re.compile(r"[A-Za-z0-9_.-]+")
SHA = re.compile(r"[0-9a-f]{64}")
SCOPE = "generation_and_required_output_presence_only"


def utc(): return dt.datetime.now(dt.timezone.utc).isoformat()


def sha256(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def require_sha(s, field):
    if not isinstance(s, str) or not SHA.fullmatch(s):
        raise ValueError(f"{field} must be a lowercase sha256 hex string")
    return s


def safe_child(name):
    if not isinstance(name, str) or not SAFE_NAME.fullmatch(name) or name in {".", ".."}:
        raise ValueError(f"unsafe case name: {name!r}")
    return name


def abs_file(path, field):
    if not isinstance(path, str) or not os.path.isabs(path):
        raise ValueError(f"{field} must be an absolute path")
    p = Path(path)
    if not p.is_file():
        raise ValueError(f"{field} is not a regular file: {path}")
    return p


def check_hash(path, expected, field):
    expected = require_sha(expected, field)
    got = sha256(path)
    if got != expected:
        raise ValueError(f"{field} hash mismatch for {path}: {got} != {expected}")
    return got


def validate_output_arg(path):
    if not os.path.isabs(path):
        raise SystemExit("--output must be absolute")
    if "'" in path or "\n" in path:
        raise SystemExit("--output cannot contain quote or newline")
    if os.path.lexists(path):
        raise SystemExit("--output already exists or is a symlink")
    return Path(path)


def validate_pattern(pattern):
    if not isinstance(pattern, str) or not pattern:
        raise ValueError("output pattern must be a non-empty string")
    p = Path(pattern)
    if p.is_absolute() or ".." in p.parts:
        raise ValueError(f"unsafe output pattern: {pattern!r}")
    return pattern


def load_manifest(path, cli):
    manifest = json.loads(abs_file(path, "manifest").read_text())
    if not isinstance(manifest, dict):
        raise ValueError("manifest must be an object")
    check_hash(cli, manifest.get("binary_sha256"), "binary")
    inputs = manifest.get("inputs")
    cases = manifest.get("cases")
    if not isinstance(inputs, dict):
        raise ValueError("inputs must be an object")
    if not isinstance(cases, list) or not cases:
        raise ValueError("cases must be a non-empty list")
    for raw, expected in inputs.items():
        check_hash(abs_file(raw, "input"), expected, "input")
    names = set()
    for i, case in enumerate(cases):
        if not isinstance(case, dict):
            raise ValueError(f"cases[{i}] must be an object")
        name = safe_child(case.get("name"))
        if name in names:
            raise ValueError(f"duplicate case name: {name}")
        names.add(name)
        case["config"] = str(abs_file(case.get("config"), f"cases[{i}].config"))
        check_hash(Path(case["config"]), case.get("config_sha256"), "config")
        outputs = case.get("outputs")
        if not isinstance(outputs, list) or not outputs:
            raise ValueError(f"case {name} outputs must be a non-empty list")
        for pattern in outputs:
            validate_pattern(pattern)
        if not isinstance(case.get("description", ""), str):
            raise ValueError(f"case {name} description must be a string")
    return manifest


def setting_value(text, key):
    if len(re.findall(rf"(?im)^\s*{re.escape(key)}\s*=", text)) != 1:
        raise ValueError(f"expected exactly one {key} assignment")
    matches = re.findall(rf"(?im)^\s*{re.escape(key)}\s*=\s*('[^'\n]*'|\"[^\"\n]*\"|[A-Za-z0-9_.+-]+)\s*(?:!.*)?$", text)
    if len(matches) != 1:
        raise ValueError(f"expected exactly one simple single-line {key} assignment")
    value = matches[0]
    return value[1:-1] if value[:1] in "'\"" else value


def replace_setting(text, key, value):
    if len(re.findall(rf"(?im)^\s*{re.escape(key)}\s*=", text)) != 1:
        raise ValueError(f"expected exactly one {key} assignment")
    pat = rf"(?im)^(\s*{re.escape(key)}\s*=\s*)('[^'\n]*'|\"[^\"\n]*\"|[A-Za-z0-9_.+-]+)(\s*(?:!.*)?)$"
    text, count = re.subn(pat, lambda m: f"{m.group(1)}{value}{m.group(3)}", text)
    if count != 1:
        raise ValueError(f"expected exactly one simple single-line {key} assignment")
    return text


def rewrite_config(src, dst, name, runs_dir, threads, expected_sha):
    before = check_hash(src, expected_sha, "config before rewrite")
    out = runs_dir.as_posix() + "/"
    if "'" in out or "\n" in out:
        raise ValueError("output path cannot be safely written to namelist")
    text = src.read_text()
    if setting_value(text, "NL%EXPNME") != name:
        raise ValueError(f"NL%EXPNME does not match case name {name!r}")
    text = replace_setting(text, "NL%base_dir", f"'{out}'")
    text = replace_setting(text, "NL%openmp", str(threads))
    dst.write_text(text)
    return before, sha256(dst)


def group_pids(pgid):
    try:
        rows = subprocess.check_output(["/bin/ps", "-axo", "pid=,pgid=,rss="], text=True)
    except Exception:
        rows = subprocess.check_output(["ps", "-axo", "pid=,pgid=,rss="], text=True)
    out = []
    for row in rows.splitlines():
        parts = row.split()
        if len(parts) == 3:
            try:
                pid, group, rss = map(int, parts)
            except ValueError:
                continue
            if group == pgid:
                out.append((pid, rss * 1024))
    return out


def group_rss(pgid): return sum(rss for _, rss in group_pids(pgid))


def signal_group(pgid, sig):
    try:
        os.killpg(pgid, sig)
    except ProcessLookupError:
        pass


def cleanup_group(proc):
    signal_group(proc.pid, signal.SIGTERM)
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass
    signal_group(proc.pid, signal.SIGKILL)
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass
    return proc.returncode


def wait_bounded(proc, timeout, rss_limit_bytes):
    started = time.monotonic(); peak = 0; limit_error = None
    try:
        while proc.poll() is None:
            wall = time.monotonic() - started
            rss = group_rss(proc.pid); peak = max(peak, rss)
            if wall > timeout:
                limit_error = f"timeout after {timeout}s"; break
            if rss > rss_limit_bytes:
                limit_error = f"RSS limit exceeded ({rss} > {rss_limit_bytes})"; break
            try: proc.wait(timeout=1)
            except subprocess.TimeoutExpired: pass
    except BaseException:
        cleanup_group(proc)
        raise
    code = cleanup_group(proc)  # engine may exit while children remain in its owned group
    return code, time.monotonic() - started, peak, limit_error


def validate_outputs(result_dir, patterns):
    found = []
    root = result_dir.resolve()
    for pattern in patterns:
        matches = [Path(p) for p in glob.glob(str(result_dir / validate_pattern(pattern)))]
        good = []
        for p in matches:
            try:
                rp = p.resolve()
                if rp.is_file() and not p.is_symlink() and (rp == root or root in rp.parents):
                    good.append(p)
            except FileNotFoundError:
                pass
        if len(good) != 1:
            raise ValueError(f"output pattern {pattern!r} matched {len(good)} regular files under {result_dir}")
        found.append({"pattern": pattern, "path": str(good[0]), "sha256": sha256(good[0])})
    return found


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2) + "\n")


def run_case(cli, case, out, threads, timeout, rss_limit_bytes, binary_expected):
    name = safe_child(case["name"])
    logs = out / "logs" / name; logs.mkdir(parents=True, exist_ok=False)
    runs = out / "runs"; cfg_dst = out / "configs" / f"{name}.nml"
    running = logs / "running.json"
    result = {"name": name, "description": case.get("description", ""), "started_utc": utc(), "input_config": case["config"], "input_config_sha256": case["config_sha256"], "threads": threads, "errors": []}
    rethrow = None
    try:
        result["binary_sha256_before"] = check_hash(cli, binary_expected, "binary before run")
        src_sha, cfg_sha = rewrite_config(Path(case["config"]), cfg_dst, name, runs, threads, case["config_sha256"])
        command = [str(cli), str(cfg_dst), "--max-tris", "20000000"]
        result.update({"source_config_sha256_before": src_sha, "generated_config_sha256": cfg_sha, "command": command, "cwd": str(runs)})
        write_json(logs / "start.json", result)
        env = os.environ.copy()
        for key in list(env):
            if key.startswith("EARTHMESH_CMRC_"):
                del env[key]
        env.update({"EARTHMESH_CMRC_TIMING": "1", "RAYON_NUM_THREADS": str(threads), "OMP_NUM_THREADS": str(threads), "OMP_STACKSIZE": "512M", "OPENBLAS_NUM_THREADS": "1"})
        proc = None
        with (logs / "stdout.log").open("wb") as stdout, (logs / "stderr.log").open("wb") as stderr:
            proc = subprocess.Popen(command, cwd=runs, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
            try:
                write_json(running, dict(result, pid=proc.pid))
            except BaseException:
                cleanup_group(proc)
                raise
            code, wall, peak, limit_error = wait_bounded(proc, timeout, rss_limit_bytes)
        result.update({"exit_code": code, "wall_seconds": wall, "peak_rss_bytes": peak})
        if limit_error: result["errors"].append(limit_error)
        result["binary_sha256_after"] = check_hash(cli, binary_expected, "binary after run")
        result["source_config_sha256_after"] = check_hash(Path(case["config"]), case["config_sha256"], "config after run")
        result["generated_config_sha256_after"] = sha256(cfg_dst)
        if result["generated_config_sha256_after"] != cfg_sha:
            result["errors"].append("generated config changed after launch")
        try: result["outputs"] = validate_outputs(runs / name / "result", case["outputs"])
        except Exception as e: result["errors"].append(f"{type(e).__name__}: {e}")
        if code != 0: result["errors"].append(f"process exited {code}")
    except BaseException as e:
        result["errors"].append(f"{type(e).__name__}: {e}")
        if isinstance(e, KeyboardInterrupt):
            rethrow = e
    finally:
        result["passed"] = not result["errors"]
        result["ended_utc"] = utc()
        write_json(logs / "result.json", result)
        try: running.unlink()
        except FileNotFoundError: pass
    if rethrow: raise rethrow
    return result


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--cli", required=True); ap.add_argument("--manifest", required=True); ap.add_argument("--output", required=True)
    ap.add_argument("--threads", type=int, default=16); ap.add_argument("--timeout", type=int, default=900); ap.add_argument("--rss-limit-gib", type=float, default=48)
    args = ap.parse_args(argv)
    cli = abs_file(args.cli, "cli"); out = validate_output_arg(args.output)
    if args.threads < 1 or args.timeout < 1 or args.rss_limit_gib <= 0:
        raise SystemExit("threads, timeout, and rss limit must be positive")
    try: manifest = load_manifest(args.manifest, cli)
    except Exception as e: raise SystemExit(f"preflight failed: {e}")
    out.mkdir(); (out / "configs").mkdir(); (out / "runs").mkdir(); (out / "logs").mkdir()
    manifest_sha = sha256(args.manifest); shutil.copy2(args.manifest, out / "manifest.json")
    results = []
    try:
        for case in manifest["cases"]:
            results.append(run_case(cli, case, out, args.threads, args.timeout, int(args.rss_limit_gib * GIB), manifest["binary_sha256"]))
    except KeyboardInterrupt:
        write_json(out / "results.json", {"passed": False, "interrupted": True, "scope": SCOPE, "manifest_sha256": manifest_sha, "binary_sha256": manifest["binary_sha256"], "results": results, "ended_utc": utc()})
        raise
    aggregate = {"passed": bool(results) and all(r.get("passed") for r in results), "scope": SCOPE, "manifest_sha256": manifest_sha, "binary_sha256": manifest["binary_sha256"], "results": results, "ended_utc": utc()}
    write_json(out / "results.json", aggregate)
    return 0 if aggregate["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
