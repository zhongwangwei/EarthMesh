# Rust build entrypoint for EarthMesh.
# The executable is built from rust/earthmesh_cli. Legacy Fortran sources are
# archived outside the active tree and tracked only by the migration manifest.

CARGO ?= cargo
CLI_MANIFEST = rust/earthmesh_cli/Cargo.toml
CARGO_TARGET_DIR ?= rust/earthmesh_cli/target
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

.PHONY: all build clean test test-fast test-slow test-full fmt clippy clippy-full release-check check-method-c-neighbors

all: build

build:
	$(CARGO) build --manifest-path $(CLI_MANIFEST) $(CARGO_PROFILE_FLAG) $(CLI_FEATURES)
	cp $(CLI_BINARY) $(EXECUTABLE)
	@echo 'EarthMesh Rust executable has been built successfully.'
	@echo 'Executable: $(EXECUTABLE)'

fmt:
	$(CARGO) fmt --manifest-path rust/earthmesh_core/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_geometry/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_mesh/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_quality/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_refine_planner/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_cli/Cargo.toml --check

# Lint gate: deny every clippy + rustc warning. Per-crate `[lints.clippy]` in each
# Cargo.toml already allows the intentionally-kept patterns (Fortran-mirroring
# signatures/loops in mesh+cli); anything else fails CI.
# `clippy` = no-netcdf crates (CI fast job); `clippy-full` adds cli (needs NetCDF).
clippy:
	$(CARGO) clippy --manifest-path rust/earthmesh_core/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets -- -D warnings

clippy-full: clippy
	$(CARGO) clippy --manifest-path rust/earthmesh_cli/Cargo.toml --all-targets $(CLI_FEATURES) -- -D warnings

# Fast regression gate: no NetCDF, no GUI — pure Rust crates only. Used by CI's
# `fast` job and as the quick local loop. Builds in seconds (no netcdf-c/HDF5).
test-fast:
	$(CARGO) test --manifest-path rust/earthmesh_core/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets

# Full crate tests (includes cli with static-netcdf — slow first build).
test:
	$(CARGO) test --manifest-path rust/earthmesh_core/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --all-targets $(CLI_FEATURES)

test-slow:
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test mkgrd_mask_restart $(CLI_FEATURES) -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test colm_coupling_csv_from_mesh $(CLI_FEATURES) mesh_plus_landtype_classifies_cells_and_writes_colm_netcdf -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test colm_coupling_csv_from_mesh $(CLI_FEATURES) mesh_plus_landtype_coupling_quality_report -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test hydro_workflow $(CLI_FEATURES) full_chain_with_mesh_landtype_coupling_quality -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test refine_end_to_end_topology $(CLI_FEATURES) specified_bbox_refine_produces_consistent_closed_mpas -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test mkgrd_gridinit $(CLI_FEATURES) run_mkgrd_gridinit_global_matches_fortran_nxp64_gridfile_fixture -- --ignored

test-full: check-method-c-neighbors test test-slow

# Release fast gate: format + no-netcdf crates. Run before tagging a release; the
# full gate adds `make test test-gui test-slow` (needs NetCDF) on top.
release-check: fmt test-fast
	@echo 'Release fast gate PASSED: fmt clean + core/geometry/mesh/quality/refine_planner green.'
	@echo 'Full gate (needs NetCDF): make test test-slow'

check-method-c-neighbors:
	bash rust/earthmesh_mesh/scripts/check-method-c-neighbors.sh

clean:
	$(CARGO) clean --manifest-path $(CLI_MANIFEST)
	rm -f $(EXECUTABLE) logmake logmake_gnu logmake_rust *.o *.mod
	@echo 'Clean complete.'
