# Rust build entrypoint for EarthMesh.
# The executable is built from rust/earthmesh_cli. Legacy Fortran sources are
# archived outside the active tree and tracked only by the migration manifest.

CARGO ?= cargo
CLI_MANIFEST = rust/earthmesh_cli/Cargo.toml
CARGO_TARGET_DIR ?= rust/earthmesh_cli/target
BUILD_PROFILE ?= release
EXECUTABLE = mkgrd.x

export CARGO_TARGET_DIR

ifeq ($(BUILD_PROFILE),release)
CARGO_PROFILE_FLAG = --release
CLI_BINARY = $(CARGO_TARGET_DIR)/release/earthmesh_cli
else
CARGO_PROFILE_FLAG =
CLI_BINARY = $(CARGO_TARGET_DIR)/debug/earthmesh_cli
endif

.PHONY: all build clean test fmt

all: build

build:
	$(CARGO) build --manifest-path $(CLI_MANIFEST) $(CARGO_PROFILE_FLAG)
	cp $(CLI_BINARY) $(EXECUTABLE)
	@echo 'EarthMesh Rust executable has been built successfully.'
	@echo 'Executable: $(EXECUTABLE)'

fmt:
	$(CARGO) fmt --manifest-path rust/earthmesh_core/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_mesh/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_cli/Cargo.toml --check

test:
	$(CARGO) test --manifest-path rust/earthmesh_core/Cargo.toml --lib
	$(CARGO) test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --lib

clean:
	$(CARGO) clean --manifest-path $(CLI_MANIFEST)
	rm -f $(EXECUTABLE) logmake logmake_gnu logmake_rust *.o *.mod
	@echo 'Clean complete.'
