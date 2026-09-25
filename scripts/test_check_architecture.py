"""Exercise the real architecture gate, including its fail-closed paths."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parent.parent


class ArchitectureGateTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="earthmesh_arch_")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "rust/fixture/src/lib.rs"
        self.source.parent.mkdir(parents=True)
        self.source.write_text("pub fn mesh() {}\n")
        (self.root / "scripts").mkdir()
        shutil.copyfile(ROOT / "scripts/check_architecture.py", self.root / "scripts/check_architecture.py")

    def gate(self, *overrides):
        return subprocess.run(
            ["make", "--no-print-directory", "-f", str(ROOT / "Makefile"),
             "check-architecture", f"EM_ARCH_OUT={self.root / 'hits'}", *overrides],
            cwd=self.root, text=True, capture_output=True, timeout=15,
        )

    def test_numerical_references_and_persisted_keys_are_allowed(self):
        self.source.write_text('''
// A mathematical reference value, not a source-origin namespace.
pub struct Patch { pub reference_positions: Vec<f64> }
fn reference_generate(reference: f64) -> f64 { reference }
const KEY: &str = "earthmesh_mpas_density_reference_width_km";
const DATA: &str = "reference_meshes/mesh.nc";
''')
        result = self.gate()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_source_origin_and_module_names_remain_forbidden(self):
        for source in [
            "// the Fortran reference", "// the v2 reference",
            "// source-origin reference", "fn reference_fortran() {}",
            "fn v2_reference_mesh() {}", "fn source_origin_reference() {}",
            "mod reference_mesh;", "pub(crate) mod reference_mesh;",
            "mod reference;", "mod reference {}", "mod r#reference_mesh;",
        ]:
            with self.subTest(source=source):
                self.source.write_text(source + "\n")
                result = self.gate()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("source-origin reference naming is forbidden", result.stdout)

    def test_other_source_rules_remain_forbidden(self):
        for source, message in [
            ("pub use mesh::*;", "wildcard public re-exports are forbidden"),
            ("#[deprecated] pub fn compatibility() {}", "deprecated compatibility facades are forbidden"),
        ]:
            with self.subTest(source=source):
                self.source.write_text(source + "\n")
                result = self.gate()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stdout)

    def test_single_child_forwarding_remains_forbidden(self):
        wrapper = self.source.parent / "wrapper"
        wrapper.mkdir()
        (wrapper / "mod.rs").write_text("mod child; pub use child::Mesh;\n")
        (wrapper / "child.rs").write_text("pub struct Mesh;\n")
        result = self.gate()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("single-child forwarding directory", result.stdout)

    def write(self, relative, text):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def test_cli_output_code_may_not_name_a_backend(self):
        self.write("rust/earthmesh_cli/src/writer.rs",
                   "use earthmesh_refine_redgreen::RedGreenMesh;\n")
        result = self.gate()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("outside the backend adapters", result.stdout)
        self.assertIn("writer.rs", result.stdout)

    def test_cli_adapters_may_name_a_backend_and_comments_never_count(self):
        self.write("rust/earthmesh_cli/src/redgreen_bridge.rs",
                   "use earthmesh_refine_redgreen::RedGreenMesh;\n")
        self.write("rust/earthmesh_cli/src/refine_pipeline/global_source.rs",
                   "fn f() { earthmesh_refine_method_c::run(); }\n")
        self.write("rust/earthmesh_cli/src/writer.rs",
                   "/// Unlike `earthmesh_refine_certified`, this names nothing.\npub fn w() {}\n")
        result = self.gate()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_foundation_and_request_crates_may_not_depend_on_a_backend(self):
        self.write("rust/earthmesh_refine/Cargo.toml",
                   '[dependencies]\nearthmesh_refine_method_c = { path = "../x" }\n')
        result = self.gate()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must not depend on one", result.stdout)
        (self.root / "rust/earthmesh_refine/Cargo.toml").write_text("[dependencies]\n")
        self.write("rust/earthmesh_mesh/src/lib.rs",
                   "pub fn f() { earthmesh_refine_certified::g(); }\n")
        result = self.gate()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must not depend on a refinement backend", result.stdout)

    def test_grep_errors_fail_closed(self):
        for status in [2, 127]:
            with self.subTest(status=status):
                tool = self.root / "failed-grep.sh"
                tool.write_text(f"exit {status}\n")
                result = self.gate(f"EM_ARCH_GREP=sh {tool}")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(f"grep exited {status}", result.stdout)

    def test_unwritable_report_fails_with_or_without_a_match(self):
        shells = [[]]
        if dash := shutil.which("dash"):
            shells.append([f"SHELL={dash}"])
        for shell in shells:
            for source in ["pub fn mesh() {}", "pub use mesh::*;"]:
                with self.subTest(shell=shell, source=source):
                    self.source.write_text(source + "\n")
                    result = self.gate(f"EM_ARCH_OUT={self.root}", *shell)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("cannot write", result.stdout)


if __name__ == "__main__":
    unittest.main()
