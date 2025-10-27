module MOD_mask_postproc
    USE consts_coms
    USE refine_vars
    USE NETCDF
    USE MOD_data_preprocess, only : lon_i, lat_i, lon_vertex, lat_vertex
    USE MOD_file_preprocess, only : Contain_Read, Unstructured_Mesh_Save, Unstructured_Mesh_Read
    USE MOD_grid_preprocess, only : set_ngrmm
    USE MOD_Area_judge     , only : CheckCrossing, minlon_DmArea, maxlon_DmArea, maxlat_DmArea, minlat_DmArea, nlons_Dm_select, nlats_Dm_select
    implicit none
    integer, dimension(:), allocatable, public :: IsInDmArea_ustr
    ! IsInDmArea_ustr also allocate public in the MOD_GetContain.F90
    Contains

    SUBROUTINE mask_postproc(mesh_type_select)
    
        implicit none
        character(*), intent(in) :: mesh_type_select

        if (mesh_type == 'landmesh') then
            CALL mask_postproc_Lnd()
        else if (mesh_type == 'oceanmesh') then
            CALL mask_postproc_Ocn()
        else if (mesh_type == 'atmosmesh') then
            CALL mask_postproc_Atmos()
        else if (mesh_type == 'earthmesh') then
            CALL mask_postproc_Earth()
        end if

    END SUBROUTINE mask_postproc

!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!   use for atmosmesh !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    SUBROUTINE mask_postproc_Atmos()

        implicit none

        if (output_format == 'MPAS') then
            CALL MPAS_Mesh_Cal()
        else if (output_format == 'MPAS-Simple') then
            CALL MPAS_Mesh_Cal_Simple() ! 
        end if 

    END SUBROUTINE mask_postproc_Atmos

    SUBROUTINE MPAS_Mesh_Cal()

        USE MOD_grid_preprocess, only : GetEdge, GetSort_verticesOnEdge, Get_ConnectOnCell, lonlat2xyz, &
            Get_Edge_DIS_Angle, GetArea, set_weightsOnEdge, IsNgrmm, edgeIDSort
        USE MOD_file_preprocess, only : MPAS_Mesh_Save, MPAS_info_Save, data_read
        implicit none
        integer :: num_sjx, num_dbx, num_edge
        integer,  allocatable :: ngrmw(:, :), ngrwm(:, :), n_ngrwm(:)
        real(r8), allocatable :: mp(:, :), wp(:, :), vp(:, :)
        real(r8), allocatable :: vp_reference(:,:)
        character(pathlen) :: lndname
        character(LEN = 5) :: nxpc
        integer :: i, j, k, n, ik
        integer :: maxEdges, maxEdges2, vertexDegree, temp
        real(r8) :: sjx(3,2)
        real(r8) :: centroid_deg(2), nominalMinDc
        real(r8), allocatable :: length_sjx(:, :)
        real(r8), allocatable :: kiteAreasOnVertex(:,:), areaTriangle(:), areaCell(:)
        real(r8), allocatable :: dcEdge(:), dvEdge(:), angleEdge(:)
        real(r8), allocatable :: xem(:), yem(:), zem(:), xew(:), yew(:), zew(:), xev(:), yev(:), zev(:)
        real(r8), allocatable :: lonVertex(:), latVertex(:), lonCell(:), latCell(:), lonEdge(:), latEdge(:)
        real(r8), allocatable :: meshDensity(:), weightsOnEdge(:,:)
        real(r8), allocatable :: error_segment(:)
        integer, allocatable :: ngrmm(:, :)        ! m点相邻的m点索引(细化前)
        integer, allocatable :: cellsOnEdge(:,:), verticesOnEdge(:,:), edgesOnVertex(:,:), cellsOnVertex(:,:)
        integer, allocatable :: nEdgesOnCell(:), cellsOnCell(:,:), verticesOnCell(:,:), edgesOnCell(:,:)
        integer, allocatable :: nEdgesOnEdge(:), edgesOnEdge(:,:)
        integer, allocatable :: cellsOnEdge_reference(:,:)

        write(nxpc,  '(I4.4)') nxp
        ! read unstructure mesh
        write(io6, *), "start to read unstructure mesh data in the module MOD_mask_make"
        ! 读取未细化初始网格数据
        lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)//'.nc4'
        write(io6, *),lndname
        ! 读入的数据除了第一个图形，其他图形都是存在的
        call Unstructured_Mesh_Read(lndname, num_sjx, num_dbx, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *), "The unstructured grid data reading have done in the SUBROUTINE mask_postproc_Atmos"
        write(io6, *), ""
        write(io6, *), "In total, triangular mesh number: ", num_sjx, "polygon mesh number: ", num_dbx
        write(io6, *), ""

        ! 构建ngrmm数组
        allocate(ngrmm(3, num_sjx)); ngrmm = 1   ! 反映三角形的相邻三角形的邻域关系(0表示没有相邻三角形)
        CALL set_ngrmm(num_sjx, ngrmw, ngrwm, n_ngrwm, ngrmm)

        num_edge = int((num_sjx-1)/2*3 + 1) ! 只适用于全局网格
        allocate(cellsOnVertex(3, num_sjx)); cellsOnVertex = ngrmw
        ! 获取在Edge相关的变量cellsOnEdge, verticesOnEdge, edgesOnVertex, vp
        allocate(vp(num_edge, 2)); vp = 0. ! 表示多边形边的中心点位（采用计算中心的方式就好了）
        allocate(cellsOnEdge(2, num_edge)); cellsOnEdge = 1
        allocate(verticesOnEdge(2, num_edge)); verticesOnEdge = 1
        allocate(edgesOnVertex(3, num_sjx)); edgesOnVertex = 1
        CALL GetEdge(num_sjx, wp, ngrmm, cellsOnVertex, cellsOnEdge, verticesOnEdge, edgesOnVertex, vp)
        
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 暂时存在，并不会长期存在的内容
        ! lndname = trim(mode_file)
        ! print*, lndname
        ! CALL data_read(lndname, cellsOnEdge_reference, vp_reference)
        ! CALL edgeIDSort(num_sjx, num_edge, cellsOnEdge_reference, cellsOnEdge, verticesOnEdge, edgesOnVertex, vp) 
        ! vp = vp_reference
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

        ! verticesOnEdge排序
        CALL GetSort_verticesOnEdge(num_edge, mp, wp, cellsOnEdge, verticesOnEdge) ! 对verticesOnEdge

        ! 获取MPAS所需要的关于cell/Edge的连接关系(还需要排序，确保正确的连接关系)
        maxEdges = 10
        maxEdges2 = 20
        allocate(nEdgesOnCell(num_dbx)); nEdgesOnCell = n_ngrwm
        allocate(verticesOnCell(maxEdges, num_dbx)); verticesOnCell = 1
        verticesOnCell(1:max(7, maxval(nEdgesOnCell)), :) = ngrwm
        allocate(edgesOnCell(maxEdges, num_dbx)); edgesOnCell = 1
        allocate(cellsOnCell(maxEdges, num_dbx)); cellsOnCell = 1
        ! 对cellsOnCell和edgesOnCell进行排序(verticesOnCell, edgesOnCell, cellsOnCell内部自洽)
        CALL Get_ConnectOnCell(num_dbx, nEdgesOnCell, cellsOnEdge, edgesOnVertex, verticesOnCell, edgesOnCell, cellsOnCell)

        allocate(lonVertex(num_sjx)); lonVertex = mp(:, 1)
        allocate(latVertex(num_sjx)); latVertex = mp(:, 2)
        allocate(lonCell(num_dbx));   lonCell   = wp(:, 1)
        allocate(latCell(num_dbx));   latCell   = wp(:, 2)
        allocate(lonEdge(num_edge));  lonEdge   = vp(:, 1)
        allocate(latEdge(num_edge));  latEdge   = vp(:, 2)
        ! 获取在笛卡尔坐标下的数值
        allocate(xem(num_sjx)); xem = 0.
        allocate(yem(num_sjx)); yem = 0.
        allocate(zem(num_sjx)); zem = 0.
        allocate(xew(num_dbx)); xew = 0.
        allocate(yew(num_dbx)); yew = 0.
        allocate(zew(num_dbx)); zew = 0.
        allocate(xev(num_edge)); xev = 0.
        allocate(yev(num_edge)); yev = 0.
        allocate(zev(num_edge)); zev = 0.
        CALL lonlat2xyz(num_sjx, lonVertex, latVertex, xem, yem, zem)
        CALL lonlat2xyz(num_dbx, lonCell,   latCell,   xew, yew, zew)
        CALL lonlat2xyz(num_edge,lonEdge,   latEdge,   xev, yev, zev)

        ! 计算获取dcEdge, dvEdge, angleEdge
        allocate(dcEdge(num_edge)); dcEdge = 0.
        allocate(dvEdge(num_edge)); dvEdge = 0.
        allocate(angleEdge(num_edge)); angleEdge = 0.
        CALL Get_Edge_DIS_Angle(num_edge, cellsOnEdge, verticesOnEdge, xem, yem, zem, xew, yew, zew, xev, yev, zev, latVertex, lonEdge, latEdge, dcEdge, dvEdge, angleEdge)

        vertexDegree = 3
        allocate(kiteAreasOnVertex(vertexDegree, num_sjx)); kiteAreasOnVertex = 0.
        allocate(areaTriangle(num_sjx)); areaTriangle = 0.
        allocate(areaCell(num_dbx)); areaCell = 0.
        CALL GetArea(num_sjx, num_dbx, num_edge, xem, yem, zem, xew, yew, zew, xev, yev, zev, cellsOnVertex, edgesOnVertex, cellsOnEdge, &
            verticesOnCell, nEdgesOnCell, kiteAreasOnVertex, areaTriangle, areaCell)

        allocate(meshDensity(num_dbx)); meshDensity = 1.0 ! 需要搭配细化地图使用
        
        allocate(nEdgesOnEdge(num_edge)); nEdgesOnEdge = 0
        allocate(edgesOnEdge(maxEdges2, num_edge)); edgesOnEdge = 1
        allocate(weightsOnEdge(maxEdges2, num_edge)); weightsOnEdge = 0.
        CALL set_weightsOnEdge(num_sjx, num_dbx, num_edge, maxEdges, maxEdges2, vertexDegree, areaCell, angleEdge, dcEdge, dvEdge, kiteAreasOnVertex, edgesOnCell, &
            cellsOnVertex, cellsOnEdge, verticesOnCell, verticesOnEdge, nEdgesOnCell, weightsOnEdge, nEdgesOnEdge, edgesOnEdge, error_segment)

        latCell = latCell * pio180_r8
        where(lonCell < 0._r8) lonCell = lonCell + 360._r8; lonCell = lonCell * pio180_r8
        latVertex = latVertex * pio180_r8
        lonVertex = lonVertex * pio180_r8
        ! where(lonVertex < 0._r8) lonVertex = lonVertex + 360._r8
        latEdge = latEdge * pio180_r8
        where(lonEdge < 0._r8) lonEdge = lonEdge + 360._r8; lonEdge = lonEdge * pio180_r8

        cellsOnCell = cellsOnCell - 1
        verticesOnCell = verticesOnCell - 1
        edgesOnCell = edgesOnCell - 1
        cellsOnVertex = cellsOnVertex - 1
        edgesOnVertex = edgesOnVertex - 1
        cellsOnEdge = cellsOnEdge - 1
        verticesOnEdge = verticesOnEdge - 1
        edgesOnEdge = edgesOnEdge - 1
        nominalMinDc = 7680 / NXP / (2**(step-1)) / erad8 * 1000 ! NXP=64 == 120km：64 * 120 = 7680
        
        lndname = trim(file_dir) // "result/MPASOUT_NXP" // trim(nxpc) // "_global.nc4"
        write(io6, *), lndname
        CALL MPAS_Mesh_Save(lndname, num_sjx, num_dbx, num_edge, latCell, lonCell, xew, yew, zew, latVertex, lonVertex, xem, yem, zem, &
            latEdge, lonEdge, xev, yev, zev, nEdgesOnCell, cellsOnCell, verticesOnCell, edgesOnCell, cellsOnVertex, edgesOnVertex, &
            cellsOnEdge, verticesOnEdge, nEdgesOnEdge, edgesOnEdge, areaCell, areaTriangle, kiteAreasOnVertex, dvEdge, dcEdge, &
            angleEdge, weightsOnEdge, meshDensity, nominalMinDc, error_segment)

        lndname = trim(file_dir) // "result/MPASOUT_NXP" // trim(nxpc) // "_global.graph.info"
        write(io6, *), lndname
        CALL MPAS_info_Save(lndname, maxEdges, num_dbx, num_edge, cellsOnCell, cellsOnEdge, nEdgesOnCell)

        deallocate(areaTriangle, areaCell, kiteAreasOnVertex)
        deallocate(dcEdge, dvEdge, angleEdge)
        deallocate(cellsOnVertex, ngrmm)
        deallocate(cellsOnEdge, verticesOnEdge, edgesOnVertex, vp)
        deallocate(nEdgesOnCell, edgesOnCell, cellsOnCell, verticesOnCell)
        deallocate(xem, yem, zem, xew, yew, zew, xev, yev, zev)
        deallocate(lonVertex, latVertex, lonCell, latCell, lonEdge, latEdge)
        deallocate(nEdgesOnEdge, edgesOnEdge, meshDensity, weightsOnEdge)

    END SUBROUTINE MPAS_Mesh_Cal

    ! 只输出 xyzCell, xyzVertex, cellsOnVertex, meshDensity(all one) and cellWidthVsXY
    SUBROUTINE MPAS_Mesh_Cal_Simple()
        USE MOD_data_preprocess, only : cellWidthVsXY
        USE MOD_grid_preprocess, only : lonlat2xyz
        USE MOD_file_preprocess, only : MPAS_Mesh_Simple_Save
        implicit none
        integer :: num_sjx, num_dbx
        integer,  allocatable :: ngrmw(:, :), ngrwm(:, :), n_ngrwm(:)
        real(r8), allocatable :: mp(:, :), wp(:, :)
        character(pathlen) :: lndname
        character(LEN = 5) :: nxpc
        real(r8), allocatable :: xem(:), yem(:), zem(:), xew(:), yew(:), zew(:)
        real(r8), allocatable :: lonVertex(:), latVertex(:), lonCell(:), latCell(:)
        real(r8), allocatable :: meshDensity(:)
        integer, allocatable :: cellsOnVertex(:,:)

        write(nxpc,  '(I4.4)') nxp
        ! read unstructure mesh
        write(io6, *), "start to read unstructure mesh data in the module MOD_mask_make"
        ! 读取未细化初始网格数据
        lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)//'.nc4'
        write(io6, *),lndname
        ! 读入的数据除了第一个图形，其他图形都是存在的
        call Unstructured_Mesh_Read(lndname, num_sjx, num_dbx, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *), "The unstructured grid data reading have done in the SUBROUTINE mask_postproc_Atmos"
        write(io6, *), ""
        write(io6, *), "In total, triangular mesh number: ", num_sjx, "polygon mesh number: ", num_dbx
        write(io6, *), ""

        allocate(cellsOnVertex(3, num_sjx)); cellsOnVertex = ngrmw
        allocate(meshDensity(num_dbx)); meshDensity = 1.0 ! 需要搭配细化地图使用

        allocate(lonVertex(num_sjx)); lonVertex = mp(:, 1)
        allocate(latVertex(num_sjx)); latVertex = mp(:, 2)
        allocate(lonCell(num_dbx));   lonCell   = wp(:, 1)
        allocate(latCell(num_dbx));   latCell   = wp(:, 2)
        ! 获取在笛卡尔坐标下的数值
        allocate(xem(num_sjx)); xem = 0.
        allocate(yem(num_sjx)); yem = 0.
        allocate(zem(num_sjx)); zem = 0.
        allocate(xew(num_dbx)); xew = 0.
        allocate(yew(num_dbx)); yew = 0.
        allocate(zew(num_dbx)); zew = 0.
        CALL lonlat2xyz(num_sjx, lonVertex, latVertex, xem, yem, zem)
        CALL lonlat2xyz(num_dbx, lonCell,   latCell,   xew, yew, zew)

        cellsOnVertex = cellsOnVertex - 1

        lndname = trim(file_dir) // "result/MPASOUT_NXP" // trim(nxpc) // "_global_Simple.nc4"
        write(io6, *), lndname
        CALL MPAS_Mesh_Simple_Save(lndname, num_sjx, num_dbx, xew, yew, zew, xem, yem, zem, cellsOnVertex, meshDensity)
    
    END SUBROUTINE MPAS_Mesh_Cal_Simple

!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!   use for earthmesh !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    SUBROUTINE mask_postproc_Earth()
        ! 进行patchtypes的计算与保存
        USE MOD_file_preprocess, only : LOCmesh_info_save
        implicit none
        integer :: i, j, k, ii, jj, kk, id, numpatch, n, sum_land_ustr, sum_sea_ustr
        integer,  dimension(:, :), allocatable :: ustr_id, ustr_ii
        integer,  dimension(:, :), allocatable :: patchtypes_select
        integer,  dimension(:),    allocatable :: IsInDmArea_ustr_read
        integer,  dimension(:),    allocatable :: seaorland_ustr, seaorland_ustr_f
        integer,  dimension(:),    allocatable :: num_step_f, refine_degree_f
        integer :: sjx_points, lbx_points, nvertices
        integer :: ustr_points, ustr_bounds
        integer :: ustr_points_new, ustr_bounds_new
        integer :: ustr_points_f, ustr_bounds_f 
        real(r8), allocatable :: mp(:, :), wp(:, :)            ! 三角形、六边形网格中心点起始数据
        integer,  allocatable :: ngrmw(:, :), ngrwm(:, :)
        integer,  allocatable :: n_ngrwm(:)
        real(r8), allocatable :: ustr_center(:, :), ustr_vertex(:, :)
        real(r8), allocatable :: ustr_center_new(:, :), ustr_vertex_new(:, :)
        real(r8), allocatable :: ustr_center_f(:, :), ustr_vertex_f(:, :)  
        integer,  allocatable :: ustr_ngr_center(:, :),     ustr_ngr_vertex(:, :)
        integer,  allocatable :: ustr_ngr_center_new(:, :), ustr_ngr_vertex_new(:, :)
        integer,  allocatable :: ustr_ngr_center_f(:, :),   ustr_ngr_vertex_f(:, :) 
        integer,  allocatable :: n_ustr_ngr_center(:),   n_ustr_ngr_vertex(:)
        integer,  allocatable :: n_ustr_ngr_center_new(:), n_ustr_ngr_vertex_new(:)
        integer,  allocatable :: n_ustr_ngr_center_f(:), n_ustr_ngr_vertex_f(:)
        integer,  allocatable :: unique_vertices(:), sorted_vertices(:), vertex_mapping(:)
        character(pathlen) :: lndname, outputfile
        character(LEN = 5) :: nxpc

        write(nxpc, '(I4.4)') NXP
        lndname = trim(file_dir) // 'contain/contain_'// trim(mesh_type) //'_domain_NXP'//trim(nxpc)//'_'//trim(mode_grid)//'.nc4'
        write(io6, *), lndname, 'reading in the SUBROUTINE mask_postproc_Earth'
        CALL Contain_Read(lndname, ustr_points, numpatch, ustr_id, ustr_ii, IsInDmArea_ustr_read)
        allocate(IsInDmArea_ustr(ustr_points)); IsInDmArea_ustr = IsInDmArea_ustr_read
        allocate(seaorland_ustr(ustr_points)); seaorland_ustr = 0 ! 0表示不存在，-1表示海洋，1表示陆地

        ! patchtypes make
        ! 可能会出现一个非结构网格含有land pixes 但是占比太少了，而被判定为海洋网格，这部分的pixes是否应该被舍弃
        write(io6, *), "patchtypes_make start"
        allocate(patchtypes_select(nlons_Dm_select, nlats_Dm_select)); patchtypes_select = 0
        sum_land_ustr = 0
        sum_sea_ustr  = 0
        do k = 2, ustr_points, 1
            if (IsInDmArea_ustr(k) /= 1) cycle ! 跳过不存在的网格
            kk = 0
            do i = 1, ustr_id(k, 1), 1 
                id = ustr_id(k, 2) + i - 1
                kk = kk + ustr_ii(id, 3) ! 记录陆地pixes个数
            end do
            if (kk / real(ustr_id(k, 1)) > mask_sea_ratio) then
                seaorland_ustr(k) = 1
                sum_land_ustr = sum_land_ustr + 1
                do i = 1, ustr_id(k, 1), 1 
                    id = ustr_id(k, 2) + i - 1
                    if (ustr_ii(id, 3) == 0) cycle
                    ii = ustr_ii(id, 1) - minlon_DmArea + 1
                    jj = ustr_ii(id, 2) - maxlat_DmArea + 1
                    patchtypes_select(ii, jj) = k
                end do
            else
                seaorland_ustr(k) = -1
                sum_sea_ustr = sum_sea_ustr + 1
            end if
        end do

        write(io6, *), "sum_land_ustr = ", sum_land_ustr
        write(io6, *), "sum_sea_ustr = ", sum_sea_ustr
        write(io6, *), "patchtypes_make finish"
        write(io6, *), ""

        write(io6, *), "PatchID_save start"
        ! 开始计算mpi模式所需的patchID文件
        outputfile = trim(file_dir) // 'patchtype/patchtype_NXP'//trim(nxpc)//'_'//trim(mode_grid)//'.nc4' ! 这是最终的输出结果
        CALL PatchID_Save(outputfile, patchtypes_select)
        write(io6, *), "PatchID_save finish"
        write(io6, *), ""

        ! 对网格进行裁剪
        ! deallocate
        ! deallocate(ustr_id, ustr_ii, patchtypes, IsInDmArea_ustr_read)

        
        ! read unstructure mesh
        write(io6, *), "start to read unstructure mesh data in the module MOD_mask_make"
        ! 读取未细化初始网格数据
        lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)//'.nc4'
        write(io6, *),lndname
        ! 读入的数据除了第一个图形，其他图形都是存在的
        call Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *), "The unstructured grid data reading have done in the SUBROUTINE mask_postproc_Earth"
        write(io6, *), ""
        write(io6, *), "In total, triangular mesh number: ", sjx_points, "polygon mesh number: ", lbx_points
        write(io6, *), ""
        
        if (mode_grid == 'tri') then
            write(io6, *), "开始制作三角形大气网格"
            ustr_points = sjx_points
            ustr_bounds = lbx_points
            allocate(ustr_center(ustr_points, 2));       ustr_center = mp
            allocate(ustr_vertex(ustr_bounds, 2));       ustr_vertex = wp
            allocate(ustr_ngr_center(3, ustr_points));   ustr_ngr_center = ngrmw
            allocate(ustr_ngr_vertex(7, ustr_bounds));   ustr_ngr_vertex = ngrwm
            allocate(n_ustr_ngr_center(ustr_points));    n_ustr_ngr_center = 3
            allocate(n_ustr_ngr_vertex(ustr_bounds));    n_ustr_ngr_vertex = n_ngrwm
        else if (mode_grid == 'hex') then
            !开始制作六边形大气网格
            ustr_points = lbx_points
            ustr_bounds = sjx_points
            allocate(ustr_center(ustr_points, 2));       ustr_center = wp
            allocate(ustr_vertex(ustr_bounds, 2));       ustr_vertex = mp
            allocate(ustr_ngr_center(7, ustr_points));   ustr_ngr_center = ngrwm
            allocate(ustr_ngr_vertex(3, ustr_bounds));   ustr_ngr_vertex = ngrmw
            allocate(n_ustr_ngr_center(ustr_points));    n_ustr_ngr_center = n_ngrwm
            allocate(n_ustr_ngr_vertex(ustr_bounds));    n_ustr_ngr_vertex = 3
            ! allocate(num_step_f(step+1));                num_step_f = 1
            ! num_step_f(1:step) = num_wp_step(0:step-1)
        else 
            stop "ERROR! tri and hex only in the MOD_mask_make()"
        end if
        allocate(num_step_f(step+1));                num_step_f = 1
        num_step_f(1:step) = num_mp_step(0:step-1)
        num_step_f(1+step) = sjx_points

        ! 完成数据初步更新，去除非海陆网格）（如果生成的是全球网格是不是就可以跳过这一步呢？）
        CALL Data_Renew(ustr_points, ustr_bounds, ustr_ngr_center, n_ustr_ngr_center, ustr_points_new, ustr_bounds_new, &
                            ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
        ! 根据center顶点，更新vertex中心点与center中心点位连接情况，以及vertex中心点
        ! 由新center中心点指向顶点，再统计顶点个数
        if (mode_grid == 'tri') then
            write(io6, *), "去除不存在三角形网格前:"
            write(io6, *), ustr_points, "三角形网格", ustr_bounds, "六边形网格"
            write(io6, *), "去除不存在三角形网格后:"
            write(io6, *), ustr_points_new, "三角形网格", ustr_bounds_new, "六边形网格(含虚假六边形)"
            write(io6, *), ""
        else if (mode_grid == 'hex') then
            write(io6, *), "去除不存在六边形网格前:"
            write(io6, *), ustr_bounds, "三角形网格", ustr_points, "六边形网格"
            write(io6, *), "去除不存在六边形网格后:"
            write(io6, *), ustr_bounds_new, "三角形网格(含虚假三角形)", ustr_points_new, "六边形网格"
            write(io6, *), ""
        end if

        ! 获取最终的六边形信息(from *_new to *_f)
        ! 获取center and vertex length
        ustr_points_f = ustr_points_new
        ustr_bounds_f = ustr_bounds_new
        CALL Data_Finial(ustr_points, ustr_bounds, ustr_points_f, ustr_bounds_f, ustr_center, ustr_vertex, ustr_ngr_center, n_ustr_ngr_center, &
                            ustr_center_f, ustr_vertex_f, ustr_ngr_center_f, ustr_ngr_vertex_f, n_ustr_ngr_center_f, n_ustr_ngr_vertex_f)


        write(io6, *), "n_ustr_ngr_vertex_f range "
        write(io6, *), minval(n_ustr_ngr_vertex_f(2:ustr_bounds_f)), maxval(n_ustr_ngr_vertex_f(2:ustr_bounds_f)) ! 单独存放   

        ! 删除部分信息后，会造成数据编号的不连续呢，
        ! ustr_center_f和ustr_vertex_f的顺序都是对的，ustr_ngr_vertex_f的顺序也是对的，但ustr_ngr_center_f却不是
        write(io6, *), "sort order of ustr_ngr_center_f (step1)"
        !  步骤1：提取唯一顶点编号
        allocate(unique_vertices(ustr_bounds)) ! 存储一维顶点,最多有ustr_bounds个顶点
        CALL extract_unique_vertices(ustr_ngr_center_f, n_ustr_ngr_center_f, unique_vertices, nvertices)

        write(io6, *), "sort order of ustr_ngr_center_f (step2)"
        ! 步骤2：排序并重新编号顶点
        allocate(sorted_vertices(nvertices)) ! 排序后的顶点
        allocate(vertex_mapping(ustr_bounds)) ! 顶点的映射数组
        call sort_and_reindex(unique_vertices, nvertices, sorted_vertices, vertex_mapping)

        write(io6, *), "sort order of ustr_ngr_center_f (step3)"
        ! 步骤3：根据新的顶点编号重新编号中心点
        do j = 2, ustr_points_f, 1
            do i = 1, n_ustr_ngr_center_f(j), 1
                ustr_ngr_center_f(i, j) = vertex_mapping(ustr_ngr_center_f(i, j))
            end do
        end do
        write(io6, *), "minval(ustr_ngr_center_f) = ", minval(ustr_ngr_center_f(1:3, 2:ustr_points_f))
        write(io6, *), "maxval(ustr_ngr_center_f) = ", maxval(ustr_ngr_center_f(1:3, 2:ustr_points_f))
        write(io6, *), "sort order of ustr_ngr_center_f finish"
        
        lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)// '_' // trim(mesh_type) //'.nc4'
        if (mask_patch_on) lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)// '_' // trim(mesh_type) //'_patch.nc4'
        write(io6, *), lndname
        if (mode_grid == 'tri') then
            CALL Unstructured_Mesh_Save(lndname, ustr_points_f, ustr_bounds_f, ustr_center_f, ustr_vertex_f, ustr_ngr_center_f, ustr_ngr_vertex_f, n_ustr_ngr_vertex_f)
        else if (mode_grid == 'hex') then
            CALL Unstructured_Mesh_Save(lndname, ustr_bounds_f, ustr_points_f, ustr_vertex_f, ustr_center_f, ustr_ngr_vertex_f, ustr_ngr_center_f, n_ustr_ngr_center_f)
        end if
        write(io6, *), "nc save finish"

        ! 保存海陆网格信息
        allocate(seaorland_ustr_f(ustr_points_f)); seaorland_ustr_f = 0
        allocate(refine_degree_f(ustr_points_f)); refine_degree_f = 0
        k = 1
        kk = 2
        if (mode_grid == 'tri') then
            do i = 2, ustr_points, 1
                if (num_step_f(kk) <= i) then
                    num_step_f(kk) = k
                    kk = kk + 1
                end if
                if (IsInDmArea_ustr(i) /= 1) cycle
                k = k + 1
                seaorland_ustr_f(k) = seaorland_ustr(i)
                refine_degree_f(k) = kk - 2
            end do
        else if (mode_grid == 'hex') then
            do i = 2, ustr_points, 1
                kk = 2
                do while (num_step_f(kk) < maxval(ustr_ngr_center(:, i)))
                    kk = kk + 1
                end do
                if (IsInDmArea_ustr(i) /= 1) cycle
                k = k + 1
                seaorland_ustr_f(k) = seaorland_ustr(i)
                refine_degree_f(k) = kk - 2
            end do
        end if
        lndname = trim(file_dir) // 'result/earthmesh_info.nc4'
        write(io6, *), lndname
        CALL LOCmesh_info_save(lndname, kk, ustr_points_f, num_step_f, refine_degree_f, seaorland_ustr_f)
        write(io6, *), "earthmesh_info save finish"

    END SUBROUTINE mask_postproc_Earth

!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!   use for landmesh !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    SUBROUTINE mask_postproc_Lnd()
        ! 进行patchtypes的计算与保存
        USE MOD_Area_judge, only : seaorland
        implicit none
        integer :: i, j, k, ii, jj, id, numpatch, n, num_sum
        integer,  dimension(:, :), allocatable :: ustr_id, ustr_ii
        integer,  dimension(:, :), allocatable :: patchtypes_select
        integer,  dimension(:),    allocatable :: IsInDmArea_ustr_read
        integer :: sjx_points, lbx_points, nvertices
        integer :: ustr_points, ustr_bounds
        integer :: ustr_points_new, ustr_bounds_new
        integer :: ustr_points_f, ustr_bounds_f 
        real(r8), allocatable :: mp(:, :), wp(:, :)            ! 三角形、六边形网格中心点起始数据
        integer,  allocatable :: ngrmw(:, :), ngrwm(:, :)
        integer,  allocatable :: n_ngrwm(:), n_ngrwm_f(:)
        real(r8), allocatable :: ustr_center(:, :), ustr_vertex(:, :)
        real(r8), allocatable :: ustr_center_new(:, :), ustr_vertex_new(:, :)
        real(r8), allocatable :: ustr_center_f(:, :), ustr_vertex_f(:, :)  
        integer,  allocatable :: ustr_ngr_center(:, :),     ustr_ngr_vertex(:, :)
        integer,  allocatable :: ustr_ngr_center_new(:, :), ustr_ngr_vertex_new(:, :)
        integer,  allocatable :: ustr_ngr_center_f(:, :),   ustr_ngr_vertex_f(:, :) 
        integer,  allocatable :: n_ustr_ngr_center(:),   n_ustr_ngr_vertex(:)
        integer,  allocatable :: n_ustr_ngr_center_new(:), n_ustr_ngr_vertex_new(:)
        integer,  allocatable :: n_ustr_ngr_center_f(:), n_ustr_ngr_vertex_f(:)
        integer,  allocatable :: unique_vertices(:), sorted_vertices(:), vertex_mapping(:)
        character(pathlen) :: lndname, outputfile
        character(LEN = 5) :: nxpc

        write(nxpc, '(I4.4)') NXP
        lndname = trim(file_dir) // 'contain/contain_'// trim(mesh_type) //'_domain_NXP'//trim(nxpc)//'_'//trim(mode_grid)//'.nc4'
        write(io6, *), lndname, 'reading in the SUBROUTINE mask_postproc_Lnd'
        CALL Contain_Read(lndname, ustr_points, numpatch, ustr_id, ustr_ii, IsInDmArea_ustr_read)
        IsInDmArea_ustr = IsInDmArea_ustr_read

        ! patchtypes make
        write(io6, *), "patchtypes_make start"
        allocate(patchtypes_select(nlons_Dm_select, nlats_Dm_select)); patchtypes_select = 0
        do k = 2, ustr_points, 1
            if (IsInDmArea_ustr(k) == 0) cycle ! 跳过不存在的网格
            do i = 1, ustr_id(k, 1), 1 
                id = ustr_id(k, 2) + i - 1
                ii = ustr_ii(id, 1)
                jj = ustr_ii(id, 2)
                seaorland(ii, jj) = 0 ! 用于检测是否domain区域的陆地是否都被编号

                ii = ustr_ii(id, 1) - minlon_DmArea + 1
                jj = ustr_ii(id, 2) - maxlat_DmArea + 1
                patchtypes_select(ii, jj) = k
            end do
        end do

        ! 确保所有的land pixes与非结构网格都有映射关系，但是一定有必要麻？？多是岛屿
        num_sum = sum(seaorland)
        if (num_sum /= 0) then
            write(io6, *), "num_sum = ", num_sum
            write(io6, *), "warning! num_sum /= 0! have some land pixes ignored!" 
            do j = maxlat_DmArea, minlat_DmArea, 1 ! 影响先内层循环
                do i = minlon_DmArea, maxlon_DmArea, 1
                    if (seaorland(i, j) == 0) cycle ! 跳过非陆地pixes
                    write(io6, *), "i, j = ", i, j
                    num_sum = num_sum - 1
                    patchtypes_select(i, j) = patchtypes_select(i, j-1)
                    seaorland(i, j) = 0
                end do
                if (num_sum == 0) exit
            end do
        end if
        write(io6, *), "patchtypes_make finish"
        write(io6, *), ""

        write(io6, *), "PatchID_save start"
        ! 开始计算mpi模式所需的patchID文件
        outputfile = trim(file_dir) // 'patchtype/patchtype_NXP'//trim(nxpc)//'_'//trim(mode_grid)//'.nc4' ! 这是最终的输出结果
        CALL PatchID_Save(outputfile, patchtypes_select)
        write(io6, *), "PatchID_save finish"
        write(io6, *), ""

        deallocate(ustr_id, ustr_ii, patchtypes_select, IsInDmArea_ustr_read)
        if (mask_domain_global) return

        ! 对网格进行裁剪        
        ! read unstructure mesh
        write(io6, *), "start to read unstructure mesh data in the module MOD_mask_make"
        ! 读取未细化初始网格数据
        lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)//'.nc4'
        write(io6, *),lndname
        ! 读入的数据除了第一个图形，其他图形都是存在的
        call Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *), "The unstructured grid data reading have done in the SUBROUTINE mask_postproc_Lnd"
        write(io6, *), ""
        write(io6, *), "In total, triangular mesh number: ", sjx_points, "polygon mesh number: ", lbx_points
        write(io6, *), ""

        if (mode_grid == 'tri') then
            write(io6, *), "开始制作三角形陆地网格"
            ustr_points = sjx_points
            ustr_bounds = lbx_points
            allocate(ustr_center(ustr_points, 2));       ustr_center = mp
            allocate(ustr_vertex(ustr_bounds, 2));       ustr_vertex = wp
            allocate(ustr_ngr_center(3, ustr_points));   ustr_ngr_center = ngrmw
            allocate(ustr_ngr_vertex(7, ustr_bounds));   ustr_ngr_vertex = ngrwm
            allocate(n_ustr_ngr_center(ustr_points));    n_ustr_ngr_center = 3
            allocate(n_ustr_ngr_vertex(ustr_bounds));    n_ustr_ngr_vertex = n_ngrwm
        else if (mode_grid == 'hex') then
            !开始制作六边形陆地网格
            ustr_points = lbx_points
            ustr_bounds = sjx_points
            allocate(ustr_center(ustr_points, 2));       ustr_center = wp
            allocate(ustr_vertex(ustr_bounds, 2));       ustr_vertex = mp
            allocate(ustr_ngr_center(7, ustr_points));   ustr_ngr_center = ngrwm
            allocate(ustr_ngr_vertex(3, ustr_bounds));   ustr_ngr_vertex = ngrmw
            allocate(n_ustr_ngr_center(ustr_points));    n_ustr_ngr_center = n_ngrwm
            allocate(n_ustr_ngr_vertex(ustr_bounds));    n_ustr_ngr_vertex = 3
        else 
            stop "ERROR! tri and hex only in the MOD_mask_make()"
        end if

        ! 完成数据初步更新，去除非海陆网格
        CALL Data_Renew(ustr_points, ustr_bounds, ustr_ngr_center, n_ustr_ngr_center, ustr_points_new, ustr_bounds_new, &
                            ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
        ! 根据center顶点，更新vertex中心点与center中心点位连接情况，以及vertex中心点
        ! 由新center中心点指向顶点，再统计顶点个数
        if (mode_grid == 'tri') then
            write(io6, *), "去除不存在三角形网格前:"
            write(io6, *), ustr_points, "三角形网格", ustr_bounds, "六边形网格"
            write(io6, *), "去除不存在三角形网格后:"
            write(io6, *), ustr_points_new, "三角形网格", ustr_bounds_new, "六边形网格(含虚假六边形)"
            write(io6, *), ""
        else if (mode_grid == 'hex') then
            write(io6, *), "去除不存在六边形网格前:"
            write(io6, *), ustr_bounds, "三角形网格", ustr_points, "六边形网格"
            write(io6, *), "去除不存在六边形网格后:"
            write(io6, *), ustr_bounds_new, "三角形网格(含虚假三角形)", ustr_points_new, "六边形网格"
            write(io6, *), ""
        end if

        ! 获取最终的六边形信息(from *_new to *_f)
        ! 获取center and vertex length
        ustr_points_f = ustr_points_new
        ustr_bounds_f = ustr_bounds_new
        CALL Data_Finial(ustr_points, ustr_bounds, ustr_points_f, ustr_bounds_f, ustr_center, ustr_vertex, ustr_ngr_center, n_ustr_ngr_center, &
                            ustr_center_f, ustr_vertex_f, ustr_ngr_center_f, ustr_ngr_vertex_f, n_ustr_ngr_center_f, n_ustr_ngr_vertex_f)


        write(io6, *), "n_ustr_ngr_vertex_f range "
        write(io6, *), minval(n_ustr_ngr_vertex_f(2:ustr_bounds_f)), maxval(n_ustr_ngr_vertex_f(2:ustr_bounds_f)) ! 单独存放   

        ! 删除部分信息后，会造成数据编号的不连续呢，
        ! ustr_center_f和ustr_vertex_f的顺序都是对的，ustr_ngr_vertex_f的顺序也是对的，但ustr_ngr_center_f却不是
        write(io6, *), "sort order of ustr_ngr_center_f (step1)"
        !  步骤1：提取唯一顶点编号
        allocate(unique_vertices(ustr_bounds)) ! 存储一维顶点,最多有ustr_bounds个顶点
        CALL extract_unique_vertices(ustr_ngr_center_f, n_ustr_ngr_center_f, unique_vertices, nvertices)

        write(io6, *), "sort order of ustr_ngr_center_f (step2)"
        ! 步骤2：排序并重新编号顶点
        allocate(sorted_vertices(nvertices)) ! 排序后的顶点
        allocate(vertex_mapping(ustr_bounds)) ! 顶点的映射数组
        call sort_and_reindex(unique_vertices, nvertices, sorted_vertices, vertex_mapping)

        write(io6, *), "sort order of ustr_ngr_center_f (step3)"
        ! 步骤3：根据新的顶点编号重新编号中心点
        do j = 2, ustr_points_f, 1
            do i = 1, n_ustr_ngr_center_f(j), 1
                ustr_ngr_center_f(i, j) = vertex_mapping(ustr_ngr_center_f(i, j))
            end do
        end do
        write(io6, *), "minval(ustr_ngr_center_f) = ", minval(ustr_ngr_center_f(1:3, 2:ustr_points_f))
        write(io6, *), "maxval(ustr_ngr_center_f) = ", maxval(ustr_ngr_center_f(1:3, 2:ustr_points_f))
        write(io6, *), "sort order of ustr_ngr_center_f finish"
        
        lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)// '_' // trim(mesh_type) //'.nc4'
        if (mask_patch_on) lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)// '_' // trim(mesh_type) //'_patch.nc4'
        write(io6, *), lndname
        if (mode_grid == 'tri') then
            CALL Unstructured_Mesh_Save(lndname, ustr_points_f, ustr_bounds_f, ustr_center_f, ustr_vertex_f, ustr_ngr_center_f, ustr_ngr_vertex_f)
        else if (mode_grid == 'hex') then
            allocate(n_ngrwm_f(ustr_points_f)); n_ngrwm_f = 1
            do i = 2, ustr_points_f, 1
                if (ustr_ngr_center_f(6, i) == 1) then
                    n_ngrwm_f(i) = 5
                else if (ustr_ngr_center_f(7, i) == 1) then
                    n_ngrwm_f(i) = 6
                else
                    n_ngrwm_f(i) = 7
                end if
            end do
            CALL Unstructured_Mesh_Save(lndname, ustr_bounds_f, ustr_points_f, ustr_vertex_f, ustr_center_f, ustr_ngr_vertex_f, ustr_ngr_center_f, n_ngrwm_f)
        end if
        deallocate(n_ngrwm_f)
        write(io6, *), "nc save finish"

    END SUBROUTINE mask_postproc_Lnd

    SUBROUTINE PatchID_Save(outputfile, patchtypes_select)

        USE NETCDF    
        USE MOD_Area_judge, only : nlons_Dm_select, nlats_Dm_select, minlon_DmArea, maxlon_DmArea, maxlat_DmArea, minlat_DmArea
        IMPLICIT NONE
    
        character(pathlen), intent(in) :: outputfile
        integer, dimension(:,:), intent(in) :: patchtypes_select !  记录各经纬度网格所在非结构网格序号
        integer :: ncID, dimID_lon, dimID_lat, varid(7)
        integer, allocatable :: Dmlons_source(:), Dmlats_source(:)
        real(r8), allocatable :: lon_w(:), lon_e(:), lat_n(:), lat_s(:), lon_select(:), lat_select(:)
        
        allocate(Dmlons_source(nlons_Dm_select))
        allocate(Dmlats_source(nlats_Dm_select))
        Dmlons_source = [minlon_DmArea : maxlon_DmArea]
        Dmlats_source = [maxlat_DmArea : minlat_DmArea]        

        ! 根据目标经纬度分辨率确定变量内存分配
        allocate(lon_e(nlons_Dm_select))
        allocate(lon_w(nlons_Dm_select))
        allocate(lat_n(nlats_Dm_select))
        allocate(lat_s(nlats_Dm_select))
        allocate(lon_select(nlons_Dm_select))
        allocate(lat_select(nlats_Dm_select))
        lon_w      = lon_vertex(Dmlons_source)
        lon_e      = lon_vertex(Dmlons_source + 1)
        lat_n      = lat_vertex(Dmlats_source)
        lat_s      = lat_vertex(Dmlats_source + 1)
        lon_select = lon_i(Dmlons_source)
        lat_select = lat_i(Dmlats_source)

        write(io6, *), trim(outputfile)
        CALL CHECK(NF90_CREATE(trim(outputfile), ior(nf90_clobber, nf90_netcdf4), ncID))
        CALL CHECK(NF90_DEF_DIM(ncID, "nlon", nlons_Dm_select, dimID_lon))
        CALL CHECK(NF90_DEF_DIM(ncID, "nlat", nlats_Dm_select, dimID_lat))
        CALL CHECK(NF90_DEF_VAR(ncID, "elmindex", NF90_INT, (/ dimID_lon, dimID_lat /), varid(1)))
        CALL CHECK(NF90_DEF_VAR(ncID, "lon_w", NF90_DOUBLE, (/ dimID_lon /), varid(2)))
        CALL CHECK(NF90_DEF_VAR(ncID, "lon_e", NF90_DOUBLE, (/ dimID_lon /), varid(3)))
        CALL CHECK(NF90_DEF_VAR(ncID, "lat_n", NF90_DOUBLE, (/ dimID_lat /), varid(4)))
        CALL CHECK(NF90_DEF_VAR(ncID, "lat_s", NF90_DOUBLE, (/ dimID_lat /), varid(5)))
        CALL CHECK(NF90_DEF_VAR(ncID, "longitude", NF90_DOUBLE, (/ dimID_lon /), varid(6)))
        CALL CHECK(NF90_DEF_VAR(ncID, "latitude",  NF90_DOUBLE, (/ dimID_lat /), varid(7)))
        CALL CHECK(NF90_ENDDEF(ncID))
        CALL CHECK(NF90_PUT_VAR(ncID, varid(1), patchtypes_select))
        CALL CHECK(NF90_PUT_VAR(ncID, varid(2), lon_w))
        CALL CHECK(NF90_PUT_VAR(ncID, varid(3), lon_e))
        CALL CHECK(NF90_PUT_VAR(ncID, varid(4), lat_n))
        CALL CHECK(NF90_PUT_VAR(ncID, varid(5), lat_s))
        CALL CHECK(NF90_PUT_VAR(ncID, varid(6), lon_select))
        CALL CHECK(NF90_PUT_VAR(ncID, varid(7), lat_select))
        CALL CHECK(NF90_CLOSE(ncID))
        deallocate(lon_w, lon_e, lat_n, lat_s, lon_select, lat_select)
    
    END SUBROUTINE PatchID_Save

!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!   use for oceanmesh !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    SUBROUTINE mask_postproc_Ocn()
        USE MOD_file_preprocess, only : FVCOM_Mesh_Save
        implicit none
        logical :: fexist
        integer :: i, j, k, n, im, ik, hhh
        integer :: num_bdy_long(3), numpatch
        integer :: sjx_points, lbx_points, nvertices
        integer :: ustr_points, ustr_bounds
        integer :: ustr_points_new, ustr_bounds_new
        integer :: ustr_points_f, ustr_bounds_f 
        real(r8), allocatable :: mp(:, :), wp(:, :)            ! 三角形、六边形网格中心点起始数据
        integer,  allocatable :: ngrmw(:, :), ngrwm(:, :)
        integer,  allocatable :: n_ngrwm(:)
        real(r8), allocatable :: ustr_center(:, :), ustr_vertex(:, :)
        real(r8), allocatable :: ustr_center_new(:, :), ustr_vertex_new(:, :)
        real(r8), allocatable :: ustr_center_f(:, :), ustr_vertex_f(:, :)  
        integer,  allocatable :: ustr_ngr_center(:, :),     ustr_ngr_vertex(:, :)
        integer,  allocatable :: ustr_ngr_center_new(:, :), ustr_ngr_vertex_new(:, :)
        integer,  allocatable :: ustr_ngr_center_f(:, :),   ustr_ngr_vertex_f(:, :) 
        integer,  allocatable :: n_ustr_ngr_center(:),   n_ustr_ngr_vertex(:)
        integer,  allocatable :: n_ustr_ngr_center_new(:), n_ustr_ngr_vertex_new(:)
        integer,  allocatable :: n_ustr_ngr_center_f(:), n_ustr_ngr_vertex_f(:)
        integer,  allocatable :: unique_vertices(:), sorted_vertices(:), vertex_mapping(:)
        integer,  allocatable :: bdy_long_order(:)
        integer,  allocatable :: ustr_id(:, :), ustr_ii(:, :), IsInDmArea_ustr_read(:)
        character(pathlen) :: lndname
        character(LEN = 5) :: nxpc

        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! Read me !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        ! 0 : 不存在 -1 : 陆地 1 : 海洋
        ! without _new and _f is orial data
        ! *_new : intermediate variable
        ! *_f : finial data
        ! for orial data to intermediate variable(*_new) to finial data(*_f)
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! Read me !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        ! read unstructure mesh
        write(io6, *), "start to read unstructure mesh data in the module MOD_mask_make"
        ! 读取未细化初始网格数据
        write(nxpc, '(I4.4)') NXP
        lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)//'.nc4'
        write(io6, *),lndname
        ! 读入的数据除了第一个图形，其他图形都是存在的
        call Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *), "The unstructured grid data reading have done in the SUBROUTINE mask_postproc_Ocn"
        write(io6, *), ""
        write(io6, *), "In total, triangular mesh number: ", sjx_points, "polygon mesh number: ", lbx_points
        write(io6, *), ""

        ! IsInDmArea_ustr数据引入
        ! 但是如果边界上含有的元素很少怎么办
        lndname = trim(file_dir) // 'contain/contain_'// trim(mesh_type) //'_domain_NXP'//trim(nxpc)//'_'//trim(mode_grid)//'.nc4'
        write(io6, *), lndname, 'reading in the SUBROUTINE mask_postproc_Ocn'
        CALL Contain_Read(lndname, ustr_points, numpatch, ustr_id, ustr_ii, IsInDmArea_ustr_read)
        IsInDmArea_ustr = IsInDmArea_ustr_read
        do i = num_vertex + 1, ustr_points, 1
            if (ustr_id(i, 1) > 0) then
                if (ustr_id(i, 1)/real(ustr_id(i, 3)) < mask_sea_ratio) IsInDmArea_ustr(i) = -1
            end if
        end do
        deallocate(ustr_id, ustr_ii, IsInDmArea_ustr_read)

        if (mode_grid == 'tri') then
            write(io6, *), "开始制作三角形海洋网格"
            ustr_points = sjx_points
            ustr_bounds = lbx_points
            allocate(ustr_center(ustr_points, 2));       ustr_center = mp
            allocate(ustr_vertex(ustr_bounds, 2));       ustr_vertex = wp
            allocate(ustr_ngr_center(3, ustr_points));   ustr_ngr_center = ngrmw
            allocate(ustr_ngr_vertex(7, ustr_bounds));   ustr_ngr_vertex = ngrwm
            allocate(n_ustr_ngr_center(ustr_points));    n_ustr_ngr_center = 3
            allocate(n_ustr_ngr_vertex(ustr_bounds));    n_ustr_ngr_vertex = n_ngrwm
        else if (mode_grid == 'hex') then
            !开始制作六边形海洋网格
            ustr_points = lbx_points
            ustr_bounds = sjx_points
            allocate(ustr_center(ustr_points, 2));       ustr_center = wp
            allocate(ustr_vertex(ustr_bounds, 2));       ustr_vertex = mp
            allocate(ustr_ngr_center(7, ustr_points));   ustr_ngr_center = ngrwm
            allocate(ustr_ngr_vertex(3, ustr_bounds));   ustr_ngr_vertex = ngrmw
            allocate(n_ustr_ngr_center(ustr_points));    n_ustr_ngr_center = n_ngrwm
            allocate(n_ustr_ngr_vertex(ustr_bounds));    n_ustr_ngr_vertex = 3
        else 
            stop "ERROR! tri and hex only in the MOD_mask_make()"
        end if
        ! 完成数据初步更新，去除非海洋网格
        CALL Data_Renew(ustr_points, ustr_bounds, ustr_ngr_center, n_ustr_ngr_center, ustr_points_new, ustr_bounds_new, &
                            ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
        ! 根据center顶点，更新vertex中心点与center中心点位连接情况，以及vertex中心点
        ! 由新center中心点指向顶点，再统计顶点个数
        if (mode_grid == 'tri') then
            write(io6, *), "去除不存在三角形/陆地三角形网格前:"
            write(io6, *), ustr_points, "三角形网格", ustr_bounds, "六边形网格"
            write(io6, *), "去除不存在三角形/陆地三角形网格后:"
            write(io6, *), ustr_points_new, "三角形网格", ustr_bounds_new, "六边形网格(含虚假六边形)"
            write(io6, *), ""
        else if (mode_grid == 'hex') then
            write(io6, *), "去除不存在六边形/陆地六边形网格前:"
            write(io6, *), ustr_bounds, "三角形网格", ustr_points, "六边形网格"
            write(io6, *), "去除不存在六边形/陆地六边形网格后:"
            write(io6, *), ustr_bounds_new, "三角形网格(含虚假三角形)", ustr_points_new, "六边形网格"
            write(io6, *), ""
        end if

        if (mode_grid == 'tri') then
            write(io6, *), "IsInDmArea_ustr Renew start" ! only use for tri
            ! 海陆关系调整(删去弱连接的三角形，增加强连接的三角形)
            CALL IsInDmArea_ustr_Renew(ustr_points, ustr_bounds, ustr_ngr_vertex, ustr_ngr_vertex_new, n_ustr_ngr_vertex, n_ustr_ngr_vertex_new, ustr_points_new)
            ! 重新更新的数据
            deallocate(ustr_ngr_center_new, n_ustr_ngr_center_new)
            deallocate(ustr_ngr_vertex_new, n_ustr_ngr_vertex_new)
            CALL Data_Renew(ustr_points, ustr_bounds, ustr_ngr_center, n_ustr_ngr_center, ustr_points_new, ustr_bounds_new, &
                            ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
            write(io6, *), "海陆关系调整后(step1)，", ustr_points_new, "个三角形网格"
            write(io6, *), "海陆关系调整后(step1)，", ustr_bounds_new, "个六边形网格(含虚假六边形)"

            ! for connect!'
            hhh = 1
            k = 1
            do while (hhh /= ustr_points_new)
                write(io6, *), "k= ", k, "hhh = ", hhh, "ustr_points_new = ", ustr_points_new
                CALL IsInDmArea_ustr_Renew_v2(ustr_bounds, ustr_ngr_vertex, n_ustr_ngr_vertex, n_ustr_ngr_vertex_new, ustr_points_new)    
                ! 重新更新数据
                deallocate(ustr_ngr_center_new, n_ustr_ngr_center_new)
                deallocate(ustr_ngr_vertex_new, n_ustr_ngr_vertex_new) 
                CALL Data_Renew(ustr_points, ustr_bounds, ustr_ngr_center, n_ustr_ngr_center, ustr_points_new, ustr_bounds_new, &
                                ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
                write(io6, *), "海陆关系调整后(step2)，", ustr_points_new, "个三角形网格"
                write(io6, *), "海陆关系调整后(step2)，", ustr_bounds_new, "个六边形网格(含虚假六边形)"
                write(io6, *), "IsInDmArea_ustr Renew finish"
                write(io6, *), ""
                hhh = ustr_points_new
                write(io6, *), "narrow_waterway_widen start"
                CALL narrow_waterway_widen(ustr_bounds, ustr_points_new, ustr_ngr_vertex, ustr_ngr_center_new, &
                                        n_ustr_ngr_vertex, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
                ! 重新更新数据
                deallocate(ustr_ngr_center_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
                CALL Data_Renew(ustr_points, ustr_bounds, ustr_ngr_center, n_ustr_ngr_center, ustr_points_new, ustr_bounds_new, &
                                ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
                write(io6, *), "narrow_waterway_widen后，", ustr_points_new, "个三角形网格"
                write(io6, *), "narrow_waterway_widen后，", ustr_bounds_new, "个六边形网格(含虚假六边形)"
                write(io6, *), "narrow_waterway_widen finish"
                write(io6, *), ""
                k = k + 1
            end do

            write(io6, *), "Isolated oceanmesh Renew start"
            CALL Isolated_Ocean_Renew(ustr_bounds, ustr_points_new, ustr_bounds_new, ustr_ngr_center, n_ustr_ngr_center, n_ustr_ngr_vertex, &
                                    ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new, num_bdy_long, bdy_long_order)
            write(io6, *), "Isolated oceanmesh Renew finish"
            ! 重新更新数据
            deallocate(ustr_ngr_center_new, n_ustr_ngr_center_new)
            deallocate(ustr_ngr_vertex_new, n_ustr_ngr_vertex_new) 
            CALL Data_Renew(ustr_points, ustr_bounds, ustr_ngr_center, n_ustr_ngr_center, ustr_points_new, ustr_bounds_new, &
                            ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
            write(io6, *), "孤立海洋网格处理后，", ustr_points_new, "个三角形网格"
            write(io6, *), "孤立海洋网格处理后，", ustr_bounds_new, "个六边形网格(含虚假六边形)"
            write(io6, *), ""
        else if (mode_grid == 'hex') then
            ! 六边形的海洋网格还需要进一步的处理
            write(io6, *), "hex do not have Special treatment now!" 
        end if

        ! 获取最终的六边形信息(from *_new to *_f)
        ! 获取center and vertex length
        ustr_points_f = ustr_points_new
        ustr_bounds_f = ustr_bounds_new
        CALL Data_Finial(ustr_points, ustr_bounds, ustr_points_f, ustr_bounds_f, ustr_center, ustr_vertex, ustr_ngr_center, n_ustr_ngr_center, &
                            ustr_center_f, ustr_vertex_f, ustr_ngr_center_f, ustr_ngr_vertex_f, n_ustr_ngr_center_f, n_ustr_ngr_vertex_f)


        write(io6, *), "n_ustr_ngr_vertex_f range "
        write(io6, *), minval(n_ustr_ngr_vertex_f(2:ustr_bounds_f)), maxval(n_ustr_ngr_vertex_f(2:ustr_bounds_f)) ! 单独存放   

        ! 删除部分信息后，会造成数据编号的不连续呢，
        ! ustr_center_f和ustr_vertex_f的顺序都是对的，ustr_ngr_vertex_f的顺序也是对的，但ustr_ngr_center_f却不是
        write(io6, *), "sort order of ustr_ngr_center_f (step1)"
        !  步骤1：提取唯一顶点编号
        allocate(unique_vertices(ustr_bounds)) ! 存储一维顶点,最多有ustr_bounds个顶点
        CALL extract_unique_vertices(ustr_ngr_center_f, n_ustr_ngr_center_f, unique_vertices, nvertices)

        write(io6, *), "sort order of ustr_ngr_center_f (step2)"
        ! 步骤2：排序并重新编号顶点
        allocate(sorted_vertices(nvertices)) ! 排序后的顶点
        allocate(vertex_mapping(ustr_bounds)) ! 顶点的映射数组
        call sort_and_reindex(unique_vertices, nvertices, sorted_vertices, vertex_mapping)

        write(io6, *), "sort order of ustr_ngr_center_f (step3)"
        ! 步骤3：根据新的顶点编号重新编号中心点
        do j = 2, ustr_points_f, 1
            do i = 1, n_ustr_ngr_center_f(j), 1
                ustr_ngr_center_f(i, j) = vertex_mapping(ustr_ngr_center_f(i, j))
            end do
        end do
        write(io6, *), "minval(ustr_ngr_center_f) = ", minval(ustr_ngr_center_f(1:3, 2:ustr_points_f))
        write(io6, *), "maxval(ustr_ngr_center_f) = ", maxval(ustr_ngr_center_f(1:3, 2:ustr_points_f))
        write(io6, *), "sort order of ustr_ngr_center_f finish"
        
        lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)// '_' // trim(mesh_type) //'.nc4'
        if (mask_patch_on) lndname = trim(file_dir) // 'result/gridfile_NXP' // trim(nxpc) // '_'//trim(mode_grid)// '_' // trim(mesh_type) //'_patch.nc4'
        write(io6, *), lndname
        if (mode_grid == 'tri') then
            CALL Unstructured_Mesh_Save(lndname, ustr_points_f, ustr_bounds_f, ustr_center_f, ustr_vertex_f, ustr_ngr_center_f, ustr_ngr_vertex_f)
        else if (mode_grid == 'hex') then
            CALL Unstructured_Mesh_Save(lndname, ustr_bounds_f, ustr_points_f, ustr_vertex_f, ustr_center_f, ustr_ngr_vertex_f, ustr_ngr_center_f)
        end if
        write(io6, *), "nc save finish"

        ! 处理开边界问题(目前之后针对三角形的情况，针对六边形的情况还需要进一步处理)
        if (mode_grid == 'tri') then
            CALL bdy_calculation(num_bdy_long, bdy_long_order, ustr_ngr_vertex, n_ustr_ngr_vertex, vertex_mapping) 
            ! if (output_format == 'FVCOM') CALL FVCOM_Mesh_Save(lndname, ustr_points_f, ustr_bounds_f, ustr_vertex_f, ustr_ngr_center_f)
        end if       

    END SUBROUTINE mask_postproc_Ocn

    SUBROUTINE Isolated_Ocean_Renew(ustr_bounds, ustr_points_new, ustr_bounds_new, ustr_ngr_center, n_ustr_ngr_center, n_ustr_ngr_vertex, &
                                    ustr_ngr_center_new, ustr_ngr_vertex_new, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new, num_bdy_long, bdy_long_order)
        ! 去除孤立的海洋网格以及或者最长的闭合边界曲线
        implicit none
        integer, intent(in) :: ustr_bounds
        integer, intent(in) :: ustr_points_new, ustr_bounds_new
        integer, allocatable, intent(in) :: ustr_ngr_center(:, :)
        integer, allocatable, intent(in) :: n_ustr_ngr_center(:), n_ustr_ngr_vertex(:)
        integer, allocatable, intent(in) :: ustr_ngr_center_new(:, :), ustr_ngr_vertex_new(:, :)
        integer, allocatable, intent(in) :: n_ustr_ngr_center_new(:)
        integer, allocatable, intent(inout) :: n_ustr_ngr_vertex_new(:)
        logical :: fexist
        integer :: i, j, k, m, n, ngr_select, center_select
        integer :: im, ik
        integer :: bdy_long_num, num_add
        integer :: bdy_num_in, num_closed_curve
        integer :: num_diff, bdy_isolated_num
        integer, allocatable :: bdy_order(:)
        integer, allocatable :: bdy_isolated_order(:)
        integer, allocatable :: close_curve(:, :), n_close_curve(:)
        integer, intent(out) :: num_bdy_long(3)
        integer, allocatable, intent(inout) :: bdy_long_order(:)

        ! 获取海洋网格边界信息（网格总数，分段数量，闭合曲线最长长度，海洋网格vertex编号）
        CALL bdy_connection(ustr_bounds, ustr_points_new, ustr_ngr_center_new, n_ustr_ngr_vertex, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new, &
                                bdy_num_in, num_closed_curve, num_bdy_long, bdy_order, close_curve, n_close_curve)

        ! 判断除了最长的闭合曲线外，还要判断剩下的闭合曲线是孤立的海洋网格，还是island
        ! bdy_long_order 最长的闭合曲线编号
        ! bdy_isolated_order 用于处理isolate oceanmesh 最长为次长的闭合曲线编号
        allocate(bdy_long_order(num_bdy_long(1))); bdy_long_order = 1
        allocate(bdy_isolated_order(num_bdy_long(1))); bdy_isolated_order = 1
        bdy_long_order(2:n_close_curve(num_bdy_long(3))+1) = close_curve(1:n_close_curve(num_bdy_long(3)), num_bdy_long(3))
        do i = 1, num_closed_curve, 1
            if (i == num_bdy_long(3)) cycle ! 跳过最长的闭合曲线
            num_diff = 0
            do j = 1, n_close_curve(i), 1
                num_diff = num_diff + 2 * n_ustr_ngr_vertex_new(close_curve(j, i)) - n_ustr_ngr_vertex(close_curve(j, i))
            end do
            if (num_diff < 0) then ! 判断为孤立海洋网格
                write(io6, *), "i = ", i, "为孤立海洋网格需要去除"
                write(io6, *), "Isolated oceanmesh remove start"
                write(io6, *), "n_close_curve(i) = ", n_close_curve(i)
                ! 获取起始最外围边界信息
                num_add = 1
                do while (num_add /= 0)
                    num_add = 0
                    bdy_isolated_num   = n_close_curve(i)
                    bdy_isolated_order = close_curve(:, i)
                    n_close_curve(i) = 0
                    close_curve(:, i) = 1

                    do j = 1, bdy_isolated_num, 1
                        ngr_select = bdy_isolated_order(j) ! 获取海洋网格边界编号
                        n = n_ustr_ngr_vertex_new(ngr_select) ! 获取边界相关联的海洋网格个数
                        n_ustr_ngr_vertex_new(ngr_select) = 0 ! 很关键！！！
                        do k = 1, n ,1
                            center_select = ustr_ngr_vertex_new(k, ngr_select) ! 获取对应的center网格编号
                            IsInDmArea_ustr(center_select) = -1 ! 海洋网格变为陆地网格
                            do m = 1, n_ustr_ngr_center(center_select), 1 ! 对这个center网格相连的vertex遍历，找到下一层边界
                                ! 将网格边界向内收缩（即找到全是海洋网格的边界点位。如果没有就退出！）
                                im = ustr_ngr_center(m, center_select) ! get vertex beside center
                                if (n_ustr_ngr_vertex_new(im) /= n_ustr_ngr_vertex(im)) cycle
                                fexist = .false.
                                do ik = 1, n_close_curve(i) + 1 ! if had added before?
                                    if (im == close_curve(ik, i)) then
                                        fexist = .true.
                                        exit
                                    end if
                                end do
                                if (.not. fexist) then
                                    n_close_curve(i) = n_close_curve(i) + 1
                                    close_curve(n_close_curve(i), i) = im
                                 end if
                            end do
                        end do
                    end do

                    num_add = n_close_curve(i)
                    if (n_close_curve(i) == 1) num_add = 0
                    write(io6, *), "n_close_curve(i) = ", n_close_curve(i)
                end do
                write(io6, *), "Isolated oceanmesh remove finish"
                write(io6, *), ""
            end if
        end do
        write(io6, *), "sum(n_close_curve) = ", sum(n_close_curve) 

    END SUBROUTINE Isolated_Ocean_Renew

    SUBROUTINE narrow_waterway_widen(ustr_bounds, ustr_points_new, ustr_ngr_vertex, ustr_ngr_center_new, n_ustr_ngr_vertex, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new)
        ! 将单行的狭窄水道拓宽到两行
        implicit none
        integer, intent(in) :: ustr_bounds, ustr_points_new
        integer, allocatable, intent(in) :: ustr_ngr_vertex(:, :)
        integer, allocatable, intent(in) :: ustr_ngr_center_new(:, :)
        integer, allocatable, intent(in) :: n_ustr_ngr_vertex(:)
        integer, allocatable, intent(in) :: n_ustr_ngr_center_new(:), n_ustr_ngr_vertex_new(:)
        logical :: fexist
        integer :: i, j, k, n, im, ik
        integer :: bdy_num_in
        integer, allocatable :: ngrvv_new(:, :), n_ngrvv_new(:)
        ! 建立海洋网格边界vertex-vertex的连接关系ngrvv_new
        ! 通过理论分析，一个海洋网格边界理论上最多有四个走向（两真实，两虚假）
        allocate(ngrvv_new(4, ustr_bounds));  ngrvv_new = 1
        allocate(n_ngrvv_new(ustr_bounds)); n_ngrvv_new = 0

        bdy_num_in = 1 ! 空出第一个位置
        do i = 2, ustr_points_new, 1
            n = n_ustr_ngr_center_new(i)
            do j = 1, n, 1
                ! im, ik分别是两个相连的顶点编号，要求全是边界点位才进行下一步计算
                ! 如果这两个顶点对于的vertex形状不亏缺则说明三角形是海洋center点位
                im = ustr_ngr_center_new(j, i)
                if (n_ustr_ngr_vertex_new(im) == n_ustr_ngr_vertex(im)) cycle
                ik = ustr_ngr_center_new(mod(j, n) + 1, i)
                if (n_ustr_ngr_vertex_new(ik) == n_ustr_ngr_vertex(ik)) cycle
                bdy_num_in = bdy_num_in + 1
                n_ngrvv_new(im) = n_ngrvv_new(im) + 1
                n_ngrvv_new(ik) = n_ngrvv_new(ik) + 1
                ngrvv_new(n_ngrvv_new(im), im) = ik
                ngrvv_new(n_ngrvv_new(ik), ik) = im
                exit
            end do
        end do
        write(io6, *), "Before handle Narrow waterway bdy_num_in = ", bdy_num_in
        write(io6, *), "Handle Narrow waterway start"
        do i = 2, ustr_bounds, 1
            if (n_ngrvv_new(i) /= 4) cycle
            bdy_num_in = bdy_num_in - 1
            fexist = .FALSE.
            do j = 1, 3, 1
                do k = j+1, 4, 1
                    if (ngrvv_new(j, i) == ngrvv_new(k, i)) then
                        im = ngrvv_new(j, i)
                        fexist = .TRUE.
                        exit
                    end if
                end do
                if (fexist) exit ! jump twice!
            end do
            ! widen!!!!!!!!!!
            do ik = 1, n_ustr_ngr_vertex(im), 1
                IsInDmArea_ustr(ustr_ngr_vertex(ik, im)) = 1
            end do
        end do
        write(io6, *), "Handle Narrow waterway finish"
        write(io6, *), "After handle Narrow waterway bdy_num_in = ", bdy_num_in
        write(io6, *), ""
   
    END SUBROUTINE narrow_waterway_widen

    SUBROUTINE bdy_connection(ustr_bounds, ustr_points_new, ustr_ngr_center_new, n_ustr_ngr_vertex, n_ustr_ngr_center_new, n_ustr_ngr_vertex_new, &
                                bdy_num_in, num_closed_curve, num_bdy_long, bdy_order, close_curve, n_close_curve)
        ! 获取海洋边界的连接信息以及bdy_long_num信息
        implicit none
        integer, intent(in) :: ustr_bounds, ustr_points_new
        integer, allocatable, intent(in) :: ustr_ngr_center_new(:, :)
        integer, allocatable, intent(in) :: n_ustr_ngr_vertex(:)
        integer, allocatable, intent(in) :: n_ustr_ngr_center_new(:), n_ustr_ngr_vertex_new(:)
        logical :: fexist
        integer :: i, j, k, m, n, im, ik
        integer :: num_points, bdy_end, ngr_select
        integer, allocatable :: ngrvv_new(:, :), n_ngrvv_new(:)
        integer, allocatable :: bdy_ngr(:, :), n_bdy_ngr(:)
        integer, allocatable :: bdy_queue(:), bdy_alternate(:)
        integer, intent(out) :: bdy_num_in, num_closed_curve, num_bdy_long(3)
        integer, allocatable, intent(out) :: bdy_order(:), close_curve(:, :), n_close_curve(:)
        integer :: ncid, varid(2), DimID_bdy, DimID_bdy2
        character(pathlen) :: lndname
        ! 建立海洋网格边界vertex-vertex的连接关系ngrvv_new
        ! 通过理论分析，一个海洋网格边界理论上最多有四个走向（两真实，两虚假）
        allocate(ngrvv_new(4, ustr_bounds));  ngrvv_new = 1
        allocate(n_ngrvv_new(ustr_bounds)); n_ngrvv_new = 0

        bdy_num_in = 1 ! 空出第一个位置
        do i = 2, ustr_points_new, 1
            n = n_ustr_ngr_center_new(i)
            do j = 1, n, 1
                ! im, ik分别是两个相连的顶点编号，要求全是边界点位才进行下一步计算
                ! 如果这两个顶点对于的vertex形状不亏缺则说明三角形是海洋center点位
                im = ustr_ngr_center_new(j, i)
                if (n_ustr_ngr_vertex_new(im) == n_ustr_ngr_vertex(im)) cycle
                ik = ustr_ngr_center_new(mod(j, n) + 1, i)
                if (n_ustr_ngr_vertex_new(ik) == n_ustr_ngr_vertex(ik)) cycle
                bdy_num_in = bdy_num_in + 1
                n_ngrvv_new(im) = n_ngrvv_new(im) + 1
                n_ngrvv_new(ik) = n_ngrvv_new(ik) + 1
                ngrvv_new(n_ngrvv_new(im), im) = ik
                ngrvv_new(n_ngrvv_new(ik), ik) = im
                exit
            end do
        end do
        ! check for n_ngrvv_new
        do i = 2, ustr_bounds, 1
            if (n_ngrvv_new(i) == 0) cycle
            if (n_ngrvv_new(i) == 2) cycle
            write(io6, *), "i = ", i, "n_ngrvv_new(i) = ", n_ngrvv_new(i)
            write(io6, *), "ngrvv_new(1:4, i) = ", ngrvv_new(1:4, i)
            stop "ERROR! n_ngrvv_new(i) must be 0 or 2!"
        end do
   
        ! 对于每一个边界vertex，真正有效的连接关系与网格形状无关，应该有却只有两个连接的vertex
        allocate(bdy_order(bdy_num_in)); bdy_order = 1
        allocate(bdy_ngr(2, ustr_bounds)); bdy_ngr = 1 ! 编号1表示不存在
        bdy_num_in = 1
        do i = 2, ustr_bounds, 1
            if (n_ngrvv_new(i) /= 2) cycle
            bdy_num_in = bdy_num_in + 1 !获取边界顶点信息
            bdy_order(bdy_num_in) = i
            k = 0
            n = 0
            do while (n /= 2)
                k = k + 1
                if (ngrvv_new(k, i) == 1) cycle
                n = n + 1
                bdy_ngr(n, i) = ngrvv_new(k, i)
            end do
            ! write(io6, *), "i = ", i, "bdy_ngr(:, i) = ", bdy_ngr(:, i)
        end do

        ! 获取num_closed_curve, num_bdy_long信息
        CALL bdy_connection_closed_curve(bdy_num_in, bdy_order, bdy_ngr, num_closed_curve, num_bdy_long)

        ! 重新遍历，获取bdy_queue信息并保留！！！！
        allocate(close_curve(num_bdy_long(1), num_closed_curve)); close_curve = 1
        allocate(n_close_curve(num_closed_curve)); n_close_curve = 0
        CALL bdy_connection_closed_curve(bdy_num_in, bdy_order, bdy_ngr, num_closed_curve, num_bdy_long, close_curve, n_close_curve)
        
        lndname = trim(file_dir) // 'result/obcv2.nc4'
        if (mask_patch_on) lndname = trim(file_dir) // 'result/obcv2_patch.nc4'
        write(io6, *), trim(lndname)
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncid, "num1", num_bdy_long(1), DimID_bdy))
        CALL CHECK(NF90_DEF_DIM(ncid, "num2", num_closed_curve, DimID_bdy2))
        CALL CHECK(NF90_DEF_VAR(ncid, "close_curve", NF90_INT, (/ DimID_bdy, DimID_bdy2 /), varid(1)))
        CALL CHECK(NF90_DEF_VAR(ncid, "n_close_curve", NF90_INT, (/ DimID_bdy2 /), varid(2)))
        CALL CHECK(NF90_ENDDEF(ncid))
        CALL CHECK(NF90_PUT_VAR(ncid, varid(1), close_curve))
        CALL CHECK(NF90_PUT_VAR(ncid, varid(2), n_close_curve))
        CALL CHECK(NF90_CLOSE(ncid))

    END SUBROUTINE bdy_connection

    SUBROUTINE bdy_connection_closed_curve(bdy_num_in, bdy_order, bdy_ngr, num_closed_curve, num_bdy_long, close_curve, n_close_curve)
        ! 进行vertex的前后连接
        integer, intent(in) :: bdy_num_in
        integer, allocatable, intent(in) :: bdy_order(:), bdy_ngr(:, :)
        integer :: i, j
        integer :: num_points, bdy_end, ngr_select
        integer, allocatable :: bdy_queue(:), bdy_alternate(:)
        integer, intent(out) :: num_closed_curve, num_bdy_long(3)
        integer, allocatable, intent(inout), optional :: close_curve(:,:), n_close_curve(:)

        allocate(bdy_queue(bdy_num_in)); bdy_queue = 1
        allocate(bdy_alternate(bdy_num_in)); bdy_alternate = 1 ! 1表示可以使用，0表示已经使用
        num_bdy_long = 0 ! 第一个位置记录最长长度，第二个位置记录次长长度
        num_closed_curve = 0 ! 记录闭合曲线个数
        do while(sum(bdy_alternate) > 1)
            ! 寻找闭合曲线的起点
            num_points = 1 ! 数据初始化
            num_closed_curve = num_closed_curve + 1
            bdy_queue = 1 ! 数据初始化
            do j = 2, bdy_num_in, 1
                if (bdy_alternate(j) == 1) then ! the start of queue
                    bdy_queue(num_points) = bdy_order(j)
                    bdy_alternate(j) = 0
                    ! write(io6, *), "start from : ", bdy_order(j)
                    exit
                end if
            end do

            ! 开始进行vertex连接使其成为闭合曲线
            bdy_end    = bdy_ngr(2, bdy_order(j)) ! the end of queue
            ngr_select = bdy_ngr(1, bdy_order(j)) ! 获取编号, 还需要知道这个编号对应的顺序编号
            do while(ngr_select /= bdy_end)
                num_points = num_points + 1
                bdy_queue(num_points)  = ngr_select
                do j = 2, bdy_num_in, 1 ! ngr_select 实际在bdy_order中的位置
                    if (bdy_order(j) == ngr_select) exit
                end do
                bdy_alternate(j) = 0
                do i = 1, 2, 1
                    if (bdy_ngr(i, ngr_select) == bdy_queue(num_points-1)) cycle
                    ngr_select = bdy_ngr(i, ngr_select)
                    exit ! aviod ngr_select change twice!
                end do
            end do

            num_points = num_points + 1
            bdy_queue(num_points)  = bdy_end
            do j = 2, bdy_num_in, 1
                if (bdy_order(j) == bdy_end) exit
            end do
            bdy_alternate(j) = 0
            if (num_points < 3) stop "ERROR! num_points < 3 !"

            ! bdy_queue是有顺序的，不可以随便处理，需要谨慎！
            ! 如果出入某一个参数，则执行这部分内容
            if (present(n_close_curve)) then
                n_close_curve(num_closed_curve) = num_points
                close_curve(1:num_points, num_closed_curve) = bdy_queue(1:num_points)
                write(io6, *), "num_closed_curve = ", num_closed_curve, "闭合曲线长度为 ：", num_points
            end if

            ! num_bdy_long 更新
            if (num_points > num_bdy_long(1)) then
                num_bdy_long(1) = num_points ! num_points变为最长
                num_bdy_long(3) = num_closed_curve ! 记录最长的闭合曲线编号
            end if
            if (num_closed_curve /= 1) then 
                if (num_points > num_bdy_long(2)) then
                    if (num_points < num_bdy_long(1)) then
                        num_bdy_long(2) = num_points
                    end if
                end if
            end if
        end do
        num_bdy_long(1:2) = num_bdy_long(1:2) + 1
        write(io6, *), "num_bdy_long = ", num_bdy_long, "start from one"

    END SUBROUTINE bdy_connection_closed_curve


    SUBROUTINE Data_Renew(ustr_points, ustr_bounds, ustr_ngr_center, n_ustr_ngr_center, ustr_points_next, ustr_bounds_next, &
                            ustr_ngr_center_next, ustr_ngr_vertex_next, n_ustr_ngr_center_next, n_ustr_ngr_vertex_next)

        implicit none
        integer, intent(in) :: ustr_points, ustr_bounds
        integer,  allocatable, intent(in) :: ustr_ngr_center(:, :)
        integer,  allocatable, intent(in) :: n_ustr_ngr_center(:)
        integer :: i, j, k, m
        integer, intent(out) :: ustr_points_next, ustr_bounds_next
        integer,  allocatable, intent(out) :: ustr_ngr_center_next(:, :), ustr_ngr_vertex_next(:, :)
        integer,  allocatable, intent(out) :: n_ustr_ngr_center_next(:),  n_ustr_ngr_vertex_next(:)

        write(io6, *), "重新计算并储存ustr_ngr_center and ustr_center_next"
        ustr_points_next = 1 ! the first is zero-sjxorlbx
        do i = 2, ustr_points, 1 ! 网格总数（含不存在0 /海洋1 /陆地-1）
            if (IsInDmArea_ustr(i) == 1) ustr_points_next = ustr_points_next + 1!  海洋网格总数
        end do

        ! 计算ustr_ngr_center_next, and n_ustr_ngr_center_next
        if (mode_grid == 'tri') then
            allocate(ustr_ngr_center_next(3, ustr_points_next))
            allocate(ustr_ngr_vertex_next(7, ustr_bounds))
        else if (mode_grid == 'hex') then
            allocate(ustr_ngr_center_next(7, ustr_points_next))
            allocate(ustr_ngr_vertex_next(3, ustr_bounds))
        end if
        ustr_ngr_center_next   = 1 ! 记录center相邻vertex编号，初始化为1
        ustr_ngr_vertex_next   = 1 ! 记录vertex相邻center编号，初始化为1

        allocate(n_ustr_ngr_center_next(ustr_points_next))
        allocate(n_ustr_ngr_vertex_next(ustr_bounds))
        n_ustr_ngr_center_next = 0 ! 记录center相邻vertex个数
        n_ustr_ngr_vertex_next = 0 ! 记录vertex相邻center个数

        k = 1
        do i = 2, ustr_points, 1 ! 网格总数（对于oceanmesh选项而言，含不存在0 /海洋1 /陆地-1）
            if (IsInDmArea_ustr(i) /= 1) cycle
            k = k + 1
            ustr_ngr_center_next(:, k) = ustr_ngr_center(:, i)
            n_ustr_ngr_center_next(k)  = n_ustr_ngr_center(i)
            ! 更新vertex相邻center信息
            do j = 1, n_ustr_ngr_center_next(k), 1
                m = ustr_ngr_center_next(j, k) ! 获取center顶点（或vertex中心点）编号信息
                n_ustr_ngr_vertex_next(m) = n_ustr_ngr_vertex_next(m) + 1 ! 累加顶点个数
                !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                ! if in the SUBROUTINE Data_Finial, Assign value to k not i
                ustr_ngr_vertex_next(n_ustr_ngr_vertex_next(m), m) = i ! 这个i就是原本顺序的i
                !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            end do
        end do        

        ! 确定ustr_bounds_next
        ustr_bounds_next = ustr_bounds
        do i = 2, ustr_bounds, 1
            if (n_ustr_ngr_vertex_next(i) == 0) ustr_bounds_next = ustr_bounds_next - 1 ! 排除不存在的六边形
        end do

    END SUBROUTINE Data_Renew

    ! from orial data to finial data
    SUBROUTINE Data_Finial(ustr_points, ustr_bounds, ustr_points_f, ustr_bounds_f, ustr_center, ustr_vertex, ustr_ngr_center, n_ustr_ngr_center, &
                            ustr_center_f, ustr_vertex_f, ustr_ngr_center_f, ustr_ngr_vertex_f, n_ustr_ngr_center_f, n_ustr_ngr_vertex_f)

        implicit none
        integer, intent(in) :: ustr_points, ustr_bounds
        integer, intent(in) :: ustr_points_f, ustr_bounds_f
        real(r8), allocatable, intent(in) :: ustr_center(:, :),     ustr_vertex(:, :)
        integer,  allocatable, intent(in) :: ustr_ngr_center(:, :)
        integer,  allocatable, intent(in) :: n_ustr_ngr_center(:)
        integer :: i, j, k, m
        integer,  allocatable :: ustr_ngr_vertex_var(:, :) ! 中间变量
        integer,  allocatable :: n_ustr_ngr_vertex_var(:) ! 中间变量
        real(r8), allocatable, intent(out) :: ustr_center_f(:, :),     ustr_vertex_f(:, :)
        integer,  allocatable, intent(out) :: ustr_ngr_center_f(:, :), ustr_ngr_vertex_f(:, :)
        integer,  allocatable, intent(out) :: n_ustr_ngr_center_f(:),  n_ustr_ngr_vertex_f(:)

        ! 定义center and vertex 经纬度坐标
        allocate(ustr_center_f(ustr_points_f, 2))
        allocate(ustr_vertex_f(ustr_bounds_f, 2))
        ustr_center_f = 0.
        ustr_vertex_f = 0.

        if (mode_grid == 'tri') then
            allocate(ustr_ngr_center_f(3, ustr_points_f))
            allocate(ustr_ngr_vertex_f(7, ustr_bounds_f))
        else if (mode_grid == 'hex') then
            allocate(ustr_ngr_center_f(7, ustr_points_f))
            allocate(ustr_ngr_vertex_f(3, ustr_bounds_f))
        end if
        ustr_ngr_center_f   = 1 ! 记录center相邻vertex编号，初始化为1
        ustr_ngr_vertex_f   = 1 ! 记录vertex相邻center编号，初始化为1

        ! 获取中间变量信息
        if (mode_grid == 'tri') then
            allocate(ustr_ngr_vertex_var(7, ustr_bounds))
        else if (mode_grid == 'hex') then
            allocate(ustr_ngr_vertex_var(3, ustr_bounds))
        end if
        allocate(n_ustr_ngr_vertex_var(ustr_bounds))
        ustr_ngr_vertex_var   = 1
        n_ustr_ngr_vertex_var = 0

        allocate(n_ustr_ngr_center_f(ustr_points_f))
        allocate(n_ustr_ngr_vertex_f(ustr_bounds_f))
        n_ustr_ngr_center_f = 0 ! 记录center相邻vertex个数
        n_ustr_ngr_vertex_f = 0 ! 记录vertex相邻center个数

        ! 注意：对于vertex而言，生成的是*_var, 对于center而言，生成的是*_f
        k = 1 ! 新center网格顺序
        do i = 2, ustr_points, 1 ! 网格总数（含不存在0 /海洋1 /陆地-1）
            if (IsInDmArea_ustr(i) /= 1) cycle
            k = k + 1
            ustr_center_f(k, :) = ustr_center(i, :)
            ustr_ngr_center_f(:, k) = ustr_ngr_center(:, i)
            n_ustr_ngr_center_f(k)  = n_ustr_ngr_center(i)

            ! 更新vertex相邻center信息
            do j = 1, n_ustr_ngr_center_f(k), 1
                m = ustr_ngr_center_f(j, k) ! 获取center顶点（或vertex中心点）编号信息
                n_ustr_ngr_vertex_var(m) = n_ustr_ngr_vertex_var(m) + 1 ! 累加顶点个数
                !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                ! if in the SUBROUTINE Data_Renew, Assign value to i not k
                ustr_ngr_vertex_var(n_ustr_ngr_vertex_var(m), m) = k ! 这个k是新center编号顺序
                !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            end do
        end do       

        ! 更新vertex信息 （from *_var to *_f）
        k = 1
        do i = 2, ustr_bounds, 1
            if (n_ustr_ngr_vertex_var(i) == 0) cycle ! 应该是vertex不存在了，才去掉
            k = k + 1
            ustr_vertex_f(k, :) = ustr_vertex(i, :)
            ustr_ngr_vertex_f(:, k) = ustr_ngr_vertex_var(:, i)
            n_ustr_ngr_vertex_f(k) = n_ustr_ngr_vertex_var(i)
        end do

    END SUBROUTINE Data_Finial

    SUBROUTINE IsInDmArea_ustr_Renew(ustr_points, ustr_bounds, ustr_ngr_vertex, ustr_ngr_vertex_new, n_ustr_ngr_vertex, n_ustr_ngr_vertex_new, ustr_points_new)

        implicit none
        integer, intent(in) :: ustr_points, ustr_bounds
        integer,  allocatable, intent(in) :: ustr_ngr_vertex(:,:), ustr_ngr_vertex_new(:,:)
        integer,  allocatable, intent(in) :: n_ustr_ngr_vertex(:), n_ustr_ngr_vertex_new(:)
        logical :: fexist
        integer :: i, j, k, num_diff
        integer,  allocatable :: n_ustr_ngr(:) ! 用于统计三角形顶点是否都是陆地顶点
        integer, intent(inout) :: ustr_points_new

        !!!!!!!!!!!!!!!!!!!!!!!!!! 删除 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        ! n_ustr_ngr用于说明三角形顶点是海洋顶点还是陆地顶点
        ! 如果该顶点（即该六边形是存在而且不完整，则认为是陆地顶点，海洋顶点必完整）
        allocate(n_ustr_ngr(ustr_points)); n_ustr_ngr = 3
        do i = 2, ustr_bounds, 1
            if (n_ustr_ngr_vertex_new(i) == 0) cycle ! 完整的陆地六边形，跳过
            if (n_ustr_ngr_vertex_new(i) == n_ustr_ngr_vertex(i)) cycle ! 完整的海洋六边形
            do j = 1, n_ustr_ngr_vertex_new(i), 1 
                n_ustr_ngr(ustr_ngr_vertex_new(j, i)) =  n_ustr_ngr(ustr_ngr_vertex_new(j, i)) + 1
            end do
        end do
        do i = 2, ustr_points, 1
            if (IsInDmArea_ustr(i) /= 1) cycle
            if (n_ustr_ngr(i) == 6) then ! refer to FVCOM
                IsInDmArea_ustr(i) = -1 ! 三个顶点都是固边界就认为是陆地三角形
                ustr_points_new = ustr_points_new - 1 ! 减少三角形
            end if
        end do
        deallocate(n_ustr_ngr)
        !!!!!!!!!!!!!!!!!!!!!!!!!! 删除 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

       !!!!!!!!!!!!!!!!!!!!!!!! 回填 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        do i = 2, ustr_bounds, 1
            num_diff = n_ustr_ngr_vertex(i) - n_ustr_ngr_vertex_new(i)
            ! 如果六边形有且只有一个三角形陆地三角形，回填
            if (num_diff == 1) then 
                do j = 1, n_ustr_ngr_vertex(i), 1
                    IsInDmArea_ustr(ustr_ngr_vertex(j, i)) = 1
                end do
                ustr_points_new = ustr_points_new + 1 ! 增加三角形
            end if
        end do

    END SUBROUTINE IsInDmArea_ustr_Renew

    SUBROUTINE IsInDmArea_ustr_Renew_v2(ustr_bounds, ustr_ngr_vertex, n_ustr_ngr_vertex, n_ustr_ngr_vertex_new, ustr_points_new)

        implicit none
        integer, intent(in) :: ustr_bounds
        integer,  allocatable, intent(in) :: ustr_ngr_vertex(:,:)
        integer,  allocatable, intent(in) :: n_ustr_ngr_vertex(:), n_ustr_ngr_vertex_new(:)
        logical :: fexist
        integer :: i, j, k, num_diff
        integer, intent(inout) :: ustr_points_new

        do i = 2, ustr_bounds, 1
            num_diff = n_ustr_ngr_vertex(i) - n_ustr_ngr_vertex_new(i)
            ! 回填2 两个非海洋角形是相对位置的
            if (num_diff /= 2) cycle
            do j = 1, n_ustr_ngr_vertex(i) - 3, 1
                if ((IsInDmArea_ustr(ustr_ngr_vertex(j, i)) /= 1) .and. &
                    (IsInDmArea_ustr(ustr_ngr_vertex(j + 3, i)) /= 1)) then
                    IsInDmArea_ustr(ustr_ngr_vertex(j, i)) = 1
                    IsInDmArea_ustr(ustr_ngr_vertex(j + 3, i)) = 1
                    ustr_points_new = ustr_points_new + 2 ! 增加三角形
                end if
            end do
        end do

    END SUBROUTINE IsInDmArea_ustr_Renew_v2

    SUBROUTINE bdy_calculation(num_bdy_long, bdy_long_order, ustr_ngr_vertex, n_ustr_ngr_vertex, vertex_mapping)

        implicit none
        integer,  intent(in) :: num_bdy_long(3)
        integer,  allocatable, intent(in) :: bdy_long_order(:) ! 自身有一定的顺序
        integer,  allocatable, intent(in) :: ustr_ngr_vertex(:, :)
        integer,  allocatable, intent(in) :: n_ustr_ngr_vertex(:)
        integer,  allocatable, intent(in) :: vertex_mapping(:)
        logical :: fexist
        integer :: i, j, k, num
        integer :: bdy_num
        integer :: ncid, dimID_bdy, varid(3)
        integer,  allocatable :: bdy_order(:), obc_order(:), ibc_order(:), ref_temp(:)
        character(pathlen) :: lndname


        ! 就是对bdy_order进行操作，具体就是这个六边形对应的若干个三角形，该三角形中是否都在DmArea内部
        bdy_num = num_bdy_long(1)
        allocate(bdy_order(bdy_num)); bdy_order = bdy_long_order
        allocate(obc_order(bdy_num)); obc_order = 1 
        allocate(ibc_order(bdy_num)); ibc_order = 1
        ! 这部分应该放在外面
        do i = 2, bdy_num, 1
            j = bdy_long_order(i) ! 获取对应的vertex编号
            fexist = .true.
            do k = 1, n_ustr_ngr_vertex(j), 1
                if (IsInDmArea_ustr(ustr_ngr_vertex(k, j)) == -1) then ! 如果存在陆地，则这个vertex点位是ibc
                    fexist = .false.
                    exit
                end if
            end do

            bdy_order(i) = vertex_mapping(bdy_order(i))
            if (.not. fexist) then
                obc_order(i) = 1 ! 表示不存在
                ibc_order(i) = bdy_order(i)
            else 
                obc_order(i) = bdy_order(i)
                ibc_order(i) = 1
            end if
        end do

        ! 还要涉及到顺序问题，做完这个才算大功告成！
        do i = 3, bdy_num-1, 1
            if (obc_order(i) /= 1) then
                if ((obc_order(i-1) == 1) .and. (obc_order(i+1) == 1)) then
                    j = bdy_long_order(i)
                    write(io6, *), "j = ", j
                    do k = 1, n_ustr_ngr_vertex(j), 1
                        write(io6, *), "ustr_ngr_vertex(k, j) = ", ustr_ngr_vertex(k, j) 
                        write(io6, *), "IsInDmArea_ustr(ustr_ngr_vertex(k, j)) = ", IsInDmArea_ustr(ustr_ngr_vertex(k, j))
                    end do
                    write(io6, *), ""
                    ! turn obc to ibc
                    ibc_order(i) = obc_order(i)
                    obc_order(i) = 1
                end if
            end if
        end do
        deallocate(IsInDmArea_ustr)

        num = 1
        allocate(ref_temp(bdy_num)); ref_temp = 1
        do i = 2, bdy_num - 2, 1
            if (obc_order(i) /= 1) cycle ! 跳过obc点位
            if ((obc_order(i+1) /= 1) .and. (obc_order(i+2) == 1)) then    
                num = i
                write(io6, *), "num = ", num, "需要调整顺序"
                ! bdy_order
                ref_temp(2:num) = bdy_order(2:num)
                ref_temp(num+1:bdy_num) = bdy_order(num+1:bdy_num)
                do j = num + 1, bdy_num, 1
                    bdy_order(j-num+1) = ref_temp(j)
                end do
                do j = 2, num, 1
                    bdy_order(1+bdy_num-j+1) = ref_temp(j)
                end do

                ! obc_order
                ref_temp(2:num) = obc_order(2:num)
                ref_temp(num+1:bdy_num) = obc_order(num+1:bdy_num)
                do j = num + 1, bdy_num, 1
                    obc_order(j-num+1) = ref_temp(j)
                end do
                do j = 2, num, 1
                    obc_order(1+bdy_num-j+1) = ref_temp(j)
                end do

                ! ibc_order
                ref_temp(2:num) = ibc_order(2:num)
                ref_temp(num+1:bdy_num) = ibc_order(num+1:bdy_num)
                do j = num + 1, bdy_num, 1
                    ibc_order(j-num+1) = ref_temp(j)
                end do
                do j = 2, num, 1
                    ibc_order(1+bdy_num-j+1) = ref_temp(j)
                end do
                exit
            end if
        end do
        if (num == 1) write(io6, *), "无需调整顺序"
            
        
        lndname = trim(file_dir) // 'result/obc.nc4'
        if (mask_patch_on) lndname = trim(file_dir) // 'result/obc_patch.nc4'
        write(io6, *), trim(lndname)
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncid, "bdy_num", bdy_num, DimID_bdy))
        CALL CHECK(NF90_DEF_VAR(ncid, "bdy_order", NF90_INT, (/ DimID_bdy /), varid(1)))
        CALL CHECK(NF90_DEF_VAR(ncid, "obc_order", NF90_INT, (/ DimID_bdy /), varid(2)))
        CALL CHECK(NF90_DEF_VAR(ncid, "ibc_order", NF90_INT, (/ DimID_bdy /), varid(3)))
        CALL CHECK(NF90_ENDDEF(ncid))
        CALL CHECK(NF90_PUT_VAR(ncid, varid(1), bdy_order))
        CALL CHECK(NF90_PUT_VAR(ncid, varid(2), obc_order))
        CALL CHECK(NF90_PUT_VAR(ncid, varid(3), ibc_order))
        CALL CHECK(NF90_CLOSE(ncid))
        deallocate(bdy_order, obc_order, ibc_order)
        write(io6, *), ""

    END SUBROUTINE bdy_calculation

    SUBROUTINE extract_unique_vertices(ustr_ngr_center_f, n_ustr_ngr_center_f, unique_vertices, nvertices)

        implicit none
        integer, dimension(:, :), intent(in) :: ustr_ngr_center_f
        integer, dimension(:),    intent(in) :: n_ustr_ngr_center_f
        integer, dimension(:), intent(out) :: unique_vertices
        integer, allocatable :: isselect(:)
        integer, intent(out) :: nvertices
        integer :: i, j, k
        allocate(isselect(size(unique_vertices))); isselect = 1
        unique_vertices = -1  ! 初始化为-1
        unique_vertices(1) = 1 ! 1表示为空值
        nvertices = 1
        do j = 2, size(ustr_ngr_center_f, 2), 1
            do i = 1, n_ustr_ngr_center_f(j), 1
                if (isselect(ustr_ngr_center_f(i, j))) then
                    nvertices = nvertices + 1
                    unique_vertices(nvertices) = ustr_ngr_center_f(i, j)
                    isselect(ustr_ngr_center_f(i, j)) = 0
                end if
            end do
        end do
        deallocate(isselect)
        write(io6, *), "nvertices = ", nvertices

    END SUBROUTINE extract_unique_vertices

    ! 排序并重新编号顶点的子程序
    SUBROUTINE sort_and_reindex(unique_vertices, nvertices, sorted_vertices, vertex_mapping)

        integer, dimension(:), intent(in) :: unique_vertices
        integer, intent(in) :: nvertices
        integer, dimension(:), intent(out) :: sorted_vertices, vertex_mapping
        integer :: i, j, temp

        ! 排序唯一顶点
        write(io6, *), "sorted start"
        sorted_vertices = unique_vertices(1:nvertices)
        CALL quicksort_nonrecursive(sorted_vertices, 1, nvertices)
        write(io6, *), "sorted finish"

        ! 为排序后的顶点重新编号
        write(io6, *), "mapping start"
        vertex_mapping = 0
        do i = 1, nvertices, 1
            vertex_mapping(sorted_vertices(i)) = i
        end do
        write(io6, *), "mapping finish"
    
    END SUBROUTINE sort_and_reindex

    subroutine quicksort_nonrecursive(arr, low, high)
        integer, intent(inout) :: arr(:)
        integer, intent(in) :: low, high
        integer :: stack(high - low + 1), top, pivot_index, l, h, temp
        ! real :: temp

        ! 初始化栈
        top = 1
        stack(top) = low
        top = top + 1
        stack(top) = high

        do while (top > 0)
            ! 弹出栈顶元素
            h = stack(top)
            top = top - 1
            l = stack(top)
            top = top - 1

            ! 分区操作
            pivot_index = partition(arr, l, h)

            ! 将左半部分压入栈
            if (pivot_index - 1 > l) then
                top = top + 1
                stack(top) = l
                top = top + 1
                stack(top) = pivot_index - 1
            end if

            ! 将右半部分压入栈
            if (pivot_index + 1 < h) then
                top = top + 1
                stack(top) = pivot_index + 1
                top = top + 1
                stack(top) = h
            end if
        end do
    
    end subroutine quicksort_nonrecursive

       ! 分区函数
    integer function partition(arr, low, high)
        integer, intent(inout) :: arr(:)
        integer, intent(in) :: low, high
        ! real :: pivot, temp
        integer :: i, j,  pivot, temp

        pivot = arr(high)
        i = low - 1

        do j = low, high - 1
            if (arr(j) <= pivot) then
                i = i + 1
                ! 交换 arr(i) 和 arr(j)
                temp = arr(i)
                arr(i) = arr(j)
                arr(j) = temp
            end if
        end do

        ! 交换 arr(i+1) 和 arr(high)
        temp = arr(i + 1)
        arr(i + 1) = arr(high)
        arr(high) = temp

        partition = i + 1
    
    end function partition

END MODULE MOD_mask_postproc 
