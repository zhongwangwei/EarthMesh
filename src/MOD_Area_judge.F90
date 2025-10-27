module MOD_Area_judge
    USE consts_coms
    USE refine_vars, only: refine_setting, mask_refine_cal_type, mask_refine_spc_type, mask_refine_ndm
    USE MOD_file_preprocess, only: bbox_mesh_read,  mode4_mesh_Read, circle_mesh_read, close_mesh_read
    USE MOD_data_preprocess, only: nlons_Rf_select, nlats_Rf_select, landtypes_global, landtypes, lon_i, lat_i, lon_vertex, lat_vertex, Threshold_Read_Lnd, Threshold_Read_Ocn, Threshold_Read_Atmos
    implicit none
    ! 判断是否是海洋>网格，判断是否在domain, 判断是否在refine
    integer, allocatable, public :: seaorland(:, :), IsInDmArea_grid(:, :)
    integer, allocatable, public :: IsInRfArea_grid(:, :), IsInRfArea_cal_grid(:, :)
    integer, public :: nlons_Dm_select, nlats_Dm_select
    integer, public :: minlon_DmArea, maxlon_DmArea, maxlat_DmArea, minlat_DmArea
    integer, public :: minlon_RfArea, maxlon_RfArea, maxlat_RfArea, minlat_RfArea
    integer, public :: minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal
    integer, public :: minlon_PaArea, maxlon_PaArea, maxlat_PaArea, minlat_PaArea
    integer, public :: sum_land_grid
    contains

    SUBROUTINE Area_judge()

        IMPLICIT NONE
        integer :: i, j, iter
        integer :: numpatch_Dm
        integer, allocatable :: IsInDmArea_select(:, :), seaorland_select(:, :)
        character(16) :: type_select
        character(LEN = 256) :: inputfile, outputfile

        if (.not. mask_restart) then
            write(io6, *) 'IsInArea_grid_Calculation start'
            allocate(IsInDmArea_grid(nlons_source, nlats_source)); IsInDmArea_grid = 0
            minlon_DmArea = nlons_source; maxlon_DmArea = 1
            maxlat_DmArea = nlats_source; minlat_DmArea = 1

            if (mask_domain_global) then
                IsInDmArea_grid = 1
                numpatch_Dm = nlons_source * nlats_source
                minlon_DmArea = 1; maxlon_DmArea = nlons_source
                maxlat_DmArea = 1; minlat_DmArea = nlats_source
            else
                iter = 0
                type_select = 'mask_domain' ! 用于domain区域的网格标记
                if (mask_domain_type == 'bbox') then
                    CALL IsInArea_bbox_Calculation(type_select, iter, mask_domain_ndm, IsInDmArea_grid, numpatch_Dm) 
                else if (mask_domain_type == 'lambert') then
                    CALL IsInArea_lambert_Calculation(type_select, iter, mask_domain_ndm, IsInDmArea_grid, numpatch_Dm)
                else if (mask_domain_type == 'circle') then
                    CALL IsInArea_circle_Calculation(type_select, iter, mask_domain_ndm, IsInDmArea_grid, numpatch_Dm)
                else if (mask_domain_type == 'close') then
                    CALL IsInArea_close_Calculation(type_select, iter, mask_domain_ndm, IsInDmArea_grid, numpatch_Dm)
                end if
            end if

            write(io6, *), 'sea or land judge start'
            allocate(seaorland(nlons_source, nlats_source)); seaorland = 0
            if ((mesh_type == 'atmosmesh') .and. (refine .eqv. .false.)) then
                write(io6, *), "seaorland no need more deal!"
            else
                !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
                !$OMP PRIVATE(i, j)
                do j = maxlat_DmArea, minlat_DmArea, 1 ! 影响先内层循环
                    do i = minlon_DmArea, maxlon_DmArea, 1
                        if (IsInDmArea_grid(i, j)) then
                            ! 包含关系计算区域内海陆网格判断, 只对0处理就好了，maxlc是冰川或者水体可以先不弄
                            if (landtypes_global(i, j) /= 0) seaorland(i, j) = 1 ! 1 表示为陆地网格
                        end if
                    end do
                end do
                !$OMP END PARALLEL DO
                sum_land_grid = sum(seaorland)
                write(io6, *),"包含关系计算区域内, 陆地网格个数为",sum_land_grid  
            end if
            nlons_Dm_select = maxlon_DmArea - minlon_DmArea + 1
            nlats_Dm_select = minlat_DmArea - maxlat_DmArea + 1 
            write(io6, *) "numpatch_Dm   = ", numpatch_Dm
            write(io6, *) "minlon_DmArea = ", minlon_DmArea
            write(io6, *) "maxlon_DmArea = ", maxlon_DmArea
            write(io6, *) "maxlat_DmArea = ", maxlat_DmArea
            write(io6, *) "minlat_DmArea = ", minlat_DmArea
            write(io6, *) 'IsInArea_grid_Calculation complete'

            ! write(io6, *) "IsInDmArea_grid save start"
            ! outputfile = trim(file_dir) // 'result/IsInDmArea_grid.nc4' ! 这是最终的输出结果
            ! CALL IsInArea_grid_Save(outputfile, IsInDmArea_grid, minlon_DmArea, maxlon_DmArea, maxlat_DmArea, minlat_DmArea)
            ! write(io6, *) "IsInDmArea_grid save finish"
            ! write(io6, *) ""
 
        end if


        ! 补丁的本质就是修改seaorland
        if (mask_patch_on) then
            write(io6, *) 'make grid with patch mesh in the MOD_Area_judge.F90'
            if (mask_restart) then ! 如果开启restart就直接读入之前保存的数据而重新计算
                allocate(seaorland(nlons_source, nlats_source)); seaorland = 0
                allocate(IsInDmArea_grid(nlons_source, nlats_source)); IsInDmArea_grid = 0
                CALL IsInArea_grid_Read(inputfile, IsInDmArea_select, seaorland_select)
                seaorland(minlon_DmArea:maxlon_DmArea, maxlat_DmArea:minlat_DmArea) = seaorland_select
                IsInDmArea_grid(minlon_DmArea:maxlon_DmArea, maxlat_DmArea:minlat_DmArea) = IsInDmArea_select
            end if
            type_select = 'mask_patch'
            CALL mask_patch_modify(type_select, iter)
        end if
        
        if ((.not. refine) .or. (refine_setting == 'specified')) return

        allocate(IsInRfArea_cal_grid(nlons_source, nlats_source)); IsInRfArea_cal_grid = 0
        minlon_RfArea = nlons_source; maxlon_RfArea = 1
        maxlat_RfArea = nlats_source; minlat_RfArea = 1
        type_select = 'mask_refine' ! 用于阈值细化区域的网格标记
        if (mask_refine_cal_type == 'bbox') then
            CALL IsInArea_bbox_Calculation(type_select, iter, mask_refine_ndm(iter), IsInRfArea_cal_grid)
        else if (mask_refine_cal_type == 'lambert') then
            CALL IsInArea_lambert_Calculation(type_select, iter, mask_refine_ndm(iter), IsInRfArea_cal_grid)
        else if (mask_refine_cal_type == 'circle') then
            CALL IsInArea_circle_Calculation(type_select, iter, mask_refine_ndm(iter), IsInRfArea_cal_grid)
        else if (mask_refine_cal_type == 'close') then
            CALL IsInArea_close_Calculation(type_select, iter, mask_refine_ndm(iter), IsInRfArea_cal_grid)
        end if
        minlon_RfArea_cal = minlon_RfArea
        maxlon_RfArea_cal = maxlon_RfArea
        maxlat_RfArea_cal = maxlat_RfArea
        minlat_RfArea_cal = minlat_RfArea
        write(io6, *) ""
        write(io6, *) "refine_degree = ", iter
        write(io6, *) "minlon_RfArea_cal = ", minlon_RfArea
        write(io6, *) "maxlon_RfArea_cal = ", maxlon_RfArea
        write(io6, *) "maxlat_RfArea_cal = ", maxlat_RfArea
        write(io6, *) "minlat_RfArea_cal = ", minlat_RfArea
        write(io6, *) ""
        if (minlon_RfArea_cal > maxlon_RfArea_cal) stop "ERROR! minlon_RfArea_cal > maxlon_RfArea_cal"
        if (maxlat_RfArea_cal > minlat_RfArea_cal) stop "ERROR! maxlat_RfArea_cal > minlat_RfArea_cal" 
        ! 不知道这个东西的运算效率如何？？
        !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
        !$OMP PRIVATE(i, j)
        do j = maxlat_RfArea_cal, minlat_RfArea_cal, 1 ! 影响先内层循环
            do i = minlon_RfArea_cal, maxlon_RfArea_cal, 1
                if (IsInRfArea_cal_grid(i, j)) then
                    if (IsInDmArea_grid(i, j) == 0) then ! 保证细化区域网格位于包含关系计算区域
                        write(io6, *),"ERROR!!! the refine area exceed the domain area!!!"
                        stop ! 错误！！！细化区域和包含关系计算区域之间没有交集！！！”
                    end if
                end if
            end do
        end do
        !$OMP END PARALLEL DO
        ! “细化区域完全位于包含关系计算区域内……”
        write(io6, *),"the refine area Completely locate at the domain area!!!"
        ! 然后才开始读取阈值数据，而且是安装给定的范围去读取数据
        nlons_Rf_select = maxlon_RfArea_cal - minlon_RfArea_cal + 1
        nlats_Rf_select = minlat_RfArea_cal - maxlat_RfArea_cal + 1
        allocate(landtypes(nlons_Rf_select, nlats_Rf_select))
        landtypes = landtypes_global(minlon_RfArea_cal:maxlon_RfArea_cal, maxlat_RfArea_cal:minlat_RfArea_cal)
        if (mesh_type == 'landmesh') then
            CALL Threshold_Read_Lnd(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal) 
        else if (mesh_type == 'oceanmesh') then
            CALL Threshold_Read_Ocn(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal)
        else if (mesh_type == 'atmosmesh') then
            CALL Threshold_Read_Atmos(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal)
        else if (mesh_type == 'LOCmesh') then
            CALL Threshold_Read_Lnd(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal)
            CALL Threshold_Read_Ocn(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal)
            CALL Threshold_Read_Atmos(minlon_RfArea_cal, maxlon_RfArea_cal, maxlat_RfArea_cal, minlat_RfArea_cal) 
        end if

        ! write(io6, *) "IsInRfArea_cal_grid save start"
        ! outputfile = trim(file_dir) // 'result/IsInRfArea_cal_grid.nc4' ! 这是最终的输出结果
        ! CALL IsInArea_grid_Save(outputfile, IsInRfArea_cal_grid, minlon_RfArea, maxlon_RfArea, maxlat_RfArea, minlat_RfArea)
        ! write(io6, *) "IsInRfArea_cal_grid save finish"
        ! write(io6, *) ""

    END SUBROUTINE Area_judge

    SUBROUTINE Area_judge_refine(iter)
        ! 针对specified的时候细化底图每一次都要重新更新的情况
        IMPLICIT NONE
        integer, intent(in) :: iter
        integer :: i, j
        character(16) :: type_select, iterc
        character(256) :: outputfile

        if (iter == 0) then
            IsInRfArea_grid = IsInRfArea_cal_grid
            minlon_RfArea   = minlon_RfArea_cal
            maxlon_RfArea   = maxlon_RfArea_cal
            maxlat_RfArea   = maxlat_RfArea_cal
            minlat_RfArea   = minlat_RfArea_cal
            return
        end if

        if (.not. allocated(IsInRfArea_grid)) allocate(IsInRfArea_grid(nlons_source, nlats_source))
        IsInRfArea_grid = 0
        minlon_RfArea = nlons_source; maxlon_RfArea = 1
        maxlat_RfArea = nlats_source; minlat_RfArea = 1

        type_select = 'mask_refine' ! 用于指定细化区域的网格标记
        if (mask_refine_spc_type == 'bbox') then
            CALL IsInArea_bbox_Calculation(type_select, iter, mask_refine_ndm(iter), IsInRfArea_grid) 
        else if (mask_refine_spc_type == 'lambert') then
            CALL IsInArea_lambert_Calculation(type_select, iter, mask_refine_ndm(iter), IsInRfArea_grid)
        else if (mask_refine_spc_type == 'circle') then
            CALL IsInArea_circle_Calculation(type_select, iter, mask_refine_ndm(iter), IsInRfArea_grid)
        else if (mask_refine_spc_type == 'close') then
            CALL IsInArea_close_Calculation(type_select, iter, mask_refine_ndm(iter), IsInRfArea_grid)
        end if
        write(io6, *) "refine_degree = ", iter
        write(io6, *) "minlon_RfArea = ", minlon_RfArea
        write(io6, *) "maxlon_RfArea = ", maxlon_RfArea
        write(io6, *) "maxlat_RfArea = ", maxlat_RfArea
        write(io6, *) "minlat_RfArea = ", minlat_RfArea
        if (minlon_RfArea > maxlon_RfArea) stop "ERROR! minlon_RfArea > maxlon_RfArea"
        if (maxlat_RfArea > maxlat_RfArea) stop "ERROR! maxlat_RfArea > maxlat_RfArea"
        ! 不知道这个东西的运算效率如何？？
        !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
        !$OMP PRIVATE(i, j)
        do j = maxlat_RfArea, minlat_RfArea, 1 ! 影响先内层循环
            do i = minlon_RfArea, maxlon_RfArea, 1
                if (IsInRfArea_grid(i, j)) then
                    if (IsInDmArea_grid(i, j) == 0) then ! 保证细化区域网格位于包含关系计算区域
                        write(io6, *),"ERROR!!! the refine area exceed the domain area!!!"
                        stop ! 错误！！！细化区域和包含关系计算区域之间没有交集！！！”
                    end if
                end if
            end do
        end do
        !$OMP END PARALLEL DO
        ! “细化区域完全位于包含关系计算区域内……”
        write(io6, *),"the refine area Completely locate at the domain area!!!"
        ! write(io6, *) "IsInRfArea_grid_spc save start"
        ! write(iterc, '(I2.2)') iter
        ! outputfile = trim(file_dir) // 'result/IsInRfArea_grid_spc_'//trim(iterc)//'.nc4' ! 这是最终的输出结果
        ! CALL IsInArea_grid_Save(outputfile, IsInRfArea_grid, minlon_RfArea, maxlon_RfArea, maxlat_RfArea, minlat_RfArea)
        ! write(io6, *) "IsInRfArea_grid_spc save finish"
        ! write(io6, *) ""
    END SUBROUTINE Area_judge_refine

    ! 只需要修改seaorland而不需要修改IsInDmArea_grid！！！！！！！！！
    SUBROUTINE mask_patch_modify(type_select, iter)
        
        IMPLICIT NONE
        character(16), intent(in) :: type_select
        integer, intent(in) :: iter
        integer, allocatable :: IsInPaArea_grid(:, :)
        integer :: i, j

        allocate(IsInPaArea_grid(nlons_source, nlats_source)); IsInPaArea_grid = 0
        minlon_PaArea = nlons_source; maxlon_PaArea = 1
        maxlat_PaArea = nlats_source; minlat_PaArea = 1

        ! 用于补丁区域的网格标记
        if (mask_patch_type == 'bbox') then
            CALL IsInArea_bbox_Calculation(type_select, iter, mask_patch_ndm(iter), IsInPaArea_grid) 
        else if (mask_patch_type == 'lambert') then
            CALL IsInArea_lambert_Calculation(type_select, iter, mask_patch_ndm(iter), IsInPaArea_grid)
        else if (mask_patch_type == 'circle') then
            CALL IsInArea_circle_Calculation(type_select, iter, mask_patch_ndm(iter), IsInPaArea_grid)
        else if (mask_patch_type == 'close') then
            CALL IsInArea_close_Calculation(type_select, iter, mask_patch_ndm(iter), IsInPaArea_grid)
        end if

        ! 修改seaorland
        write(io6, *) "修改seaorland start!"
        !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
        !$OMP PRIVATE(i, j)
        do j = maxlat_PaArea, minlat_PaArea, 1 ! 影响先内层循环
            do i = minlon_PaArea, maxlon_PaArea, 1
                if (IsInPaArea_grid(i, j)) seaorland(i, j) = 0 ! 陆地转为海洋piexs
            end do
        end do
        !$OMP END PARALLEL DO
        deallocate(IsInPaArea_grid)
        write(io6, *) "修改seaorland finish!"

    END SUBROUTINE mask_patch_modify

    SUBROUTINE IsInArea_bbox_Calculation(type_select, iter, ndm, IsInArea_grid, numpatch)
    ! 判断经纬度网格是否位于细化区域/包含关系计算区域
        implicit none
        character(16), intent(in) :: type_select
        integer, intent(in) :: iter, ndm
        character(pathlen) :: lndname
        real(r8) :: edgee_temp, edgew_temp, edges_temp, edgen_temp
        integer :: n, i, bbox_num, temp1, temp2, temp3, temp4
        real(r8), allocatable :: bbox_points(:,:)
        integer, intent(inout) :: IsInArea_grid(:, :)
        integer, intent(out), optional :: numpatch
        character(5) :: numc, refinec

        if (present(numpatch)) numpatch = 0
        write(refinec, '(I1)') iter
        do n = 1, ndm, 1
            write(numc, '(I2.2)') n
            lndname = trim(file_dir)// 'tmpfile/'//trim(type_select)//'_bbox_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            write(io6, *) trim(lndname), ': reading'
            CALL bbox_Mesh_Read(lndname, bbox_num, bbox_points)
            do i = 1, bbox_num, 1
                edgew_temp = bbox_points(i, 1)
                edgee_temp = bbox_points(i, 2)
                edgen_temp = bbox_points(i, 3)
                edges_temp = bbox_points(i, 4)
                CALL minmax_range_make(type_select, edgew_temp, edgee_temp, edgen_temp, edges_temp, temp1, temp2, temp3, temp4)
                ! calculate the size of ustrgrid need and adjust IsInArea_ustr
                IsInArea_grid(temp1:temp2,temp3:temp4) = 1
                if (present(numpatch)) numpatch = numpatch + (temp2 - temp1 + 1) * (temp4 - temp3 + 1)
            end do
        end do

    END SUBROUTINE IsInArea_bbox_Calculation

    SUBROUTINE IsInArea_lambert_Calculation(type_select, iter, ndm, IsInArea_grid, numpatch)
        ! 判断经纬度网格是否位于细化区域/包含关系计算区域
        implicit none
        character(16), intent(in) :: type_select
        integer, intent(in) :: iter, ndm
        character(pathlen) :: lndname
        integer :: i, j, k, ii, num_edges, n
        integer :: temp1, temp2, temp3, temp4 
        integer :: minlon_source, maxlon_source, maxlat_source, minlat_source
        integer :: bound_points, mode_points! use for mode4
        logical :: is_inside_temp_points
        real(r8):: temp_points(7, 2), point_i(2), point_c(2), edgee_temp, edgew_temp, edges_temp, edgen_temp
        real(r8), dimension(:, :), allocatable :: lonlat_bound! , lonlat_bound ! use for mode4
        integer,  dimension(:, :), allocatable :: ngr_bound ! use for mode4
        integer,  dimension(:),    allocatable :: n_ngr
        integer,  dimension(:),    allocatable :: icl_points, num_points
        integer, dimension(:,:), intent(inout) :: IsInArea_grid
        integer, intent(out), optional :: numpatch
        character(5) :: numc, refinec

        num_edges = 4
        temp_points = 0.    
        write(refinec, '(I1)') iter
        do n = 1, ndm, 1
            ! lon from -180 to 180   lat from 90 to -90
            write(numc, '(I2.2)') n
            lndname = trim(file_dir)// 'tmpfile/'//trim(type_select)//'_lambert_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL Mode4_Mesh_Read(lndname, bound_points, mode_points, lonlat_bound, ngr_bound, n_ngr)
            edgew_temp = minval(lonlat_bound(2:bound_points, 1))
            edgee_temp = maxval(lonlat_bound(2:bound_points, 1))
            edgen_temp = maxval(lonlat_bound(2:bound_points, 2))
            edges_temp = minval(lonlat_bound(2:bound_points, 2))
            write(io6, *) "n = ", n
            write(io6, *) "edgew_temp = ", edgew_temp
            write(io6, *) "edgee_temp = ", edgee_temp
            write(io6, *) "edgen_temp = ", edgen_temp
            write(io6, *) "edges_temp = ", edges_temp
            ! 如果出现跨越180经线的情况, 需要额外处理
            point_c = 0.
            do i = 2, bound_points - 1, 1
                if (point_c(1) < abs(lonlat_bound(i+1, 1) - lonlat_bound(i, 1))) then
                    point_c(1) = abs(lonlat_bound(i+1, 1) - lonlat_bound(i, 1))
                end if
            end do
            point_c(2) = abs(edgee_temp-edgew_temp)
            if (point_c(1) > point_c(2)) then
                write(io6, *) "cross 180! need modified"
                edgew_temp = -180.
                edgee_temp =  180.
                write(io6, *) "调整后："
                write(io6, *) "edgew_temp = ", edgew_temp
                write(io6, *) "edgee_temp = ", edgee_temp
                write(io6, *) "edgen_temp = ", edgen_temp
                write(io6, *) "edges_temp = ", edges_temp
            else
                write(io6, *) "not cross 180! not need modified"
            end if
            ! 这个好像不可以跨越180，否则会出错的
            CALL minmax_range_make(type_select, edgew_temp, edgee_temp, edgen_temp, edges_temp)

            ! calculate the size of ustrgrid need and adjust IsInArea_ustr
            allocate(icl_points(mode_points)); icl_points = 0
            allocate(num_points(mode_points)); num_points = 0
            !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
            !$OMP PRIVATE(i, j, k, ii, temp_points, minlon_source, maxlon_source, maxlat_source, minlat_source, point_i, is_inside_temp_points)
            do k = 2, mode_points, 1
                temp_points(1:num_edges, :) = lonlat_bound(ngr_bound(1:num_edges, k), :)! 直接矩阵赋值，不要循环
                ! if cross 180 longitude
                if (maxval(temp_points(:, 1)) - minval(temp_points(:, 1)) > 180.) then
                    icl_points(k) = 1
                    call CheckCrossing(num_edges, temp_points)
                end if

                ! lon from -180 to 180   lat from 90 to -90
                CALL Source_Find(minval(temp_points(1:num_edges, 1)), lon_vertex, 'lon', minlon_source)! minlon_source
                CALL Source_Find(maxval(temp_points(1:num_edges, 1)), lon_vertex, 'lon', maxlon_source)! maxlon_source
                CALL Source_Find(maxval(temp_points(1:num_edges, 2)), lat_vertex, 'lat', maxlat_source)! maxlat_source
                CALL Source_Find(minval(temp_points(1:num_edges, 2)), lat_vertex, 'lat', minlat_source)! minlat_source 纬度大值反而是小的索引值
                minlon_source = max(1, minlon_source - 1) ! why？？？
                maxlat_source = max(1, maxlat_source - 1) ! why？？？

                ! point inside temp_points
                if (icl_points(k)) then ! 跨越180经线
                    do i = minlon_source, maxlon_source - 1, 1
                        do j = maxlat_source, minlat_source - 1, 1
                            point_i = [lon_i(i), lat_i(j)]
                            is_inside_temp_points = is_point_in_convex_polygon(temp_points, point_i, num_edges)
                            if (i < nlons_source/2 + 1) then ! 还原
                                ii = int( i + nlons_source/2)
                            else
                                ii = int( i - nlons_source/2)
                            end if
                            if (is_inside_temp_points) then 
                                IsInArea_grid(ii, j) = 1
                                num_points(k) = num_points(k) + 1
                            end if
                        end do
                    end do
                else
                    do i = minlon_source, maxlon_source - 1, 1
                        do j = maxlat_source, minlat_source - 1, 1
                            point_i = [lon_i(i), lat_i(j)]
                            is_inside_temp_points = is_point_in_convex_polygon(temp_points, point_i, num_edges)
                            if (is_inside_temp_points) then
                                IsInArea_grid(i, j) = 1
                                num_points(k) = num_points(k) + 1
                            end if 
                        end do
                    end do
                end if
            end do
            !$OMP END PARALLEL DO
            if (present(numpatch)) numpatch = sum(num_points)
            deallocate(icl_points, num_points)
        end do

    END SUBROUTINE IsInArea_lambert_Calculation

    SUBROUTINE IsInArea_circle_Calculation(type_select, iter, ndm, IsInArea_grid, numpatch)
        ! 要求知道圆与各个纬度的交点的坐标
        implicit none
        character(16), intent(in) :: type_select
        integer, intent(in) :: iter, ndm
        character(pathlen) :: lndname
        integer :: i, j, k, ii, circle_num, n
        integer :: minlon_source, maxlon_source, maxlat_source, minlat_source
        real(r8) :: point_i(2), point_c(2), radius_c, temp
        real(r8) :: edgee_temp, edgew_temp, edges_temp, edgen_temp
        real(r8), allocatable :: circle_points(:,:), circle_radius(:)
        integer,  dimension(:), allocatable :: num_points
        integer,  dimension(:, :), allocatable :: num_points_ij
        integer, dimension(:,:), intent(inout) :: IsInArea_grid
        integer, intent(out), optional :: numpatch
        character(5) :: numc, refinec
        logical :: is_inside_circle, exist
        
        write(refinec, '(I1)') iter
        do n = 1, ndm, 1
            write(numc, '(I2.2)') n
            lndname = trim(file_dir)// 'tmpfile/'//trim(type_select)//'_circle_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL circle_Mesh_Read(lndname, circle_num, circle_points, circle_radius)
            ! calculate the size of ustrgrid need and adjust IsInArea_ustr
            allocate(num_points(circle_num)); num_points = 0

            write(io6, *) "circle_num = ", circle_num
            do k = 1, circle_num, 1 ! start from 1
                point_c  = circle_points(k, :)
                radius_c = circle_radius(k)
                ! lon from -180 to 180   lat from 90 to -90 edge*_temp 还需要考虑半径的影响
                ! 先确定edgew_temp, edgee_temp, edgen_temp, edges_temp，如果存在include pole则小心
                temp = pio180_r8 * erad/1000
                edgew_temp = point_c(1) - (radius_c/(temp*cos(point_c(2)*pio180_r8))) * 1.2_r8
                edgee_temp = point_c(1) + (radius_c/(temp*cos(point_c(2)*pio180_r8))) * 1.2_r8
                edgen_temp = point_c(2) + (radius_c/temp) * 1.2_r8
                edges_temp = point_c(2) - (radius_c/temp) * 1.2_r8
                write(io6, *) "point_c = ", point_c, "radius_c = ", radius_c
                write(io6, *) "edgew_temp = ", edgew_temp
                write(io6, *) "edgee_temp = ", edgee_temp
                write(io6, *) "edgen_temp = ", edgen_temp
                write(io6, *) "edges_temp = ", edges_temp
                exist = .false.
                ! 如果出现跨越180经线的情况,假设移动后不会再次出现跨越180经线的情况
                if ((edgee_temp > 180.) .or. (edgew_temp < -180.) .or. &
                    (edgen_temp >  90.) .or. (edgen_temp < -90.)) then
                    edgew_temp = -180.
                    edgee_temp =  180.
                    exist = .true.
                end if

                ! 如果出现包含极点的情况
                if (edgen_temp > 90. ) then
                    edges_temp = min(edges_temp, edgen_temp)
                    edgen_temp = 90.
                    exist = .true.
                else if (edges_temp < -90. ) then
                    edgen_temp = max(edges_temp, edgen_temp)
                    edges_temp = -90.
                    exist = .true.
                end if

                if (exist) then
                    write(io6, *) "调整后："
                    write(io6, *) "edgew_temp = ", edgew_temp
                    write(io6, *) "edgee_temp = ", edgee_temp
                    write(io6, *) "edgen_temp = ", edgen_temp
                    write(io6, *) "edges_temp = ", edges_temp
                end if

                ! 存在跨越180经线的情况，需要小心处理
                ! 先确定minlon_source, maxlon_source, maxlat_source, minlat_source
                CALL Source_Find(edgew_temp, lon_vertex, 'lon', minlon_source)! minlon_source
                CALL Source_Find(edgee_temp, lon_vertex, 'lon', maxlon_source)! maxlon_source
                CALL Source_Find(edgen_temp, lat_vertex, 'lat', maxlat_source)! maxlat_source
                CALL Source_Find(edges_temp, lat_vertex, 'lat', minlat_source)! minlat_source 纬度大值反而是小的索引值
                CALL minmax_range_make(type_select, edgew_temp, edgee_temp, edgen_temp, edges_temp)
                write(io6, *) "minlon_source = ", minlon_source
                write(io6, *) "maxlon_source = ", maxlon_source
                write(io6, *) "maxlat_source = ", maxlat_source
                write(io6, *) "minlat_source = ", minlat_source
                write(io6, *) ""
                if (minlon_source >= maxlon_source) stop "ERROR! minlon_source >= maxlon_source"
                if (maxlat_source >= minlat_source) stop "ERROR! maxlat_source >= minlat_source" 
                allocate(num_points_ij(maxlon_source-minlon_source+1, minlat_source-maxlat_source+1)); num_points = 0
                !$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC)&
                !$OMP PRIVATE(i, j, point_i)
                do i = minlon_source, maxlon_source - 1, 1
                    do j = maxlat_source, minlat_source - 1, 1
                        if (IsInArea_grid(i, j)) cycle ! 不计算重复点位
                        point_i = [lon_i(i), lat_i(j)]
                        is_inside_circle = is_point_in_circle(point_i, point_c, radius_c)
                        if (is_inside_circle) then 
                            IsInArea_grid(i, j) = 1
                            num_points_ij(i-minlon_source+1, j-maxlat_source+1) = 1
                        end if
                    end do
                end do
                !$OMP END PARALLEL DO
                num_points(k) = sum(num_points_ij)
                deallocate(num_points_ij)
            end do
            if (present(numpatch)) numpatch = sum(num_points)
            deallocate(num_points, circle_points, circle_radius)
        end do

    END SUBROUTINE IsInArea_circle_Calculation

    ! 存在过多的write，需要进一步简化
    SUBROUTINE IsInArea_close_Calculation(type_select, iter, ndm, IsInArea_grid, numpatch)
        ! 计算闭合曲线中的pixes包含关系（采用射线法，注意跨180经线的情况）
        IMPLICIT NONE
        character(16), intent(in) :: type_select
        integer, intent(in) :: iter, ndm
        character(pathlen) :: lndname
        integer :: i, j, k, ii, close_num, n, icl_point, num_intersect
        integer :: minlon_source, maxlon_source, maxlat_source, minlat_source
        real(r8) :: point_i(2), point_c(2), lon1, lat1, lon2, lat2, lon_intersect
        real(r8) :: edgee_temp, edgew_temp, edges_temp, edgen_temp
        real(r8), allocatable :: close_points(:,:), ray_segment_intersect_lon(:,:)
        integer,  allocatable :: ray_segment_intersect_num(:)
        integer, dimension(:,:), intent(inout) :: IsInArea_grid
        integer, intent(out), optional :: numpatch
        character(LEN = 5) :: numc, refinec

        if (present(numpatch)) numpatch = 0
        do n = 1, ndm, 1
            write(refinec, '(I1)') iter
            write(numc, '(I2.2)') n
            lndname = trim(file_dir)// 'tmpfile/'//trim(type_select)//'_close_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            write(io6, *) trim(lndname)
            CALL close_Mesh_Read(lndname, close_num, close_points)
            ! 先检查线段之间是否存在自交线段,并返回线段的排序结果
            CALL check_self_intersection(close_num, close_points)
            write(io6, *) "Not line segment self intersection! "

            edgew_temp = minval(close_points(:, 1))
            edgee_temp = maxval(close_points(:, 1))
            edgen_temp = maxval(close_points(:, 2))
            edges_temp = minval(close_points(:, 2))
            write(io6, *) "n = ", n
            write(io6, *) "edgew_temp = ", edgew_temp
            write(io6, *) "edgee_temp = ", edgee_temp
            write(io6, *) "edgen_temp = ", edgen_temp
            write(io6, *) "edges_temp = ", edges_temp
            ! 如果出现跨越180经线的情况, 需要额外处理
            point_c = 0.
            do i = 1, close_num - 1, 1
                if (point_c(1) < abs(close_points(i+1, 1) - close_points(i, 1))) then
                    point_c(1) = abs(close_points(i+1, 1) - close_points(i, 1))
                end if
            end do
            point_c(2) = abs(edgee_temp-edgew_temp)
            if (point_c(1) > point_c(2)) then
                write(io6, *) "cross 180! need modified"
                icl_point = 1
                edgew_temp = -180.
                edgee_temp =  180.
                write(io6, *) "调整后："
                write(io6, *) "edgew_temp = ", edgew_temp
                write(io6, *) "edgee_temp = ", edgee_temp
                write(io6, *) "edgen_temp = ", edgen_temp
                write(io6, *) "edges_temp = ", edges_temp
                CALL CheckCrossing(close_num, close_points) ! 跨越180经线
            else
                write(io6, *) "not cross 180! not need modified"
                icl_point = 0
            end if
            ! 这个好像不可以跨越180，否则会出错的
            ! 如果出现跨越180经线，那就整体移动！！！！！！！
            CALL minmax_range_make(type_select, edgew_temp, edgee_temp, edgen_temp, edges_temp)

            ! calculate the size of ustrgrid need and adjust IsInArea_ustr
            CALL Source_Find(edgen_temp, lat_vertex, 'lat', maxlat_source)! maxlat_source
            CALL Source_Find(edges_temp, lat_vertex, 'lat', minlat_source)! minlat_source 纬度大值反而是小的索引值
            write(io6, *) "maxlat_source = ", maxlat_source
            write(io6, *) "minlat_source = ", minlat_source 
            ! 获取射线与线段相交的经度与线段编号
            allocate(ray_segment_intersect_lon(minlat_source - maxlat_source,100)); ray_segment_intersect_lon = 0.
            allocate(ray_segment_intersect_num(minlat_source - maxlat_source));     ray_segment_intersect_num = 0
            do j = maxlat_source, minlat_source - 1, 1
                point_i = [real(-200.0, r8), lat_i(j)] ! 射线起点。从左往右
                do i = 1, close_num, 1
                    lon1 = close_points(i, 1)
                    lat1 = close_points(i, 2)
                    lon2 = close_points(mod(i, close_num) + 1, 1)
                    lat2 = close_points(mod(i, close_num) + 1, 2)
                    CALL ray_segment_intersect(point_i, lat1, lon1, lat2, lon2, lon_intersect)
                    if (lon_intersect /= point_i(1)) then 
                        ! write(io6, *) "i = ", i, "j = ", j, "lon_intersect = ", lon_intersect
                        ray_segment_intersect_num(j - maxlat_source + 1) = ray_segment_intersect_num(j - maxlat_source + 1) + 1 ! 记录相交的个数
                        ray_segment_intersect_lon(j - maxlat_source + 1,   ray_segment_intersect_num(j - maxlat_source + 1)) = lon_intersect
                    end if
                end do
            end do

            ! 采用射线法, 奇偶之间为内部，偶奇之间为外部，但是要求先从小到大的排序
            CALL bubble_sort(ray_segment_intersect_num, ray_segment_intersect_lon)


            if (icl_point) then ! 跨越180经线
                do j = maxlat_source, minlat_source - 1, 1
                    num_intersect = int(ray_segment_intersect_num(j - maxlat_source + 1)/2)
                    do k = 1, num_intersect, 1
                        edgew_temp = ray_segment_intersect_lon(j - maxlat_source + 1, 2*k-1)
                        edgee_temp = ray_segment_intersect_lon(j - maxlat_source + 1, 2*k)
                        CALL Source_Find(edgew_temp, lon_vertex, 'lon', minlon_source)! minlon_source
                        CALL Source_Find(edgee_temp, lon_vertex, 'lon', maxlon_source)! maxlon_source
                        if (present(numpatch)) numpatch = numpatch + maxlon_source - minlon_source
                        do i = minlon_source, maxlon_source - 1, 1
                            if (i < nlons_source/2 + 1) then ! 还原
                                ii = int( i + nlons_source/2)
                            else
                                ii = int( i - nlons_source/2)
                            end if
                            IsInArea_grid(ii, j) = 1
                        end do
                    end do
                end do
            else
                do j = maxlat_source, minlat_source - 1, 1
                    num_intersect = int(ray_segment_intersect_num(j - maxlat_source + 1)/2)
                    do k = 1, num_intersect, 1
                        edgew_temp = ray_segment_intersect_lon(j - maxlat_source + 1, 2*k-1)
                        edgee_temp = ray_segment_intersect_lon(j - maxlat_source + 1, 2*k)
                        CALL Source_Find(edgew_temp, lon_vertex, 'lon', minlon_source)! minlon_source
                        CALL Source_Find(edgee_temp, lon_vertex, 'lon', maxlon_source)! maxlon_source
                        if (present(numpatch)) numpatch = numpatch + maxlon_source - minlon_source
                        IsInArea_grid(minlon_source:maxlon_source - 1, j) = 1
                    end do
                end do
            end if
            deallocate(ray_segment_intersect_lon, ray_segment_intersect_num)
        end do

    END SUBROUTINE IsInArea_close_Calculation

    SUBROUTINE ray_segment_intersect(point_i, lat1, lon1, lat2, lon2, lon_intersect)
        implicit none
        real(r8), intent(in)  :: point_i(2), lat1, lon1, lat2, lon2
        real(r8), intent(out) :: lon_intersect
        real(r8) :: lat_p, lon_p,  m

        ! 分别获取射线的经度与纬度
        lon_p = point_i(1); lat_p = point_i(2)

        ! 如果线段的两个纬度相同，等同于不相交
        if (lat1 == lat2) then
            lon_intersect = lon_p
            return
        end if

        ! 判断射线是否与线段相交，如果射线不在线段的纬度范围之内，不相交
        if ((lat1 > lat_p .and. lat2 > lat_p) .or. (lat1 < lat_p .and. lat2 < lat_p)) then
            lon_intersect = lon_p
            return
        end if

        m = (lat2 - lat1) / (lon2 - lon1)
        lon_intersect = lon1 + (lat_p - lat1) / m

    END SUBROUTINE ray_segment_intersect

    SUBROUTINE bubble_sort(ray_segment_intersect_num, ray_segment_intersect_lon)

        implicit none
        integer,  intent(in) :: ray_segment_intersect_num(:)
        integer :: i, j, k, num
        real(r8) :: temp
        real(r8), intent(inout) :: ray_segment_intersect_lon(:, :)

        do k = 1, size(ray_segment_intersect_num), 1
            num = ray_segment_intersect_num(k)
            do i = 1, num - 1
                do j = i + 1, num, 1
                    if (ray_segment_intersect_lon(k, i) > ray_segment_intersect_lon(k, j)) then
                        temp = ray_segment_intersect_lon(k, i)
                        ray_segment_intersect_lon(k, i) = ray_segment_intersect_lon(k, j)
                        ray_segment_intersect_lon(k, j) = temp
                    end if
                end do
            end do
        end do

    END SUBROUTINE bubble_sort

    SUBROUTINE check_self_intersection(close_num, close_points)
        ! 使用平面几何中的 线段相交算法，通过叉积法判断两条线段是否相交
        implicit none
        integer, intent(in) :: close_num
        real(r8), allocatable, intent(in) :: close_points(:,:)
        integer :: n, i, j
        real(r8), allocatable :: lon(:), lat(:)
        logical :: intersect

        allocate(lon(close_num+1)); lon = 0.
        allocate(lat(close_num+1)); lat = 0.
        ! 读取经纬度并转换为笛卡尔坐标(对于这个操作我感到疑问)
        ! do i = 1, close_num, 1
        !     ! 转为笛卡尔坐标
        !     ! lon(i) = erad * cos(close_points(i, 2)*pio180_r8) * cos(close_points(i, 1)*pio180_r8) ! 3.141592653589793 / 180.0
        !     ! lat(i) = erad * cos(close_points(i, 2)*pio180_r8) * sin(close_points(i, 1)*pio180_r8)
        ! end do
        lon(1:close_num) = close_points(1:close_num, 1)
        lat(1:close_num) = close_points(1:close_num, 2)
        lon(1+close_num) = lon(1)
        lat(1+close_num) = lat(1)

        ! 检测线段是否自交
        do i = 1, close_num - 2, 1
            do j = i + 2, close_num, 1
                intersect = segments_intersect(lat(i), lon(i), lat(i+1), lon(i+1), &
                                        lat(j), lon(j), lat(j+1), lon(j+1))
                if (intersect) then
                    write(io6, *) "i = ", i, "j = ", j
                    write(io6, *) "lon(i) = ", lon(i), "lat(i) = ", lat(i)
                    write(io6, *) "lon(j) = ", lon(j), "lat(j) = ", lat(j)
                    write(io6, *) "lon(i+1) = ", lon(i+1), "lat(i+1) = ", lat(i+1)
                    write(io6, *) "lon(j+1) = ", lon(j+1), "lat(j+1) = ", lat(j+1)
                    stop "ERROR! Segments i and Segments j intersect."
                end if
            end do
        end do

        ! 释放内存
        deallocate(lon, lat)


    END SUBROUTINE check_self_intersection

    LOGICAL function segments_intersect(lat1, lon1, lat2, lon2, lat3, lon3, lat4, lon4)
        implicit none
        real(r8), intent(in) :: lat1, lon1, lat2, lon2, lat3, lon3, lat4, lon4
        real(r8) :: cp1, cp2, cp3, cp4

        ! 计算叉积
        cp1 = cross_product2(lon2 - lon1, lat2 - lat1, lon3 - lon1, lat3 - lat1)!, cp1)
        cp2 = cross_product2(lon2 - lon1, lat2 - lat1, lon4 - lon1, lat4 - lat1)!, cp2)
        cp3 = cross_product2(lon4 - lon3, lat4 - lat3, lon1 - lon3, lat1 - lat3)!, cp3)
        cp4 = cross_product2(lon4 - lon3, lat4 - lat3, lon2 - lon3, lat2 - lat3)!, cp4)

        ! 判断是否相交
        if ((cp1 * cp2 < 0) .and. (cp3 * cp4 < 0)) then
            segments_intersect = .true.
        else
            segments_intersect = .false.
        end if

    end function segments_intersect

    !SUBROUTINE cross_product2(x1, y1, x2, y2)
    Real(r8) function cross_product2(x1, y1, x2, y2)
        implicit none
        real(r8), intent(in)  :: x1, y1, x2, y2
        cross_product2 = x1 * y2 - x2 * y1
        
    end function cross_product2

    SUBROUTINE minmax_range_make(type_select, edgew_temp, edgee_temp, edgen_temp, edges_temp, temp1, temp2, temp3, temp4)
        ! 判断可以包含全部数据的最小矩形
        implicit none
        character(16), intent(in) :: type_select
        real(8), intent(in) :: edgew_temp, edgee_temp, edgen_temp, edges_temp
        integer, intent(out), optional :: temp1, temp2, temp3, temp4  ! 声明为可选的输出变量
        integer :: temp1_local, temp2_local, temp3_local, temp4_local  ! 本地变量用于计算

        ! 调用 Source_Find 计算边界
        CALL Source_Find(edgew_temp, lon_vertex, 'lon', temp1_local)  ! minlon_source
        CALL Source_Find(edgee_temp, lon_vertex, 'lon', temp2_local)  ! maxlon_source
        CALL Source_Find(edgen_temp, lat_vertex, 'lat', temp3_local)  ! maxlat_source
        CALL Source_Find(edges_temp, lat_vertex, 'lat', temp4_local)  ! minlat_source

        ! 调整边界
        temp2_local = temp2_local - 2
        temp4_local = temp4_local - 2
        if (temp2_local == nlons_source-1) temp2_local = temp2_local + 1
        if (temp4_local == nlats_source-1) temp4_local = temp4_local + 1

        ! 如果需要，更新边界
        if (type_select == 'mask_domain') then
            if (temp1_local < minlon_DmArea) minlon_DmArea = temp1_local
            if (temp2_local > maxlon_DmArea) maxlon_DmArea = temp2_local
            if (temp3_local < maxlat_DmArea) maxlat_DmArea = temp3_local
            if (temp4_local > minlat_DmArea) minlat_DmArea = temp4_local
        else if (type_select == 'mask_refine') then
            if (temp1_local < minlon_RfArea) minlon_RfArea = temp1_local
            if (temp2_local > maxlon_RfArea) maxlon_RfArea = temp2_local
            if (temp3_local < maxlat_RfArea) maxlat_RfArea = temp3_local
            if (temp4_local > minlat_RfArea) minlat_RfArea = temp4_local
        else if (type_select == 'mask_patch') then
            if (temp1_local < minlon_PaArea) minlon_PaArea = temp1_local
            if (temp2_local > maxlon_PaArea) maxlon_PaArea = temp2_local
            if (temp3_local < maxlat_PaArea) maxlat_PaArea = temp3_local
            if (temp4_local > minlat_PaArea) minlat_PaArea = temp4_local
        end if

        ! 如果输出参数存在，则将本地变量的值赋给输出参数
        if (present(temp1)) temp1 = temp1_local
        if (present(temp2)) temp2 = temp2_local
        if (present(temp3)) temp3 = temp3_local
        if (present(temp4)) temp4 = temp4_local

    END SUBROUTINE minmax_range_make

    SUBROUTINE Source_Find(temp, seq_lonlat, str1, source)
        USE consts_coms, only : gridnum_perdegree
        implicit none
        real(r8), intent(in) :: temp
        real(r8), dimension(:), intent(in) :: seq_lonlat
        character(LEN = 3), intent(in) :: str1
        integer :: i, minsource, maxsource
        integer, intent(out) :: source

        if (trim(str1) == 'lon') then ! 针对lon序列! 经度度是从-180到180 !注意出现跨越180经线的>问题
            minsource = ( floor(temp)   - (-180)) * gridnum_perdegree
            maxsource = ( ceiling(temp) - (-180)) * gridnum_perdegree
            minsource = max(1,              minsource-10)
            maxsource = min(1+nlons_source, maxsource+10)
            do i = minsource, maxsource, 1 ! because
                if (temp <= seq_lonlat(i)) then ! 满足条件就退出，等号是针对极点出现的情况
                    source = i ! 找下/左边界,上/右边界都是一样的
                    return ! exit
                end if
            end do
            write(io6, *) "temp <= seq_lonlat(i) is not exist!!!!!!!!!!!!!!!"
            write(io6, *) "temp = ", temp
            write(io6, *) "minsource = ", minsource, "maxsource = ", maxsource
            write(io6, *) "seq_lonlat(minsource) = ", seq_lonlat(minsource)
            write(io6, *) "seq_lonlat(maxsource) = ", seq_lonlat(maxsource)
            source = i
        else
            minsource = ( 90 - ceiling(temp)) * gridnum_perdegree
            maxsource = ( 90 -   floor(temp)) * gridnum_perdegree
            minsource = max(1,              minsource-10)
            maxsource = min(1+nlats_source, maxsource+10)
            do i = minsource, maxsource, 1
                if (temp >= seq_lonlat(i)) then ! 满足条件就退出，等号是针对极点出现的情况
                    source = i ! 找下/左边界,上/右边界都是一样的
                    return ! exit
                end if
            end do
            write(io6, *) "temp >= seq_lonlat(i) is not exist!!!!!!!!!!!!!!!"
            write(io6, *) "temp = ", temp
            write(io6, *) "minsource = ", minsource, "maxsource = ", maxsource
            write(io6, *) "seq_lonlat(minsource) = ", seq_lonlat(minsource)
            write(io6, *) "seq_lonlat(maxsource) = ", seq_lonlat(maxsource)
            source = i
        end if

    END SUBROUTINE Source_Find

    SUBROUTINE CheckCrossing(num_edges, points)

        implicit none

        integer, intent(in) :: num_edges
        real(r8), dimension(num_edges, 2), intent(inout) :: points
        integer :: j
        do j = 1, num_edges, 1
            if(points(j, 1) < 0.)then
                points(j, 1) = points(j, 1) + 180.
            else
                points(j, 1) = points(j, 1) - 180.
            end if
        end do

    END SUBROUTINE CheckCrossing

    LOGICAL FUNCTION is_point_in_circle(point_i, point_c, center_radius)
        ! 根据距离判断点是否在圆内部
        real(r8), intent(in) :: point_i(2), point_c(2), center_radius
        real(r8) :: distance

        is_point_in_circle = .true.
        distance = haversine(point_i, point_c)
        ! write(io6, *) "distance = ", distance
        if (distance > center_radius) is_point_in_circle = .false.

    END FUNCTION is_point_in_circle

    REAL(r8) Function haversine(point_i, point_c)

        implicit none
        ! https://zhuanlan.zhihu.com/p/658990378
        ! 半正矢公式, 以弧度制度量!! 中距离：适用于几百到几千公里，
        real(r8), intent(in) :: point_i(2), point_c(2)
        real(r8) :: px1, py1, px2, py2, v
        px1 = point_i(1) * pio180_r8 ! lon1
        py1 = point_i(2) * pio180_r8 ! lat1
        px2 = point_c(1) * pio180_r8 ! lon2
        py2 = point_c(2) * pio180_r8 ! lat2
        ! old
        ! v = sqrt( sin(py1/2-py2/2)**2 + cos(py2)*cos(py1)*sin(px1/2-px2/2)**2 )
        ! haversine = erad * 2 * asin(v)
        ! new
        v = sin(py1/2-py2/2)**2 + cos(py2)*cos(py1)*sin(px1/2-px2/2)**2
        haversine = erad / 1000 * 2 * atan2(sqrt(v), sqrt(1-v)) ! from m to km
        return

    END Function haversine

    LOGICAL FUNCTION is_point_in_convex_polygon(polygon, ndm_point, num_edges)
        ! 判断点是否在凸多边形内
        real(r8), intent(in) :: polygon(7, 2), ndm_point(2)
        real(r8) :: p1(2), p2(2)
        integer, intent(in) :: num_edges
        integer :: i, next_index
        real(r8) :: prev_cross, cross
        real(r8), dimension(:,:), allocatable :: polygon_select
        allocate(polygon_select(num_edges,2))
        polygon_select = polygon(1:num_edges, :)
        prev_cross = 0.
        is_point_in_convex_polygon = .true.

        do i = 1, num_edges
            next_index = mod(i, num_edges) + 1  ! 处理循环索引
            p1 = polygon_select(i, :)
            p2 = polygon_select(next_index, :)
            cross = cross_product(p1, p2, ndm_point)
            if (cross /= 0.) then
                if (prev_cross == 0.) then
                    prev_cross = cross
                else if ((prev_cross > 0.) /= (cross > 0.)) then
                    is_point_in_convex_polygon = .false.
                    deallocate(polygon_select)
                    return
                end if
            end if
        end do
        deallocate(polygon_select)

    END FUNCTION is_point_in_convex_polygon

    REAL FUNCTION cross_product(p1, p2, p3)
        ! 计算叉积: (p2 - p1) × (p3 - p1)
        real(8), intent(in) :: p1(2), p2(2), p3(2)
        cross_product = (p2(1) - p1(1)) * (p3(2) - p1(2)) - (p2(2) - p1(2)) * (p3(1) - p1(1))

    END FUNCTION cross_product
    
   SUBROUTINE IsInArea_grid_Read(inputfile, IsInDmArea_select, seaorland_select)

      USE NETCDF  
      ! USE MOD_Area_judge, only : minlon_DmArea, maxlon_DmArea, maxlat_DmArea, minlat_DmArea
      IMPLICIT NONE
   
      character(len = 256), intent(in) :: inputfile
      integer :: ncID, dimID_lon, dimID_lat, varid(12), nlons_select, nlats_select
      integer, allocatable, intent(out) :: IsInDmArea_select(:, :), seaorland_select(:, :)
      
      write(io6, *) trim(inputfile)
      CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件
      CALL CHECK(NF90_INQ_DIMID(ncid, "nlons_select", dimID_lon))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQ_DIMID(ncid, "nlats_select", dimID_lat))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, DimID_lon, len = nlons_select))
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, DimID_lat, len = nlats_select))

      allocate(IsInDmArea_select(nlons_select,  nlats_select))
      allocate(seaorland_select(nlons_select, nlats_select))

      CALL CHECK(NF90_INQ_VARID(ncid, 'IsInDmArea_select', varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'seaorland_select',  varid(2)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'minlon_DmArea', varid(3)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'maxlon_DmArea', varid(4)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'maxlat_DmArea', varid(5)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'minlat_DmArea', varid(6)))

      CALL CHECK(NF90_GET_VAR(ncID, varid(1), IsInDmArea_select))
      CALL CHECK(NF90_GET_VAR(ncID, varid(2), seaorland_select))
      CALL CHECK(NF90_GET_VAR(ncID, varid(3), minlon_DmArea))
      CALL CHECK(NF90_GET_VAR(ncID, varid(4), maxlon_DmArea))
      CALL CHECK(NF90_GET_VAR(ncID, varid(5), maxlat_DmArea))
      CALL CHECK(NF90_GET_VAR(ncID, varid(6), minlat_DmArea))
      CALL CHECK(NF90_CLOSE(ncID))
   
   END SUBROUTINE IsInArea_grid_Read

   SUBROUTINE IsInArea_grid_Save(outputfile, IsInArea_grid, minlon_Area, maxlon_Area, maxlat_Area, minlat_Area)

      USE NETCDF 
      USE MOD_data_preprocess, only : lon_i, lat_i 
      IMPLICIT NONE
   
      character(len = 256), intent(in) :: outputfile
      integer, intent(in) :: minlon_Area, maxlon_Area, maxlat_Area, minlat_Area
      integer, allocatable, intent(in) :: IsInArea_grid(:, :)
      integer :: ncID, dimID_lon, dimID_lat, varid(7), nlons_select, nlats_select
      integer, allocatable :: lons_select(:), lats_select(:), IsInArea_select(:, :)! , seaorland_select(:, :)
      real(r8), allocatable :: longitude(:), latitude(:)
      
      allocate(lons_select(maxlon_Area - minlon_Area + 1))
      allocate(lats_select(minlat_Area - maxlat_Area + 1))
      lons_select = [minlon_Area : maxlon_Area]
      lats_select = [maxlat_Area : minlat_Area]        
      nlons_select = size(lons_select)
      nlats_select = size(lats_select)
      
      ! 根据目标经纬度分辨率确定变量内存分配
      allocate(longitude(nlons_select))
      allocate(latitude(nlats_select))
      allocate(IsInArea_select(nlons_select, nlats_select))
      longitude = lon_i(lons_select)
      latitude  = lat_i(lats_select)  
      IsInArea_select = IsInArea_grid(lons_select, lats_select) 

      ! allocate(seaorland_select(nlons_select, nlats_select))
      ! seaorland_select  = seaorland(lons_select, lats_select) 

      write(io6, *) trim(outputfile)
      CALL CHECK(NF90_CREATE(trim(outputfile), ior(nf90_clobber, nf90_netcdf4), ncID))
      CALL CHECK(NF90_DEF_DIM(ncID, "nlons_select", nlons_select, dimID_lon))
      CALL CHECK(NF90_DEF_DIM(ncID, "nlats_select", nlats_select, dimID_lat))
      CALL CHECK(NF90_DEF_VAR(ncID, "minlon_DmArea", NF90_INT, varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncID, "maxlon_DmArea", NF90_INT, varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncID, "maxlat_DmArea", NF90_INT, varid(3)))
      CALL CHECK(NF90_DEF_VAR(ncID, "minlat_DmArea", NF90_INT, varid(4)))
      CALL CHECK(NF90_DEF_VAR(ncID, "longitude", NF90_DOUBLE, (/ dimID_lon /), varid(5)))
      CALL CHECK(NF90_DEF_VAR(ncID, "latitude", NF90_DOUBLE, (/ dimID_lat /), varid(6)))
      CALL CHECK(NF90_DEF_VAR(ncID, "IsInArea_select", NF90_INT, (/ dimID_lon, dimID_lat /), varid(7)))
      ! CALL CHECK(NF90_DEF_VAR(ncID, "seaorland_select",NF90_INT, (/ dimID_lon, dimID_lat /), varid(8)))
      CALL CHECK(NF90_ENDDEF(ncID))
      CALL CHECK(NF90_PUT_VAR(ncID, varid(1), minlon_Area))
      CALL CHECK(NF90_PUT_VAR(ncID, varid(2), maxlon_Area))
      CALL CHECK(NF90_PUT_VAR(ncID, varid(3), maxlat_Area))
      CALL CHECK(NF90_PUT_VAR(ncID, varid(4), minlat_Area))
      CALL CHECK(NF90_PUT_VAR(ncID, varid(5), longitude))
      CALL CHECK(NF90_PUT_VAR(ncID, varid(6), latitude))
      CALL CHECK(NF90_PUT_VAR(ncID, varid(7), IsInArea_select))
      ! CALL CHECK(NF90_PUT_VAR(ncID, varid(8), seaorland_select))
      CALL CHECK(NF90_CLOSE(ncID))
      deallocate(lons_select, lats_select, IsInArea_select, longitude, latitude) ! , seaorland_select
   
   END SUBROUTINE IsInArea_grid_Save

END Module MOD_Area_judge
