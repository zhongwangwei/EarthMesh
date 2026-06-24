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
OLAM_SRC=${OLAM_SRC:-}
USER_WORKDIR_SET=${WORKDIR+x}
WORKDIR=${WORKDIR:-/tmp/olam-reduced-probe-run-$$}
CLEAN_WORKDIR=${CLEAN_WORKDIR:-1}
FC=${FC:-gfortran}

check_mode=0
dump_tables=0
spring_mode=0
case_name=all
for arg in "$@"; do
  case "$arg" in
    --check) check_mode=1 ;;
    --dump-tables) dump_tables=1 ;;
    --spring) spring_mode=1 ;;
    -h|--help) case_name=$arg ;;
    *) case_name=$arg ;;
  esac
done

cases="nxp6_circle nxp7_circle nxp6_corridor nxp7_corridor nxp6_variable_corridor nxp6_three_point_corridor nxp6_two_circle nxp7_two_circle nxp6_two_corridor nxp7_two_corridor nxp6_bad_two_circle nxp6_bad_two_corridor"

usage() {
  cat <<USAGE
Usage: $0 [case]
       $0 --check [case]
       $0 --dump-tables [case]
       $0 --spring [case]

Cases:
  all
$(printf '  %s\n' $cases)

Environment:
  OLAM_SRC  path to OLAM source tree, default: $OLAM_SRC
  WORKDIR   scratch build directory, default: $WORKDIR
  CLEAN_WORKDIR
            clean the auto-generated scratch directory on exit, default: $CLEAN_WORKDIR
  FC        Fortran compiler, default: $FC
USAGE
}

cleanup() {
  if [ "$CLEAN_WORKDIR" = "1" ] && [ -z "${USER_WORKDIR_SET:-}" ]; then
    rm -rf "$WORKDIR"
  fi
}

trap cleanup EXIT

if [ "$case_name" = "-h" ] || [ "$case_name" = "--help" ]; then
  usage
  exit 0
fi

if [ -z "$OLAM_SRC" ]; then
  echo "OLAM source tree not found. Set OLAM_SRC or place it next to this repository." >&2
  exit 2
fi

compile_support() {
  rm -rf "$WORKDIR"
  mkdir -p "$WORKDIR"
  cd "$WORKDIR"

  cat > stubs.f90 <<'F90'
module oname_coms
  implicit none
  type oname_vars
     integer :: gridplot_base = 2
     integer :: sfcgridplot_base = 1
  end type oname_vars
  type(oname_vars) :: nl
end module oname_coms

module oplot_coms
  implicit none
  type oplot_vars
     real :: xmin = -1.0e30, xmax = 1.0e30, ymin = -1.0e30, ymax = 1.0e30
     real :: coneang = 0., viewazim = 0., plon3 = 0., plat3 = 0.
     real :: sinplat = 0., cosplat = 1., sinplon = 0., cosplon = 1.
     real :: pxe = 0., pye = 0., pze = 0.
     real :: h1 = 0., h2 = 1., v1 = 0., v2 = 1., fnamey = 0.
     logical :: has_high_res = .false., has_med_res = .false.
     character(len=1) :: projectn(150) = 'N'
  end type oplot_vars
  type(oplot_vars) :: op
end module oplot_coms

module mem_sfcg
  use max_dims, only: maxgrds, maxngrdll
  implicit none
  integer :: nsfcgrids = 0
  integer :: nxp_sfc = 0
  integer, target :: nsfcgrdll(maxgrds) = 0
  real, target :: sfcgrdrad(maxgrds,maxngrdll) = 0.0
  real, target :: sfcgrdlat(maxgrds,maxngrdll) = 0.0
  real, target :: sfcgrdlon(maxgrds,maxngrdll) = 0.0
end module mem_sfcg

module mem_para
  implicit none
contains
  subroutine olam_stop(message)
    character(*), intent(in) :: message
    write(*,*) trim(message)
    stop 2
  end subroutine olam_stop
end module mem_para

subroutine o_reopnwk()
end subroutine
subroutine plotback()
end subroutine
subroutine o_frstpt(x,y)
  real, intent(in) :: x,y
end subroutine
subroutine o_vector(x,y)
  real, intent(in) :: x,y
end subroutine
subroutine o_plchlq(x,y,s,a,b,c)
  real, intent(in) :: x,y,a,b,c
  character(*), intent(in) :: s
end subroutine
subroutine o_frame()
end subroutine
subroutine o_clswk()
end subroutine
subroutine o_sflush()
end subroutine
subroutine o_gsplci(i)
  integer, intent(in) :: i
end subroutine
subroutine o_gsfaci(i)
  integer, intent(in) :: i
end subroutine
subroutine o_gstxci(i)
  integer, intent(in) :: i
end subroutine
subroutine o_gslwsc(x)
  real, intent(in) :: x
end subroutine
subroutine o_set(a,b,c,d,e,f,g,h,i)
  real, intent(in) :: a,b,c,d,e,f,g,h
  integer, intent(in) :: i
end subroutine
subroutine o_pcsetr(s,x)
  character(*), intent(in) :: s
  real, intent(in) :: x
end subroutine
subroutine o_pcseti(s,i)
  character(*), intent(in) :: s
  integer, intent(in) :: i
end subroutine
subroutine o_plchhq(x,y,s,a,b,c)
  real, intent(in) :: x,y,a,b,c
  character(*), intent(in) :: s
end subroutine
subroutine o_mapint()
end subroutine
subroutine o_mappos(a,b,c,d)
  real, intent(in) :: a,b,c,d
end subroutine
subroutine o_mapsti(s,i)
  character(*), intent(in) :: s
  integer, intent(in) :: i
end subroutine
subroutine o_mapstr(s,x)
  character(*), intent(in) :: s
  real, intent(in) :: x
end subroutine
subroutine o_mapstc(a,b)
  character(*), intent(in) :: a,b
end subroutine
subroutine o_maproj(s,a,b,c)
  character(*), intent(in) :: s
  real, intent(in) :: a,b,c
end subroutine
subroutine o_mapset(s,a,b,c,d)
  character(*), intent(in) :: s
  real, intent(in) :: a,b,c,d
end subroutine
subroutine o_mplndr(s,i)
  character(*), intent(in) :: s
  integer, intent(in) :: i
end subroutine
subroutine o_maplot()
end subroutine
subroutine o_mapgrd()
end subroutine
subroutine niceinc20(a,b,c,d)
  real, intent(in) :: a,b
  real, intent(out) :: c
  integer, intent(out) :: d
  c = 1.0
  d = 1
end subroutine
subroutine gridlines(a,b,c,d,e,f,g,h)
  real, intent(in) :: a,b,c,d,e,f
  integer, intent(in) :: g,h
end subroutine
subroutine oplot_panel()
end subroutine
subroutine oplot_transform_xyz(x,y,z,xp,yp)
  real, intent(in) :: x,y,z
  real, intent(out) :: xp,yp
  xp = x
  yp = y
end subroutine
subroutine oplot_xy2(x,y,xp,yp)
  real, intent(in) :: x,y
  real, intent(out) :: xp,yp
  xp = x
  yp = y
end subroutine
subroutine trunc_segment(x1,y1,x2,y2,xmin,xmax,ymin,ymax,skip)
  real, intent(inout) :: x1,y1,x2,y2
  real, intent(in) :: xmin,xmax,ymin,ymax
  integer, intent(out) :: skip
  skip = 0
end subroutine
F90

  if [ "$spring_mode" -eq 0 ]; then
    cat >> stubs.f90 <<'F90'
subroutine spring_dynamics(niter)
  integer, intent(in) :: niter
end subroutine
F90
  fi

  "$FC" -I. -c "$OLAM_SRC/modules/max_dims.f90"
  "$FC" -I. -c stubs.f90
  "$FC" -I. -c "$OLAM_SRC/modules/consts_coms.f90"
  "$FC" -I. -c "$OLAM_SRC/modules/misc_coms.f90"
  "$FC" -I. -c "$OLAM_SRC/modules/mem_ijtabs.f90"
  "$FC" -I. -c "$OLAM_SRC/modules/mem_delaunay.f90"
  "$FC" -I. -c "$OLAM_SRC/outils/map_proj_gn.f90"
  "$FC" -I. -c "$OLAM_SRC/outils/map_proj_or.f90"
  "$FC" -I. -c "$OLAM_SRC/outils/map_proj_ps.f90"
  "$FC" -I. -c "$OLAM_SRC/outils/map_proj.f90"
  "$FC" -I. -c "$OLAM_SRC/omodel/fill_itabs.f90"
  "$FC" -I. -c "$OLAM_SRC/omodel/triangle_utils.f90"
  "$FC" -I. -c "$OLAM_SRC/omodel/icosahedron.f90"
  "$FC" -I. -c "$OLAM_SRC/omodel/expand_global.f90"
  if [ "$spring_mode" -eq 1 ]; then
    "$FC" -I. -c "$OLAM_SRC/omodel/spring_dynamics.f90"
  fi
  "$FC" -I. -c "$OLAM_SRC/omodel/spawn_nest.f90"
}

case_setup() {
  case "$1" in
    nxp6_circle)
      nxp=6; ngrids=2; setup="ngrdll(2)=1; grdrad(2,1)=2500000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0" ;;
    nxp7_circle)
      nxp=7; ngrids=2; setup="ngrdll(2)=1; grdrad(2,1)=2500000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0" ;;
    nxp6_corridor)
      nxp=6; ngrids=2; setup="ngrdll(2)=2; grdrad(2,1)=2500000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; grdrad(2,2)=2500000.0; grdlat(2,2)=25.0; grdlon(2,2)=130.0" ;;
    nxp7_corridor)
      nxp=7; ngrids=2; setup="ngrdll(2)=2; grdrad(2,1)=2500000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; grdrad(2,2)=2500000.0; grdlat(2,2)=25.0; grdlon(2,2)=130.0" ;;
    nxp6_variable_corridor)
      nxp=6; ngrids=2; setup="ngrdll(2)=2; grdrad(2,1)=2500000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; grdrad(2,2)=1250000.0; grdlat(2,2)=25.0; grdlon(2,2)=130.0" ;;
    nxp6_three_point_corridor)
      nxp=6; ngrids=2; setup="ngrdll(2)=3; grdrad(2,1)=2500000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; grdrad(2,2)=2500000.0; grdlat(2,2)=25.0; grdlon(2,2)=130.0; grdrad(2,3)=2500000.0; grdlat(2,3)=0.0; grdlon(2,3)=150.0" ;;
    nxp6_two_circle)
      nxp=6; ngrids=3; setup="ngrdll(2)=1; grdrad(2,1)=4000000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; ngrdll(3)=1; grdrad(3,1)=1000000.0; grdlat(3,1)=25.0; grdlon(3,1)=115.0" ;;
    nxp7_two_circle)
      nxp=7; ngrids=3; setup="ngrdll(2)=1; grdrad(2,1)=3000000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; ngrdll(3)=1; grdrad(3,1)=1000000.0; grdlat(3,1)=25.0; grdlon(3,1)=115.0" ;;
    nxp6_two_corridor)
      nxp=6; ngrids=3; setup="ngrdll(2)=2; grdrad(2,1)=6000000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; grdrad(2,2)=6000000.0; grdlat(2,2)=25.0; grdlon(2,2)=130.0; ngrdll(3)=2; grdrad(3,1)=1000000.0; grdlat(3,1)=25.0; grdlon(3,1)=120.0; grdrad(3,2)=1000000.0; grdlat(3,2)=25.0; grdlon(3,2)=125.0" ;;
    nxp7_two_corridor)
      nxp=7; ngrids=3; setup="ngrdll(2)=2; grdrad(2,1)=2500000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; grdrad(2,2)=2500000.0; grdlat(2,2)=25.0; grdlon(2,2)=130.0; ngrdll(3)=2; grdrad(3,1)=500000.0; grdlat(3,1)=25.0; grdlon(3,1)=120.0; grdrad(3,2)=500000.0; grdlat(3,2)=25.0; grdlon(3,2)=125.0" ;;
    nxp6_bad_two_circle)
      nxp=6; ngrids=3; setup="ngrdll(2)=1; grdrad(2,1)=2500000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; ngrdll(3)=1; grdrad(3,1)=1000000.0; grdlat(3,1)=25.0; grdlon(3,1)=115.0" ;;
    nxp6_bad_two_corridor)
      nxp=6; ngrids=3; setup="ngrdll(2)=2; grdrad(2,1)=6000000.0; grdlat(2,1)=25.0; grdlon(2,1)=115.0; grdrad(2,2)=6000000.0; grdlat(2,2)=25.0; grdlon(2,2)=130.0; ngrdll(3)=2; grdrad(3,1)=1000000.0; grdlat(3,1)=25.0; grdlon(3,1)=115.0; grdrad(3,2)=1000000.0; grdlat(3,2)=25.0; grdlon(3,2)=130.0" ;;
    *) echo "unknown case: $1" >&2; usage >&2; exit 2 ;;
  esac
}

emit_probe() {
  case_setup "$1"
  if [ "$spring_mode" -eq 1 ]; then
    cat > "probe_$1.f90" <<F90
program probe_method_c
  use consts_coms, only: init_consts
  use misc_coms, only: io6, runtype, mdomain, nxp, ngrids, ngrdll, grdrad, grdlat, grdlon
  use mem_delaunay, only: nmd, nud, nwd, xemd, yemd, zemd
  use oname_coms, only: nl
  implicit none
  integer :: samples(8)
  integer :: j, im

  call init_consts(0, 0, 0.0)
  io6 = 6
  runtype = 'MAKEGRID_PLOT'
  mdomain = 0
  nxp = $nxp
  ngrids = 1
  nl%gridplot_base = 2
  ngrdll = 0
  grdrad = 0.0
  grdlat = 0.0
  grdlon = 0.0

  call icosahedron(nxp)

  write(*,'(A,I0,A,I0,A,I0)') 'spring counts nmd=', nmd, ' nud=', nud, ' nwd=', nwd
  samples = (/ 2, 3, 4, 5, nmd / 4, nmd / 2, (nmd * 3) / 4, nmd /)
  do j = 1, 8
     im = samples(j)
     write(*,'(A,I0,A,F0.3,A,F0.3,A,F0.3)') 'spring M ', im, ' x=', xemd(im), ' y=', yemd(im), ' z=', zemd(im)
  enddo
end program probe_method_c
F90
    return
  fi

  cat > "probe_$1.f90" <<F90
program probe_method_c
  use consts_coms, only: init_consts
  use misc_coms, only: io6, runtype, mdomain, nxp, ngrids, ngrdll, grdrad, grdlat, grdlon
  use mem_delaunay, only: nmd, nud, nwd, itab_md, itab_ud, itab_wd
  use oname_coms, only: nl
  implicit none
  integer :: iw, ngr, minrow, maxrow, nrow
  integer :: counts(20)

  call init_consts(0, 0, 0.0)
  io6 = 6
  runtype = 'MAKEGRID'
  mdomain = 0
  nxp = $nxp
  ngrids = $ngrids
  nl%gridplot_base = 2
  ngrdll = 0
  grdrad = 0.0
  grdlat = 0.0
  grdlon = 0.0
  $setup

  call icosahedron(nxp)
  call spawn_nest(.true.)

  counts = 0
  minrow = 999999
  maxrow = -999999
  nrow = 0
  do iw = 2, nwd
     ngr = itab_wd(iw)%ngr
     if (ngr >= 1 .and. ngr <= size(counts)) counts(ngr) = counts(ngr) + 1
     if (itab_wd(iw)%mrow /= 0) then
        nrow = nrow + 1
        minrow = min(minrow, itab_wd(iw)%mrow)
        maxrow = max(maxrow, itab_wd(iw)%mrow)
     endif
  enddo
  write(*,'(A,3I12)') 'summary nmd nud nwd', nmd, nud, nwd
  write(*,'(A,6I12)') 'summary ngr counts', counts(1), counts(2), counts(3), counts(4), counts(5), counts(6)
  write(*,'(A,3I12)') 'summary mrow min max count', minrow, maxrow, nrow
  if ($dump_tables == 1) call dump_tables()

contains

  subroutine write_array(values, count)
    integer, intent(in) :: values(count)
    integer, intent(in) :: count
    integer :: j
    do j = 1, count
       write(*,'(A,I0)', advance='no') ' ', values(j)
    enddo
  end subroutine write_array

  subroutine write_array_padded(values, active_count, count)
    integer, intent(in) :: values(count)
    integer, intent(in) :: active_count, count
    integer :: j
    do j = 1, count
       if (j <= active_count) then
          write(*,'(A,I0)', advance='no') ' ', values(j)
       else
          write(*,'(A,I0)', advance='no') ' ', 1
       endif
    enddo
  end subroutine write_array_padded

  subroutine dump_tables()
    integer :: im, iu, iw
    write(*,'(A,I0,A,I0,A,I0)') 'counts nmd=', nmd, ' nud=', nud, ' nwd=', nwd
    do im = 2, nmd
       write(*,'(A,I0,A,I0,A,I0,A,I0,A,I0,A)', advance='no') 'M ', im, ' npoly=', itab_md(im)%npoly, ' mrlm=', itab_md(im)%mrlm, ' mrlm_orig=', itab_md(im)%mrlm_orig, ' ngr=', itab_md(im)%ngr, ' im'
       call write_array(itab_md(im)%im, 7)
       write(*,'(A)', advance='no') ' iu'
       call write_array_padded(itab_md(im)%iu, itab_md(im)%npoly, 7)
       write(*,'(A)', advance='no') ' iw'
       call write_array_padded(itab_md(im)%iw, itab_md(im)%npoly, 7)
       write(*,*)
    enddo
    do iu = 2, nud
       write(*,'(A,I0,A,I0,A)', advance='no') 'U ', iu, ' mrlu=', itab_ud(iu)%mrlu, ' im'
       call write_array(itab_ud(iu)%im, 2)
       write(*,'(A)', advance='no') ' iu'
       call write_array(itab_ud(iu)%iu, 12)
       write(*,'(A)', advance='no') ' iw'
       call write_array(itab_ud(iu)%iw, 6)
       write(*,*)
    enddo
    do iw = 2, nwd
       write(*,'(A,I0,A,I0,A,I0,A,I0,A,I0,A,I0,A)', advance='no') 'W ', iw, ' npoly=', itab_wd(iw)%npoly, ' mrlw=', itab_wd(iw)%mrlw, ' mrlw_orig=', itab_wd(iw)%mrlw_orig, ' mrow=', itab_wd(iw)%mrow, ' ngr=', itab_wd(iw)%ngr, ' im'
       call write_array(itab_wd(iw)%im, 3)
       write(*,'(A)', advance='no') ' iu'
       call write_array(itab_wd(iw)%iu, 3)
       write(*,'(A)', advance='no') ' iw'
       call write_array(itab_wd(iw)%iw, 9)
       write(*,*)
    enddo
  end subroutine dump_tables
end program probe_method_c
F90
}

run_case() {
  name=$1
  echo "== $name =="
  emit_probe "$name"
  "$FC" -I. -c "probe_$name.f90"
  spring_object=
  if [ "$spring_mode" -eq 1 ]; then
    spring_object=spring_dynamics.o
  fi
  "$FC" -o "probe_$name" "probe_$name.o" stubs.o max_dims.o consts_coms.o misc_coms.o mem_ijtabs.o mem_delaunay.o map_proj_gn.o map_proj_or.o map_proj_ps.o map_proj.o fill_itabs.o triangle_utils.o icosahedron.o expand_global.o spawn_nest.o $spring_object
  set +e
  "./probe_$name" > "probe_$name.out" 2>&1
  status=$?
  set -e
  cat "probe_$name.out"
  echo "status $status"
  if [ "$check_mode" -eq 1 ]; then
    check_case "$name" "$status" "probe_$name.out"
  fi
  if [ $status -ne 0 ]; then
    case "$name" in
      nxp6_bad_two_circle|nxp6_bad_two_corridor) ;;
      *) exit $status ;;
    esac
  fi
}

check_case() {
  name=$1
  status=$2
  output_file=$3

  case "$name" in
    nxp6_circle)
      expected_status=0
      expected_lines='summary nmd nud nwd         435        1297         865
summary ngr counts           0         864           0           0           0           0
summary mrow min max count          -6          12         864'
      ;;
    nxp7_circle)
      expected_status=0
      expected_lines='summary nmd nud nwd         565        1687        1125
summary ngr counts          57        1067           0           0           0           0
summary mrow min max count          -6          13        1067'
      ;;
    nxp6_corridor)
      expected_status=0
      expected_lines='summary nmd nud nwd         474        1414         943
summary ngr counts           0         942           0           0           0           0
summary mrow min max count          -6          12         942'
      ;;
    nxp7_corridor)
      expected_status=0
      expected_lines='summary nmd nud nwd         643        1921        1281
summary ngr counts          25        1255           0           0           0           0
summary mrow min max count          -8          13        1255'
      ;;
    nxp6_variable_corridor)
      expected_status=0
      expected_lines='summary nmd nud nwd         435        1297         865
summary ngr counts           0         864           0           0           0           0
summary mrow min max count          -6          12         864'
      ;;
    nxp6_three_point_corridor)
      expected_status=0
      expected_lines='summary nmd nud nwd         552        1648        1099
summary ngr counts           0        1098           0           0           0           0
summary mrow min max count          -9          12        1098'
      ;;
    nxp6_two_circle)
      expected_status=0
      expected_lines='summary nmd nud nwd         624        1864        1243
summary ngr counts           0         154        1088           0           0           0
summary mrow min max count          -6          11        1242'
      ;;
    nxp7_two_circle)
      expected_status=0
      expected_lines='summary nmd nud nwd         754        2254        1503
summary ngr counts           3         335        1164           0           0           0
summary mrow min max count          -6          13        1499'
      ;;
    nxp6_two_corridor)
      expected_status=0
      expected_lines='summary nmd nud nwd         783        2341        1561
summary ngr counts           0         294        1266           0           0           0
summary mrow min max count          -6          11        1560'
      ;;
    nxp7_two_corridor)
      expected_status=0
      expected_lines='summary nmd nud nwd         715        2137        1425
summary ngr counts          25         287        1112           0           0           0
summary mrow min max count          -6          13        1399'
      ;;
    nxp6_bad_two_circle|nxp6_bad_two_corridor)
      expected_status=2
      expected_lines='Current nested grid 3 crosses (or is too close to)'
      ;;
    *)
      echo "no golden check registered for case: $name" >&2
      exit 2
      ;;
  esac

  if [ "$status" -ne "$expected_status" ]; then
    echo "case $name expected status $expected_status but got $status" >&2
    exit 1
  fi

  printf '%s\n' "$expected_lines" | while IFS= read -r expected_line; do
    if ! grep -F "$expected_line" "$output_file" >/dev/null; then
      echo "case $name missing expected output line: $expected_line" >&2
      exit 1
    fi
  done

  echo "check ok $name"
}

compile_support

if [ "$case_name" = "all" ]; then
  for name in $cases; do
    run_case "$name"
  done
else
  run_case "$case_name"
fi
