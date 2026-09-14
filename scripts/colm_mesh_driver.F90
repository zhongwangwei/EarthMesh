#include <define.h>

program colm_mesh_driver
   use, intrinsic :: iso_fortran_env, only: int32, int64
   use MOD_Precision, only: r8
   use MOD_SPMD_Task
   use MOD_Namelist
   use MOD_NetCDFSerial, only: ncio_read_bcast_serial
   use MOD_Block, only: gblock
   use MOD_Pixel, only: pixel
   use MOD_Mesh, only: mesh_build, mesh, numelm, gridmesh
   use MOD_LandElm, only: landelm, landelm_build
   use MOD_SrfdataRestart, only: mesh_save_to_file, pixelset_save_to_file, &
      mesh_load_from_file, pixelset_load_from_file
   implicit none

   character(len=*), parameter :: magic = 'EMCOLM01'
   integer, parameter :: lc_year = 2005

   character(len=256) :: mode, input_mesh, landdata_dir, dump_prefix
   integer :: numset

#ifdef USEMPI
   call spmd_init()
#endif

   call read_cli(mode, input_mesh, landdata_dir, dump_prefix)
   call validate_safe_path('LANDDATA_DIR', landdata_dir, require_absolute=.true.)
   call validate_safe_path('DUMP_PREFIX', dump_prefix, require_absolute=.false.)
   call configure_mesh_only(input_mesh, landdata_dir)

   select case (trim(mode))
   case ('build')
      call run_build(landdata_dir, dump_prefix)
      call success_marker('EARTHMESH_COLM_MESH_BUILD_OK')
   case ('load')
      call run_load(landdata_dir, dump_prefix, numset)
      call success_marker('EARTHMESH_COLM_MESH_LOAD_OK')
   case default
      call fail('first argument must be build or load')
   end select

#ifdef USEMPI
   call spmd_exit()
#endif

contains

   subroutine read_cli(mode, input_mesh, landdata_dir, dump_prefix)
      character(len=256), intent(out) :: mode, input_mesh, landdata_dir, dump_prefix
      integer :: status, length

      if (command_argument_count() /= 4) then
         call fail('usage: colm_mesh_driver build|load INPUT_COLM_NC LANDDATA_DIR DUMP_PREFIX')
      endif

      call get_checked_arg(1, mode)
      call get_checked_arg(2, input_mesh)
      call get_checked_arg(3, landdata_dir)
      call get_checked_arg(4, dump_prefix)

      if (len_trim(mode) <= 0 .or. len_trim(input_mesh) <= 0 .or. &
          len_trim(landdata_dir) <= 0 .or. len_trim(dump_prefix) <= 0) then
         call fail('empty command-line argument')
      endif
   end subroutine read_cli

   subroutine get_checked_arg(iarg, value)
      integer, intent(in) :: iarg
      character(len=256), intent(out) :: value
      integer :: status, length

      value = ''
      call get_command_argument(iarg, value, length=length, status=status)
      if (status /= 0) then
         call fail('argument too long or unavailable')
      endif
      if (length > len(value)) then
         call fail('argument exceeds 256 characters')
      endif
   end subroutine get_checked_arg

   subroutine configure_mesh_only(input_mesh, landdata_dir)
      character(len=*), intent(in) :: input_mesh, landdata_dir
      real(r8), allocatable :: lat_s(:), lat_n(:), lon_w(:), lon_e(:)

      DEF_file_mesh = trim(input_mesh)
      DEF_dir_landdata = trim(landdata_dir)
      DEF_LC_YEAR = lc_year

      DEF_BlockInfoFile = 'null'
      DEF_AverageElementSize = -1.0_r8
      DEF_nx_blocks = 72
      DEF_ny_blocks = 36
      DEF_PIO_groupsize = 2
      DEF_nIO_eq_nBlock = .false.
      DEF_LANDONLY = .false.

      call ncio_read_bcast_serial(trim(input_mesh), 'lat_s', lat_s)
      call ncio_read_bcast_serial(trim(input_mesh), 'lat_n', lat_n)
      call ncio_read_bcast_serial(trim(input_mesh), 'lon_w', lon_w)
      call ncio_read_bcast_serial(trim(input_mesh), 'lon_e', lon_e)

      if (.not. allocated(lat_s) .or. .not. allocated(lat_n) .or. &
          .not. allocated(lon_w) .or. .not. allocated(lon_e)) then
         call fail('mesh coordinate variables were not read')
      endif
      if (size(lat_s) <= 0 .or. size(lat_n) <= 0 .or. &
          size(lon_w) <= 0 .or. size(lon_e) <= 0) then
         call fail('mesh coordinate variables must be non-empty')
      endif
      if (size(lat_s) /= size(lat_n) .or. size(lon_w) /= size(lon_e)) then
         call fail('mesh coordinate edge arrays have inconsistent lengths')
      endif

      DEF_domain%edges = min(minval(lat_s), minval(lat_n))
      DEF_domain%edgen = max(maxval(lat_s), maxval(lat_n))
      DEF_domain%edgew = lon_w(1)
      DEF_domain%edgee = lon_e(size(lon_e))

      deallocate(lat_s, lat_n, lon_w, lon_e)
   end subroutine configure_mesh_only

   subroutine run_build(landdata_dir, dump_prefix)
      character(len=*), intent(in) :: landdata_dir, dump_prefix

      call gblock%set()
      call pixel%set_edges(DEF_domain%edges, DEF_domain%edgen, &
         DEF_domain%edgew, DEF_domain%edgee)
      call pixel%assimilate_gblock()
      call gridmesh%define_from_file(DEF_file_mesh)
      call pixel%assimilate_grid(gridmesh)
      call pixel%map_to_grid(gridmesh)

      call mesh_build()
      call landelm_build()
      call check_landelm_consistency('build')

      call gblock%save_to_file(landdata_dir)
      call pixel%save_to_file(landdata_dir)
      call mesh_save_to_file(landdata_dir, lc_year)
      call pixelset_save_to_file(landdata_dir, 'landelm', landelm, lc_year)

      call dump_worker_mesh(dump_prefix)
   end subroutine run_build

   subroutine run_load(landdata_dir, dump_prefix, numset)
      character(len=*), intent(in) :: landdata_dir, dump_prefix
      integer, intent(out) :: numset

      call pixel%load_from_file(landdata_dir)
      call gblock%load_from_file(landdata_dir)
      call mesh_load_from_file(landdata_dir, lc_year)
      call pixelset_load_from_file(landdata_dir, 'landelm', landelm, numset, lc_year)
      call check_landelm_consistency('load')
      call dump_worker_mesh(dump_prefix)
   end subroutine run_load

   subroutine check_landelm_consistency(phase)
      character(len=*), intent(in) :: phase
      integer :: ie

      if (.not. p_is_worker) return

      if (landelm%nset /= numelm) then
         call fail(trim(phase)//': landelm nset differs from numelm')
      endif
      if (numelm < 0) then
         call fail(trim(phase)//': numelm is negative')
      endif
      if (numelm > 0) then
         if (.not. allocated(mesh)) call fail(trim(phase)//': mesh not allocated')
         if (.not. allocated(landelm%eindex)) call fail(trim(phase)//': landelm eindex not allocated')
         if (.not. allocated(landelm%ipxstt)) call fail(trim(phase)//': landelm ipxstt not allocated')
         if (.not. allocated(landelm%ipxend)) call fail(trim(phase)//': landelm ipxend not allocated')
         if (.not. allocated(landelm%settyp)) call fail(trim(phase)//': landelm settyp not allocated')
         if (.not. allocated(landelm%ielm)) call fail(trim(phase)//': landelm ielm not allocated')
      endif

      do ie = 1, numelm
         if (landelm%eindex(ie) /= mesh(ie)%indx) then
            call fail(trim(phase)//': landelm eindex does not match mesh indx')
         endif
         if (landelm%ipxstt(ie) /= 1) then
            call fail(trim(phase)//': landelm ipxstt is not 1')
         endif
         if (landelm%ipxend(ie) /= mesh(ie)%npxl) then
            call fail(trim(phase)//': landelm ipxend does not match mesh npxl')
         endif
         if (landelm%settyp(ie) /= 0) then
            call fail(trim(phase)//': landelm settyp is not 0')
         endif
         if (landelm%ielm(ie) /= ie) then
            call fail(trim(phase)//': landelm ielm is not local mesh index')
         endif
         if (mesh(ie)%npxl <= 0) then
            call fail(trim(phase)//': mesh element has no pixels')
         endif
         if (.not. allocated(mesh(ie)%ilon) .or. .not. allocated(mesh(ie)%ilat)) then
            call fail(trim(phase)//': mesh pixel arrays not allocated')
         endif
         if (size(mesh(ie)%ilon) /= mesh(ie)%npxl .or. size(mesh(ie)%ilat) /= mesh(ie)%npxl) then
            call fail(trim(phase)//': mesh pixel array length mismatch')
         endif
      enddo
   end subroutine check_landelm_consistency

   subroutine dump_worker_mesh(dump_prefix)
      character(len=*), intent(in) :: dump_prefix
      character(len=512) :: filename
      character(len=32) :: rank_text
      integer :: unit, ie, ios
      logical :: file_exists

      if (.not. p_is_worker) return

      write(rank_text, '(I0)') p_iam_glb
      filename = trim(dump_prefix)//'.'//trim(rank_text)//'.bin'

      inquire(file=trim(filename), exist=file_exists)
      if (file_exists) call fail('mesh dump file already exists')

      open(newunit=unit, file=trim(filename), status='new', access='stream', &
         form='unformatted', action='write', iostat=ios, convert='little_endian')
      if (ios /= 0) call fail('could not open mesh dump file')

      write(unit) magic
      do ie = 1, numelm
         write(unit) int(mesh(ie)%indx, int64)
         write(unit) int(mesh(ie)%npxl, int64)
         write(unit) int(mesh(ie)%ilon, int32)
         write(unit) int(mesh(ie)%ilat, int32)
      enddo
      close(unit)
   end subroutine dump_worker_mesh

   subroutine validate_safe_path(label, path, require_absolute)
      character(len=*), intent(in) :: label, path
      logical, intent(in) :: require_absolute
      integer :: i, c, n

      n = len_trim(path)
      if (n <= 0) call fail(trim(label)//': empty path')
      if (n > 180) call fail(trim(label)//': path exceeds 180 characters')
      if (require_absolute .and. path(1:1) /= '/') then
         call fail(trim(label)//': path must be absolute')
      endif

      do i = 1, n
         c = iachar(path(i:i))
         if (.not. ((c >= iachar('A') .and. c <= iachar('Z')) .or. &
                    (c >= iachar('a') .and. c <= iachar('z')) .or. &
                    (c >= iachar('0') .and. c <= iachar('9')) .or. &
                    path(i:i) == '_' .or. path(i:i) == '.' .or. &
                    path(i:i) == '/' .or. path(i:i) == '-')) then
            call fail(trim(label)//': path contains unsafe character')
         endif
      enddo
   end subroutine validate_safe_path

   subroutine success_marker(marker)
      character(len=*), intent(in) :: marker

#ifdef USEMPI
      call mpi_barrier(p_comm_glb, p_err)
#endif
      if (p_is_master) then
         write(*,'(A)') trim(marker)
      endif
#ifdef USEMPI
      call mpi_barrier(p_comm_glb, p_err)
#endif
   end subroutine success_marker

   subroutine fail(message)
      character(len=*), intent(in) :: message

      write(*,'(A)') 'EARTHMESH_COLM_MESH_DRIVER_ERROR: '//trim(message)
#ifdef USEMPI
      call mpi_abort(p_comm_glb, 2, p_err)
#else
      error stop 2
#endif
   end subroutine fail

end program colm_mesh_driver
