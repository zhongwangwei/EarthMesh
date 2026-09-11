import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
RUNNER = HERE / "run_cmrc_system_regression.py"
spec = importlib.util.spec_from_file_location("runner", RUNNER)
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def write(path, text, mode=0o644):
    path.write_text(text)
    path.chmod(mode)
    return path


class CmrcRunnerTests(unittest.TestCase):
    def setUp(self):
        self.td = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: shutil.rmtree(self.td, ignore_errors=True))
        self.config = write(self.td / "case.nml", """
NL%EXPNME = 'L1'
NL%base_dir = '/old/'
NL%openmp = 99
NL%kept = 'same'
""".lstrip())

    def fake_engine(self, body):
        return write(self.td / "engine.py", "#!/usr/bin/env python3\n" + body, 0o755)

    def manifest(self, cli, cases=None, sha=None):
        data = {"binary_sha256": sha or runner.sha256(cli), "inputs": {}, "cases": cases if cases is not None else [{"name": "L1", "config": str(self.config), "config_sha256": runner.sha256(self.config), "outputs": ["gridfile*.nc4"], "description": "test"}]}
        path = self.td / ("manifest" + str(time.time_ns()) + ".json")
        path.write_text(json.dumps(data))
        return path

    def run_cli(self, manifest, output, cli=None, timeout=5):
        return subprocess.run([sys.executable, "-B", str(RUNNER), "--cli", str(cli or self.td / "engine.py"), "--manifest", str(manifest), "--output", str(output), "--timeout", str(timeout), "--threads", "2", "--rss-limit-gib", "1"], text=True, capture_output=True, timeout=30)

    def test_preflight_rejects_output_and_manifest_shapes(self):
        cli = self.fake_engine("import sys; sys.exit(0)\n")
        out = self.td / "out"; out.mkdir()
        self.assertIn("already exists", self.run_cli(self.manifest(cli), out, cli).stderr)
        self.assertNotEqual(self.run_cli(self.manifest(cli), self.td / "bad'out", cli).returncode, 0)
        self.assertNotEqual(self.run_cli(self.manifest(cli, sha="0" * 64), self.td / "bad-hash", cli).returncode, 0)
        bad = {"name": "../x", "config": str(self.config), "config_sha256": runner.sha256(self.config), "outputs": ["x"], "description": ""}
        self.assertNotEqual(self.run_cli(self.manifest(cli, [bad]), self.td / "bad-name", cli).returncode, 0)
        bad["name"] = "L1"; bad["outputs"] = ["../x"]
        self.assertNotEqual(self.run_cli(self.manifest(cli, [bad]), self.td / "bad-pattern", cli).returncode, 0)
        self.assertNotEqual(self.run_cli(self.manifest(cli, []), self.td / "empty-cases", cli).returncode, 0)
        bad["outputs"] = []
        self.assertNotEqual(self.run_cli(self.manifest(cli, [bad]), self.td / "empty-outputs", cli).returncode, 0)
        self.assertFalse((self.td / "bad-hash").exists())

    def test_preflight_allows_hashed_readonly_symlink_inputs(self):
        cli = self.fake_engine("import sys; sys.exit(0)\n")
        target = write(self.td / "landtype_usgs_update.nc", "pinned")
        link = self.td / "landtype-link.nc"
        link.symlink_to(target)
        data = json.loads(self.manifest(cli).read_text())
        data["inputs"] = {str(link): runner.sha256(target)}
        manifest = self.td / "symlink-manifest.json"
        manifest.write_text(json.dumps(data))
        self.assertIn("matched 0 regular files", self.run_cli(manifest, self.td / "symlink-ok", cli).stderr + json.dumps(json.loads((self.td / "symlink-ok" / "results.json").read_text())))

        data["inputs"] = {str(link): "0" * 64}
        bad_manifest = self.td / "symlink-bad-manifest.json"
        bad_manifest.write_text(json.dumps(data))
        bad = self.run_cli(bad_manifest, self.td / "symlink-bad", cli)
        self.assertNotEqual(bad.returncode, 0)
        self.assertFalse((self.td / "symlink-bad").exists())

    def test_rewrite_preserves_unrelated_fields_and_rejects_crowded_line(self):
        out = self.td / "cfg.nml"
        runner.rewrite_config(self.config, out, "L1", self.td / "runs", 7, runner.sha256(self.config))
        text = out.read_text()
        self.assertIn("NL%base_dir = '" + (self.td / "runs").as_posix() + "/'", text)
        self.assertIn("NL%openmp = 7", text)
        self.assertIn("NL%kept = 'same'", text)
        crowded = write(self.td / "crowded.nml", "NL%EXPNME = 'L1'\nNL%base_dir = '/x/' NL%other = 1\nNL%openmp = 1\n")
        with self.assertRaises(ValueError):
            runner.rewrite_config(crowded, self.td / "bad.nml", "L1", self.td / "runs", 2, runner.sha256(crowded))
        duplicate = write(self.td / "duplicate.nml", "NL%EXPNME = 'L1'\nNL%base_dir = '/x/'\nNL%base_dir = /malformed extra\nNL%openmp = 1\n")
        with self.assertRaises(ValueError):
            runner.rewrite_config(duplicate, self.td / "dup.nml", "L1", self.td / "runs", 2, runner.sha256(duplicate))

    def test_success_missing_output_and_failed_engine_are_recorded(self):
        cli = self.fake_engine("""
import pathlib, re, sys
cfg = open(sys.argv[1]).read()
name = re.search(r"NL%EXPNME\\s*=\\s*'([^']+)'", cfg).group(1)
base = re.search(r"NL%base_dir\\s*=\\s*'([^']+)'", cfg).group(1)
p = pathlib.Path(base) / name / 'result'
p.mkdir(parents=True, exist_ok=True)
(p / 'gridfile001.nc4').write_text('ok')
""")
        out = self.td / "ok"
        r = self.run_cli(self.manifest(cli), out, cli)
        self.assertEqual(r.returncode, 0, r.stderr + r.stdout)
        results = json.loads((out / "results.json").read_text())
        self.assertTrue(results["passed"])
        self.assertEqual(results["scope"], "generation_and_required_output_presence_only")
        one = results["results"][0]
        self.assertEqual(len(one["outputs"]), 1)
        self.assertFalse((out / "logs" / "L1" / "running.json").exists())
        self.assertTrue((out / "logs" / "L1" / "start.json").exists())

        missing_cli = self.fake_engine("import sys; sys.exit(0)\n")
        r_missing = self.run_cli(self.manifest(missing_cli), self.td / "missing", missing_cli)
        self.assertNotEqual(r_missing.returncode, 0)
        missing = json.loads((self.td / "missing" / "results.json").read_text())["results"][0]
        self.assertIn("matched 0 regular files", "\n".join(missing["errors"]))

        fail_cli = self.fake_engine("import sys; sys.exit(3)\n")
        r2 = self.run_cli(self.manifest(fail_cli), self.td / "fail", fail_cli)
        self.assertNotEqual(r2.returncode, 0)
        fail = json.loads((self.td / "fail" / "results.json").read_text())["results"][0]
        self.assertIn("process exited 3", "\n".join(fail["errors"]))

    def assert_dead(self, pid):
        time.sleep(0.5)
        alive = subprocess.run(["ps", "-p", str(pid)], capture_output=True).returncode == 0
        self.assertFalse(alive, f"child process {pid} survived cleanup")

    def test_timeout_kills_sigterm_ignoring_child_group(self):
        cli = self.fake_engine("""
import pathlib, subprocess, sys, time
pidfile = pathlib.Path(__file__).with_name('child.pid')
code = 'import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)'
p = subprocess.Popen([sys.executable, '-c', code])
pidfile.write_text(str(p.pid))
time.sleep(60)
""")
        r = self.run_cli(self.manifest(cli), self.td / "timeout", cli, timeout=1)
        self.assertNotEqual(r.returncode, 0)
        self.assert_dead(int((self.td / "child.pid").read_text()))

    def test_monitor_failure_still_reaps_numeric_exit_and_kills_group(self):
        cli = self.fake_engine("""
import pathlib, subprocess, sys, time
pidfile = pathlib.Path(__file__).with_name('child3.pid')
p = subprocess.Popen([sys.executable, '-c', 'import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)'])
pidfile.write_text(str(p.pid))
time.sleep(60)
""")
        proc = subprocess.Popen([str(cli), str(self.config)], start_new_session=True)
        pidfile = self.td / "child3.pid"
        deadline = time.time() + 5
        while not pidfile.exists() and time.time() < deadline:
            time.sleep(0.05)
        self.assertTrue(pidfile.exists())
        with mock.patch.object(runner, "group_rss", side_effect=RuntimeError("ps broke")):
            with self.assertRaises(RuntimeError):
                runner.wait_bounded(proc, 30, 1024 ** 3)
        self.assertIsInstance(proc.returncode, int)
        self.assert_dead(int(pidfile.read_text()))

    def test_normal_leader_exit_still_kills_leftover_child(self):
        cli = self.fake_engine("""
import pathlib, subprocess, sys
pidfile = pathlib.Path(__file__).with_name('child2.pid')
p = subprocess.Popen([sys.executable, '-c', 'import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)'])
pidfile.write_text(str(p.pid))
""")
        r = self.run_cli(self.manifest(cli), self.td / "leftover", cli)
        self.assertNotEqual(r.returncode, 0)  # no output, but cleanup must still happen
        self.assert_dead(int((self.td / "child2.pid").read_text()))


if __name__ == "__main__":
    unittest.main()
