#!/usr/bin/env bash
set -eu

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
if [ -z "${OLAM_SRC:-}" ]; then
  for candidate in \
    "$REPO_ROOT/olam-model-code-r1095-trunk" \
    "$REPO_ROOT/third_party/olam-model-code-r1095-trunk" \
    "$REPO_ROOT/../olam-model-code-r1095-trunk"; do
    if [ -d "$candidate" ]; then
      OLAM_SRC=$candidate
      break
    fi
  done
fi
if [ -z "${OLAM_SRC:-}" ]; then
  echo "OLAM source tree not found. Set OLAM_SRC or place it next to this repository." >&2
  exit 2
fi
FC=${FC:-gfortran}
CC=${CC:-cc}
REAL_FC=$(command -v "$FC")
WORKDIR=${WORKDIR:-/tmp/olam-full-gfortran-build-$$}
CLEAN_WORKDIR=${CLEAN_WORKDIR:-1}
HDF5_INCS=${HDF5_INCS:-"-I/opt/homebrew/include"}
HDF5_LIBS=${HDF5_LIBS:-"-L/opt/homebrew/lib -lhdf5_hl_fortran -lhdf5_hl -lhdf5_fortran -lhdf5 -lsz -lz"}
NETCDF_INCS=${NETCDF_INCS:-$(nf-config --fflags 2>/dev/null || true)}
NETCDF_LIBS=${NETCDF_LIBS:-$(nf-config --flibs 2>/dev/null || true)}

cleanup() {
  if [ "$CLEAN_WORKDIR" = "1" ]; then
    rm -rf "$WORKDIR"
  fi
}
trap cleanup EXIT

if [ ! -d "$OLAM_SRC/build_olam_test" ]; then
  echo "OLAM build_olam_test directory not found: $OLAM_SRC/build_olam_test" >&2
  exit 2
fi

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
cp -R "$OLAM_SRC/build_olam_test" "$WORKDIR/build_olam_test"
rm -rf "$WORKDIR/build_olam_test/objects"
mkdir -p "$WORKDIR/build_olam_test/objects"
mkdir -p "$WORKDIR/modules"
for module_path in "$OLAM_SRC"/modules/*; do
  module_name=$(basename "$module_path")
  if [ "$module_name" = "mem_para.F90" ]; then
    cp "$module_path" "$WORKDIR/modules/$module_name"
  else
    ln -s "$module_path" "$WORKDIR/modules/$module_name"
  fi
done
perl -0pi -e 's/^\s*import,[^\n]*&\n(?:[^\n]*&\n)*[^\n]*\n//mg; s/^\s*import,[^\n]*\n//mg' "$WORKDIR/modules/mem_para.F90"
for source_dir in omodel oisan convect lake land leaf sfcgrid sea outils radiate UMWM hurricane lagpart test_cases CMAQ MEGAN; do
  if [ -e "$OLAM_SRC/$source_dir" ]; then
    ln -s "$OLAM_SRC/$source_dir" "$WORKDIR/$source_dir"
  fi
done
cd "$WORKDIR/build_olam_test"

mkdir -p "$WORKDIR/bin"
cat > "$WORKDIR/bin/gfortran-olam-filter" <<EOF_INNER
#!/usr/bin/env bash
set -eu
for arg in "\$@"; do
  case "\$arg" in
    *.f90|*.F90|*.f|*.F)
      if [ -f "\$arg" ]; then
        perl -0pi -e 's/^\\s*import,[^\\n]*&\\n(?:[^\\n]*&\\n)*[^\\n]*\\n//mg; s/^\\s*import,[^\\n]*\\n//mg' "\$arg"
        if [ "\$(basename "\$arg")" = "umwm_source_functions.F90" ]; then
          perl -0pi -e 's/(use\\s+umwm_module,\\s*only:[\\s\\S]*?)(\\n\\s*implicit none)/my \$s=\$1; \$s=~s\/,\\s*sdt\\b\/\/g; \$s=~s\/,\\s*sds\\b\/\/g; "\$s\$2"/eg' "\$arg"
        fi
        if [ "\$(basename "\$arg")" = "tileslab_grid.F90" ]; then
          perl -0pi -e 's/(use\\s+map_proj,\\s*only:[^\\n]*),\\s*get_weights_lonlat/\$1/g' "\$arg"
        fi
      fi
      ;;
  esac
done
exec "$REAL_FC" "\$@"
EOF_INNER
chmod +x "$WORKDIR/bin/gfortran-olam-filter"

cat > objects/ncarg_stubs.f90 <<'EOF_INNER'
subroutine gclwk
end subroutine
subroutine gclks
end subroutine
subroutine gdawk
end subroutine
subroutine gfa
end subroutine
subroutine gngpat
end subroutine
subroutine gopks
end subroutine
subroutine gopwk
end subroutine
subroutine gacwk
end subroutine
subroutine gqops
end subroutine
subroutine gsasf
end subroutine
subroutine gsclip
end subroutine
subroutine gscr
end subroutine
subroutine gsfaci
end subroutine
subroutine gsfais
end subroutine
subroutine gslwsc
end subroutine
subroutine gsplci
end subroutine
subroutine gstxci
end subroutine
subroutine hlsrgb
end subroutine
subroutine mapgrd
end subroutine
subroutine mapint
end subroutine
subroutine maplot
end subroutine
subroutine mappos
end subroutine
subroutine maproj
end subroutine
subroutine mapset
end subroutine
subroutine mapstc
end subroutine
subroutine mapsti
end subroutine
subroutine mapstr
end subroutine
subroutine mplndr
end subroutine
subroutine ngckop
end subroutine
subroutine ngreop
end subroutine
subroutine ngsetc
end subroutine
subroutine ngsrat
end subroutine
subroutine pcseti
end subroutine
subroutine pcsetr
end subroutine
subroutine plchhq
end subroutine
subroutine plchlq
end subroutine
subroutine plchmq
end subroutine
subroutine plotif
end subroutine
subroutine set
end subroutine
subroutine setusv
end subroutine
subroutine sfseti
end subroutine
subroutine vector
end subroutine
subroutine frame
end subroutine
subroutine frstpt
end subroutine
EOF_INNER
"$REAL_FC" -c objects/ncarg_stubs.f90 -o objects/ncarg_stubs.o

cat > include.mk <<EOF_INNER
F_COMP=$WORKDIR/bin/gfortran-olam-filter
F_OPTS=-O0 -g -fallow-argument-mismatch -fallow-invalid-boz -fno-range-check -cpp
FIXED_SRC_FLAGS=-ffixed-form -ffixed-line-length-none
C_COMP=$CC
C_OPTS=-O0 -g
LIBNCARG=
LIBS=
HDF5_INCS=$HDF5_INCS
HDF5_LIBS=$HDF5_LIBS
NETCDF_INCS=$NETCDF_INCS
NETCDF_LIBS=$NETCDF_LIBS
PAR_INCS=
PAR_LIBS=
LOADER=$FC
LOADER_OPTS=-O0 -g
OLAM_MPI=no
OLAM_PARALLEL_HDF5=no
EOF_INNER

set +e
make NOMAKEDEP=1 > build.log 2>&1
status=$?
set -e

if [ "$status" -ne 0 ]; then
  echo "full OLAM gfortran build failed in $WORKDIR" >&2
  tail -80 build.log >&2
  exit "$status"
fi

exe=$(find . -maxdepth 1 -type f -perm -111 -name 'olam-*' | head -1)
if [ -z "$exe" ]; then
  echo "full OLAM gfortran build finished but no olam-* executable was produced" >&2
  exit 1
fi

echo "full OLAM gfortran build ok: $WORKDIR/$exe"
