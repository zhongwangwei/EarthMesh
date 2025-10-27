module MOD_GetContain
    USE consts_coms, only : pathlen, io6, step, r8, mesh_type, mode_grid, nxp, refine, file_dir, openmp, nlons_source, nlats_source, num_mp_step, num_wp_step, num_vertex, num_center
    USE refine_vars, only : max_iter
    USE netcdf
    USE MOD_file_preprocess, only : Mode4_Mesh_Read, Unstructured_Mesh_Read, Contain_Save
    USE MOD_data_preprocess, only : lon_i, lat_i, lon_vertex, lat_vertex
    USE MOD_Area_judge, only : sum_land_grid, IsInDmArea_grid, seaorland, IsInRfArea_grid, Source_Find, is_point_in_convex_polygon, cross_product, CheckCrossing
    implicit none
    integer, dimension(:), allocatable, public :: IsInRfArea_sjx, IsInDmArea_ustr
    contains

    ! 只有包含关系的计算
    SUBROUTINE  Get_Contain(iter)

        implicit none
        integer, intent(in) :: iter
        character(16) :: type_select
        integer :: ustr_points, sjx_points, lbx_points            ! 三角形网格数与多边形网格数
        integer :: bound_points, mode_points ! use for mode4
        integer :: numpatch_new
        integer :: i, j, k
        integer :: varid, ncid, Dim_ustrID, Dim_aID
        integer,  dimension(:), allocatable :: n_ngrmw, n_ngrwm, ustr_n_ngr, n_ngr
        integer,  dimension(:, :), allocatable :: ngrmw, ngrwm, ustr_ngr ! in the subroutine as ngrwm (sjx--ngrmw lbx--ngrwm)
        real(r8), dimension(:, :), allocatable :: mp, wp, ustr_vertex ! in the subroutine as wp or mp
        real(r8), dimension(:, :), allocatable :: lonlat_bound! , lonlat_bound ! use for mode4
        integer,  dimension(:, :), allocatable :: ngr_bound ! use for mode4
        integer,  dimension(:, :), allocatable :: ustr_id_new, ustr_ii_new
        character(pathlen) :: lndname, outputfile
        character(LEN = 5) :: nxpc, stepc
        
        write(nxpc, '(I4.4)') NXP ! 相当于是integer转字符串character
        write(stepc, '(I2.2)') step 
        if (refine == .true. .and. step <= max_iter) then ! 
            type_select = 'refine'
            write(io6, *), "开始读取非结构网格数据 in the refine area MOD_GetContain.F90"
            lndname = trim(file_dir) // 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_' // trim(mode_grid) // '.nc4'
            write(io6, *), lndname
            CALL Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
            write(io6, *), "mesh read : lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm"
            write(io6, *), "非结构网格数据读取完成 in the refine area MOD_GetContain.F90"
            write(io6, *), ""
            write(io6, *), "共有", sjx_points, "个三角形网格和", lbx_points, "个多边形网格"
            write(io6, *), ""
            write(io6, *), "mode_grid = ", mode_grid, "in the refine area"
            num_mp_step(step) = sjx_points ! will be modified in the MOD_refine.F90
            num_wp_step(step) = lbx_points ! will be modified in the MOD_refine.F90
            num_vertex = num_mp_step(step-1)
            num_center = num_wp_step(step-1)
            ! if (step /= 1) then ! 很好的确认了循环开始的起点
            !     write(io6, *), "before num_center = ", num_center
            !     do i = num_vertex + 1, sjx_points, 1
            !         do j = 1, 3, 1
            !            k = ngrmw(j, i)
            !            if (k < num_center) num_center = k
            !         end do 
            !     end do
            !     write(io6, *), "after num_center = ", num_center
            ! end if
            write(io6, *), "step = ", step, "num_vertex = ", num_vertex, "num_center = ", num_center
            write(io6, *), ""

            allocate(n_ngrmw(sjx_points)); n_ngrmw = 3; n_ngrmw(1) = -1
            ! 获取当前迭代次数下，细化区域内需要计算的三角形/多边形
            !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 
            write(io6, *), "IsInArea_sjx_Calculation start"
            allocate(IsInRfArea_sjx(sjx_points)); IsInRfArea_sjx = 0
            CALL IsInArea_ustr_Calculation(type_select, sjx_points, wp, ngrmw, n_ngrmw, IsInRfArea_sjx) 
            write(io6, *), "IsInArea_sjx_Calculation finish"

            write(io6, *), "Contain_Calculation start"
            ! 但是在阈值细化里面就是都是当作三角形来看，而且采用的是细化区域的信息 
            CALL Contain_Calculation(sjx_points, wp, ngrmw, n_ngrmw, IsInRfArea_grid, IsInRfArea_sjx, ustr_id_new, ustr_ii_new, numpatch_new)
            write(io6, *), "Contain_Calculation finish"
            write(io6, *), ""

            write(io6, *), "Contain_Save start in the GetContain.F90"
            if (iter == 0) then
                lndname = trim(file_dir) // 'contain/contain_'// trim(mesh_type) // '_refine_cal_NXP' //trim(nxpc)//'_'//trim(stepc)//'_tri.nc4'
            else
                lndname = trim(file_dir) // 'contain/contain_'// trim(mesh_type) // '_refine_spc_NXP' //trim(nxpc)//'_'//trim(stepc)//'_tri.nc4'
            end if
            write(io6, *), trim(lndname)
            CALL Contain_Save(lndname, sjx_points, numpatch_new, ustr_id_new, ustr_ii_new, IsInRfArea_sjx)
            write(io6, *), "Contain_Save finish in the GetContain.F90"
            write(io6, *), ""

            ! deallocate
            deallocate(n_ngrmw)
            deallocate(ustr_id_new, ustr_ii_new)

        else
            lndname = trim(file_dir) //  "gridfile/gridfile_NXP"//trim(nxpc)//"_"//trim(stepc)// '_' // trim(mode_grid) // ".nc4"
            outputfile = trim(file_dir)//"result/gridfile_NXP"//trim(nxpc)// "_"//trim(mode_grid) //".nc4"
            CALL execute_command_line('cp '//trim(lndname)//' '//trim(outputfile))
            if (mesh_type == 'atmosmesh') return ! 如果是大气网格，无所谓海陆pixes归属
            type_select = 'domain'
            write(io6, *),"refine finish and contain calculate in the domain region"
            write(io6, *), "开始读取非结构网格数据 in the GetContain.F90"
            write(io6, *), lndname
            write(io6, *), "mode_grid = ", mode_grid, "in the domain"
            write(io6, *), ""
            if ((mode_grid /= 'tri') .and. &
                (mode_grid /= 'hex')) then
                CALL Mode4_Mesh_Read(lndname, bound_points, mode_points, lonlat_bound, ngr_bound, n_ngr)
                write(io6, *), "共有", bound_points, "个网格顶点和", mode_points, "个网格中心"
                write(io6, *), "开始计算", mode_grid, "网格与经纬度网格包含关系数组大小......"
                write(io6, *), ""
                ustr_points = mode_points
                allocate(ustr_vertex(bound_points, 2)); ustr_vertex = lonlat_bound ! use mp rather than wp ??
                allocate(ustr_ngr(4, ustr_points));     ustr_ngr    = ngr_bound
                allocate(ustr_n_ngr(ustr_points));      ustr_n_ngr  = n_ngr
            else
                CALL Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
                write(io6, *), "共有", sjx_points, "个三角形网格和", lbx_points, "个多边形网格"
                write(io6, *), ""
                num_vertex = 1
                if (mode_grid == 'tri') then
                    write(io6, *), "开始计算三角形网格与经纬度网格包含关系数组大小......"
                    ustr_points = sjx_points
                    allocate(ustr_vertex(lbx_points, 2)); ustr_vertex = wp ! use wp rather than mp ???
                    allocate(ustr_ngr(3, ustr_points));   ustr_ngr = ngrmw
                    allocate(ustr_n_ngr(ustr_points)); ustr_n_ngr = 3; ustr_n_ngr(1) = -1
                else if (mode_grid == 'hex') then
                    write(io6, *), "开始计算六边形网格与经纬度网格包含关系数组大小......"
                    ustr_points = lbx_points
                    allocate(ustr_vertex(sjx_points, 2)); ustr_vertex = mp ! use mp rather than wp ??
                    allocate(ustr_ngr(7, ustr_points));   ustr_ngr = ngrwm
                    allocate(ustr_n_ngr(ustr_points));    ustr_n_ngr = n_ngrwm
                    deallocate(n_ngrwm)
                end if
            end if
            write(io6, *), "非结构网格数据读取完成 in the GetContain.F90"
                      
            write(io6, *), "IsInArea_ustr_Calculation start"
            ! 获取包含关系计算区域内需要计算的三角形/多边形
            allocate(IsInDmArea_ustr(ustr_points)); IsInDmArea_ustr = 0
            call IsInArea_ustr_Calculation(type_select, ustr_points, ustr_vertex, ustr_ngr, ustr_n_ngr, IsInDmArea_ustr)
            write(io6, *), "IsInArea_ustr_Calculation finish"

            write(io6, *), "Contain_Calculation start"
            ! 这里应该是包含关系区域的才要计算
            CALL Contain_Calculation(ustr_points, ustr_vertex, ustr_ngr, ustr_n_ngr, IsInDmArea_grid, IsInDmArea_ustr, ustr_id_new, ustr_ii_new, numpatch_new)
            write(io6, *), "包含关系数组大小计算完成......"
            write(io6, *), "Contain_Calculation finish"

            lndname = trim(file_dir) // 'contain/contain_'// trim(mesh_type) // '_domain_NXP'//trim(nxpc)//'_'//trim(mode_grid)//'.nc4'
            write(io6, *), lndname
            write(io6, *), "非结构网格包含数组开始进行存储 in the GetContain.F90"
            CALL Contain_Save(lndname, ustr_points, numpatch_new, ustr_id_new, ustr_ii_new, IsInDmArea_ustr)
            write(io6, *), "非结构网格包含数组存储完成 in the GetContain.F90"
            write(io6, *), ""

            deallocate(ustr_vertex, ustr_ngr, ustr_n_ngr)
            deallocate(ustr_id_new, ustr_ii_new, IsInDmArea_ustr)
        end if

    END SUBROUTINE Get_Contain

    SUBROUTINE IsInArea_ustr_Calculation(type_select, ustr_points, ustr_vertex, ustr_ngr, ustr_n_ngr, IsInArea_ustr)
        ! 判断非结构网格顶点是否位于细化区域/包含关系计算区域内容，只要有一个顶点在就可以了
        USE MOD_Area_judge, only : minlon_DmArea, maxlon_DmArea, maxlat_DmArea, minlat_DmArea, minlon_RfArea, maxlon_RfArea, maxlat_RfArea, minlat_RfArea
        implicit none
        character(16), intent(in) :: type_select
        integer,  intent(in) :: ustr_points
        integer,  dimension(:,:),intent(in) :: ustr_ngr ! 三角形/多边形的顶点编号
        integer,  dimension(:),  intent(in) :: ustr_n_ngr 
        real(r8), dimension(:,:), allocatable, intent(in) :: ustr_vertex ! 三角形/多边形顶点的的经，纬度
        integer :: num_edges, num
        integer :: i, j, k
        real(r8) :: ndm_points(4, 2), ustr(7, 2), ndm_point(2), edgee_temp, edgew_temp, edges_temp, edgen_temp
        integer,  dimension(:)  , allocatable, intent(inout) :: IsInArea_ustr
        logical :: is_inside_temp(4)

        if (type_select == 'domain') then
            edgew_temp = lon_vertex(minlon_DmArea); edgee_temp = lon_vertex(maxlon_DmArea);
            edgen_temp = lat_vertex(maxlat_DmArea); edges_temp = lat_vertex(minlat_DmArea);  
        else if (type_select == 'refine') then
            edgew_temp = lon_vertex(minlon_RfArea); edgee_temp = lon_vertex(maxlon_RfArea);
            edgen_temp = lat_vertex(maxlat_RfArea); edges_temp = lat_vertex(minlat_RfArea); 
        end if
        ndm_points(1, 1) = edgee_temp; ndm_points(1, 2) = edgen_temp
        ndm_points(2, 1) = edgew_temp; ndm_points(2, 2) = edgen_temp
        ndm_points(3, 1) = edgew_temp; ndm_points(3, 2) = edges_temp
        ndm_points(4, 1) = edgee_temp; ndm_points(4, 2) = edges_temp

        ustr = 0.
        num = 0
        ! the first : determind the vertex of ustr in the domain or refine area
        do i = num_vertex+1, ustr_points, 1 ! 第一个都是空三角形/多边形
            ! if (IsInArea_ustr(i)) cycle
            num_edges = ustr_n_ngr(i)
            ustr(1:num_edges, :) = ustr_vertex(ustr_ngr(1:num_edges, i), :)! 直接矩阵赋值，不要循环
            do j = 1, num_edges, 1
                if( (ustr(j, 1) > edgew_temp).and.(ustr(j, 1) < edgee_temp) .and. &
                    (ustr(j, 2) > edges_temp).and.(ustr(j, 2) < edgen_temp) )then
                    IsInArea_ustr(i) = 1 ! 只有一个在就好了
                    num = num + 1
                    exit
                end if
            end do
        end do

        ! the second : determind the vertex of domain/refine area in the ustr
        ! 这是对于跨越180经线的情况,具体有点遗忘了，但是应该还可以优化
        do i = num_vertex+1, ustr_points, 1 ! 第一个都是空三角形/多边形
            if (IsInArea_ustr(i)) cycle
            num_edges = ustr_n_ngr(i)
            ustr(1:num_edges, :) = ustr_vertex(ustr_ngr(1:num_edges, i), :)! 直接矩阵赋值，不要循环
            is_inside_temp = .false.
            do k = 1, 4, 1
                ndm_point = ndm_points(k, :)
                is_inside_temp(k) = is_point_in_convex_polygon(ustr, ndm_point, num_edges)
                if (is_inside_temp(k)) then
                    if (maxval(ustr(:, 1))-minval(ustr(:, 1)) > 180.) then
                        exit ! make sure ustr not cross 180 lontitude
                    end if
                    IsInArea_ustr(i) = 1
                    num = num + 1
                    write(io6, *), "is_inside_temp is TRUE i = ",i, "k = ", k
                    write(io6, *), "ustr = ",ustr
                    write(io6, *), ""
                    exit
                end if
            end do
        end do
        write(io6, *), "需要计算包含关系的网格个数（含重复网格）为 = ", num

    END SUBROUTINE IsInArea_ustr_Calculation

    SUBROUTINE Contain_Calculation(ustr_points, ustr_vertex, ustr_ngr, ustr_n_ngr, IsInArea_grid, IsInArea_ustr, ustr_id_new, ustr_ii_new, numpatch_new)
        implicit none

        integer,  intent(in) :: ustr_points ! 三角形/多边形个数
        real(r8), dimension(:,:),  allocatable, intent(in) :: ustr_vertex
        integer,  dimension(:,:),  intent(in) :: ustr_ngr
        integer,  dimension(:),    intent(in) :: ustr_n_ngr !
        integer,  dimension(:,:),  intent(in) :: IsInArea_grid 
        integer,  dimension(:),    allocatable :: icl_points
        integer,  dimension(:),    allocatable:: ustr_n_ngr_in !
        real(r8), dimension(:,:,:),allocatable :: ustr_move_set
        integer :: ustr_points_in, num_pole
        integer :: id, i, j, k, num_edges, numpatch, ad1, ii, id2
        integer :: maxlon_source, minlon_source, maxlat_source, minlat_source
        real(r8) :: ustr_move(7, 2), point_i(2)
        character(pathlen) :: lndname
        character(LEN = 5) :: nxpc, stepc
        logical :: is_inside_ustr_move
        integer,  dimension(:),    allocatable :: IsInArea_ustr_in
        integer,  dimension(:),    allocatable :: ustr_id_reset
        integer,  dimension(:, :), allocatable :: ustr_id, ustr_ii, Min_matrix_index
        integer,  dimension(:),    allocatable, intent(inout) :: IsInArea_ustr
        integer,  dimension(:, :), allocatable, intent(out) :: ustr_id_new, ustr_ii_new 
        integer, intent(out) :: numpatch_new

        ! 这样的设计，便于处理极点地区，而不影响传入的数值
        ustr_points_in = ustr_points + 5 ! better use for pole in the dbx 因为只有南极需要分为五个三角形
        allocate(IsInArea_ustr_in(ustr_points_in))
        IsInArea_ustr_in = 0
        IsInArea_ustr_in(1:ustr_points) = IsInArea_ustr

        allocate(ustr_n_ngr_in(ustr_points_in))
        ustr_n_ngr_in = 0
        ustr_n_ngr_in(1:ustr_points) = ustr_n_ngr

        allocate(ustr_move_set(7,2,ustr_points_in)); ustr_move_set = 0.
        if (mesh_type == 'landmesh') then
            allocate(ustr_id(ustr_points_in, 2)); ustr_id = 0 ! 网格内陆地pixes总数，陆地网格起点
        else if (mesh_type == 'oceanmesh') then
            allocate(ustr_id(ustr_points_in, 3)); ustr_id = 0 ! 网格内海洋pixes总数，海洋网格起点，网格内pixes总数
        else if (mesh_type == 'atmosmesh') then
            allocate(ustr_id(ustr_points_in, 2)); ustr_id = 0 ! 网格内pixes总数，网格起点
        else if (mesh_type == 'LOCmesh') then
            allocate(ustr_id(ustr_points_in, 2)); ustr_id = 0 ! 网格内pixes总数，网格起点
        end if
        allocate(icl_points(ustr_points_in)); icl_points = 0
        allocate(Min_matrix_index(4, ustr_points_in)); Min_matrix_index = 0

        CALL Data_Updata(ustr_points_in, ustr_vertex, ustr_ngr, IsInArea_ustr_in, ustr_n_ngr_in, ustr_move_set, ustr_id, icl_points, Min_matrix_index)

        ustr_id(num_vertex, 2) = 1
        do i = num_vertex + 1, ustr_points_in, 1
           ustr_id(i, 2) = ustr_id(i - 1, 2) + ustr_id(i - 1, 1)
        end do
        allocate(ustr_id_reset(ustr_points_in)); ustr_id_reset = ustr_id(:, 1)

        numpatch = INT(sum(ustr_id(:, 1)))
        write(io6, *), "非结构网格包含经纬度网格总数（含重复网格）numpatch = ", numpatch
        if (numpatch == 0) stop "ERROR! numpatch == 0 is not right!"
        if ((mesh_type == 'atmosmesh') .or. (mesh_type == 'LOCmesh')) then
            allocate(ustr_ii(numpatch, 3)); ustr_ii = 0
        else
            allocate(ustr_ii(numpatch, 2)); ustr_ii = 0
        end if
        ustr_id(:, 1) = 0

        if (refine == .true. .and. step <= max_iter) then !
            write(io6, *), "开始计算三角形网格与经纬度网格包含关系 in the refine area"
        else
            if (mode_grid == 'tri') then
                write(io6, *), "开始计算三角形网格与经纬度网格包含关系 in the domain area"
            else if (mode_grid == 'hex') then
                write(io6, *), "开始计算多边形网格与经纬度网格包含关系 in the domain area"
            else
                write(io6, *), "开始计算mode4网格与经纬度网格包含关系 in the domain area"
            end if
        end if

        if (mesh_type == 'landmesh') then
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!! use for landmesh !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
            !$OMP PRIVATE(i, j, k, num_edges, ustr_move, minlon_source, maxlon_source, maxlat_source, minlat_source, point_i, is_inside_ustr_move, id)
            do k = num_vertex + 1, ustr_points_in, 1 
                if (IsInArea_ustr_in(k) /= 1) cycle
                if (icl_points(k)) cycle
                num_edges = ustr_n_ngr_in(k)
                ustr_move = ustr_move_set(:, :, k) ! no cross 180
                ! 获取三角形最小外接矩阵的经纬度索引值
                minlon_source = Min_matrix_index(1, k)
                maxlon_source = Min_matrix_index(2, k)
                maxlat_source = Min_matrix_index(3, k)
                minlat_source = Min_matrix_index(4, k)
                ! from 90 to -90 so range from maxlat_source to minlat_source
                do i = minlon_source, maxlon_source-1, 1
                    do j = maxlat_source, minlat_source-1, 1 
                        if (IsInArea_grid(i, j) == 0) cycle
                        if (seaorland(i, j) == 0) cycle! 海洋网格
                        point_i = [lon_i(i), lat_i(j)]
                        is_inside_ustr_move = is_point_in_convex_polygon(ustr_move, point_i, num_edges)
                        if (is_inside_ustr_move) then
                            ustr_id(k, 1) = ustr_id(k, 1) + 1
                            id = ustr_id(k, 2) + ustr_id(k, 1) - 1
                            ustr_ii(id, 1) = i
                            ustr_ii(id, 2) = j
                        end if
                    end do     
                end do
            end do
            !$OMP END PARALLEL DO

            ! 用于计算上一步中出现跨域180经线的事情
            if (sum(icl_points) /= 0) then
                !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
                !$OMP PRIVATE(i, j, k, num_edges, ustr_move, minlon_source, maxlon_source, maxlat_source, minlat_source, point_i, ii, is_inside_ustr_move, id)
                do k = num_vertex + 1, ustr_points_in, 1
                    if (IsInArea_ustr_in(k) /= 1) cycle
                    if (icl_points(k) == 0) cycle
                    num_edges = ustr_n_ngr_in(k)
                    ustr_move = ustr_move_set(:, :, k) ! cross 180
                    ! 获取三角形最小外接矩阵的经纬度索引值
                    minlon_source = Min_matrix_index(1, k)
                    maxlon_source = Min_matrix_index(2, k)
                    maxlat_source = Min_matrix_index(3, k)
                    minlat_source = Min_matrix_index(4, k)
                    ! force on cross 180 lontitude
                    do i = minlon_source, maxlon_source-1, 1
                        do j = maxlat_source, minlat_source-1, 1
                            if (i < nlons_source/2 + 1) then
                                ii = int( i + nlons_source/2)
                            else
                                ii = int( i - nlons_source/2)
                            end if 
                            if (IsInArea_grid(ii, j) == 0) cycle 
                            if (seaorland(ii, j) == 0) cycle
                            
                            point_i = [lon_i(i), lat_i(j)]
                            is_inside_ustr_move = is_point_in_convex_polygon(ustr_move, point_i, num_edges)
                            if (is_inside_ustr_move) then
                                ustr_id(k, 1) = ustr_id(k, 1) + 1
                                id = ustr_id(k, 2) + ustr_id(k, 1) - 1
                                ustr_ii(id, 1) = ii
                                ustr_ii(id, 2) = j
                            end if
                        end do
                    end do
                end do
                !$OMP END PARALLEL DO
            end if
 
        else if (mesh_type == 'oceanmesh') then
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!! use for oceanmesh !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
            !$OMP PRIVATE(i, j, k, num_edges, ustr_move, minlon_source, maxlon_source, maxlat_source, minlat_source, point_i, is_inside_ustr_move, id)
            do k = num_vertex + 1, ustr_points_in, 1
                if (IsInArea_ustr_in(k) /= 1) cycle 
                if (icl_points(k)) cycle
                num_edges = ustr_n_ngr_in(k)
                ustr_move = ustr_move_set(:, :, k) ! no cross 180
                ! 获取三角形最小外接矩阵的经纬度索引值
                minlon_source = Min_matrix_index(1, k)
                maxlon_source = Min_matrix_index(2, k)
                maxlat_source = Min_matrix_index(3, k)
                minlat_source = Min_matrix_index(4, k)
                ! from 90 to -90 so range from maxlat_source to minlat_source
                do i = minlon_source, maxlon_source-1, 1
                    do j = maxlat_source, minlat_source-1, 1 
                        if (IsInArea_grid(i, j) == 0) cycle
                        point_i = [lon_i(i), lat_i(j)]
                        is_inside_ustr_move = is_point_in_convex_polygon(ustr_move, point_i, num_edges)
                        if (is_inside_ustr_move) then
                            ustr_id(k, 3) = ustr_id(k, 3) + 1 ! center 网格中含有指定范围内的pixel个数
                            if (seaorland(i, j) == 0) then
                                ustr_id(k, 1) = ustr_id(k, 1) + 1 ! center 网格中含有指定范围内的海洋pixel个数
                                id = ustr_id(k, 2) + ustr_id(k, 1) - 1
                                ustr_ii(id, 1) = i
                                ustr_ii(id, 2) = j
                            end if
                        end if
                    end do     
                end do
            end do
            !$OMP END PARALLEL DO

            ! 用于计算上一步中出现跨域180经线的事情
            if (sum(icl_points) /= 0) then
                !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
                !$OMP PRIVATE(i, j, k, num_edges, ustr_move, minlon_source, maxlon_source, maxlat_source, minlat_source, point_i, ii, is_inside_ustr_move, id)
                do k = num_vertex + 1, ustr_points_in, 1
                    if (IsInArea_ustr_in(k) /= 1) cycle
                    if (icl_points(k) == 0) cycle
                    num_edges = ustr_n_ngr_in(k)
                    ustr_move = ustr_move_set(:, :, k) ! cross 180
                    ! 获取三角形最小外接矩阵的经纬度索引值
                    minlon_source = Min_matrix_index(1, k)
                    maxlon_source = Min_matrix_index(2, k)
                    maxlat_source = Min_matrix_index(3, k)
                    minlat_source = Min_matrix_index(4, k)
                    ! force on cross 180 lontitude
                    do i = minlon_source, maxlon_source-1, 1
                        do j = maxlat_source, minlat_source-1, 1
                            if (i < nlons_source/2 + 1) then
                                ii = int( i + nlons_source/2)
                            else
                                ii = int( i - nlons_source/2)
                            end if 
                            if (IsInArea_grid(ii, j) == 0) cycle                         
                            point_i = [lon_i(i), lat_i(j)]
                            is_inside_ustr_move = is_point_in_convex_polygon(ustr_move, point_i, num_edges)
                            if (is_inside_ustr_move) then
                                ustr_id(k, 3) = ustr_id(k, 3) + 1 ! 表示非结构网格内含有pixes数量
                                if (seaorland(ii, j) == 0) then
                                    ustr_id(k, 1) = ustr_id(k, 1) + 1 ! 表示非结构网格内含有海洋pixes数量
                                    id = ustr_id(k, 2) + ustr_id(k, 1) - 1
                                    ustr_ii(id, 1) = ii
                                    ustr_ii(id, 2) = j
                                end if
                            end if
                        end do
                    end do
                end do
                !$OMP END PARALLEL DO
            end if

        else if (mesh_type == 'atmosmesh') then
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!! use for atmosmesh !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            !!$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
            !!$OMP PRIVATE(i, j, k, num_edges, ustr_move, minlon_source, maxlon_source, maxlat_source, minlat_source, point_i, is_inside_ustr_move, id)
            do k = num_vertex + 1, ustr_points_in, 1
                if (IsInArea_ustr_in(k) /= 1) cycle 
                if (icl_points(k)) cycle
                num_edges = ustr_n_ngr_in(k)
                ustr_move = ustr_move_set(:, :, k) ! no cross 180
                ! 获取三角形最小外接矩阵的经纬度索引值
                minlon_source = Min_matrix_index(1, k)
                maxlon_source = Min_matrix_index(2, k)
                maxlat_source = Min_matrix_index(3, k)
                minlat_source = Min_matrix_index(4, k)
                ! from 90 to -90 so range from maxlat_source to minlat_source
                do i = minlon_source, maxlon_source-1, 1
                    do j = maxlat_source, minlat_source-1, 1 
                        if (IsInArea_grid(i, j) == 0) cycle
                        point_i = [lon_i(i), lat_i(j)]
                        is_inside_ustr_move = is_point_in_convex_polygon(ustr_move, point_i, num_edges)
                        if (is_inside_ustr_move) then
                            ustr_id(k, 1) = ustr_id(k, 1) + 1 ! center 网格中含有指定范围内的海洋pixel个数
                            id = ustr_id(k, 2) + ustr_id(k, 1) - 1
                            ustr_ii(id, 1) = i
                            ustr_ii(id, 2) = j
                            if (seaorland(i, j) == 1) ustr_ii(id, 3) = 1 ! 0为海洋pixes
                        end if
                    end do     
                end do
            end do
            !!$OMP END PARALLEL DO

            ! 用于计算上一步中出现跨域180经线的事情
            if (sum(icl_points) /= 0) then
                !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
                !$OMP PRIVATE(i, j, k, num_edges, ustr_move, minlon_source, maxlon_source, maxlat_source, minlat_source, point_i, ii, is_inside_ustr_move, id)
                do k = num_vertex + 1, ustr_points_in, 1
                    if (IsInArea_ustr_in(k) /= 1) cycle
                    if (icl_points(k) == 0) cycle
                    num_edges = ustr_n_ngr_in(k)
                    ustr_move = ustr_move_set(:, :, k) ! cross 180
                    ! 获取三角形最小外接矩阵的经纬度索引值
                    minlon_source = Min_matrix_index(1, k)
                    maxlon_source = Min_matrix_index(2, k)
                    maxlat_source = Min_matrix_index(3, k)
                    minlat_source = Min_matrix_index(4, k)
                    ! force on cross 180 lontitude
                    do i = minlon_source, maxlon_source-1, 1
                        do j = maxlat_source, minlat_source-1, 1
                            if (i < nlons_source/2 + 1) then
                                ii = int( i + nlons_source/2)
                            else
                                ii = int( i - nlons_source/2)
                            end if 
                            if (IsInArea_grid(ii, j) == 0) cycle                         
                            point_i = [lon_i(i), lat_i(j)]
                            is_inside_ustr_move = is_point_in_convex_polygon(ustr_move, point_i, num_edges)
                            if (is_inside_ustr_move) then
                                ustr_id(k, 1) = ustr_id(k, 1) + 1 ! 表示非结构网格内含有海洋pixes数量
                                id = ustr_id(k, 2) + ustr_id(k, 1) - 1
                                ustr_ii(id, 1) = ii
                                ustr_ii(id, 2) = j
                                if (seaorland(ii, j) == 1) ustr_ii(id, 3) = 1 ! 0为海洋pixes
                            end if
                        end do
                    end do
                end do
                !$OMP END PARALLEL DO
            end if

        else if (mesh_type == 'LOCmesh') then
            stop "ERROR! mesh_type == 'LOCmesh' can not use now"
        end if

        deallocate(icl_points)
        do k = num_vertex + 1, ustr_points_in, 1
            if (ustr_id(k, 1) == 0) IsInArea_ustr_in(k) = 0
        end do

        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        ! 完成从ustr_points_in到ustr_points，要求去掉后面五个空白位置! 这个可能只是用单独的海洋或者陆地网格，大气网格不适用！！！！！！
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        write(6, *) "代码的正确性存疑，需要进一步测试"

        if (sum(IsInArea_ustr_in(ustr_points+1:ustr_points_in)) > 0) then
            write(io6, *), "sum(IsInArea_ustr_in(ustr_points+1:ustr_points_in)) > 0 is true!"
            ! 更新num_pole的ustr_id和ustr_ii
            if (mesh_type == 'oceanmesh') ustr_id(num_pole, 3) = ustr_id(num_pole, 3) + sum(ustr_id(ustr_points + 1:ustr_points_in, 3))
            IsInArea_ustr_in(num_pole) = 1
            IsInArea_ustr_in(ustr_points+1:ustr_points_in) = 0
            do k = ustr_points + 1, ustr_points_in, 1
                do i = 1, ustr_id(k, 1), 1
                    ustr_id(num_pole, 1) = ustr_id(num_pole, 1) + 1 ! 记录海洋pixes总数
                    id = ustr_id(num_pole, 2) + ustr_id(num_pole, 1) - 1
                    ustr_ii(id, 1) = ustr_ii(i + ustr_id(k, 2) - 1, 1)
                    ustr_ii(id, 2) = ustr_ii(i + ustr_id(k, 2) - 1, 2)
                end do
                if (mesh_type /= 'LOCmesh') cycle
                stop "ERROR! never have the fourth columns in the varibale ustr_ii"
                do i = 1, ustr_id(k, 3), 1 ! 针对大气网格中的陆地部分
                    ustr_id(num_pole, 3) = ustr_id(num_pole, 3) + 1 ! 记录海洋pixes总数
                    id = ustr_id(num_pole, 4) + ustr_id(num_pole, 3) - 1
                    ustr_ii(id, 3) = ustr_ii(i + ustr_id(k, 4) - 1, 3)
                    ustr_ii(id, 4) = ustr_ii(i + ustr_id(k, 4) - 1, 4)
                end do
            end do
        end if
 
        ustr_move = 0.
        ! check for data range if sufficient
        do k = num_vertex + 1, ustr_points, 1           ! 循环遍历三角形网格
            if (ustr_id(k, 1) > ustr_id_reset(k) ) then
                write(io6, *), "k = ", k, "ustr_id(k, 1) = ", ustr_id(k, 1), "ustr_id_reset(k) = ",ustr_id_reset(k)
                num_edges = ustr_n_ngr_in(k)
                ustr_move = ustr_move_set(:, :, k) ! ustr_id_reset check
                write(io6, *), "num_edges = ", num_edges
                write(io6, *), "ustr_move = ", ustr_move
                ! stop "stop for data range over reset in the MOD_Getcontain.F90 Line 499 please "
            end if
        end do
        
        write(io6, *), "Contain_Renew start"
        ! 完成ustr_id ustr_ii数据更新! 将由于多预留空间导致的空值删去! 并重新计算序号
        if (mesh_type /= 'oceanmesh') then
            allocate(ustr_id_new(ustr_points, 2)); ustr_id_new = 0
        else if (mesh_type == 'oceanmesh') then
            allocate(ustr_id_new(ustr_points, 3)); ustr_id_new = 0
            ustr_id_new(num_vertex:ustr_points, 3) = ustr_id(num_vertex:ustr_points, 3)
        end if
        ustr_id_new(num_vertex:ustr_points, 1) = ustr_id(num_vertex:ustr_points, 1)

        numpatch_new = INT(sum(ustr_id_new(:, 1)))
        if ((mesh_type == 'atmosmesh') .or. (mesh_type == 'LOCmesh')) then
            allocate(ustr_ii_new(numpatch_new, 3)); ustr_ii_new = 0
        else
            allocate(ustr_ii_new(numpatch_new, 2)); ustr_ii_new = 0
        end if

        ! 更新ustr_id_new(i, 2) 
        ustr_id_new(num_vertex, 2) = 1
        do i = num_vertex + 1, ustr_points, 1
            ustr_id_new(i, 2) = ustr_id_new(i - 1, 2) + ustr_id_new(i - 1, 1)
        end do
        ! 更新ustr_ii_new(i, :) 
        do i = num_vertex + 1, ustr_points, 1
            if (ustr_id_new(i, 1) == 0) cycle
            ustr_ii_new(ustr_id_new(i, 2) : ustr_id_new(i, 2) + ustr_id_new(i, 1) - 1, :) &
            =   ustr_ii(    ustr_id(i, 2) : ustr_id(i, 2)     + ustr_id_new(i, 1) - 1, :)
        end do
        
        write(io6, *),"非结构网格实际包含经纬度网格总数 numpatch_new = ", numpatch_new
        if ((refine == .true.) .and. (step <= max_iter)) then !
            write(io6, *), "三角形网格包含数组计算完成 in the refine area"
        else
            if (mode_grid == 'tri') then
                write(io6, *), "三角形网格包含数组计算完成 in the domain area"
            else if (mode_grid == 'hex') then
                write(io6, *), "多边形网格包含数组计算完成  in the domain area"
            else
                write(io6, *), "mode4网格包含数组计算完成  in the domain area"
            end if
            if (numpatch_new /= sum_land_grid) write(io6, *), "warning! numpatch_new /= sum_land_grid"
        end if

        IsInArea_ustr = IsInArea_ustr_in(1:ustr_points)
        deallocate(ustr_id, ustr_ii, ustr_move_set, IsInArea_ustr_in)
        write(io6, *), "Contain_Renew finish"

    END SUBROUTINE Contain_Calculation

    SUBROUTINE Data_Updata(ustr_points_in, ustr_vertex, ustr_ngr, IsInArea_ustr_in, ustr_n_ngr_in, ustr_move_set, ustr_id, icl_points, Min_matrix_index)
        implicit none

        integer,  intent(in) :: ustr_points_in ! 三角形/多边形个数
        real(r8), dimension(:,:),  allocatable, intent(in) :: ustr_vertex
        integer,  dimension(:,:),  intent(in) :: ustr_ngr
        integer :: ustr_points
        integer :: i, j, k, num_edges, ad1, ii, num_pole, numpatch_select
        integer :: maxlon_source, minlon_source, maxlat_source, minlat_source
        real(r8) :: ustr_move(7, 2), maxlat_m, minlat_m, point_i(2)
        integer,  dimension(:),    allocatable, intent(inout) :: IsInArea_ustr_in
        integer,  dimension(:),    allocatable, intent(inout) :: ustr_n_ngr_in
        real(r8), dimension(:,:,:),allocatable, intent(inout) :: ustr_move_set
        integer,  dimension(:, :), allocatable, intent(inout) :: ustr_id
        integer,  dimension(:),    allocatable, intent(inout) :: icl_points
        integer,  dimension(:, :), allocatable, intent(inout) :: Min_matrix_index

        maxlat_m = maxval(ustr_vertex(:, 2))
        minlat_m = minval(ustr_vertex(:, 2))
        write(io6, *), "minlat_m = ", minlat_m, "maxlat_m = ", maxlat_m
        ustr_points = ustr_points_in - 5
        do k = num_vertex + 1, ustr_points_in, 1           ! 循环遍历三角形网格
            ! 在阈值计算的时候判断是否在细化区域内，在包含关系计算时候判断是否在包含区域内
            if (IsInArea_ustr_in(k) /= 1) cycle
            ustr_move = 0.
            num_edges = ustr_n_ngr_in(k) ! 主要是针对多边形，但是非极点三角形也是使用的
            if (ustr_move_set(1, 1, k) /= 0.) then
                ustr_move = ustr_move_set(:, :, k) ! 针对多边形在南极以及附近的情况
            else
                ustr_move(1:num_edges, :) = ustr_vertex(ustr_ngr(1:num_edges, k), 1:2)! 直接矩阵赋值，不要循环
            end if

            if (minval(ustr_move(:, 2)) == minlat_m) then
                if (num_edges == 3) then
                    write(io6, *), "k = ", k, "south pole in the triangle"
                    ustr_move(3:4, :) = ustr_vertex(ustr_ngr(2:3, k), 1:2)
                    ustr_move(2, 1)   = ustr_vertex(ustr_ngr(2, k), 1)
                    ustr_move(1, 1)   = ustr_vertex(ustr_ngr(3, k), 1)
                    ustr_move(1:2, 2) = ustr_vertex(ustr_ngr(1, k), 2)
                    ustr_move_set(:, :, k) = ustr_move ! north or south pole in the triangle
                    num_edges = num_edges + 1 ! 便于ustr_id使用
                    ustr_n_ngr_in(k) = num_edges ! 便于后续计算包含关系使用
                    write(io6, *), "ustr_move = ", ustr_move
                else if (num_edges == 5) then !!!!!!!! 这是针对mode = 6的情况
                    IsInArea_ustr_in(k) = 0 ! 取消原本的多边形
                    write(io6, *), "k = ", k, "south pole in the polygon"
                    write(io6, *), "orial ustr_move = ", ustr_move
                    do i = 1, num_edges - 1, 1
                        do j = i + 1, num_edges, 1
                            if (ustr_move(i, 1) > ustr_move(j, 1)) then
                                ustr_move(7, :) = ustr_move(i, :)
                                ustr_move(i, :) = ustr_move(j, :)
                                ustr_move(j, :) = ustr_move(7, :)
                            end if
                        end do
                    end do
                    write(io6, *), "sorted ustr_move = ", ustr_move
                    ustr_move_set(:, :, k) = ustr_move
                    num_pole = k
                    write(io6, *), "num_pole = ", num_pole

                    do i = 1, num_edges, 1
                        if (refine .and. step > 1) then
                            ! 处理与南极五边形相连的五个六边形
                            ustr_move = 0.
                            ii = num_pole+1+(i-1)*nxp*nxp
                            write(io6, *), 'order of adjacent_polygons = ', ii
                            ustr_move(1:6, :) = ustr_vertex(ustr_ngr(1:6, ii), 1:2)
                            write(io6, *), 'Before adjacent_polygons = ', ustr_move
                            ustr_move(1, 1) = ustr_move(num_edges + 1, 1)
                            ustr_move(2, 1) = ustr_move(3, 1)
                            ustr_move_set(:, :, ii) = ustr_move
                            write(io6, *), 'After adjacent_polygons = ', ustr_move
                        end if

                        ustr_move = 0.
                        j = mod(i, num_edges) + 1
                        ustr_move(1, :)   = ustr_move_set(j, :, k)
                        ustr_move(2, :)   = ustr_move_set(i, :, k)
                        ustr_move(3, 1)   = ustr_move_set(i, 1, k)
                        ustr_move(4, 1)   = ustr_move_set(j, 1, k)
                        ustr_move(3:4, 2) = -90.
                        ustr_move_set(:, :, ustr_points + i) = ustr_move ! north or south pole in the triangle
                        ustr_n_ngr_in(ustr_points + i) = num_edges - 1 ! 便于后续计算包含关系使用
                        IsInArea_ustr_in(ustr_points + i) = 1
                        write(io6, *), "i = ", i
                        write(io6, *), "ustr_move = ", ustr_move
                    end do
                end if
            else if (ustr_move(1, 2) == maxlat_m) then
                write(io6, *), "north pole in the triangle or mode4 or dbx jump out"
                IsInArea_ustr_in(k) = 0
                cycle
            end if
            
            ! if cross 180 longitude
            if (maxval(ustr_move(1:num_edges, 1)) - minval(ustr_move(1:num_edges, 1)) > 180.) then
                icl_points(k) = 1
                call CheckCrossing(num_edges, ustr_move) ! 已经进行跨越180经线的处理，再计算最小外接矩形
            end if
            ustr_move_set(:, :, k) = ustr_move ! after 180 cross
            ! lon from -180 to 180   lat from 90 to -90
            CALL Source_Find(minval(ustr_move(1:num_edges, 1)), lon_vertex, 'lon', Min_matrix_index(1, k))! minlon_source
            CALL Source_Find(maxval(ustr_move(1:num_edges, 1)), lon_vertex, 'lon', Min_matrix_index(2, k))! maxlon_source
            CALL Source_Find(maxval(ustr_move(1:num_edges, 2)), lat_vertex, 'lat', Min_matrix_index(3, k))! maxlat_source
            CALL Source_Find(minval(ustr_move(1:num_edges, 2)), lat_vertex, 'lat', Min_matrix_index(4, k))! minlat_source 纬度大值反而是小的索引值
            Min_matrix_index(1, k) = max(1, Min_matrix_index(1, k) - 1)
            Min_matrix_index(3, k) = max(1, Min_matrix_index(3, k) - 1)
            ! calculate the size of ustrgrid need and adjust IsInArea_ustr_in
            numpatch_select = (Min_matrix_index(2, k) - Min_matrix_index(1, k)) * (Min_matrix_index(4, k) - Min_matrix_index(3, k))
            ad1 = 0
            if (icl_points(k)) then
                ! write(io6, *)," cross 180 lontitude ustr_move"
                do i = Min_matrix_index(1, k), Min_matrix_index(2, k)-1, 1
                    if (i < nlons_source/2 + 1) then
                        ii = int(i + nlons_source/2)
                    else
                        ii = int(i - nlons_source/2)
                    end if
                    ad1 = ad1 + sum(seaorland(ii, Min_matrix_index(3, k):Min_matrix_index(4, k)-1))
                end do
            else
                ad1 = sum(seaorland(Min_matrix_index(1, k):Min_matrix_index(2, k)-1, Min_matrix_index(3, k):Min_matrix_index(4, k)-1))
            end if
             
            if (trim(mesh_type) == 'landmesh') then
                if (ad1 == 0) IsInArea_ustr_in(k) = -1 ! 这是针对陆地网格，说明这个网格没有陆地pixes
                ustr_id(k, 1) = min(ad1, numpatch_select)
            else if (trim(mesh_type) == 'oceanmesh') then 
                ustr_id(k, 1) = numpatch_select
                if (ad1 == numpatch_select) then
                    ustr_id(k, 1) = 0
                    IsInArea_ustr_in(k) = -1 ! 这是针对海洋网格，说明这个网格没有海洋pixes
                end if
            else if (trim(mesh_type) == 'atmosmesh') then
                ustr_id(k, 1) = numpatch_select
            else if (trim(mesh_type) == 'LOCmesh') then
                ustr_id(k, 1) = numpatch_select
            end if
        end do

        ! 要给南极地区的多边形留下足够的空位置
        if (sum(IsInArea_ustr_in(ustr_points+1:ustr_points_in)) > 0) then 
            write(io6, *), "pole in the dbx"
            ustr_id(num_pole, 1) = sum(ustr_id(ustr_points+1:ustr_points_in, 1))
        end if

    END SUBROUTINE Data_Updata


END module MOD_GetContain
