MODULE MOD_GetRef
    ! 进行判断如果ref_sjx存在那里不在额外定义了
    USE consts_coms, only: io6, mesh_type, step, r8, nxp, file_dir, openmp, maxlc, num_vertex, num_mp_step
    USE refine_vars 
    USE netcdf
    USE MOD_utilities,       only: Contain_Read, ref_sjx_save
    USE MOD_data_preprocess, only: nlons_Rf_select, nlats_Rf_select, onelayer_Lnd, twolayer_Lnd, onelayer_Ocn, onelayer_Atmos, landtypes
    USE MOD_GetContain,      only: IsInRfArea_sjx
    USE MOD_Area_judge,      only: minlon_RfArea_cal, maxlat_RfArea_cal
    USE MOD_utilities, only : CHECK
    implicit none
    integer, public :: ref_colnum
    integer, dimension(:),    allocatable, public :: ref_sjx ! 需要细化的三角形标记 1和0，注意每次赋值之后都要重新赋值为0！！！！
    integer, dimension(:, :), allocatable, public :: ref_th, ref_th_Lnd, ref_th_Ocn, ref_th_Atmos
    type :: var_data2d
        real(r8), allocatable :: varms(:)
    end type
    type(var_data2d), allocatable, public :: output2d(:)
    type :: var_data3d
        real(r8), allocatable :: varms(:, :)
    end type
    type(var_data3d), allocatable, public :: output3d(:)
    
    !interface mean_std_cal
    !    module procedure mean_std_cal2d
    !    module procedure mean_std_cal3d
    !end interface mean_std_cal

    contains
    ! IsInRfArea_sjx(i) == 1 : 对应的三角形， == -1 相反的三角形（如果1是陆地网格，-1就是海洋网格，反之） == 0 不存在的三角形（可能是不在指定区域内）
    SUBROUTINE GetRef(iter)

        implicit none
        integer, intent(in) :: iter
        integer :: i, sjx_points, numpatch, spDimID, twoDimID, DimID, ncid, varid(18)
        integer,  dimension(:, :), allocatable :: mp_id, mp_ii
        character(LEN = 5) :: nxpc, stepc
        character(LEN = 256) :: lndname

        write(nxpc, '(I4.4)') nxp
        write(stepc, '(I2.2)') step
        if (iter /= 0) then
            sjx_points = num_mp_step(step)
            write(io6, *)  "step = ", step, "num_mp_step(step) = ", num_mp_step(step)
            if (.not. allocated(ref_sjx)) then
                allocate(ref_sjx(sjx_points))
                ref_sjx = IsInRfArea_sjx ! spc only
                where(ref_sjx < 1)  ref_sjx = 0 ! 防止出现负数
                write(io6, *)  "需要细化的三角形个数(spc only): ",INT(sum(ref_sjx))
            else
                ref_sjx = ref_sjx + IsInRfArea_sjx ! mixed
                where(ref_sjx > 1)  ref_sjx = 1
                where(ref_sjx < 1)  ref_sjx = 0
                write(io6, *)  "需要细化的三角形个数(mixed): ",INT(sum(ref_sjx))
            end if

            ! 保存ref_sjx_specified
            lndname = trim(file_dir) // "threshold/threshold_specified_NXP" // trim(nxpc) //'_'// trim(stepc) // ".nc4"
            ! write(io6, *)  lndname
            CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber,nf90_netcdf4), ncid))
            CALL CHECK(NF90_DEF_DIM(ncid, "sjx_points", sjx_points, spDimID))
            CALL CHECK(NF90_DEF_VAR(ncid, "IsInRfArea_sjx_specified", NF90_INT, (/ spDimID /), varid(1)))
            CALL CHECK(NF90_PUT_VAR(ncid, varid(1), IsInRfArea_sjx))
            CALL CHECK(NF90_CLOSE(ncid))
            deallocate(IsInRfArea_sjx) ! allocate in the MOD_GetContain.F90

            ! Orial ref_sjx save
            if (sum(ref_sjx(1:num_vertex)) > 0) then
                write(io6, *)  "sum(ref_sjx(1:num_vertex)) = ", sum(ref_sjx(1:num_vertex))
                write(io6, *)  "Warning! Please check for SUBROUTINE GetRef"
                ref_sjx(1:num_vertex) = 0 ! 进一步保证N+1级细化出现在N级细化内
            end if
            lndname = trim(file_dir) // "tmpfile/ref_sjx_before_halo_protect_NXP" // trim(nxpc)  // "_"//  trim(stepc) // ".nc4"
            ! write(io6, *)  lndname
            CALL ref_sjx_save(lndname, sjx_points, ref_sjx)
            
            return
        end if

        if (mesh_type == 'landmesh') then
            write(io6, *)  "开始读取陆面非结构网格数据 in the GetRef.F90"
        else if (mesh_type == 'oceanmesh') then
            write(io6, *)  "开始读取海洋非结构网格数据 in the GetRef.F90"
        else if (mesh_type == 'atmosmesh') then
            write(io6, *)  "开始读取大气非结构网格数据 in the GetRef.F90"
        else if (mesh_type == 'LOCmesh') then
            write(io6, *)  "开始读取海陆非结构网格数据 in the GetRef.F90"
        end if

        lndname = trim(file_dir) // 'contain/contain_'//trim(mesh_type)//'_refine_cal_NXP' // trim(nxpc) //'_'// trim(stepc) //'_tri.nc4'
        write(io6, *)  trim(lndname)
        call Contain_Read(lndname, sjx_points, numpatch, mp_id, mp_ii) ! 保存文件
        allocate(ref_sjx(sjx_points)); ref_sjx = 0 ! will deallocate in the MOD_refine.F90
        mp_ii(:, 1) = mp_ii(:, 1) - minlon_RfArea_cal + 1
        mp_ii(:, 2) = mp_ii(:, 2) - maxlat_RfArea_cal + 1       

        if (mesh_type == 'landmesh') then
            write(io6, *)  "陆面非结构网格数据读取完成 in the GetRef.F90"
            CALL GetRef_Lnd(sjx_points, mp_id, mp_ii)
            allocate(ref_th(sjx_points, size(ref_th_Lnd, 2)))
            ref_th = ref_th_Lnd
            deallocate(ref_th_Lnd)

        else if (mesh_type == 'oceanmesh') then
            write(io6, *)  "海洋非结构网格数据读取完成 in the GetRef.F90"
            CALL GetRef_Ocn(sjx_points, mp_id, mp_ii)
            allocate(ref_th(sjx_points, size(ref_th_Ocn, 2)))
            ref_th = ref_th_Ocn
            deallocate(ref_th_Ocn)

        else if (mesh_type == 'atmosmesh') then
            write(io6, *)  "大气非结构网格数据读取完成 in the GetRef.F90"
            CALL GetRef_Atmos(sjx_points, mp_id, mp_ii)
            allocate(ref_th(sjx_points, size(ref_th_Atmos, 2)))
            ref_th = ref_th_Atmos
            deallocate(ref_th_Atmos)

        else if (mesh_type == 'LOCmesh') then
            write(io6, *)  "海陆非结构网格数据读取完成 in the GetRef.F90 in Line"
            CALL GetRef_LOC(sjx_points, mp_id, mp_ii) ! 在subroutine内部自行打开，而且可以调用GetRef_Lnd和Get_OcnGet_Ocn和Get_OcnGet_Atmos
            ref_colnum = 0
            if (allocated(ref_th_Lnd)) ref_colnum = ref_colnum + size(ref_th_Lnd, 2)
            if (allocated(ref_th_Ocn)) ref_colnum = ref_colnum + size(ref_th_Ocn, 2)
            allocate(ref_th(sjx_points, ref_colnum))

            ref_colnum = 1
            if (allocated(ref_th_Lnd)) then
                ref_th(:, ref_colnum:size(ref_th_Lnd, 2)) = ref_th_Lnd
                ref_colnum = ref_colnum + size(ref_th_Lnd, 2)
                deallocate(ref_th_Lnd)
            end if

            if (allocated(ref_th_Ocn)) then
                ref_th(:, ref_colnum:size(ref_th_Ocn, 2)+ref_colnum-1) = ref_th_Ocn
                ref_colnum = ref_colnum + size(ref_th_Ocn, 2)
                deallocate(ref_th_Ocn)
            end if

        end if
        write(io6, *)  "threshold_Calculation finish in the SUBROUTINE GetRef"
        write(io6, *)  ""

        deallocate(IsInRfArea_sjx) ! allocate in the MOD_GetContain.F90

        ! 对refth, ref_colnum, ref_sjx进行统计聚合
        do i = num_vertex + 1, sjx_points, 1
            if (sum(ref_th(i, :)) > 0) ref_sjx(i) = 1
        end do
        write(io6, *)  "需要细化的三角形个数: ",INT(sum(ref_sjx)), " in the SUBROUTINE GetRef"
        deallocate(ref_th)

        ! Orial ref_sjx save
        if (sum(ref_sjx(1:num_vertex)) > 0) then
            write(io6, *)  "sum(ref_sjx(1:num_vertex)) = ", sum(ref_sjx(1:num_vertex))
            write(io6, *)  "Warning! Please check for SUBROUTINE GetRef"
            ref_sjx(1:num_vertex) = 0 ! 进一步保证N+1级细化出现在N级细化内
        end if
        lndname = trim(file_dir) // "tmpfile/ref_sjx_before_halo_protect_NXP" // trim(nxpc)  // "_"//  trim(stepc) // ".nc4"
        ! write(io6, *)  lndname
        CALL ref_sjx_save(lndname, sjx_points, ref_sjx)

    END SUBROUTINE GetRef

    SUBROUTINE GetRef_Lnd(sjx_points, Lnd_id, Lnd_ii)

        USE MOD_data_preprocess, only: onelayer_Lnd, twolayer_Lnd, input2d_Lnd, input3d_Lnd, landtypes ! 土地覆盖类型编号0为海洋
        implicit none
        integer, intent(in) :: sjx_points
        integer,  dimension(:, :), allocatable, intent(in) :: Lnd_id, Lnd_ii
        integer :: spDimID, twoDimID, DimID, ncid, varid(18), flag_temps
        integer,  dimension(:),    allocatable :: n_landtypes, nlaa, p_num
        real(r8), dimension(:),  allocatable :: f_mainarea
        real(r8), allocatable :: var2d_temp(:, :), var3d_temp(:, :, :)
        character(LEN = 5) :: nxpc, stepc
        character(LEN = 256) :: lndname
        integer :: i, j, row, col, L, num_onelayer, num_twolayer

        ref_colnum = 0
        if (refine_num_landtypes) ref_colnum = ref_colnum + 1
        if (refine_area_mainland) ref_colnum = ref_colnum + 1
        ref_colnum = ref_colnum + count(refine_onelayer_Lnd) + count(refine_twolayer_Lnd)
        write(io6, *)  "Lnd_threshold num : ", ref_colnum
        allocate(ref_th_Lnd(sjx_points, ref_colnum)); ref_th_Lnd = 0

        ref_colnum = 0
        if (refine_num_landtypes) then
            write(io6, *)  "threshold_Calculation for refine_num_landtypes"
            allocate(n_landtypes(sjx_points)); n_landtypes = 0
            allocate(nlaa(0:maxlc)) ! zero is ocean
            !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC,1) &
            !$OMP PRIVATE(i, j, row, col, L, nlaa)
            do i = num_vertex + 1, sjx_points, 1
                ! 当三角形网格不位于细化区域内，or number_select is all in the sea跳过
                if (IsInRfArea_sjx(i) /= 1) cycle
                nlaa = 0  ! 单个非结构网格是否包含该种土地类型的标志（是一个数组），初值全部为0
                do j = 0, Lnd_id(i, 1)-1, 1
                    row = Lnd_ii(Lnd_id(i, 2)+j, 1) ! 获取非结构网格包含经纬度网格的经度序号
                    col = Lnd_ii(Lnd_id(i, 2)+j, 2) ! 获取非结构网格包含经纬度网格的纬度序号
                    L = landtypes(row, col)
                    if (L /= maxlc) nlaa(L) = 1
                end do
                n_landtypes(i) = sum(nlaa) ! 获取单个非结构网格中含有的土地类型数量总和
                if (n_landtypes(i) > th_num_landtypes) ref_sjx(i) = 1
            end do
            !$OMP END PARALLEL DO
            write(io6, *)  "n_landtypes",minval(n_landtypes), maxval(n_landtypes)
            ref_colnum  = ref_colnum + 1
            ref_th_Lnd(:, ref_colnum) = ref_sjx
            ref_sjx = 0 ! 初始化
            deallocate(nlaa)
            write(io6, *)  "threshold_Calculation for refine_num_landtypes : done"
        end if

        if (refine_area_mainland) then
            write(io6, *)  "threshold_Calculation for refine_area_mainland"
            allocate(f_mainarea(sjx_points)); f_mainarea = 0.
            allocate(nlaa(0:maxlc)) ! zero is ocean
            !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC,1) &
            !$OMP PRIVATE(i, j, row, col, L, nlaa)
            do i = num_vertex + 1, sjx_points, 1
                if (IsInRfArea_sjx(i) /= 1) cycle
                nlaa = 0. ! 单个非结构网格内各土地类型面积，（是一个数组），初值为0
                do j = 0, Lnd_id(i, 1) - 1, 1
                    row = Lnd_ii(Lnd_id(i, 2) + j, 1) ! 获取非结构网格包含经纬度网格的经度序号
                    col = Lnd_ii(Lnd_id(i, 2) + j, 2) ! 获取非结构网格包含经纬度网格的纬度序号
                    L = landtypes(row, col) ! 获取这个经纬度网格的土地类型的编号 not ocean
                    if (L /= maxlc) nlaa(L) = nlaa(L) + 1 ! 该类型的土地面积累加
                end do
                f_mainarea(i) = min( maxval(nlaa)/Lnd_id(i, 1), 1) ! 非结构网格内主要土地类型占比
                if (f_mainarea(i) < th_area_mainland) ref_sjx(i) = 1
            end do
            !$OMP END PARALLEL DO
            write(io6, *)  "f_mainarea",minval(f_mainarea), maxval(f_mainarea)
            ref_colnum  = ref_colnum + 1
            ref_th_Lnd(:, ref_colnum) = ref_sjx
            ref_sjx = 0 ! 初始化
            deallocate(nlaa)
            write(io6, *)  "threshold_Calculation for refine_area_mainland : done"
        end if

        ! 改写为循环的形式， 而且先计算均值，再计算标准差, 单层与双层分开
        if (any(refine_onelayer_Lnd .eqv. .true.) .or. &
            any(refine_twolayer_Lnd .eqv. .true.)) then

            write(io6, *)  "threshold_Calculation for refine_onelayer_Lnd or refine_twolayer_Lnd"
            allocate(p_num(sjx_points))

            ! refine_onelayer_Lnd
            if (any(refine_onelayer_Lnd .eqv. .true.)) then
                num_onelayer = 0 ! 单层数据计数
                allocate(output2d(size(refine_onelayer_Lnd)/2))
                allocate(var2d_temp(nlons_Rf_select, nlats_Rf_select)); var2d_temp = 0.
                do i = 1, size(refine_onelayer_Lnd)/2, 1 !
                    if ((refine_onelayer_Lnd(2*i-1) .eqv. .false.) .and. (refine_onelayer_Lnd(2*i) .eqv. .false.)) cycle ! 跳过
                    p_num = 0
                    flag_temps = 0
                    if (refine_onelayer_Lnd(2*i)) flag_temps = 1
                    var2d_temp = input2d_Lnd(i)%var2d
                    CALL mean_std_cal2d(i, sjx_points, flag_temps, Lnd_id, Lnd_ii, var2d_temp, num_onelayer, p_num, ref_th_Lnd)
                end do
                deallocate(var2d_temp)
            end if

            ! refine_twolayer_Lnd
            if (any(refine_twolayer_Lnd .eqv. .true.)) then
                num_twolayer = 0 ! 单层数据计数
                allocate(output3d(size(refine_twolayer_Lnd)/2))
                allocate(var3d_temp(2, nlons_Rf_select, nlats_Rf_select)); var3d_temp = 0.
                do i = 1, size(refine_twolayer_Lnd)/2, 1 !
                    if ((refine_twolayer_Lnd(2*i-1) .eqv. .false.) .and. (refine_twolayer_Lnd(2*i) .eqv. .false.)) cycle ! 跳过
                    p_num = 0
                    flag_temps = 0
                    if (refine_twolayer_Lnd(2*i)) flag_temps = 1
                    var3d_temp = input3d_Lnd(i)%var3d
                    CALL mean_std_cal3d(i, sjx_points, flag_temps, Lnd_id, Lnd_ii, var3d_temp, num_twolayer, p_num, ref_th_Lnd)
                end do
                deallocate(var3d_temp)
            end if

            write(io6, *)  "threshold_Calculation for refine_onelayer_Lnd or refine_twolayer_Lnd : done"
        end if

        ! 阈值计算结果的输出
        write(nxpc, '(I4.4)') nxp
        write(stepc, '(I2.2)') step
        lndname = trim(file_dir) // "threshold/threshold_calculate_land_NXP" // trim(nxpc) //'_'// trim(stepc) // ".nc4"
        write(io6, *)  trim(lndname)
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber,nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncid, "sjx_points", sjx_points, spDimID))
        CALL CHECK(NF90_DEF_DIM(ncid, "dima", 2, twoDimID))
        CALL CHECK(NF90_DEF_DIM(ncid, "ref_colnum", ref_colnum, DimID))
        ref_colnum = 0
        ! NF90_DEF_VAR
        if (refine_num_landtypes) then
            ref_colnum = ref_colnum + 1
            CALL CHECK(NF90_DEF_VAR(ncid, "n_landtypes", NF90_INT, (/ spDimID /), varid(ref_colnum)))
        end if

        if (refine_area_mainland) then
            ref_colnum = ref_colnum + 1
            CALL CHECK(NF90_DEF_VAR(ncid, "f_mainarea", NF90_INT, (/ spDimID /), varid(ref_colnum)))
        end if

        if (any(refine_onelayer_Lnd .eqv. .true.) .or. &
            any(refine_twolayer_Lnd .eqv. .true.)) then
            do i = 1, size(refine_onelayer_Lnd)/2, 1
                if (refine_onelayer_Lnd(2*i-1)) then
                    ref_colnum = ref_colnum + 1
                    CALL CHECK(NF90_DEF_VAR(ncid, trim(onelayer_Lnd(i))//"_m", NF90_DOUBLE , (/ spDimID /), varid(ref_colnum)))
                end if
                if (refine_onelayer_Lnd(2*i)) then
                    ref_colnum = ref_colnum + 1
                    CALL CHECK(NF90_DEF_VAR(ncid, trim(onelayer_Lnd(i))//"_s", NF90_DOUBLE , (/ spDimID /), varid(ref_colnum)))
                end if
            end do
            do i = 1, size(refine_twolayer_Lnd)/2, 1
                if (refine_twolayer_Lnd(2*i-1)) then
                    ref_colnum = ref_colnum + 1
                    CALL CHECK(NF90_DEF_VAR(ncid, trim(twolayer_Lnd(i))//"_m", NF90_DOUBLE , (/ spDimID, twoDimID /), varid(ref_colnum)))
                end if
                if (refine_twolayer_Lnd(2*i)) then
                    ref_colnum = ref_colnum + 1
                    CALL CHECK(NF90_DEF_VAR(ncid, trim(twolayer_Lnd(i))//"_s", NF90_DOUBLE , (/ spDimID, twoDimID /), varid(ref_colnum)))
                end if 
            end do
            CALL CHECK(NF90_DEF_VAR(ncid, "p_num", NF90_INT, (/ spDimID /), varid(ref_colnum + 1)))
        end if

        CALL CHECK(NF90_DEF_VAR(ncid, "ref_th_Lnd", NF90_INT, (/ spDimID, DimID/), varid(ref_colnum + 2)))
        CALL CHECK(NF90_ENDDEF(ncid))

        ! NF90_PUT_VAR
        ref_colnum = 0
        num_onelayer = 0 ! 单层数据计数
        num_twolayer = 0 ! 双层数据计数
        if (refine_num_landtypes) then
            ref_colnum = ref_colnum + 1
            CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum), n_landtypes))
            deallocate(n_landtypes)
        end if

        if (refine_area_mainland) then
            ref_colnum = ref_colnum + 1
            CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum), f_mainarea))
            deallocate(f_mainarea)
        end if

        if (any(refine_onelayer_Lnd .eqv. .true.) .or. &
            any(refine_twolayer_Lnd .eqv. .true.)) then
            do i = 1, size(refine_onelayer_Lnd), 1
                if (refine_onelayer_Lnd(i)) then
                    ref_colnum = ref_colnum + 1
                    num_onelayer = num_onelayer + 1
                    CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum), output2d(num_onelayer)%varms))
                end if
            end do

            do i = 1, size(refine_twolayer_Lnd), 1
                if (refine_twolayer_Lnd(i)) then
                    ref_colnum = ref_colnum + 1
                    num_twolayer = num_twolayer + 1
                    CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum), output3d(num_twolayer)%varms))
                end if
            end do
            CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum+1), p_num))
            deallocate(p_num)
        end if

        CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum+2), ref_th_Lnd(:, 1:ref_colnum)))
        CALL CHECK(NF90_CLOSE(ncid))
        if (allocated(output2d)) deallocate(output2d)
        if (allocated(output3d)) deallocate(output3d)

    END SUBROUTINE GetRef_Lnd

    SUBROUTINE GetRef_Ocn(sjx_points, Ocn_id, Ocn_ii)
        USE MOD_data_preprocess, only: onelayer_Ocn, input2d_Ocn
        implicit none
        integer, intent(in) :: sjx_points
        integer,  dimension(:, :), allocatable, intent(in) :: Ocn_id, Ocn_ii
        integer :: i, num_onelayer, flag_temps
        integer :: spDimID, DimID, ncid, varid(10)
        integer,  dimension(:),    allocatable :: p_num
        real(r8), dimension(:),  allocatable :: sea_ratio
        real(r8), dimension(:,:),allocatable :: var2d_temp
        character(LEN = 5) :: nxpc, stepc
        character(LEN = 256) :: lndname

        ref_colnum = 0
        if (refine_sea_ratio) ref_colnum = ref_colnum + 1
        ref_colnum = ref_colnum + count(refine_onelayer_Ocn)
        write(io6, *)  "Ocn_threshold num : ", ref_colnum
        allocate(ref_th_Ocn(sjx_points, ref_colnum)); ref_th_Ocn = 0
        
        ref_colnum = 0
        if (refine_sea_ratio) then
            write(io6, *)  "threshold_Calculation for refine_sea_ratio"
            allocate(sea_ratio(sjx_points)); sea_ratio = 0.
            do i = num_vertex + 1, sjx_points, 1
                if (IsInRfArea_sjx(i) /= 1) cycle
                sea_ratio(i) = Ocn_id(i, 1)/real(Ocn_id(i, 3))
                if (sea_ratio(i) > th_sea_ratio(1) .and. sea_ratio(i) < th_sea_ratio(2)) ref_sjx(i) = 1
            end do
            write(io6, *)  "sea_ratio",minval(sea_ratio), maxval(sea_ratio)
            ref_colnum  = ref_colnum + 1
            ref_th_Ocn(:, ref_colnum) = ref_sjx
            ref_sjx = 0 ! 初始化
            write(io6, *)  "threshold_Calculation for refine_sea_ratio : done"
        end if

        ! refine_onelayer_Ocn
        if (any(refine_onelayer_Ocn .eqv. .true.)) then
            write(io6, *)  "threshold_Calculation for refine_onelayer_Ocn"
            allocate(p_num(sjx_points))

            num_onelayer = 0 ! 单层数据计数
            allocate(output2d(size(refine_onelayer_Ocn)/2))
            allocate(var2d_temp(nlons_Rf_select, nlats_Rf_select)); var2d_temp = 0.
            do i = 1, size(refine_onelayer_Ocn)/2, 1 !
                if ((refine_onelayer_Ocn(2*i-1) .eqv. .false.) .and. (refine_onelayer_Ocn(2*i) .eqv. .false.)) cycle ! 跳过
                p_num = 0
                flag_temps = 0
                if (refine_onelayer_Ocn(2*i)) flag_temps = 1
                var2d_temp = input2d_Ocn(i)%var2d
                CALL mean_std_cal2d(i, sjx_points, flag_temps, Ocn_id, Ocn_ii, var2d_temp, num_onelayer, p_num, ref_th_Ocn)
            end do
            deallocate(var2d_temp)
            write(io6, *)  "threshold_Calculation for refine_onelayer_Ocn: done"
        end if

        ! ref_th_Ocn_save(ref_colnum, ref_th_Ocn) ! 阈值计算结果的输出
        write(nxpc, '(I4.4)') nxp
        write(stepc, '(I2.2)') step
        lndname = trim(file_dir) // "threshold/threshold_calculate_ocean_NXP" // trim(nxpc) //'_'// trim(stepc) // ".nc4"
        write(io6, *)  trim(lndname)
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber,nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncid, "sjx_points", sjx_points, spDimID))
        CALL CHECK(NF90_DEF_DIM(ncid, "ref_colnum", ref_colnum, DimID))

        ! NF90_DEF_VAR
        ref_colnum = 0
        if (refine_sea_ratio) then
            ref_colnum = ref_colnum + 1
            CALL CHECK(NF90_DEF_VAR(ncid, "sea_ratio", NF90_DOUBLE, (/ spDimID /), varid(ref_colnum)))
        end if

        if (any(refine_onelayer_Ocn .eqv. .true.)) then
            do i = 1, size(refine_onelayer_Ocn)/2, 1
                if (refine_onelayer_Ocn(2*i-1)) then
                    ref_colnum = ref_colnum + 1
                    CALL CHECK(NF90_DEF_VAR(ncid, trim(onelayer_Ocn(i))//"_m", NF90_DOUBLE , (/ spDimID /), varid(ref_colnum)))
                end if
                if (refine_onelayer_Ocn(2*i)) then
                    ref_colnum = ref_colnum + 1
                    CALL CHECK(NF90_DEF_VAR(ncid, trim(onelayer_Ocn(i))//"_s", NF90_DOUBLE , (/ spDimID /), varid(ref_colnum)))
                end if
            end do
            CALL CHECK(NF90_DEF_VAR(ncid, "p_num", NF90_INT, (/ spDimID /), varid(ref_colnum+1)))
        end if
        CALL CHECK(NF90_DEF_VAR(ncid, "ref_th_Ocn", NF90_INT, (/ spDimID, DimID/), varid(ref_colnum+2)))
        CALL CHECK(NF90_ENDDEF(ncid))

        ! NF90_PUT_VAR
        ref_colnum = 0
        num_onelayer = 0 ! 单层数据计数
        if (refine_sea_ratio) then
            ref_colnum = ref_colnum + 1
            CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum), sea_ratio))
            deallocate(sea_ratio)
        end if

        if (any(refine_onelayer_Ocn .eqv. .true.)) then
            do i = 1, size(refine_onelayer_Ocn), 1
                if (refine_onelayer_Ocn(i)) then
                    ref_colnum = ref_colnum + 1
                    num_onelayer = num_onelayer + 1
                    CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum), output2d(num_onelayer)%varms))
                end if
            end do
            CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum+1), p_num))
            deallocate(p_num)
        end if

        CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum+2), ref_th_Ocn(:, 1:ref_colnum)))
        CALL CHECK(NF90_CLOSE(ncid))
        if (allocated(output2d)) deallocate(output2d)

    END SUBROUTINE GetRef_Ocn

    SUBROUTINE GetRef_Atmos(sjx_points, Atmos_id, Atmos_ii)
        USE MOD_data_preprocess, only: onelayer_Atmos, input2d_Atmos
        implicit none
        integer, intent(in) :: sjx_points
        integer,  dimension(:, :), allocatable, intent(in) :: Atmos_id, Atmos_ii
        integer :: i, num_onelayer, flag_temps
        integer :: spDimID, DimID, ncid, varid(10)
        integer,  dimension(:),    allocatable :: p_num
        real(r8), dimension(:),  allocatable :: sea_ratio
        real(r8), dimension(:,:),allocatable :: var2d_temp
        character(LEN = 5) :: nxpc, stepc
        character(LEN = 256) :: lndname

        ref_colnum = 0
        ref_colnum = ref_colnum + count(refine_onelayer_Atmos)
        write(io6, *)  "Atmos_threshold num : ", ref_colnum
        allocate(ref_th_Atmos(sjx_points, ref_colnum)); ref_th_Atmos = 0
        
        ref_colnum = 0
        ! refine_onelayer_Atmos
        if (any(refine_onelayer_Atmos .eqv. .true.)) then
            write(io6, *)  "threshold_Calculation for refine_onelayer_Atmos"
            allocate(p_num(sjx_points))

            num_onelayer = 0 ! 单层数据计数
            allocate(output2d(size(refine_onelayer_Atmos)/2))
            allocate(var2d_temp(nlons_Rf_select, nlats_Rf_select)); var2d_temp = 0.
            do i = 1, size(refine_onelayer_Atmos)/2, 1 !
                if ((refine_onelayer_Atmos(2*i-1) .eqv. .false.) .and. (refine_onelayer_Atmos(2*i) .eqv. .false.)) cycle ! 跳过
                p_num = 0
                flag_temps = 0
                if (refine_onelayer_Atmos(2*i)) flag_temps = 1
                var2d_temp = input2d_Atmos(i)%var2d
                CALL mean_std_cal2d(i, sjx_points, flag_temps, Atmos_id, Atmos_ii, var2d_temp, num_onelayer, p_num, ref_th_Atmos)
            end do
            deallocate(var2d_temp)
            write(io6, *)  "threshold_Calculation for refine_onelayer_Atmos: done"
        end if

        ! ref_th_Atmos_save(ref_colnum, ref_th_Atmos) ! 阈值计算结果的输出
        write(nxpc, '(I4.4)') nxp
        write(stepc, '(I2.2)') step
        lndname = trim(file_dir) // "threshold/threshold_calculate_atmos_NXP" // trim(nxpc) //'_'// trim(stepc) // ".nc4"
        write(io6, *)  trim(lndname)
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber,nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncid, "sjx_points", sjx_points, spDimID))
        CALL CHECK(NF90_DEF_DIM(ncid, "ref_colnum", ref_colnum, DimID))

        ! NF90_DEF_VAR
        ref_colnum = 0
        if (any(refine_onelayer_Atmos .eqv. .true.)) then
            do i = 1, size(refine_onelayer_Atmos)/2, 1
                if (refine_onelayer_Atmos(2*i-1)) then
                    ref_colnum = ref_colnum + 1
                    CALL CHECK(NF90_DEF_VAR(ncid, trim(onelayer_Atmos(i))//"_m", NF90_DOUBLE , (/ spDimID /), varid(ref_colnum)))
                end if
                if (refine_onelayer_Atmos(2*i)) then
                    ref_colnum = ref_colnum + 1
                    CALL CHECK(NF90_DEF_VAR(ncid, trim(onelayer_Atmos(i))//"_s", NF90_DOUBLE , (/ spDimID /), varid(ref_colnum)))
                end if
            end do
            CALL CHECK(NF90_DEF_VAR(ncid, "p_num", NF90_INT, (/ spDimID /), varid(ref_colnum+1)))
        end if
        CALL CHECK(NF90_DEF_VAR(ncid, "ref_th_Atmos", NF90_INT, (/ spDimID, DimID/), varid(ref_colnum+2)))
        CALL CHECK(NF90_ENDDEF(ncid))

        ! NF90_PUT_VAR
        ref_colnum = 0
        num_onelayer = 0 ! 单层数据计数
        if (any(refine_onelayer_Atmos .eqv. .true.)) then
            do i = 1, size(refine_onelayer_Atmos), 1
                if (refine_onelayer_Atmos(i)) then
                    ref_colnum = ref_colnum + 1
                    num_onelayer = num_onelayer + 1
                    CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum), output2d(num_onelayer)%varms))
                end if
            end do
            CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum+1), p_num))
            deallocate(p_num)
        end if

        CALL CHECK(NF90_PUT_VAR(ncid, varid(ref_colnum+2), ref_th_Atmos(:, 1:ref_colnum)))
        CALL CHECK(NF90_CLOSE(ncid))
        if (allocated(output2d)) deallocate(output2d)

    END SUBROUTINE GetRef_Atmos

    SUBROUTINE GetRef_LOC(sjx_points, LOC_id, LOC_ii)
        ! 可以调用lnd与ocn的阈值信息
        implicit none
        integer, intent(in) :: sjx_points
        integer,  dimension(:, :), allocatable, intent(in) :: LOC_id, LOC_ii
        integer :: i, numpatch, num1, num2, num_onelayer, flag_temps
        integer :: spDimID, DimID, ncid, varid(1)
        integer,  dimension(:, :), allocatable :: mp_id, mp_ii
        integer,  dimension(:),    allocatable :: p_num
        real(r8), dimension(:),  allocatable :: sea_ratio
        real(r8), dimension(:,:),allocatable :: var2d_temp
        character(LEN = 5) :: nxpc, stepc
        character(LEN = 256) :: lndname


        ! 将海陆非结构网格数据划分为陆地/海洋非结构网格数据再进行阈值细化
        ! 针对陆地部分
        ref_colnum = 0
        if (refine_num_landtypes) ref_colnum = ref_colnum + 1
        if (refine_area_mainland) ref_colnum = ref_colnum + 1
        ref_colnum = ref_colnum + count(refine_onelayer_Lnd) + count(refine_twolayer_Lnd)
        if (ref_colnum == 0) then 
            write(io6, *)  "no lnd threshold for GetRef in the ", mesh_type
        else
            numpatch = INT(sum(LOC_ii(:, 3)))
            allocate(mp_ii(numpatch, 2)); mp_ii = 0
            numpatch = 0
            do i = 1, size(LOC_ii, 1), 1
                if (LOC_ii(i, 3) == 0) cycle ! 跳过海洋
                numpatch = numpatch + 1
                mp_ii(numpatch, 1:2) = LOC_ii(i, 1:2)
            end do

            allocate(mp_id(sjx_points, 2)); mp_id = 0
            do i = num_vertex + 1, size(LOC_id, 1), 1
                if (LOC_id(i, 1) == 0) cycle
                mp_id(i, 1) = sum(LOC_ii(LOC_id(i, 2):LOC_id(i, 2) + LOC_id(i, 1) - 1, 3)) 
            end do
            mp_id(num_vertex, 2) = 1
            do i = num_vertex + 1, sjx_points, 1
                mp_id(i, 2) = mp_id(i - 1, 2) + mp_id(i - 1, 1)
            end do
            CALL GetRef_Lnd(sjx_points, mp_id, mp_ii)
            deallocate(mp_id, mp_ii)
        end if

        ! 针对海洋部分
        ref_colnum = 0
        if (refine_sea_ratio) ref_colnum = ref_colnum + 1
        if (ref_colnum == 0) then 
            write(io6, *)  "no Ocn threshold for GetRef in the ", mesh_type
        else
            numpatch = INT(size(LOC_ii, 1) - sum(LOC_ii(:, 3)))
            allocate(mp_ii(numpatch, 2)); mp_ii = 0
            numpatch = 0
            do i = 1, size(LOC_ii, 1), 1
                if (LOC_ii(i, 3) == 1) cycle ! 跳过陆地
                numpatch = numpatch + 1
                mp_ii(numpatch, 1:2) = LOC_ii(i, 1:2)
            end do

            allocate(mp_id(sjx_points, 3)); mp_id = 0
            do i = num_vertex + 1, size(LOC_id, 1), 1
                if (LOC_id(i, 1) == 0) cycle
                mp_id(i, 3) = LOC_id(i, 1)
                mp_id(i, 1) = LOC_id(i, 1) - sum(LOC_ii(LOC_id(i, 2):LOC_id(i, 2) + LOC_id(i, 1) - 1, 3)) 
            end do
            mp_id(num_vertex, 2) = 1
            do i = num_vertex + 1, sjx_points, 1
                mp_id(i, 2) = mp_id(i - 1, 2) + mp_id(i - 1, 1)
            end do
            CALL GetRef_Ocn(sjx_points, mp_id, mp_ii)
            deallocate(mp_id, mp_ii)
        end if

        ! 针对大气部分
        ref_colnum = 0
        ref_colnum = ref_colnum + count(refine_onelayer_atmos)
        if (ref_colnum == 0) then
            write(io6, *)  "no atmos threshold for GetRef in the ", mesh_type
            return
        else
            write(io6, *)  "atmos_threshold num : ", ref_colnum
            allocate(mp_id(sjx_points, 2)); mp_id = LOC_id(:,1:2)
            allocate(mp_ii(sjx_points, 2)); mp_ii = LOC_ii
            CALL GetRef_Atmos(sjx_points, mp_id, mp_ii)
            deallocate(mp_id, mp_ii)
        end if

    END SUBROUTINE GetRef_LOC

    SUBROUTINE mean_std_cal2d(ii, sjx_points, flag_temps, mp_id, mp_ii, var2d, num_onelayer, p_num, ref_th_in)
        integer, intent(in) :: ii, sjx_points, flag_temps
        integer,  dimension(:, :), intent(in) :: mp_id, mp_ii
        real(r8), dimension(:, :), intent(in) :: var2d
        
        real(r8), dimension(:),   allocatable :: tempm, temps! 均值计算/标准差计算的中间变量
        integer :: i, j, row, col, L
        integer,  intent(inout) :: num_onelayer
        integer,  dimension(:), intent(inout) :: p_num
        integer, dimension(:, :), allocatable, intent(inout) :: ref_th_in

        allocate(tempm(sjx_points)); tempm = 0.
        !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC,1) &
        !$OMP PRIVATE(i, j, row, col, L)
        do i = num_vertex + 1, sjx_points, 1
            if (IsInRfArea_sjx(i) /= 1) cycle
            do j = 0, mp_id(i, 1) - 1, 1
                row = mp_ii(mp_id(i, 2) + j, 1) ! 获取非结构网格包含经纬度网格的经度序号
                col = mp_ii(mp_id(i, 2) + j, 2) ! 获取非结构网格包含经纬度网格的纬度序号
                L   = landtypes(row, col)
                if (L /= maxlc) then 
                    tempm(i) = tempm(i) + var2d(row, col)
                    p_num(i) = p_num(i) + 1
                end if
            end do
            tempm(i) = tempm(i) / p_num(i) !求均值
        end do
        !$OMP END PARALLEL DO

        if (refine_onelayer_Lnd(2*ii-1)) then
            write(io6, *)  trim(onelayer_Lnd(ii))//"_m", minval(tempm, mask=(tempm /= 0.)), maxval(tempm, mask=(tempm /= 0.)) 
            num_onelayer = num_onelayer + 1
            allocate(output2d(num_onelayer)%varms(sjx_points))
            output2d(num_onelayer)%varms = tempm
            do i = num_vertex + 1, sjx_points, 1
                if (tempm(i) > th_onelayer_Lnd(2*ii-1)) ref_sjx(i) = 1
            end do
            ref_colnum  = ref_colnum + 1
            ref_th_in(:, ref_colnum) = ref_sjx
            ref_sjx = 0 ! 初始化
        end if

        if (flag_temps == 0) then
            deallocate(tempm)
            return
        end if

        allocate(temps(sjx_points)); temps = 0.
        !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC,1) &
        !$OMP PRIVATE(i, j, row, col, L)
        do i = num_vertex + 1, sjx_points, 1
            if (IsInRfArea_sjx(i) /= 1) cycle
            do j = 0, mp_id(i, 1) - 1, 1
                row = mp_ii(mp_id(i, 2) + j, 1)
                col = mp_ii(mp_id(i, 2) + j, 2)
                L   = landtypes(row, col)
                if (L /= maxlc) temps(i) = temps(i) + (var2d(row, col)-tempm(i))**2
            end do
            ! 求标准差
            temps(i) = sqrt(temps(i) / p_num(i))
            if (temps(i) > th_onelayer_Lnd(2*ii)) ref_sjx(i) = 1
        end do
        !$OMP END PARALLEL DO

        write(io6, *)  trim(onelayer_Lnd(ii))//"_s", minval(temps, mask=(temps /= 0.)), maxval(temps, mask=(temps /= 0.))
        num_onelayer = num_onelayer + 1
        allocate(output2d(num_onelayer)%varms(sjx_points))
        output2d(num_onelayer)%varms = temps
        deallocate(temps)
        ref_colnum  = ref_colnum + 1
        ref_th_in(:, ref_colnum) = ref_sjx
        ref_sjx = 0 ! 初始化

    END SUBROUTINE mean_std_cal2d

    SUBROUTINE mean_std_cal3d(ii, sjx_points, flag_temps, mp_id, mp_ii, var3d, num_twolayer, p_num, ref_th_in)
        integer, intent(in) :: ii, sjx_points, flag_temps
        integer,  dimension(:, :), intent(in) :: mp_id, mp_ii
        real(r8), dimension(:, :, :), intent(in) :: var3d
        real(r8), dimension(:, :), allocatable :: tempm, temps! 均值计算/标准差计算的中间变量
        integer :: i, j, L, row, col
        integer,  intent(inout) :: num_twolayer
        integer,  dimension(:), intent(inout) :: p_num
        integer, dimension(:, :), allocatable, intent(inout) :: ref_th_in

        allocate(tempm(sjx_points, 2)); tempm = 0.
        !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC,1) &
        !$OMP PRIVATE(i, j, row, col, L)
        do i = num_vertex + 1, sjx_points, 1
            if (IsInRfArea_sjx(i) /= 1) cycle
            do j = 0, mp_id(i, 1)-1, 1
                row = mp_ii(mp_id(i, 2)+j, 1) ! 获取非结构网格包含经纬度网格的经度序号
                col = mp_ii(mp_id(i, 2)+j, 2) ! 获取非结构网格包含经纬度网格的纬度序号
                L   = landtypes(row, col)
                if (L /= maxlc) then
                    tempm(i, :) = tempm(i, :) + var3d(:, row, col)
                    p_num(i) = p_num(i) + 1
                end if
            end do
            tempm(i, :) = tempm(i, :) / p_num(i) !求均值
        end do
        !$OMP END PARALLEL DO

        if (refine_twolayer_Lnd(2*ii-1)) then 
            num_twolayer = num_twolayer + 1
            allocate(output3d(num_twolayer)%varms(sjx_points, 2))
            output3d(num_twolayer)%varms = tempm
            write(io6, *)  trim(twolayer_Lnd(ii))//"_m", minval(tempm, mask=(tempm /= 0.)), maxval(tempm, mask=(tempm /= 0.))
            do i = num_vertex + 1, sjx_points, 1
                if ((tempm(i, 1) > th_twolayer_Lnd(2*ii-1, 1)) .or. (tempm(i, 2) > th_twolayer_Lnd(2*ii, 2))) ref_sjx(i) = 1
            end do
            ref_colnum = ref_colnum + 1
            ref_th_in(:, ref_colnum) = ref_sjx
            ref_sjx = 0 ! 初始化
        end if

        if (flag_temps == 0) then
            deallocate(tempm)
            return
        end if

        allocate(temps(sjx_points, 2)); temps = 0.
        !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC,1) &
        !$OMP PRIVATE(i, j, row, col, L)
        do i = num_vertex + 1, sjx_points, 1
            if (IsInRfArea_sjx(i) /= 1) cycle
            do j = 0, mp_id(i, 1) - 1, 1
                row = mp_ii(mp_id(i, 2) + j, 1)
                col = mp_ii(mp_id(i, 2) + j, 2)
                L   = landtypes(row, col)
                if (L /= maxlc) temps(i, :) = temps(i, :) + (var3d(:, row, col)-tempm(i, :))**2
            end do
            ! 求标准差
            temps(i, :) = sqrt(temps(i, :) / p_num(i))
            if ((temps(i, 1) > th_twolayer_Lnd(2*ii-1, 1)) .or. (temps(i, 2) > th_twolayer_Lnd(2*ii, 2))) ref_sjx(i) = 1
        end do
        !$OMP END PARALLEL DO

        num_twolayer = num_twolayer + 1
        allocate(output3d(num_twolayer)%varms(sjx_points, 2))
        output3d(num_twolayer)%varms = temps
        deallocate(temps)
        ref_colnum = ref_colnum + 1
        ref_th_in(:, ref_colnum) = ref_sjx
        ref_sjx = 0 ! 初始化

    END SUBROUTINE mean_std_cal3d

END MODULE MOD_GetRef



