module MOD_data_preprocess

    USE consts_coms, only : output_format, NXP, io6, r8, landtype_file, nlons_source, nlats_source, maxlc, refine, mesh_type
    use refine_vars, only: refine_spc, max_iter_spc, threshold_dir, refine_onelayer_Lnd, refine_twolayer_Lnd, refine_onelayer_Ocn, refine_onelayer_Atmos
    USE netcdf
    implicit none
    integer,  allocatable, public :: landtypes_global(:, :), landtypes(:, :)
    real(r8), allocatable, public :: lon_i(:), lat_i(:)
    real(r8), allocatable, public :: lon_vertex(:), lat_vertex(:)  ! 经纬度顶点信息
    real(r8), allocatable, public :: gridx(:), gridy(:), cellWidthVsXY(:,:)
    real(r8), public :: maxcellWidthVsXY
    integer,  public :: nlons_xy, nlats_xy
    integer,  public :: nlons_Rf_select, nlats_Rf_select
    character(LEN = 10), dimension(:), public :: onelayer_Lnd(2) = (/"lai", "slope_avg"/) ! Add by Rui Zhang
    character(LEN = 10), dimension(:), public :: twolayer_Lnd(5) = (/"k_s", "k_solids", "tkdry", "tksatf", "tksatu"/) ! Add by Rui Zhang
    character(LEN = 10), dimension(:), public :: onelayer_Ocn(1) = (/"sst"/) ! Add by Rui Zhang
    character(LEN = 10), dimension(:), public :: onelayer_Atmos(1) = (/"typhoon"/) ! Add by Rui Zhang

    ! 尽量在未来不要有两层阈值计算的，很影响计算效率
    type :: var_data2d
        real(r8), allocatable :: var2d(:, :)
    end type
    type(var_data2d), allocatable, public :: input2d_Lnd(:), input2d_Ocn(:), input2d_Atmos(:)

    type :: var_data3d
       real(r8), allocatable :: var3d(:, :, :)
    end type
    type(var_data3d), allocatable, public :: input3d_Lnd(:)

    contains

    SUBROUTINE data_preprocess()
      USE consts_coms, only : gridnum_perdegree
      IMPLICIT NONE
      real(r8) :: dx, dy
      integer :: ncid, varid, dimID_lon, dimID_lat
      integer(kind=1), allocatable :: byte_data(:,:)
      character(LEN = 256) :: lndname

      ! 基础分辨率数据计算与获取
      nlons_source = gridnum_perdegree * 360
      nlats_source = gridnum_perdegree * 180
      write(io6, *), "nlons_source = ", nlons_source
      write(io6, *), "nlats_source = ", nlats_source

      dx = 360. / nlons_source
      dy = 180. / nlats_source
      ! 经纬度网格中心点经纬度值
      allocate(lon_i(nlons_source)); allocate(lat_i(nlats_source))
      lon_i = -180. + (2 * [1:nlons_source] - 1) * dx / 2. ! [] mean array
      lat_i =   90. - (2 * [1:nlats_source] - 1) * dy / 2.

      allocate(lon_vertex(1+nlons_source)); allocate(lat_vertex(1+nlats_source))
      ! lon_vertex combined lone and lonw from -180 to 180
      lon_vertex(2:nlons_source+1) = lon_i + dx / 2.
      lon_vertex(1) = -180. ! lon_vertex(nlons_source+1) 与 lon_vertex(1) 会不会冲突呢？要小心了
      lon_vertex(nlons_source+1) = 180.
      ! lat_vertex combined latn and lats from 90 to -90
      lat_vertex(2:nlats_source+1) = lat_i - dy / 2.
      lat_vertex(1) = 90. ! 也可以考虑去掉
      lat_vertex(nlats_source+1) = -90.

      if ((mesh_type == 'atmosmesh') .and. (refine .eqv. .false.)) then
         write(io6, *), "when mesh_type == 'atmosmesh' .and. refine .eqv. .false."
         write(io6, *), "no need to read landtype data"
      else
         write(io6, *), "need to read landtype data"
         write(io6, *), trim(landtype_file)
         CALL CHECK(NF90_OPEN(trim(landtype_file), nf90_nowrite, ncid))
         if (gridnum_perdegree == 240) then
            CALL CHECK(NF90_INQ_DIMID(ncid, "lon", dimID_lon))
            CALL CHECK(NF90_INQ_DIMID(ncid, "lat", dimID_lat))
         else if (gridnum_perdegree == 120) then
            CALL CHECK(NF90_INQ_DIMID(ncid, "longitude", dimID_lon))
            CALL CHECK(NF90_INQ_DIMID(ncid, "latitude", dimID_lat))
         end if

         CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lon, len = nlons_source))
         CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lat, len = nlats_source))
         if (nlons_source /= gridnum_perdegree * 360) then
            STOP "ERROR! nlons_source from landtype_file /= gridnum_perdegree * 360"
         end if
         if (nlats_source /= gridnum_perdegree * 180) then
            STOP "ERROR! nlons_source from landtype_file /= gridnum_perdegree * 360"
         end if
         
         allocate(byte_data(nlons_source, nlats_source)); byte_data = 0
         allocate(landtypes_global(nlons_source, nlats_source)); landtypes_global = 0
         CALL CHECK(NF90_INQ_VARID(ncid, "landtype", varid))
         CALL CHECK(NF90_GET_VAR(ncid, varid, byte_data))
         CALL CHECK(NF90_CLOSE(ncid))
         landtypes_global = int(byte_data)
         write(io6, *), "landtypes", minval(landtypes_global), maxval(landtypes_global)
         maxlc = maxval(landtypes_global)
         write(io6, *), "landtypes read finish"
         deallocate(byte_data)
      end if

      ! 提供cellwidthVSXY的初始分辨率
      if (refine .eq. .true.) then
         if (output_format == 'MPAS') CALL CellWidthVsXY_Initial()
         if (output_format == 'MPAS-Simple') CALL CellWidthVsXY_Initial()
      end if

    END SUBROUTINE data_preprocess

    ! nlons_xy, nlats_xy, gridx, gridy, cellWidthVsXY make
    SUBROUTINE CellWidthVsXY_Initial()
      USE consts_coms, only : pathlen, file_dir, gridnum_perdegree
      IMPLICIT NONE
      integer :: num, num_interval, i
      integer :: ncid, lonDimID, latDimID, ncvarid(3)
      character(pathlen) :: lndname
      character(LEN = 5) :: stepc

      num_interval = 2**(max_iter_spc-1)
      if (num_interval < 10) num_interval = 10
      ! 确保nlons_source大于nlons_xy
      if (gridnum_perdegree < num_interval) then
         write(io6, *) "gridnum_perdegree = ", gridnum_perdegree
         write(io6, *) "2**max_iter_spc = ", 2**max_iter_spc
         write(io6, *) "gridnum_perdegree < 2**max_iter_spc"
         STOP
      end if

      ! 确保nlons_source可以整除nlons_xy
      do while (.true.)
         if (mod(gridnum_perdegree, num_interval) == 0) then
            write(io6, *) "num_interval = ", num_interval
            exit
         end if
         num_interval = num_interval + 1 
      end do
      nlons_xy = 360 * num_interval + 1
      nlats_xy = 180 * num_interval + 1
      allocate(gridx(nlons_xy)); gridx = 0.0
      allocate(gridy(nlats_xy)); gridy = 0.0
      num = gridnum_perdegree / num_interval
      do i = 1, nlons_xy, 1
         gridx(i) = lon_vertex(num * (i-1) + 1)
      end do
      do i = 1, nlats_xy, 1
         gridy(i) = lat_vertex(num * (i-1) + 1)
      end do

      allocate(cellWidthVsXY(nlons_xy, nlats_xy))
      cellWidthVsXY = 7680.0 / NXP ! 初始分辨率

      write(stepc, '(I2.2)') 0
      lndname = trim(file_dir) // "tmpfile/CellWidthVsXY_" // trim(stepc) // ".nc4"
      write(io6, *), lndname
      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncID, "nlons_xy", nlons_xy, lonDimID))
      CALL CHECK(NF90_DEF_DIM(ncID, "nlats_xy", nlats_xy, latDimID))
      CALL CHECK(NF90_DEF_VAR(ncID, "gridx", NF90_DOUBLE, (/ lonDimID /), ncVarID(1)))
      CALL CHECK(NF90_DEF_VAR(ncID, "gridy", NF90_DOUBLE, (/ latDimID /), ncVarID(2)))
      CALL CHECK(NF90_DEF_VAR(ncID, "CellWidthVsXY", NF90_DOUBLE, (/ lonDimID, latDimID /), ncVarID(3)))
      CALL CHECK(NF90_ENDDEF(ncID))
      CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(1), gridx)) ! 含有非全细化三角形的多边形编号
      CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(2), gridy)) ! 含有非全细化三角形的多边形编号
      CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(3), CellWidthVsXY)) ! 含有非全细化三角形的多边形编号
      CALL CHECK(NF90_CLOSE(ncID))

    END SUBROUTINE CellWidthVsXY_Initial

    SUBROUTINE Threshold_Read_Lnd(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal)
      ! only use for refine_onelayer_Lnd and refine_twolayer_Lnd but not refine_num_landtypes and refine_area_mainland
      IMPLICIT NONE
      integer, intent(in) :: minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal
      integer :: num_dataset
      character(len = 256) :: lndname
      character(len = 20) :: varname_select
      integer :: i, start(2), count(2)
      real(r8), allocatable :: input2d_temp(:, :), input3d_temp(:, :, :)

      if (all(refine_onelayer_Lnd .eqv. .false.) .and. &
         all(refine_twolayer_Lnd .eqv. .false.)) then
         write(io6, *), "all false in refine_onelayer_Lnd and refine_twolayer_Lnd and return!"
         return
      end if
      write(io6, *), "true exist in refine_onelayer_Lnd or refine_twolayer_Lnd and go on!", "mesh_type = ", mesh_type
      start = [minlon_RfArea_cal, maxlat_RfArea_cal]
      count = [nlons_Rf_select, nlats_Rf_select]

      ! refine_onelayer_Lnd
      if (any(refine_onelayer_Lnd .eqv. .true.)) then
         num_dataset = 0 ! 确定需要的阈值文件个数
         allocate(input2d_Lnd(size(refine_onelayer_Lnd)/2)) !%var2d(nlons_Rf_select, nlats_Rf_select) ! 因为最多只有两个一层数据
         allocate(input2d_temp(nlons_Rf_select, nlats_Rf_select)); input2d_temp = 0.

         ! 还需要分配inputdata的数组大小, 目前还没有数据裁剪，后面会加上
         do i = 1, size(refine_onelayer_Lnd)/2, 1 ! 这个7在未来可以更加智能化
               if ((refine_onelayer_Lnd(2*i-1) .eqv. .true.) .or. &
                  (refine_onelayer_Lnd(2*i)   .eqv. .true.)) then! 说明该数据集需要读入
                  num_dataset = num_dataset + 1
                  varname_select = onelayer_Lnd(i)
                  lndname = trim(threshold_dir) // trim(varname_select) //'.nc' ! slope 应该为 slope_avg.nc
                  write(io6, *),lndname
                  allocate(input2d_Lnd(i)%var2d(nlons_Rf_select, nlats_Rf_select))
                  CALL data_read_onelayer(lndname, i, start, count, varname_select, input2d_temp)
                  input2d_Lnd(i)%var2d = input2d_temp
               end if
         end do
         deallocate(input2d_temp)
         write(io6, *), "onelayer num_dataset = ", num_dataset
      end if

      ! refine_twolayer_Lnd
      if (any(refine_twolayer_Lnd .eqv. .true.)) then
         num_dataset = 0 ! 确定需要的阈值文件个数
         allocate(input3d_Lnd(size(refine_twolayer_Lnd)/2)) ! 因为最多只有FIVE个一层数据
         allocate(input3d_temp(2, nlons_Rf_select, nlats_Rf_select)); input3d_temp = 0.

         ! 还需要分配inputdata的数组大小, 目前还没有数据裁剪，后面会加上
         do i = 1, size(refine_twolayer_Lnd)/2, 1 ! 这个7在未来可以更加智能化
               if ((refine_twolayer_Lnd(2*i-1) .eqv. .true.) .or. &
                  (refine_twolayer_Lnd(2*i)   .eqv. .true.)) then! 说明该数据集需要读入
                  num_dataset = num_dataset + 1
                  varname_select = twolayer_Lnd(i)
                  lndname = trim(threshold_dir) // trim(varname_select) //'.nc' ! slope 应该为 slope_avg.nc
                  write(io6, *),lndname
                  allocate(input3d_Lnd(i)%var3d(2, nlons_Rf_select, nlats_Rf_select))
                  CALL data_read_twolayer(lndname, i, start, count, varname_select, input3d_temp)
                  input3d_Lnd(i)%var3d = input3d_temp
               end if
         end do
         deallocate(input3d_temp)
         write(io6, *), "twolayer num_dataset = ", num_dataset
      end if

    END SUBROUTINE Threshold_Read_Lnd

    SUBROUTINE Threshold_Read_Ocn(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal)
      IMPLICIT NONE
      integer, intent(in) :: minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal
      integer :: num_dataset
      character(len = 256) :: lndname
      character(len = 20) :: varname_select
      integer :: i, start(2), count(2)
      real(r8), allocatable :: input2d_temp(:, :)

      if (all(refine_onelayer_Ocn .eqv. .false.)) then
         write(io6, *), "all false in refine_onelayer_Ocn and return!"
         return
      end if
      write(io6, *), "true exist in refine_onelayer_Ocn and go on!", "mesh_type = ", mesh_type
      start = [minlon_RfArea_cal, maxlat_RfArea_cal]
      count = [nlons_Rf_select, nlats_Rf_select]

      ! refine_onelayer_Ocn
      if (any(refine_onelayer_Ocn .eqv. .true.)) then
         num_dataset = 0 ! 确定需要的阈值文件个数
         allocate(input2d_Ocn(size(refine_onelayer_Ocn)/2)) !%var2d(nlons_Rf_select, nlats_Rf_select) ! 因为最多只有两个一层数据
         allocate(input2d_temp(nlons_Rf_select, nlats_Rf_select)); input2d_temp = 0.

         ! 还需要分配inputdata的数组大小, 目前还没有数据裁剪，后面会加上
         do i = 1, size(refine_onelayer_Ocn)/2, 1 ! 这个7在未来可以更加智能化
               if ((refine_onelayer_Ocn(2*i-1) .eqv. .true.) .or. &
                  (refine_onelayer_Ocn(2*i)   .eqv. .true.)) then! 说明该数据集需要读入
                  num_dataset = num_dataset + 1
                  varname_select = onelayer_Ocn(i)
                  lndname = trim(threshold_dir) // trim(varname_select) //'.nc'
                  write(io6, *),lndname
                  allocate(input2d_Ocn(i)%var2d(nlons_Rf_select, nlats_Rf_select))
                  CALL data_read_onelayer(lndname, i, start, count, varname_select, input2d_temp)
                  input2d_Ocn(i)%var2d = input2d_temp
               end if
         end do
         deallocate(input2d_temp)
         write(io6, *), "onelayer num_dataset = ", num_dataset
      end if
   
    END SUBROUTINE Threshold_Read_Ocn

    SUBROUTINE Threshold_Read_Atmos(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal)
      IMPLICIT NONE
      integer, intent(in) :: minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal
      integer :: num_dataset
      character(len = 256) :: lndname
      character(len = 20) :: varname_select
      integer :: i, start(2), count(2)
      real(r8), allocatable :: input2d_temp(:, :)

      if (all(refine_onelayer_Atmos .eqv. .false.)) then
         write(io6, *), "all false in refine_onelayer_Atmos and return!"
         return
      end if
      write(io6, *), "true exist in refine_onelayer_Atmos and go on!", "mesh_type = ", mesh_type
      start = [minlon_RfArea_cal, maxlat_RfArea_cal]
      count = [nlons_Rf_select, nlats_Rf_select]

      ! refine_onelayer_Atmos
      if (any(refine_onelayer_Atmos .eqv. .true.)) then
         num_dataset = 0 ! 确定需要的阈值文件个数
         allocate(input2d_Atmos(size(refine_onelayer_Atmos)/2)) !%var2d(nlons_Rf_select, nlats_Rf_select) ! 因为最多只有两个一层数据
         allocate(input2d_temp(nlons_Rf_select, nlats_Rf_select)); input2d_temp = 0.

         ! 还需要分配inputdata的数组大小, 目前还没有数据裁剪，后面会加上
         do i = 1, size(refine_onelayer_Atmos)/2, 1 ! 这个7在未来可以更加智能化
               if ((refine_onelayer_Atmos(2*i-1) .eqv. .true.) .or. &
                  (refine_onelayer_Atmos(2*i)   .eqv. .true.)) then! 说明该数据集需要读入
                  num_dataset = num_dataset + 1
                  varname_select = onelayer_Atmos(i)
                  lndname = trim(threshold_dir) // trim(varname_select) //'.nc'
                  write(io6, *),lndname
                  allocate(input2d_Atmos(i)%var2d(nlons_Rf_select, nlats_Rf_select))
                  CALL data_read_onelayer(lndname, i, start, count, varname_select, input2d_temp)
                  input2d_Atmos(i)%var2d = input2d_temp
               end if
         end do
         deallocate(input2d_temp)
         write(io6, *), "onelayer num_dataset = ", num_dataset
      end if

    END SUBROUTINE Threshold_Read_Atmos

    SUBROUTINE data_read_onelayer(lndname, i, start, count, varname_select, input2d_temp)

      IMPLICIT NONE
      character(len = 256), intent(in) :: lndname
      integer, intent(in) :: i, start(2), count(2)
      character(len = 20), intent(in) :: varname_select
      integer :: ncid, varid
      real(r8), dimension(nlons_Rf_select, nlats_Rf_select), intent(out) :: input2d_temp

      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid)) ! 文件打开
      CALL CHECK(NF90_INQ_VARID(ncid, trim(varname_select), varid))
      CALL CHECK(NF90_GET_VAR(ncid, varid, input2d_temp, start=start, count=count))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
      write(io6, *), varname_select, minval(input2d_temp), maxval(input2d_temp)

    END SUBROUTINE data_read_onelayer

    SUBROUTINE data_read_twolayer(lndname, i, start, count, varname_select, input3d_temp)

      IMPLICIT NONE
      character(len = 256), intent(in) :: lndname
      integer, intent(in) :: i, start(2), count(2)
      character(len = 20), intent(in) :: varname_select
      character(len = 20)  :: varname_new ! 用于存放需要读取数据的数据集名字
      integer :: k, ncid, varid(2)
      real(r8), dimension(2, nlons_Rf_select, nlats_Rf_select), intent(out) :: input3d_temp

      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid)) ! 文件打开
      do k = 1, 2, 1
         if (k == 1) then! ["k_s", "k_solids", "tkdry", "tksatf", "tksatu"] ! 双层信息
               varname_new =  trim(varname_select)//"_l1"
         else
               varname_new =  trim(varname_select)//"_l2"
         end if
         CALL CHECK(NF90_INQ_VARID(ncid,varname_new,varid(k))) 
         CALL CHECK(NF90_GET_VAR(ncid, varid(k), input3d_temp(k, :, :), start=start, count=count))
         write(io6, *), varname_new, minval(input3d_temp(k, :, :)), maxval(input3d_temp(k, :, :))
      end do
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件

    END SUBROUTINE data_read_twolayer

END Module MOD_data_preprocess
