# Rust build entrypoint for EarthMesh.
# The executable is built from rust/earthmesh_cli.

CARGO ?= cargo
CLI_MANIFEST = rust/earthmesh_cli/Cargo.toml
CARGO_TARGET_DIR ?= rust/earthmesh_cli/target
GUI_TARGET_DIR ?= gui-tauri/src-tauri/target
BUILD_PROFILE ?= release
EXECUTABLE = mkgrd.x
CLI_FEATURES ?= --features static-netcdf

export CARGO_TARGET_DIR

ifeq ($(BUILD_PROFILE),release)
CARGO_PROFILE_FLAG = --release
CLI_BINARY = $(CARGO_TARGET_DIR)/release/earthmesh_cli
else
CARGO_PROFILE_FLAG =
CLI_BINARY = $(CARGO_TARGET_DIR)/debug/earthmesh_cli
endif

.PHONY: all build build-python build-gui-bundle clean test test-fast test-gui check-gui-js check-architecture check-mesh-quality-views test-slow test-full test-real-hydro release-full-real fmt fmt-gui clippy clippy-gui clippy-full release-check check-method-c-neighbors

all: build

build:
	$(CARGO) build --manifest-path $(CLI_MANIFEST) $(CARGO_PROFILE_FLAG) $(CLI_FEATURES)
	cp $(CLI_BINARY) $(EXECUTABLE)
	@echo 'EarthMesh Rust executable has been built successfully.'
	@echo 'Executable: $(EXECUTABLE)'

# Build the PyPI-compatible wheel. The pyproject enables static-netcdf so the
# installed earthmesh_cli command does not depend on a system NetCDF runtime.
build-python:
	maturin build --release --out dist

# `cargo tauri build` runs the configured sidecar staging hook before bundling.
# Drop the Makefile-wide target override so both workspaces use their own target.
build-gui-bundle:
	env -u CARGO_TARGET_DIR sh -c 'cd gui-tauri && cargo tauri build --config src-tauri/tauri.bundle.conf.json'

fmt:
	$(CARGO) fmt --manifest-path rust/earthmesh_core/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_geometry/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_hfield/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_mesh/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_quality/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_refine_planner/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_project/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_boundary/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_refine/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_refine_harp_dv/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_refine_method_c/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_refine_redgreen/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_cli/Cargo.toml --check

fmt-gui:
	$(CARGO) fmt --manifest-path gui-tauri/src-tauri/Cargo.toml --check

# Lint gate: deny every clippy + rustc warning. Per-crate `[lints.clippy]` in each
# Cargo.toml already allows the intentionally-kept algorithm-shaped
# signatures/loops in mesh+cli; anything else fails CI.
# `clippy` = no-netcdf crates (CI fast job); `clippy-full` adds cli (needs NetCDF).
clippy:
	$(CARGO) clippy --manifest-path rust/earthmesh_core/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_hfield/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_project/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_boundary/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_refine/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_refine_harp_dv/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_refine_method_c/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_refine_redgreen/Cargo.toml --all-targets -- -D warnings

clippy-gui:
	CARGO_TARGET_DIR=$(GUI_TARGET_DIR) $(CARGO) clippy --manifest-path gui-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings

clippy-full: clippy
	$(CARGO) clippy --manifest-path rust/earthmesh_cli/Cargo.toml --all-targets $(CLI_FEATURES) -- -D warnings

# Fast regression gate: no NetCDF, no GUI — pure Rust crates only. Used by CI's
# `fast` job and as the quick local loop. Builds in seconds (no netcdf-c/HDF5).
test-fast:
	$(CARGO) test --manifest-path rust/earthmesh_core/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_hfield/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_project/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_boundary/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_harp_dv/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_method_c/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_redgreen/Cargo.toml --all-targets

check-gui-js:
	node scripts/check_gui_js.js

check-architecture:
	@if rg -n '^[[:space:]]*pub use .*\*' rust --glob '*.rs'; then \
		echo 'wildcard public re-exports are forbidden'; exit 1; \
	fi
	@if rg -n '#\[deprecated' rust --glob '*.rs'; then \
		echo 'deprecated compatibility facades are forbidden'; exit 1; \
	fi
	@if rg -n -i '\breference\b|reference_' rust --glob '*.rs'; then \
		echo 'source-origin reference naming is forbidden'; exit 1; \
	fi
	@python3 scripts/check_architecture.py .

check-mesh-quality-views:
	CARGO="$(CARGO)" scripts/check_mesh_quality_views.sh

test-gui: check-gui-js
	CARGO_TARGET_DIR=$(GUI_TARGET_DIR) $(CARGO) test --manifest-path gui-tauri/src-tauri/Cargo.toml --all-targets

# Full crate tests (includes cli with static-netcdf — slow first build).
test:
	$(CARGO) test --manifest-path rust/earthmesh_core/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_hfield/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_project/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --all-targets $(CLI_FEATURES)

# Fixture-backed slow tests require EARTHMESH_LANDTYPE, defaulting to the
# repository input/landtype_igbp_update.nc. Missing data is a hard failure.
test-slow:
	$(CARGO) test --manifest-path rust/earthmesh_mesh/Cargo.toml -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_refine_method_c/Cargo.toml -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test mkgrd_mask_restart $(CLI_FEATURES) -- --ignored
	CARGO="$(CARGO)" CLI_FEATURES="$(CLI_FEATURES)" scripts/run_slow_fixture_e2e.sh
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test mkgrd_gridinit $(CLI_FEATURES) run_mkgrd_gridinit_global_matches_canonical_nxp64_gridfile_fixture -- --ignored

test-full: check-method-c-neighbors test test-gui test-slow

# External production-data gate. Kept separate from ordinary CI because it
# requires the mounted MERIT-Hydro/CaMa datasets and a production gridfile.
test-real-hydro:
	CARGO="$(CARGO)" scripts/run_real_hydro_e2e.sh

release-full-real: test-full test-real-hydro

# Release fast gate: format + no-netcdf crates. Run before tagging a release; the
# full gate adds `make test-full` (GUI + CLI/static-netcdf + ignored slow tests) on top.
release-check: check-architecture fmt test-fast
	@echo 'Release fast gate PASSED: fmt clean + core/geometry/hfield/mesh/quality/refine_planner/project green.'
	@echo 'Full gate (needs NetCDF): make test-full'

# End-to-end acceptance on real meshes: run the case, measure quality, assert
# every field, and re-run to prove the bytes repeat. Slower than the unit gates
# and not part of them; run before releasing a refinement change.
regression-boundary:
	bash scripts/run_refinement_boundary_regression.sh

regression-basin-hole:
	bash scripts/run_basin_hole_regression.sh

regression: regression-boundary regression-basin-hole
	@echo 'End-to-end regressions PASSED: polar/dateline boundaries + basin hole.'

check-method-c-neighbors:
	bash rust/earthmesh_mesh/scripts/check-method-c-neighbors.sh

clean:
	$(CARGO) clean --manifest-path $(CLI_MANIFEST)
	rm -f $(EXECUTABLE) logmake logmake_gnu logmake_rust *.o *.mod
	@echo 'Clean complete.'
