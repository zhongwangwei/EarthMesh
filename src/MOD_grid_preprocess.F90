Module MOD_grid_preprocess
    USE NETCDF
    USE consts_coms
    USE refine_vars
    USE MOD_file_preprocess, only : quality_save_global
    implicit none
    Contains

    SUBROUTINE Grid_Quality_Check_Global(inputfile, num_sjx, num_dbx, mp, wp, ngrmw, ngrwm, n_ngrwm)
        USE consts_coms, only : impent
        IMPLICIT NONE
        ! Extr_*** 指网格的最小角度与最大角度
        ! Eavg_*** 指网格的最小角度与最大角度平均值
        ! Savg_*** 指网格角度与正多边形角度的标准差
        character(LEN = 256) :: inputfile
        integer,  intent(in) :: num_sjx, num_dbx
        integer,  allocatable, intent(in) :: ngrmw(:, :), ngrwm(:, :), n_ngrwm(:)
        real(r8), allocatable, intent(in) :: mp(:, :), wp(:, :)
        character(LEN = 5) :: stepc, nxpc
        integer :: i, j, k
        integer :: mk, ik, wj, n
        integer :: num_wbx, num_lbx, num_qbx, num_less, num_more, temp
        real(r8) :: sjx(3,2)
        real :: centroid_deg(2)
        real(r8) :: Extr_sjx(2), Eavg_sjx(2), Savg_sjx
        real(r8) :: Extr_wbx(2), Eavg_wbx(2), Savg_wbx
        real(r8) :: Extr_lbx(2), Eavg_lbx(2), Savg_lbx
        real(r8) :: Extr_qbx(2), Eavg_qbx(2), Savg_qbx
        integer,  allocatable :: adjust_sjx_flag(:), adjust_dbx_flag(:)
        integer,  allocatable :: less_sjx(:), more_sjx(:), less_wbx(:), more_wbx(:), less_lbx(:), more_lbx(:), less_qbx(:), more_qbx(:)
        real(r8), allocatable :: length_sjx(:, :), length_wbx(:, :), length_lbx(:, :), length_qbx(:, :)
        real(r8), allocatable :: angle_sjx(:, :), angle_wbx(:, :), angle_lbx(:, :), angle_qbx(:, :)
        ! real(r8), allocatable :: centroid_spherical(:,:), circumcenter_spherical(:,:)
        write(stepc, '(I2.2)') step
        write(nxpc,  '(I4.4)') nxp

        ! obtain num_wbx, num_lbx, num_qbx, num_less, num_more
        num_wbx = 0; num_lbx = 0; num_qbx = 0; num_less = 0; num_more = 0 
        do i = 2, num_dbx, 1
            if (n_ngrwm(i) == 5) then
                num_wbx = num_wbx + 1
            else if (n_ngrwm(i) == 6) then
                num_lbx = num_lbx + 1
            else if (n_ngrwm(i) == 7) then
                num_qbx = num_qbx + 1
            else
                if (n_ngrwm(i)<5) then
                    num_less = num_less + 1
                else 
                    num_more = num_more + 1
                end if
            end if
        end do
        write(io6, *) "num_wbx = ", num_wbx
        write(io6, *) "num_lbx = ", num_lbx
        write(io6, *) "num_qbx = ", num_qbx 
        if (num_dbx-1 /= num_wbx + num_lbx + num_qbx) then
            write(io6, *) "存在小于五边形的多边形的个数 num_less = ", num_less
            write(io6, *) "存在大于七边形的多边形的个数 num_more = ", num_more
        end if
        write(io6, *) " "

        if ((num_wbx == 12) .and. (impent(1) == 0)) then
            write(io6, *) "impent save start" 
            num_wbx = 0
            do i = 2, num_dbx, 1
                if (n_ngrwm(i) == 5) then
                    num_wbx = num_wbx + 1
                    impent(num_wbx) = i
                end if
            end do
            write(io6, *) " "
        end if    

        allocate(length_sjx(num_sjx, 3));          length_sjx(:, :) = 0. ! for sjx
        allocate(angle_sjx(num_sjx, 3));           angle_sjx(:, :)  = 0. ! for sjx
        allocate(less_sjx(num_sjx));               less_sjx = 0
        allocate(more_sjx(num_sjx));               more_sjx = 0
        Extr_sjx = 0.; Eavg_sjx = 0.; Savg_sjx = 0.;

        allocate(length_wbx(num_wbx, 5));          length_wbx(:, :) = 0. ! for wbx
        allocate(angle_wbx(num_wbx, 5));           angle_wbx(:, :)  = 0. ! for wbx
        allocate(less_wbx(num_wbx));               less_wbx = 0
        allocate(more_wbx(num_wbx));               more_wbx = 0
        Extr_wbx = 0.; Eavg_wbx = 0.; Savg_wbx = 0.;

        allocate(length_lbx(num_lbx, 6));          length_lbx(:, :) = 0. ! for lbx
        allocate(angle_lbx(num_lbx, 6));           angle_lbx(:, :)  = 0. ! for lbx
        allocate(less_lbx(num_lbx));               less_lbx = 0
        allocate(more_lbx(num_lbx));               more_lbx = 0
        Extr_lbx = 0.; Eavg_lbx = 0.; Savg_lbx = 0.;

        if (num_qbx /= 0) then
            allocate(length_qbx(num_qbx, 7));          length_qbx(:, :) = 0. ! for qbx
            allocate(angle_qbx(num_qbx, 7));           angle_qbx(:, :)  = 0. ! for qbx
            allocate(less_qbx(num_qbx));               less_qbx = 0
            allocate(more_qbx(num_qbx));               more_qbx = 0
            Extr_qbx = 0.; Eavg_qbx = 0.; Savg_qbx = 0.;
        end if

        allocate(adjust_sjx_flag(num_sjx));  adjust_sjx_flag = 1 ! 三角形初始化的时候都需要使用，不可跳过
        allocate(adjust_dbx_flag(num_dbx));  adjust_dbx_flag = 1 ! 多边形初始化的时候都需要使用，不可跳过=
        Call TriMeshQuality(    num_sjx, wp, ngrmw,          adjust_sjx_flag, length_sjx, angle_sjx, Extr_sjx, Eavg_sjx, Savg_sjx, less_sjx, more_sjx)! 三角形网格质量
        Call PolyMeshQuality(5, num_dbx, mp, ngrwm, n_ngrwm, adjust_dbx_flag, length_wbx, angle_wbx, Extr_wbx, Eavg_wbx, Savg_wbx, less_wbx, more_wbx)! 五边形网格质量
        Call PolyMeshQuality(6, num_dbx, mp, ngrwm, n_ngrwm, adjust_dbx_flag, length_lbx, angle_lbx, Extr_lbx, Eavg_lbx, Savg_lbx, less_lbx, more_lbx)! 六边形网格质量
        if (num_qbx /= 0) CALL PolyMeshQuality(7, num_dbx, mp, ngrwm, n_ngrwm, adjust_dbx_flag, length_qbx, angle_qbx, Extr_qbx, Eavg_qbx, Savg_qbx, less_qbx, more_qbx)! 七边形网格质量

        write(io6, *) "三角形边长范围：", minval(length_sjx(2:num_sjx, :)), maxval(length_sjx(2:num_sjx, :))
        write(io6, *) "五边形边长范围：", minval(length_wbx), maxval(length_wbx)
        write(io6, *) "六边形边长范围：", minval(length_lbx), maxval(length_lbx)
        if (num_qbx /= 0) write(io6, *) "七边形边长范围：", minval(length_qbx), maxval(length_qbx)
        write(io6, *) ""

        write(io6, *) "三角形角度范围：", Extr_sjx(1), Extr_sjx(2)
        write(io6, *) "五边形角度范围：", Extr_wbx(1), Extr_wbx(2)
        write(io6, *) "六边形角度范围：", Extr_lbx(1), Extr_lbx(2)
        if (num_qbx /= 0) write(io6, *) "七边形角度范围：", Extr_qbx(1), Extr_qbx(2)
        write(io6, *) ""

        write(io6, *) inputfile
        CALL quality_save_global(inputfile, num_sjx, num_wbx, num_lbx, num_qbx, & 
            length_sjx, angle_sjx, Extr_sjx, Eavg_sjx, Savg_sjx, less_sjx, more_sjx, & 
            length_wbx, angle_wbx, Extr_wbx, Eavg_wbx, Savg_wbx, less_wbx, more_wbx, & 
            length_lbx, angle_lbx, Extr_lbx, Eavg_lbx, Savg_lbx, less_lbx, more_lbx, &
            length_qbx, angle_qbx, Extr_lbx, Eavg_qbx, Savg_qbx, less_qbx, more_qbx)

        deallocate(adjust_sjx_flag, adjust_dbx_flag)
        deallocate(length_sjx, length_wbx, length_lbx)
        deallocate(angle_sjx, angle_wbx, angle_lbx)
        deallocate(less_sjx, more_sjx, less_wbx, more_wbx, less_lbx, more_lbx)
        if (num_qbx /= 0) deallocate(length_qbx, angle_qbx, less_qbx, more_qbx)

    END SUBROUTINE Grid_Quality_Check_Global

    ! 适用于全局网格平衡长度已知（无论均一还是非均一）
    SUBROUTINE Springjustment_global(num_sjx, num_dbx, num_edge, mp, wp, ngrmw, ngrwm, n_ngrwm)

        USE NETCDF
        USE consts_coms, only : r8, nxp, step, io6
        USE MOD_file_preprocess, only : distsOnEdge_save, cellwidth_save
        IMPLICIT NONE
        integer, intent(in) :: num_sjx, num_dbx, num_edge
        integer,  allocatable, intent(in) :: ngrmw(:, :), ngrwm(:, :), n_ngrwm(:)
        real(r8), allocatable, intent(inout) :: mp(:,:), wp(:, :)
        real(r8), allocatable :: vp(:,:)
        integer :: i
        real(r8) :: centroid(3), centroid_deg(2), circum(3), circum_deg(2)
        integer,  allocatable :: ngrmm(:, :)
        integer,  allocatable :: edgesOnVertex(:,:), cellsOnVertex(:,:)
        integer,  allocatable :: verticesOnCell(:,:), edgesOnCell(:,:)
        integer,  allocatable :: nEdgesOnCell(:)
        integer,  allocatable :: CellsOnEdge(:,:), verticesOnEdge(:,:)
        integer,  allocatable :: EdgesOnedge_tri(:,:)
        real(r8), allocatable :: xem8(:), yem8(:), zem8(:)
        real(r8), allocatable :: xew8(:), yew8(:), zew8(:)
        real(r8), allocatable :: xev8(:), yev8(:), zev8(:)
        real(r8), allocatable :: distsOnEdge(:), cellwidth(:)
        character(LEN = 256) :: lndname
        character(LEN = 5) :: stepc, nxpc

        write(stepc, '(I2.2)') step
        write(nxpc,  '(I4.4)') nxp

        ! 构建ngrmm数组
        allocate(ngrmm(3, num_sjx)); ngrmm = 1   ! 反映三角形的相邻三角形的邻域关系(0表示没有相邻三角形)
        CALL set_ngrmm(num_sjx, ngrmw, ngrwm, n_ngrwm, ngrmm)

        allocate(cellsOnVertex(3, num_sjx)); cellsOnVertex = ngrmw ! 获取cellsOnVertex

        ! DEBUG: Print cellsOnVertex for vertex 81458 BEFORE orderVertexArrays
        if (81458 >= 2 .and. 81458 <= num_sjx) then
            write(io6,*) "=== DEBUG: Vertex 81458 BEFORE orderVertexArrays ==="
            write(io6,*) "cellsOnVertex (from ngrmw):", cellsOnVertex(:, 81458)
        end if

        ! 获取在Edge相关的变量cellsOnEdge, verticesOnEdge, edgesOnVertex
        allocate(cellsOnEdge(2, num_edge)); cellsOnEdge = 1
        allocate(verticesOnEdge(2, num_edge)); verticesOnEdge = 1
        allocate(edgesOnVertex(3, num_sjx)); edgesOnVertex = 1
        allocate(vp(num_edge, 2)); vp = 0.0_r8
        CALL GetEdge(num_sjx, wp, ngrmm, cellsOnVertex, cellsOnEdge, verticesOnEdge, edgesOnVertex, vp)

        ! verticesOnEdge排序
        CALL GetSort_verticesOnEdge(num_edge, mp, wp, cellsOnEdge, verticesOnEdge) ! 对verticesOnEdge

        ! 利用edgesOnVertex获取nEdgesOnCell, edgesOnCell, verticesOnCell
        allocate(nEdgesOnCell(num_dbx)); nEdgesOnCell = n_ngrwm
        allocate(verticesOnCell(7, num_dbx)); verticesOnCell = ngrwm
        allocate(edgesOnCell(7, num_dbx)); edgesOnCell = 1
        CALL Get_ConnectOnCell(num_dbx, nEdgesOnCell, cellsOnEdge, edgesOnVertex, verticesOnCell, edgesOnCell)


        ! 获取笛卡尔坐标下m/w的坐标
        allocate(xew8(num_dbx)); xew8 = 0.0_r8 ! real(xew, r8)
        allocate(yew8(num_dbx)); yew8 = 0.0_r8 ! real(yew, r8)
        allocate(zew8(num_dbx)); zew8 = 0.0_r8 ! real(zew, r8)
        CALL lonlat2xyz(num_dbx, wp(1:num_dbx, 1), wp(1:num_dbx, 2), xew8, yew8, zew8)
        xew8 = xew8 * erad8; yew8 = yew8 * erad8; zew8 = zew8 * erad8 ! 因为原本lonlat2xyz没有乘以erad8

        ! 获取对偶三角形非公共边的edge编号
        allocate(EdgesOnedge_tri(4, num_edge)); EdgesOnedge_tri = 1
        CALL set_edgesOnEdge_tri(num_sjx, num_edge, ngrmm, verticesOnEdge, edgesOnVertex, edgesOnEdge_tri)


        ! 设置边的平衡长度是多少
        allocate(distsOnEdge(num_edge)); distsOnEdge = 0.0_r8       
        if ((output_format /= 'MPAS') .and. (output_format /= 'MPAS-Simple')) then
            CALL set_distsOnEdge_global(num_sjx, num_dbx, num_edge, ngrwm, n_ngrwm, CellsOnEdge, edgesOnVertex, mp, distsOnEdge)
        else
            ! 设置cellwidth
            allocate(cellwidth(num_dbx)); cellwidth = 0.0_r8
            CALL set_distsOnEdge_global(num_sjx, num_dbx, num_edge, ngrwm, n_ngrwm, CellsOnEdge, edgesOnVertex, mp, distsOnEdge, ngrmw, cellwidth)
            lndname = trim(file_dir) // "result/cellwidth_NXP" // trim(nxpc) // "_global.nc4"
            write(io6, *) lndname
            CALL cellwidth_save(lndname, num_dbx, wp, cellwidth)
        end if      
        
        lndname = trim(file_dir) // "result/distsOnEdge_NXP" // trim(nxpc) // '_' // trim(stepc) // "_global.nc4"
        write(io6, *) lndname
        CALL distsOnEdge_save(lndname, num_edge, vp, distsOnEdge)


        ! 全局弹性调整，调整w点位置
        CALL spring_dynamics_global(step, num_dbx, num_edge, nEdgesOnCell, edgesOnCell, CellsOnEdge, distsOnEdge, EdgesOnedge_tri, xew8, yew8, zew8)

        ! 从笛卡尔坐标调整为经纬度坐标
        wp = 0.
        do i = 2, num_dbx, 1
            centroid(1) = xew8(i); centroid(2) = yew8(i); centroid(3) = zew8(i); 
            call xyz2lonlat(centroid, centroid_deg)
            wp(i, :) = centroid_deg
        end do

        ! 基于w点获取球面m点的重心经纬度坐标
        mp = 0.
        CALL centroid_spherical_calculation(num_sjx, mp, wp, ngrmw)

        ! 计算三角形的球面外心经纬度坐标（基于球面重心经纬度坐标）
        allocate(xem8(num_sjx)); xem8 = 0.0_r8
        allocate(yem8(num_sjx)); yem8 = 0.0_r8
        allocate(zem8(num_sjx)); zem8 = 0.0_r8
        CALL lonlat2xyz(num_sjx, mp(1:num_sjx, 1), mp(1:num_sjx, 2), xem8, yem8, zem8)
        xem8 = xem8 * erad8; yem8 = yem8 * erad8; zem8 = zem8 * erad8 ! 因为原本lonlat2xyz没有乘以erad8
        CALL circumcenter_spherical_calculation(num_sjx, ngrmw, xem8, yem8, zem8, xew8, yew8, zew8)
        do i = 2, num_sjx, 1
            circum(1) = xem8(i); circum(2) = yem8(i); circum(3) = zem8(i);
            CALL xyz2lonlat(circum, circum_deg)
            mp(i, :) = circum_deg
        end do

        ! 获取边的笛卡尔坐标（从vp转换）
        allocate(xev8(num_edge)); xev8 = 0.0_r8
        allocate(yev8(num_edge)); yev8 = 0.0_r8
        allocate(zev8(num_edge)); zev8 = 0.0_r8
        CALL lonlat2xyz(num_edge, vp(1:num_edge, 1), vp(1:num_edge, 2), xev8, yev8, zev8)
        xev8 = xev8 * erad8; yev8 = yev8 * erad8; zev8 = zev8 * erad8

        ! Order cellsOnVertex and edgesOnVertex counter-clockwise (MPAS-Tools compatible)
        CALL orderVertexArrays(num_sjx, xem8, yem8, zem8, xev8, yev8, zev8, &
                                verticesOnEdge, cellsOnEdge, edgesOnVertex, cellsOnVertex)

        ! DEBUG: Print cellsOnVertex for vertex 81458 AFTER orderVertexArrays
        if (81458 >= 2 .and. 81458 <= num_sjx) then
            write(io6,*) "=== DEBUG: Vertex 81458 AFTER orderVertexArrays ==="
            write(io6,*) "cellsOnVertex:", cellsOnVertex(:, 81458)
            write(io6,*) "edgesOnVertex:", edgesOnVertex(:, 81458)
        end if

        deallocate(ngrmm)
        deallocate(nEdgesOnCell, edgesOnCell, verticesOnCell)
        deallocate(cellsOnVertex)
        deallocate(CellsOnEdge, verticesOnEdge)
        deallocate(distsOnEdge)
        deallocate(xem8, yem8, zem8, xew8, yew8, zew8, xev8, yev8, zev8, vp)
        deallocate(EdgesOnedge_tri)

    END SUBROUTINE Springjustment_global

    SUBROUTINE Springjustment_regional_step(set_dis, num_sjx_ref, num_sjx, num_dbx, num_edge, mp, wp, ngrmw, ngrwm, n_ngrwm)

        USE NETCDF
        USE consts_coms , only : r8, nxp, step, io6
        IMPLICIT NONE
        integer, intent(in) :: set_dis, num_sjx_ref
        integer, intent(in) :: num_sjx, num_dbx, num_edge
        integer,  allocatable, intent(in) :: ngrmw(:, :), ngrwm(:, :), n_ngrwm(:)
        real(r8), allocatable, intent(inout) :: mp(:,:), wp(:, :)
        integer  :: i
        real(r8) :: centroid(3), centroid_deg(2), circum(3), circum_deg(2)
        integer,  allocatable :: ngrmm(:, :)
        integer,  allocatable :: edgesOnVertex(:,:), cellsOnVertex(:,:)
        integer,  allocatable :: verticesOnCell(:,:), edgesOnCell(:,:)
        integer,  allocatable :: nEdgesOnCell(:)
        integer,  allocatable :: CellsOnEdge(:,:), verticesOnEdge(:,:)
        integer,  allocatable :: cellsOnCell(:,:), IsdbxMove(:)
        integer,  allocatable :: IsEdgesMove(:), EdgesOnedge_tri(:,:)
        real(r8), allocatable :: xem8(:), yem8(:), zem8(:)
        real(r8), allocatable :: xew8(:), yew8(:), zew8(:)

        ! 构建ngrmm数组
        allocate(ngrmm(3, num_sjx)); ngrmm = 1   ! 反映三角形的相邻三角形的邻域关系(0表示没有相邻三角形)
        CALL set_ngrmm(num_sjx, ngrmw, ngrwm, n_ngrwm, ngrmm)

        allocate(cellsOnVertex(3, num_sjx)); cellsOnVertex = ngrmw ! 获取cellsOnVertex
        ! 获取在Edge相关的变量cellsOnEdge, verticesOnEdge, edgesOnVertex
        allocate(cellsOnEdge(2, num_edge)); cellsOnEdge = 1
        allocate(verticesOnEdge(2, num_edge)); verticesOnEdge = 1
        allocate(edgesOnVertex(3, num_sjx)); edgesOnVertex = 1
        CALL GetEdge(num_sjx, wp, ngrmm, cellsOnVertex, cellsOnEdge, verticesOnEdge, edgesOnVertex)

        ! verticesOnEdge排序
        CALL GetSort_verticesOnEdge(num_edge, mp, wp, cellsOnEdge, verticesOnEdge) ! 对verticesOnEdge

        ! NOTE: orderVertexArrays not called here as coordinates (xem8, xev, etc.) not yet computed
        ! This subroutine (Springjustment_regional_step) may not need CCW ordering

        ! 获取nEdgesOnCell, edgesOnCell, verticesOnCell
        allocate(nEdgesOnCell(num_dbx)); nEdgesOnCell = n_ngrwm
        allocate(verticesOnCell(7, num_dbx)); verticesOnCell = ngrwm
        allocate(edgesOnCell(7, num_dbx)); edgesOnCell = 1
        allocate(cellsOnCell(7, num_dbx)); cellsOnCell = 1
        CALL Get_ConnectOnCell(num_dbx, nEdgesOnCell, cellsOnEdge, edgesOnVertex, verticesOnCell, edgesOnCell, cellsOnCell)
        
        ! 获取笛卡尔坐标下m/w的坐标
        allocate(xew8(num_dbx)); xew8 = 0.0_r8 ! real(xew, r8)
        allocate(yew8(num_dbx)); yew8 = 0.0_r8 ! real(yew, r8)
        allocate(zew8(num_dbx)); zew8 = 0.0_r8 ! real(zew, r8)
        CALL lonlat2xyz(num_dbx, wp(1:num_dbx, 1), wp(1:num_dbx, 2), xew8, yew8, zew8)
        xew8 = xew8 * erad8; yew8 = yew8 * erad8; zew8 = zew8 * erad8 ! 因为原本lonlat2xyz没有乘以erad8

        ! 确定那些顶点可以移动（并对部分顶点进行保护，不允许移动），每次都要重新标记
        if (num_sjx_ref /= 0) then
            CALL set_dbxMove_regional_step(set_dis, num_sjx_ref, num_sjx, num_dbx, ngrmw, ngrwm, n_ngrwm, IsdbxMove)
        else
            CALL set_dbxMove_regional_step(set_dis, num_sjx_ref, num_sjx, num_dbx, ngrmw, ngrwm, n_ngrwm, IsdbxMove, mp)
        end if

        ! 局弹性调整，调整w点位置
        CALL spring_dynamics_regionalv2(step, num_dbx, nEdgesOnCell, cellsOnCell, IsdbxMove, xew8, yew8, zew8)
        
        ! 从笛卡尔坐标调整为经纬度坐标
        wp = 0.
        do i = 2, num_dbx, 1
            centroid(1) = xew8(i); centroid(2) = yew8(i); centroid(3) = zew8(i); 
            call xyz2lonlat(centroid, centroid_deg)
            wp(i, :) = centroid_deg
        end do

        ! 基于w点获取球面m点的重心经纬度坐标
        mp = 0.
        CALL centroid_spherical_calculation(num_sjx, mp, wp, ngrmw)

        ! 计算三角形的球面外心经纬度坐标（基于球面重心经纬度坐标）
        allocate(xem8(num_sjx)); xem8 = 0.0_r8
        allocate(yem8(num_sjx)); yem8 = 0.0_r8
        allocate(zem8(num_sjx)); zem8 = 0.0_r8
        CALL lonlat2xyz(num_sjx, mp(1:num_sjx, 1), mp(1:num_sjx, 2), xem8, yem8, zem8)
        xem8 = xem8 * erad8; yem8 = yem8 * erad8; zem8 = zem8 * erad8 ! 因为原本lonlat2xyz没有乘以erad8
        CALL circumcenter_spherical_calculation(num_sjx, ngrmw, xem8, yem8, zem8, xew8, yew8, zew8)
        do i = 2, num_sjx, 1
            circum(1) = xem8(i); circum(2) = yem8(i); circum(3) = zem8(i); 
            CALL xyz2lonlat(circum, circum_deg)
            mp(i, :) = circum_deg
        end do

        deallocate(ngrmm, IsdbxMove)
        deallocate(nEdgesOnCell, edgesOnCell, verticesOnCell, cellsOnCell)
        deallocate(cellsOnVertex, CellsOnEdge, verticesOnEdge)
        deallocate(xem8, yem8, zem8, xew8, yew8, zew8)

    END SUBROUTINE Springjustment_regional_step

    SUBROUTINE find_frac_index(n, grid, point, frac, index)

        IMPLICIT NONE
        integer, intent(in) :: n
        real(r8), intent(in) :: grid(:)
        real(r8), intent(in) :: point
        real(r8), intent(out) :: frac
        integer, intent(out) :: index
        integer :: i
        real(r8) :: dx
        ! 会不会出现跨越180经线的情况呢？？？？是的，不会出现
        if (grid(1) < grid(n)) then  ! 适用于-180到180
            do i = 1, n-1
                if (point >= grid(i) .and. point <= grid(i+1)) then
                    index = i
                    exit
                end if
            end do
        else  ! 适用于90到-90
            do i = 1, n-1
                if (point <= grid(i) .and. point >= grid(i+1)) then
                    index = i
                    exit
                end if
            end do
        end if
        dx = grid(i+1) - grid(i) ! dx > 0
        frac = (point - grid(i)) / dx
        frac = min(1.0_r8, max(0.0_r8, frac))
        return
        ! Shouldn't get here if bounds checking passed
        write(io6, *) "point = ", point
        write(io6, *) "grid = ", grid
        error STOP "Point not found in grid"

    END SUBROUTINE find_frac_index

    ! 一次性将多层细化的平衡长度都设置好(适用于非均一值)
    SUBROUTINE set_distsOnEdge_global(num_sjx, num_dbx, num_edge, ngrwm, n_ngrwm, CellsOnEdge, edgesOnVertex, mp, distsOnEdge, ngrmw, cellwidth)
        USE consts_coms, only : r8, pi2_r8, erad8, nxp, step, beta, output_format
        USE refine_vars, only : num_rc
        USE refine_vars, only : max_iter, exit_loop_step
        implicit none
        integer, intent(in) :: num_sjx, num_dbx, num_edge
        integer, allocatable, intent(in) :: ngrwm(:,:), n_ngrwm(:)
        integer, allocatable, intent(in) :: CellsOnEdge(:,:), edgesOnVertex(:, :)
        real(r8),allocatable, intent(in) :: mp(:,:)
        real(r8), intent(inout) :: distsOnEdge(num_edge)
        integer, allocatable, intent(in), optional :: ngrmw(:,:)
        real(r8), intent(inout), optional :: cellwidth(num_dbx)
        integer :: i, dist_len
        real(r8) :: dist00, mindist00
        real(r8) :: mincellwidth00, cellwidth00
        integer, allocatable :: mrl_bk_orial(:)
        real(r8),allocatable :: dist_layers(:)

        ! Compute mean length of coarse mesh U segments
        mindist00 = real(beta, r8) * pi2_r8 * erad8 / (5._r8 * real(nxp))
        distsOnEdge = mindist00 ! 局部背景值

        ! Compute mincellwidth00 and cellwidth00
        if ((output_format == 'MPAS') .or. (output_format == 'MPAS-Simple')) then
            mincellwidth00 =  7680 / NXP
            cellwidth      = mincellwidth00 ! 背景值
        end if
        write(io6, *) "num_rc = ", num_rc

        allocate(mrl_bk_orial(num_sjx)) ! 标记是n级细化还是n-1级细化
        do i = 1, max_iter, 1
            if (i > step) cycle
            if (exit_loop_step(i) .eqv. .TRUE.) cycle ! 跳过没有细化三角形的情况
            dist_len = halo(i) + num_rc

            ! 标记细化三角形
            CALL refine_sjx_regional_make(i, num_sjx, mp, mrl_bk_orial)

            ! 设置dist00和mindist00的大小
            dist00 = mindist00 ! 背景值
            mindist00 = dist00 / 2.0_r8
            write(io6, *) "dist00 = ", dist00
            write(io6, *) "mindist00 = ", mindist00

            ! 设置dist_layers的大小
            allocate(dist_layers(2*dist_len)); dist_layers = dist00
            CALL dist_layers_make(2*dist_len, dist00, dist_layers)

            ! 设置细化三角形区域的过渡区域distsOnEdge
            CALL distsOnEdge_layers_make(i, num_rc, dist_len, num_sjx, num_dbx, num_edge, ngrwm, n_ngrwm, CellsOnEdge, edgesOnVertex, dist_layers, mrl_bk_orial, distsOnEdge)
            deallocate(dist_layers)



            if ((output_format /= 'MPAS') .and. (output_format /= 'MPAS-Simple')) cycle
            ! 设置cellwidth00 and mincellwidth00
            cellwidth00    = mincellwidth00 ! 背景值
            mincellwidth00 = cellwidth00 / 2.0_r8
            write(io6, *) "cellwidth00 = ", cellwidth00
            write(io6, *) "mincellwidth00 = ", mincellwidth00

            ! 设置cellwidth_layers
            allocate(dist_layers(dist_len)); dist_layers = cellwidth00
            CALL dist_layers_make(dist_len, cellwidth00, dist_layers)

            ! 设置过渡区域的cellwidth
            CALL cellwidth_layers_make(i, num_rc, dist_len, num_sjx, num_dbx, ngrmw, ngrwm, n_ngrwm, dist_layers, mrl_bk_orial, cellwidth)
            deallocate(dist_layers)
        end do

        deallocate(mrl_bk_orial)

    END SUBROUTINE set_distsOnEdge_global

    SUBROUTINE dist_layers_make(dist_len, dist_select, dist_layers)

        USE refine_vars, only : set_dis_type
        IMPLICIT NONE
        integer, intent(in) :: dist_len
        real(r8),intent(in) :: dist_select
        real(r8), allocatable, intent(inout) :: dist_layers(:)
        integer :: i
        real(r8) :: mindist_select, a, b

        mindist_select = dist_select / 2.0_r8
        if (trim(set_dis_type) == 'linear') then
            ! 线性设置y = a * x + b
            write(io6, *) "set_dis_type = linear y = a * x + b"
            a = mindist_select / (dist_len)
            b = mindist_select - a
            do i = 1, dist_len, 1
                dist_layers(i) = a * (i+1) + b
            end do
        else if (trim(set_dis_type) == 'nonlinear1') then
            ! 非线性设置1(幂函数形式):y = a * x ^ b
            write(io6, *) "set_dis_type = nonlinear1 y = a * x ^ b"
            a = mindist_select
            b = log(2.0_r8) / log(real(dist_len+1, r8))
            do i = 1, dist_len, 1
                dist_layers(i) = a * (i+1) ** b
            end do
        else if (trim(set_dis_type) == 'nonlinear2') then
            ! 非线性设置2(指数形式):y = a * b ^ x
            write(io6, *) "set_dis_type = nonlinear2 y = a * b ^ x"
            b = 2.0_r8 ** (1.0_r8/(dist_len))
            a = mindist_select / b
            do i = 1, dist_len, 1
                dist_layers(i) = a * b ** (i+1)
            end do
        else if (trim(set_dis_type) == 'nonlinear3') then
            ! 非线性设置3(对数形式):y = a * log(x) + b
            write(io6, *) "set_dis_type = nonlinear3 y = a * log(x) + b"
            b = mindist_select
            a = b / log(real(dist_len+1, r8))
            do i = 1, dist_len, 1
                dist_layers(i) = a * log(real(i+1, r8)) + b
            end do
        else
            write(io6, *) "set_dis_type:", trim(set_dis_type)
            STOP "ERROR! no find set_dis_type in SUBROUTINE dist_layers_make"
        end if
        write(io6, *) "dist_layers = ", dist_layers
        write(io6, *) ""

    END SUBROUTINE dist_layers_make

    SUBROUTINE refine_sjx_regional_make(iter, num_sjx, mp, mrl_bk_orial)
        USE NETCDF
        USE consts_coms, only : mask_patch_ndm, nlons_source, nlats_source, num_mp_step, NXP, file_dir
        USE MOD_data_preprocess, only : lon_vertex, lat_vertex
        USE MOD_Area_judge, only : Source_Find, IsInArea_close_Calculation, minlon_PaArea, maxlon_PaArea, maxlat_PaArea, minlat_PaArea
        IMPLICIT NONE
        integer, intent(in) :: iter, num_sjx
        real(r8), allocatable, intent(in) :: mp(:,:)
        integer,  allocatable, intent(inout) :: mrl_bk_orial(:)
        integer :: i, minlon_source, maxlat_source, DimaID, ncVarID, ncid
        real(r8) :: lonm, latm
        character(16) :: type_select
        integer, allocatable :: IsInPaArea_grid(:,:)
        character(pathlen) :: lndname
        character(LEN = 5) :: nxpc, stepc
        
        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') iter

        mrl_bk_orial = 0
        allocate(IsInPaArea_grid(nlons_source, nlats_source)); IsInPaArea_grid = 0
        minlon_PaArea = nlons_source; maxlon_PaArea = 1
        maxlat_PaArea = nlats_source; minlat_PaArea = 1

        ! 读取对应的边界文件数据并对IsInPaArea_grid进行赋值
        type_select = 'mask_patch'
        CALL IsInArea_close_Calculation(type_select, iter, mask_patch_ndm(iter), IsInPaArea_grid)

        ! 根据IsInPaArea_grid对mrl_bk_orial进行赋值，判断是否为细化边界内的三角形
        write(io6, *) "num_mp_step(iter) = ", num_mp_step(iter)
        do i = num_mp_step(iter), num_sjx, 1
            lonm = mp(i, 1) 
            latm = mp(i, 2)
            ! 判断三角形中心点是否在IsInPaArea_grid标记为1的范围内
            CALL Source_Find(lonm, lon_vertex, 'lon', minlon_source)! minlon_source
            CALL Source_Find(latm, lat_vertex, 'lat', maxlat_source)! maxlat_source
            minlon_source = max(1, minlon_source-1)
            maxlat_source = max(1, maxlat_source-1)
            if (IsInPaArea_grid(minlon_source, maxlat_source) == 1) mrl_bk_orial(i) = 1 !标记在细化区域的三角形
        end do

        deallocate(IsInPaArea_grid)

    END SUBROUTINE refine_sjx_regional_make

    SUBROUTINE distsOnEdge_layers_make(iter, num_rc, dist_len, num_sjx, num_dbx, num_edge, ngrwm, n_ngrwm, CellsOnEdge, edgesOnVertex, dist_layers, mrl_bk_orial, distsOnEdge)
        USE consts_coms, only : num_mp_step, num_wp_step
        IMPLICIT NONE
        integer, intent(in) :: iter, num_rc, dist_len, num_sjx, num_dbx, num_edge
        integer, allocatable, intent(in) :: ngrwm(:,:), n_ngrwm(:)
        integer, allocatable, intent(in) :: CellsOnEdge(:,:), edgesOnVertex(:, :)
        real(r8),allocatable, intent(in) :: dist_layers(:)
        integer, allocatable, intent(in) :: mrl_bk_orial(:)
        real(r8), intent(inout) :: distsOnEdge(num_edge)
        integer, allocatable :: mrl_bk(:), mrl_in(:), isbdy_array(:), IsEdgesMove(:)
        integer :: num_vertex_in, num_center_in
        integer :: i, j, k, m, ik, mk, num_edges, num
        real(r8) :: mindist00

        allocate(mrl_bk(num_sjx)); mrl_bk = mrl_bk_orial ! 标记需要处理的三角形
        allocate(mrl_in(num_sjx)); mrl_in = 0 ! 标记需要处理的三角形
        allocate(isbdy_array(num_dbx)); isbdy_array = 0
        allocate(IsEdgesMove(num_edge)); IsEdgesMove = 0

        mindist00 = dist_layers(2*dist_len) / 2.0_r8
        num_vertex_in = num_mp_step(iter)
        num_center_in = num_wp_step(iter)

        ! 向细化区域内部找几圈
        if (num_rc /= 0) then
            m = 0
            do while(m < num_rc)

                ! 先确认边界点位
                isbdy_array = 0
                do i = num_center_in + 1, num_dbx, 1
                    num_edges = n_ngrwm(i)
                    num = sum(mrl_bk(ngrwm(1:num_edges, i)))
                    if (num == 0) cycle ! 多边形内没有细化三角形，跳过
                    if (num == num_edges) cycle ! 多边形内全是细化三角形，跳过
                    isbdy_array(i) = 1
                end do

                ! 确认需要边界内的三角形
                do i = num_center_in + 1, num_dbx, 1
                    if (isbdy_array(i) /= 1) cycle
                    num_edges = n_ngrwm(i) ! 获取相连的三角形个数
                    do j = 1, num_edges ,1
                        k = ngrwm(j, i) ! 获取对应的center网格编号
                        if (mrl_bk(k) == 0) cycle ! 跳过未细化三角形
                        mrl_bk(k) = 0 ! 找到细化网格标记为非细化网格，便于下一次循环
                    end do
                end do

                m = m + 1
            end do
        end if

        do i = num_vertex_in + 1, num_sjx, 1 ! 这里的2是1+1，未来需要修改
            if (mrl_bk(i) /= 1) cycle ! 跳过细化区域最外几圈三角形网格
            do j = 1, 3, 1
                k = edgesOnVertex(j, i) ! 获取三角形边的编号
                distsOnEdge(k) = mindist00 ! 这是一分四细化的三角形，平衡长度减半
                IsEdgesMove(k) = 1 ! 说明已经被修改，后续不能再修改
            end do
        end do

        ! 处理缓冲区域三角形边长的平衡长度（含细化区域最外两圈+全部过渡区域）
        m = 0
        do while(m < dist_len+1)

            ! 先确认边界点位
            isbdy_array = 0
            do i = num_center_in + 1, num_dbx, 1
                num_edges = n_ngrwm(i)
                num = sum(mrl_bk(ngrwm(1:num_edges, i)))
                if (num == 0) cycle ! 多边形内没有细化三角形，跳过
                if (num == num_edges) cycle ! 多边形内全是细化三角形，跳过
                isbdy_array(i) = 1
            end do
            if (m == dist_len) exit ! 找到最外圈边界就退出

            ! 再确认需要边界外的三角形
            do i = num_center_in + 1, num_dbx, 1
                if (isbdy_array(i) /= 1) cycle
                num_edges = n_ngrwm(i) ! 获取相连的三角形个数
                do j = 1, num_edges ,1
                    k = ngrwm(j, i) ! 获取对应的center网格编号
                    if (mrl_bk(k) == 1) cycle ! 跳过处于细化halo的三角形
                    mrl_bk(k) = 1 ! 标记halo区域三角形，便于下一次循环
                    mrl_in(k) = 1 ! 找到这一轮中需要处理的三角形编号
                end do
            end do

            ! 对于mrl_in(k)==1的三角形的边进行平衡长度的设置
            do i = 2, num_sjx, 1 ! 这里的2是1+1，未来需要修改
                if (mrl_in(i) /= 1) cycle ! 跳过不需要处理的三角形 
                do j = 1, 3, 1
                    k = edgesOnVertex(j, i) ! 获取三角形边的编号
                    if (IsEdgesMove(k) == 1) cycle ! 跳过已经修改的边的编号
                    ik = isbdy_array(CellsOnEdge(1, k))
                    mk = isbdy_array(CellsOnEdge(2, k))
                    if ((ik+mk) == 1) then ! 与细化边界有夹角
                        distsOnEdge(k) = dist_layers(2*m+1)
                    else ! 平行于细化边界
                        distsOnEdge(k) = dist_layers(2*m+2)
                    end if
                    IsEdgesMove(k) = 1
                end do
            end do
            mrl_in = 0 ! 更新
            m = m + 1
        end do

        deallocate(mrl_in, isbdy_array, IsEdgesMove)

    END SUBROUTINE distsOnEdge_layers_make

    SUBROUTINE cellwidth_layers_make(iter, num_rc, dist_len, num_sjx, num_dbx, ngrmw, ngrwm, n_ngrwm, dist_layers, mrl_bk_orial, cellwidth)
        USE consts_coms, only : num_mp_step, num_wp_step
        IMPLICIT NONE
        integer, intent(in) :: iter, num_rc, dist_len, num_sjx, num_dbx
        integer, allocatable, intent(in) :: ngrmw(:,:), ngrwm(:,:), n_ngrwm(:)
        real(r8),allocatable, intent(in) :: dist_layers(:)
        integer, allocatable, intent(in) :: mrl_bk_orial(:)
        real(r8), intent(inout) :: cellwidth(num_dbx)
        integer, allocatable :: mrl_bk(:), mrl_in(:), isbdy_array(:)
        integer :: num_vertex_in, num_center_in
        integer :: i, j, k, m, num_edges, num

        allocate(mrl_bk(num_sjx)); mrl_bk = mrl_bk_orial ! 标记需要处理的三角形
        allocate(mrl_in(num_sjx)); mrl_in = 0 ! 标记需要处理的三角形
        allocate(isbdy_array(num_dbx)); isbdy_array = 0

        num_vertex_in = num_mp_step(iter)
        num_center_in = num_wp_step(iter)

        ! 细化区域内部找几圈
        if (num_rc /= 0) then
            m = 0
            do while(m < num_rc)

                ! 先确认边界点位
                isbdy_array = 0
                do i = num_center_in + 1, num_dbx, 1
                    num_edges = n_ngrwm(i)
                    num = sum(mrl_bk(ngrwm(1:num_edges, i)))
                    if (num == 0) cycle ! 多边形内没有细化三角形，跳过
                    if (num == num_edges) cycle ! 多边形内全是细化三角形，跳过
                    isbdy_array(i) = 1
                end do

                ! 确认需要边界内的三角形
                do i = num_center_in + 1, num_dbx, 1
                    if (isbdy_array(i) /= 1) cycle
                    num_edges = n_ngrwm(i) ! 获取相连的三角形个数
                    do j = 1, num_edges ,1
                        k = ngrwm(j, i) ! 获取对应的center网格编号
                        if (mrl_bk(k) == 0) cycle ! 跳过未细化三角形
                        mrl_bk(k) = 0 ! 找到细化网格标记为非细化网格，便于下一次循环
                    end do
                end do

                m = m + 1
            end do
        end if

        do i = num_vertex_in + 1, num_sjx, 1 ! 这里的2是1+1，未来需要修改
            if (mrl_bk(i) /= 1) cycle ! 跳过细化区域最外几圈三角形网格
            do j = 1, 3, 1
                k = ngrmw(j, i) ! 获取三角形边的编号
                cellwidth(k) = dist_layers(dist_len) / 2.0_r8 ! 这是一分四细化的三角形，平衡长度减半
            end do
        end do

        ! 处理缓冲区域三角形边长的平衡长度（含细化区域最外两圈+全部过渡区域）
        m = 0
        do while(m < dist_len+1)

            ! 先确认边界点位
            isbdy_array = 0
            do i = num_center_in + 1, num_dbx, 1
                num_edges = n_ngrwm(i)
                num = sum(mrl_bk(ngrwm(1:num_edges, i)))
                if (num == 0) cycle ! 多边形内没有细化三角形，跳过
                if (num == num_edges) cycle ! 多边形内全是细化三角形，跳过
                isbdy_array(i) = 1
            end do
            if (m == dist_len) exit ! 找到最外圈边界就退出

            ! 再确认需要边界外的三角形
            do i = num_center_in + 1, num_dbx, 1
                if (isbdy_array(i) /= 1) cycle
                num_edges = n_ngrwm(i) ! 获取相连的三角形个数
                do j = 1, num_edges ,1
                    k = ngrwm(j, i) ! 获取对应的center网格编号
                    if (mrl_bk(k) == 1) cycle ! 跳过处于细化halo的三角形
                    mrl_bk(k) = 1 ! 标记halo区域三角形，便于下一次循环
                    mrl_in(k) = 1 ! 找到这一轮中需要处理的三角形编号
                end do
            end do

            ! 对于mrl_in(k)==1的三角形的边进行平衡长度的设置
            do i = 2, num_sjx, 1 ! 这里的2是1+1，未来需要修改
                if (mrl_in(i) /= 1) cycle ! 跳过不需要处理的三角形 
                do j = 1, 3, 1
                    k = ngrmw(j, i) ! 获取三角形边的编号
                    if (isbdy_array(k) == 1) cycle ! 跳过已经修改的多边形编号
                    cellwidth(k) = dist_layers(m+1)
                end do
            end do
            mrl_in = 0 ! 更新
            m = m + 1
        end do

        deallocate(mrl_bk, mrl_in, isbdy_array)

    END SUBROUTINE cellwidth_layers_make

    ! 适用于多层细化逐步调整和多层细化最后一起调整的时候关于那些顶点可以移动的设置
    SUBROUTINE set_dbxMove_regional_step(set_dis, num_sjx_ref, num_sjx, num_dbx, ngrmw, ngrwm, n_ngrwm, IsdbxMove, mp)
        USE consts_coms, only : num_vertex, impent
        USE refine_vars, only : vertex_pretect_layers
        IMPLICIT NONE
        integer, intent(in) :: set_dis, num_sjx_ref
        integer, intent(in) :: num_sjx, num_dbx
        integer, allocatable, intent(in) :: ngrmw(:,:), ngrwm(:,:), n_ngrwm(:)
        real(r8),allocatable, intent(in), optional :: mp(:,:)
        integer, allocatable, intent(out) :: IsdbxMove(:)
        integer :: i, j, k, m, num_total, num_edges, num, IsOrialVert_refine(12)
        integer, allocatable :: mrl_bk(:)
        integer, allocatable :: isbdy_array(:)

        allocate(mrl_bk(num_sjx)); mrl_bk = 0 ! 标记三角形细化情况
        allocate(isbdy_array(num_dbx)); isbdy_array = 0
        allocate(IsdbxMove(num_dbx)); IsdbxMove = 0


        if (num_sjx_ref /= 0) then ! 用于逐步细化调整的情况
            num_total = num_vertex + 4*num_sjx_ref
            mrl_bk(num_vertex + 1:num_total) = 1 ! 标记是由于一分四细化得到的三角形
        else ! 用于多层细化最后一起调整的情况
            CALL refine_sjx_regional_make(1, num_sjx, mp, mrl_bk)
        end if

        ! 标记需要处理的三角形编号
        m = 0
        do while(m < set_dis+1)

            ! 先确认边界点位
            isbdy_array = 0
            do i = 2, num_dbx, 1
                num_edges = n_ngrwm(i)
                num = sum(mrl_bk(ngrwm(1:num_edges, i)))
                if (num == 0) cycle ! 多边形内没有细化三角形，跳过
                if (num == num_edges) cycle ! 多边形内全是细化三角形，跳过
                isbdy_array(i) = 1
            end do
            if (m == set_dis) exit

            ! 再确认需要边界外的三角形
            do i = 2, num_dbx, 1
                if (isbdy_array(i) /= 1) cycle
                num_edges = n_ngrwm(i) ! 获取相连的三角形个数
                do j = 1, num_edges ,1
                    k = ngrwm(j, i) ! 获取对应的center网格编号
                    if (mrl_bk(k) == 1) cycle ! 跳过处于细化halo的三角形
                    mrl_bk(k) = 1 ! 标记halo区域三角形，便于下一次循环
                end do
            end do

            m = m + 1
        end do

        ! 根据mrl_bk标记IsdbxMove
        do i = 2, num_sjx, 1
            if (mrl_bk(i) /= 1) cycle ! 跳过不需要处理的三角形 
            do j = 1, 3, 1
                k = ngrmw(j, i) ! 获取三角形边的编号
                IsdbxMove(k) = 1 ! 标记为可以移动
            end do
        end do

        ! 根据isbdy_array更新IsdbxMove
        do i = 2, num_dbx, 1
            if (isbdy_array(i) == 1) IsdbxMove(i) = 0
        end do

        ! 暂定12 orial vertices vertex_pretect_layers内的细化顶点不可移动
        if (vertex_pretect_layers == 0) then
            deallocate(mrl_bk, isbdy_array)
            return
        end if

        IsOrialVert_refine = 0
        do i = 1, 12, 1
            j = impent(i)
            do k = 1, 5, 1
                if (mrl_bk(ngrwm(k, j)) == 1) then
                    IsOrialVert_refine(i) = 1 ! 判断12个orial顶点相连三角形是否被细化
                    exit
                end if
            end do
        end do

        if (sum(IsOrialVert_refine) == 0) then
            deallocate(mrl_bk, isbdy_array)
            return
        else 
            mrl_bk = 0; isbdy_array = 0
            do i = 1, 12, 1
                if (IsOrialVert_refine(i) == 0) cycle ! 跳过12 orial 顶点附近没有相连细化三角形的情况
                j = impent(i)
                do k = 1, 5, 1
                    mrl_bk(ngrwm(k, j)) = 1 ! 标记需要处理的三角形
                end do
            end do
        end if

        ! 如果原始12顶点需要移动，说明位于细化区域内，因此在之前的细化中应该已经将位于保护区内部的三角形都细化了
        ! 相关内容应该在MOD_refine.F90而且是SpringRegional设定时相匹配
        m = 0
        do while(m < vertex_pretect_layers)

            ! 先确认边界点位
            isbdy_array = 0
            do i = 2, num_dbx, 1
                num_edges = n_ngrwm(i)
                num = sum(mrl_bk(ngrwm(1:num_edges, i)))
                if (num == 0) cycle ! 多边形内没有细化三角形，跳过
                if (num == num_edges) cycle ! 多边形内全是细化三角形，跳过
                isbdy_array(i) = 1
            end do
            if (m == vertex_pretect_layers) exit

            ! 再确认需要边界外的三角形
            do i = 2, num_dbx, 1
                if (isbdy_array(i) /= 1) cycle
                num_edges = n_ngrwm(i) ! 获取相连的三角形个数
                do j = 1, num_edges ,1
                    k = ngrwm(j, i) ! 获取对应的center网格编号
                    if (mrl_bk(k) == 1) cycle ! 跳过处于细化halo的三角形
                    mrl_bk(k) = 1 ! 标记halo区域三角形，便于下一次循环
                end do
            end do

            m = m + 1
        end do

        ! 根据mrl_bk去除12orial顶点周围的IsdbxMove标记
        do i = 2, num_sjx, 1
            if (mrl_bk(i) /= 1) cycle ! 跳过不需要处理的三角形 
            do j = 1, 3, 1
                k = ngrmw(j, i) ! 获取三角形边的编号
                IsdbxMove(k) = 0 ! 标记为不可以移动
            end do
        end do
        deallocate(mrl_bk, isbdy_array)

    END SUBROUTINE set_dbxMove_regional_step

    ! 适用于给定平衡长度的全局弹性调整（无论平衡长度均一与否）
    SUBROUTINE spring_dynamics_global(ngr, num_dbx, num_edge, nEdgesOnCell, edgesOnCell, CellsOnEdge, distsOnEdge, EdgesOnedge_tri, xew, yew, zew)

        USE consts_coms, only : r8, erad8, io6, relax
        USE refine_vars, only : niter_refine
        implicit none
        integer,  intent(in) :: ngr, num_dbx, num_edge
        integer,  intent(in) :: nEdgesOnCell(num_dbx), edgesOnCell(7, num_dbx)
        integer,  intent(in) :: CellsOnEdge(2, num_edge), EdgesOnedge_tri(4, num_edge)
        real(r8), intent(in) :: distsOnEdge(num_edge)
        real(r8), intent(inout) :: xew(num_dbx), yew(num_dbx), zew(num_dbx)

        integer, parameter :: nprnt = 100
        integer  :: iv, iu, iu1, iu2, iu3, iu4
        integer  :: iw, iw1, iw2
        integer  :: j, iter

        real(r8) :: expansion
        real(r8) :: distm, frac_change, ratio
        real(r8) :: twocosphi3, twocosphi4

        real(r8), allocatable :: dsm(:),  dirs(:, :)
        real(r8), allocatable :: xew8(:), yew8(:), zew8(:)
        real(r8), allocatable :: xew0(:), yew0(:), zew0(:)

        real(r8), allocatable :: dist(:)
        real(r8), allocatable :: dx(:), dy(:), dz(:)
        real(r8), allocatable :: frac_change2(:)

        allocate(dirs(7, num_dbx)); dirs = 0.
        allocate(xew8(num_dbx)); xew8 = xew
        allocate(yew8(num_dbx)); yew8 = yew
        allocate(zew8(num_dbx)); zew8 = zew
        allocate(xew0(num_dbx)); xew0 = xew
        allocate(yew0(num_dbx)); yew0 = yew
        allocate(zew0(num_dbx)); zew0 = zew

        allocate(dsm(num_dbx)); dsm = -99.
        allocate(dist(num_edge)); dist = -99
        allocate(dx(num_edge)); dx = -99;
        allocate(dy(num_edge)); dy = -99;
        allocate(dz(num_edge)); dz = -99;
        allocate(frac_change2(num_edge)); frac_change2 = 0.0_r8

        ! 设置弹性调整的力度与方向
        ! dirs也可以保存出来看一看到底方向对不对，有可能不对
        do iw = 2, num_dbx
            do j = 1, nEdgesOnCell(iw)
                iu = edgesOnCell(j, iw)
                if (CellsOnEdge(2, iu) == iw) then
                    dirs(j, iw) =  relax
                else
                    dirs(j, iw) = -relax
                endif
            end do
        end do

        write(io6,'(a,4i9)') "In spring dynamics: ngr, num_dbx, niter, relax = "
        write(io6,*) ngr, num_dbx, niter_refine, relax

        ! Main iteration loop
        !$omp PARALLEL private(iter, iw, iu, j, iv, iw1, iw2, iu1, iu2, iu3, iu4, &
        !$omp&              twocosphi3, twocosphi4, distm, frac_change, ratio, expansion)
        do iter = 1, niter_refine
            ! 更新 xew0/yew0/zew0（需同步）
            if ((iter == 1) .or. mod(iter, nprnt) == 0) then
                !$omp do
                do iw = 2, num_dbx
                    xew0(iw) = xew8(iw)
                    yew0(iw) = yew8(iw)
                    zew0(iw) = zew8(iw)
                end do
                !$omp end do
            end if

            !$omp do
            do iu = 2, num_edge
                iw1 = CellsOnEdge(1, iu)
                iw2 = CellsOnEdge(2, iu)
                dx(iu) = real(xew8(iw2) - xew8(iw1))
                dy(iu) = real(yew8(iw2) - yew8(iw1))
                dz(iu) = real(zew8(iw2) - zew8(iw1))
                dist(iu) = sqrt(dx(iu)**2 + dy(iu)**2 + dz(iu)**2)
            end do
            !$omp end do

            !$omp do
            do iu = 2, num_edge
                iu1 = EdgesOnedge_tri(1, iu)
                iu2 = EdgesOnedge_tri(2, iu)
                iu3 = EdgesOnedge_tri(3, iu)
                iu4 = EdgesOnedge_tri(4, iu)
                twocosphi3 = (dist(iu1)**2 + dist(iu2)**2 - dist(iu)**2) / (dist(iu1) * dist(iu2))
                twocosphi4 = (dist(iu3)**2 + dist(iu4)**2 - dist(iu)**2) / (dist(iu3) * dist(iu4))
                ratio = max(0.15_r8, min(twocosphi3 + twocosphi4, 1.2_r8))
                distm = distsOnEdge(iu) / 1.2_r8 * ratio
                frac_change = (distm - dist(iu)) / dist(iu)
                dx(iu) = dx(iu) * frac_change
                dy(iu) = dy(iu) * frac_change
                dz(iu) = dz(iu) * frac_change
                frac_change2(iu) = frac_change * frac_change
            end do
            !$omp end do

            !$omp do
            do iw = 2, num_dbx
                do j = 1, nEdgesOnCell(iw)
                    iv = edgesOnCell(j, iw)
                    xew8(iw) = xew8(iw) + dirs(j, iw) * dx(iv)
                    yew8(iw) = yew8(iw) + dirs(j, iw) * dy(iv)
                    zew8(iw) = zew8(iw) + dirs(j, iw) * dz(iv)
                enddo
            enddo
            !$omp end do

            ! 标准化坐标（无竞态）
            !$omp do private(expansion)
            do iw = 2, num_dbx
                expansion = erad8 / sqrt(xew8(iw)**2 + yew8(iw)**2 + zew8(iw)**2)
                xew8(iw) = xew8(iw) * expansion
                yew8(iw) = yew8(iw) * expansion
                zew8(iw) = zew8(iw) * expansion
            enddo
            !$omp end do

            ! 输出结果（线程安全）
            if (iter == 1 .or. mod(iter, nprnt) == 0) then
                !$omp do
                do iw = 2, num_dbx
                    dsm(iw) = (xew8(iw) - xew0(iw))**2 &
                            + (yew8(iw) - yew0(iw))**2 &
                            + (zew8(iw) - zew0(iw))**2 
                enddo
                !$omp end do

                !$omp single
                write(*, '(3x,A,I5,A,I5,A,f0.4,A)') "Iteration ", iter, " of ", niter, ",  Max DS = ", sqrt(maxval(dsm)), " meters."
                !$omp end single
            endif

        end do
        !$omp end parallel

        xew = xew8
        yew = yew8
        zew = zew8
        deallocate(xew8, yew8, zew8, xew0, yew0, zew0)
        deallocate(dx, dy, dz, dsm, dist, dirs, frac_change2)

    END SUBROUTINE spring_dynamics_global

    ! 真正局部网格进行调整，并不会全局网格调整
    SUBROUTINE spring_dynamics_regionalv2(ngr, num_dbx, nEdgesOnCell, cellsOnCell, IsdbxMove, xew, yew, zew)

        USE consts_coms, only : r8, erad8, io6
        implicit none
        integer,  intent(in) :: ngr, num_dbx
        integer,  intent(in) :: nEdgesOnCell(num_dbx), cellsOnCell(7, num_dbx)
        integer,  intent(in) :: IsdbxMove(num_dbx)
        real(r8), intent(inout) :: xew(num_dbx), yew(num_dbx), zew(num_dbx)

        integer, parameter :: nprnt = 10
        integer  :: num_dbx_cal, nummax
        integer  :: iv, iw, j, iter, n, i
        real(r8) :: expansion
        integer,  allocatable :: IsdbxCal(:), dbxCal_reserve(:), IsdbxCal_Move(:)
        integer,  allocatable :: nEdgesOnCell_Cal(:), cellsOnCell_Cal(:,:)
        real(r8), allocatable :: dsm(:)
        real(r8), allocatable :: xew8(:), yew8(:), zew8(:)
        real(r8), allocatable :: xew0(:), yew0(:), zew0(:)

        allocate(IsdbxCal(num_dbx)); IsdbxCal = 1
        allocate(dbxCal_reserve(num_dbx)); dbxCal_reserve = 1

        ! 将不需要移动的边界点位也标记上
        IsdbxCal = IsdbxMove
        do iw = 2, num_dbx
            if (IsdbxMove(iw) /= 1) cycle ! 跳过不需要移动的点位
            n = nEdgesOnCell(iw)
            do j = 1, n
                iv = cellsOnCell(j, iw)
                IsdbxCal(iv) = 1
            end do
        end do

        num_dbx_cal = sum(IsdbxCal)
        nummax = maxval(nEdgesOnCell)
        allocate(IsdbxCal_Move(num_dbx_cal)); IsdbxCal_Move = 0
        allocate(nEdgesOnCell_Cal(num_dbx_cal)); nEdgesOnCell_Cal = 0
        allocate(cellsOnCell_Cal(nummax, num_dbx_cal)); cellsOnCell_Cal = 1

        allocate(dsm(num_dbx_cal)); dsm = -99.
        allocate(xew8(num_dbx_Cal)); xew8 = 0.0_r8
        allocate(yew8(num_dbx_Cal)); yew8 = 0.0_r8
        allocate(zew8(num_dbx_Cal)); zew8 = 0.0_r8
        allocate(xew0(num_dbx_Cal)); 
        allocate(yew0(num_dbx_Cal)); 
        allocate(zew0(num_dbx_Cal)); 

        num_dbx_cal = 0
        do iw = 2, num_dbx
            if (IsdbxCal(iw) /= 1) cycle ! 跳过不需要的点位
            num_dbx_cal = num_dbx_cal + 1
            dbxCal_reserve(iw) = num_dbx_cal

            if (IsdbxMove(iw) == 1) IsdbxCal_Move(num_dbx_cal) = 1
            nEdgesOnCell_Cal(num_dbx_Cal) = nEdgesOnCell(iw)
            cellsOnCell_Cal(:,num_dbx_Cal) = cellsOnCell(:, iw)

            xew8(num_dbx_Cal) = xew(iw)
            yew8(num_dbx_Cal) = yew(iw)
            zew8(num_dbx_Cal) = zew(iw)
        end do
        xew0 = xew8
        yew0 = yew8
        zew0 = zew8

        ! Main iteration loop
        write(io6,'(a,4i9)') "In spring dynamics: ngr, num_dbx, niter_refine = ", ngr, num_dbx, niter_refine

        !$omp PARALLEL private(iter, i, iw, n, j, iv, expansion)
        do iter = 1, niter_refine
            ! 更新 xew0/yew0/zew0（每一步都需要更新）
            !$omp do
            do i = 1, num_dbx_Cal
                if (IsdbxCal_Move(i) /= 1) cycle ! 跳过不需要移动的点位
                xew0(i) = xew8(i)
                yew0(i) = yew8(i)
                zew0(i) = zew8(i)
            end do
            !$omp end do

            !$omp do
            do i = 1, num_dbx_Cal
                if (IsdbxCal_Move(i) /= 1) cycle ! 跳过不需要移动的点位
                xew8(i) = 0.0_r8
                yew8(i) = 0.0_r8
                zew8(i) = 0.0_r8
                n = nEdgesOnCell_Cal(i)
                do j = 1, n
                    iw = cellsOnCell_Cal(j, i)
                    iv = dbxCal_reserve(iw)
                    xew8(i) = xew8(i) + xew0(iv) / n
                    yew8(i) = yew8(i) + yew0(iv) / n
                    zew8(i) = zew8(i) + zew0(iv) / n
                enddo
            enddo
            !$omp end do

            ! 标准化坐标
            !$omp do private(expansion)
            do i = 1, num_dbx_Cal
                if (IsdbxCal_Move(i) /= 1) cycle ! 跳过不需要移动的点位
                expansion = erad8 / sqrt(xew8(i)**2 + yew8(i)**2 + zew8(i)**2)
                xew8(i) = xew8(i) * expansion
                yew8(i) = yew8(i) * expansion
                zew8(i) = zew8(i) * expansion
            enddo
            !$omp end do

            ! 输出结果（线程安全）
            if (iter == 1 .or. mod(iter, nprnt) == 0) then
                !$omp do
                do i = 1, num_dbx_Cal
                    if (IsdbxCal_Move(i) /= 1) cycle ! 跳过不需要移动的点位
                    dsm(i) = (xew8(i) - xew0(i))**2 &
                           + (yew8(i) - yew0(i))**2 &
                           + (zew8(i) - zew0(i))**2 
                enddo
                !$omp end do

                !$omp single
                write(*, '(3x,A,I5,A,I5,A,f0.4,A)') "Iteration ", iter, " of ", niter_refine, ",  Max DS = ", sqrt(maxval(dsm)), " meters."
                !$omp end single
            endif

        end do
        !$omp end parallel

        ! 将数据范围到原始数组中
        num_dbx_cal = 0
        do iw = 2, num_dbx
            if (IsdbxCal(iw) /= 1) cycle ! 跳过不需要的点位
            num_dbx_cal = num_dbx_cal + 1
            xew(iw) = xew8(num_dbx_Cal)
            yew(iw) = yew8(num_dbx_Cal)
            zew(iw) = zew8(num_dbx_Cal)
        end do

        deallocate(IsdbxCal, dbxCal_reserve, IsdbxCal_Move, nEdgesOnCell_Cal, cellsOnCell_Cal)
        deallocate(dsm, xew8, yew8, zew8, xew0, yew0, zew0)

    END SUBROUTINE spring_dynamics_regionalv2

    ! 构建ngrmm数组
    SUBROUTINE set_ngrmm(num_sjx, ngrmw, ngrwm, n_ngrwm, ngrmm)

        IMPLICIT NONE
        integer, intent(in) :: num_sjx
        integer, allocatable, intent(in) :: ngrmw(:,:), ngrwm(:,:), n_ngrwm(:)
        integer, allocatable, intent(inout) :: ngrmm(:,:)
        integer :: i, j, k, n, mk, ik, wj, num_ngrmm

        write(io6, *) 'ngrmm start'
        do i = 2, num_sjx, 1
            num_ngrmm = 0
            do j = 1, 3, 1
                wj = ngrmw(j, i)
                n = n_ngrwm(wj) ! 现在还不可以用于边长多于7的情况
                if (num_ngrmm == 3) exit ! 找到三个邻域直接退出New
                do k = 1, n, 1
                    mk = ngrwm(k, wj)
                    if (i == mk) cycle ! 避免mi和mk是同一个三角形
                    ik = IsNgrmm(ngrmw(1:3, i), ngrmw(1:3, mk))
                    ! i号m点的第error个w顶点的对边的另一个相邻m点索引为j
                    if (ik == 0) cycle ! 跳过无连接关系的三角形
                    ngrmm(ik, i) = mk
                    num_ngrmm = num_ngrmm + 1
                end do
            end do
        end do
        write(io6, *) 'ngrmm finish'
        write(io6, *) ''

    END SUBROUTINE set_ngrmm

    ! 获取对偶三角形非公共边的edge编号
    SUBROUTINE set_edgesOnEdge_tri(num_sjx, num_edge, ngrmm, verticesOnEdge, edgesOnVertex, edgesOnEdge_tri)

        IMPLICIT NONE
        integer, intent(in) :: num_sjx, num_edge
        integer, intent(in) :: ngrmm(:,:), verticesOnEdge(:,:), edgesOnVertex(:,:)
        integer, intent(inout) :: edgesOnEdge_tri(:,:)
        integer :: iEdge, i, j, k, m, n, vertex1, vertex2, hhh(4)

        hhh = [2,3,1,2] ! 用于简化计算
        do iEdge = 2, num_edge, 1
            vertex1 = verticesOnEdge(1, iEdge)
            vertex2 = verticesOnEdge(2, iEdge)

            do j = 1, 3, 1
                if (edgesOnVertex(j, vertex1) == iEdge) exit
            end do
            edgesOnEdge_tri(1, iEdge) = edgesOnVertex(hhh(j),   vertex1)
            edgesOnEdge_tri(2, iEdge) = edgesOnVertex(hhh(j+1), vertex1)

            do j = 1, 3, 1
                if (edgesOnVertex(j, vertex2) == iEdge) exit
            end do
            edgesOnEdge_tri(3, iEdge) = edgesOnVertex(hhh(j),   vertex2)
            edgesOnEdge_tri(4, iEdge) = edgesOnVertex(hhh(j+1), vertex2)
        end do

    END SUBROUTINE set_edgesOnEdge_tri

    SUBROUTINE edgeIDSort(num_sjx, num_edge, cellsOnEdge_reference, cellsOnEdge, verticesOnEdge, edgesOnVertex, vp)

        IMPLICIT NONE
        integer, intent(in) :: num_sjx, num_edge
        integer, allocatable, intent(in) :: cellsOnEdge_reference(:,:)
        integer, allocatable, intent(inout) :: cellsOnEdge(:,:), verticesOnEdge(:,:), edgesOnVertex(:,:)
        real(r8), allocatable, intent(inout) :: vp(:,:)
        integer, allocatable :: SortID(:)
        integer :: i, j, k, idx, idx1, idx2
        integer, allocatable :: nedgesOnVertex(:), cellsOnEdge_temp(:,:), verticesOnEdge_temp(:,:), edgesOnVertex_temp(:,:)
        real(r8), allocatable :: vp_temp(:,:)

        allocate(SortID(num_edge)); SortID = 1
        allocate(cellsOnEdge_temp(2, num_edge)); cellsOnEdge_temp = 1
        allocate(verticesOnEdge_temp(2, num_edge)); cellsOnEdge_temp = 1
        allocate(edgesOnVertex_temp(3, num_sjx)); cellsOnEdge_temp = 1
        allocate(vp_temp(num_edge, 2)); vp_temp = 0.
        allocate(nedgesOnVertex(num_sjx)); nedgesOnVertex = 0

        !!$OMP PARALLEL DO NUM_THREADS(openmp) SCHEDULE(DYNAMIC,1) &
        !!$OMP PRIVATE(i, j, idx1, idx2, idx)
        do i = 2, num_edge, 1
            idx1 = cellsOnEdge_reference(1, i)
            idx2 = cellsOnEdge_reference(2, i)
            do j = 2, num_edge, 1
                idx = cellsOnEdge(1, j)
                if (idx1 /= idx) cycle
                if (idx2 /= cellsOnEdge(2, j)) cycle
                SortID(i) = j
                exit
            end do
            if (SortID(i) == 1) STOP "ERROR! SortID(i) == 1"
        end do
        !!$OMP END PARALLEL DO
        write(io6, *)"SortID cell finfish"

        do i = 2, num_edge, 1
            vp_temp(i, :) = vp(SortID(i), :)
            cellsOnEdge_temp(:, i) = cellsOnEdge(:, SortID(i))
            verticesOnEdge_temp(:, i) = verticesOnEdge(:, SortID(i))
        end do

        do i = 2, num_edge, 1
            do j = 1, 2, 1
                idx = verticesOnEdge_temp(j, i)
                nedgesOnVertex(idx) = nedgesOnVertex(idx) + 1
                edgesOnVertex_temp(nedgesOnVertex(idx), idx) = i
            end do
        end do

        vp = vp_temp
        cellsOnEdge = cellsOnEdge_temp
        verticesOnEdge = verticesOnEdge_temp
        edgesOnVertex = edgesOnVertex_temp
        deallocate(SortID)
        deallocate(nedgesOnVertex, cellsOnEdge_temp, verticesOnEdge_temp, edgesOnVertex_temp, vp_temp)

    END SUBROUTINE edgeIDSort

    ! 计算三角形的球面重心经纬度坐标
    SUBROUTINE centroid_spherical_calculation(sjx_points, mp, wp, ngrmw)

        IMPLICIT NONE
        integer, intent(in) :: sjx_points
        integer, intent(in) :: ngrmw(:, :)
        real(r8), intent(inout) :: mp(:,:)
        real(r8), intent(in) :: wp(:,:)
        integer :: i
        real(r8) :: sjx(3, 2)
        real(r8) :: centroid_deg(2), centroid(3)
        real(r8) :: x(3), y(3), z(3)

        do i = 2, sjx_points, 1
            sjx = wp(ngrmw(:, i), :)
            ! (1) 将经纬度坐标转换为笛卡尔坐标
            call lonlat2xyz(3, sjx(:, 1), sjx(:, 2), x, y, z)
            ! (2) 计算重心 (向量平均后归一化)
            centroid(1) = sum(x)/3.0
            centroid(2) = sum(y)/3.0
            centroid(3) = sum(z)/3.0
            call xyz2lonlat(centroid, centroid_deg)
            mp(i, :) = centroid_deg
        end do

    END SUBROUTINE centroid_spherical_calculation

    ! 适用于单个三角形重心计算的形式,未来可以用于代替在一分四细化中的简单平均
    ! 修改为适用于边求中心点位
    SUBROUTINE centroid_spherical_single(len_temp, lon_deg, lat_deg, centroid_deg)

        IMPLICIT NONE
        integer, intent(in) :: len_temp
        real(r8), intent(in) :: lon_deg(len_temp), lat_deg(len_temp)
        real(r8), intent(out) :: centroid_deg(2)
        real(r8) :: x(len_temp), y(len_temp), z(len_temp), centroid(3)

        ! (1) 将经纬度坐标转换为笛卡尔坐标
        call lonlat2xyz(len_temp, lon_deg, lat_deg, x, y, z)
        ! (2) 计算重心 (向量平均后归一化)
        centroid(1) = sum(x)/len_temp
        centroid(2) = sum(y)/len_temp
        centroid(3) = sum(z)/len_temp
        call xyz2lonlat(centroid, centroid_deg)

    END SUBROUTINE centroid_spherical_single

    ! 计算三角形的球面外心经纬度坐标（基于球面重心经纬度坐标）
    SUBROUTINE circumcenter_spherical_calculation(sjx_points, ngrmw, xem8, yem8, zem8, xew8, yew8, zew8)
        ! 由三角形顶点经纬度坐标，计算三角形外心经纬度坐标,已经弄清楚了对应的代码，但是似乎对于精度很敏感（尤其是取了r8之后）
        ! 要求进来的xem8, yem8, zem8, xew8, yew8, zew8是以及乘以地球半径
        IMPLICIT NONE
        integer, intent(in) :: sjx_points
        integer, intent(in) :: ngrmw(:, :)
        real(r8), intent(inout) :: xem8(:), yem8(:), zem8(:)
        real(r8), intent(in) :: xew8(:), yew8(:), zew8(:)
        integer :: im, iw1, iw2, iw3
        real(r8) :: xew8_temp(3), yew8_temp(3), zew8_temp(3)
        real(r8) :: x1, x2, x3, y1, y2, y3
        real(r8) :: xebc, yebc, zebc
        real(r8) :: dxe, dye, dze
        real(r8) :: raxis, raxisi, expansion
        real(r8) :: sinwlat, coswlat, sinwlon, coswlon
        real(r8) :: dx12, dx13, dx23
        real(r8) :: s1, s2, s3
        real(r8) :: xcc, ycc

        do im = 2, sjx_points, 1

            ! These were initialized to be the barycenter of each triangle
            xebc = xem8(im)
            yebc = yem8(im)
            zebc = zem8(im)

            iw1 = ngrmw(1, im); iw2 = ngrmw(2, im); iw3 = ngrmw(3, im)
            xew8_temp(1) = xew8(iw1); xew8_temp(2) = xew8(iw2); xew8_temp(3) = xew8(iw3)
            yew8_temp(1) = yew8(iw1); yew8_temp(2) = yew8(iw2); yew8_temp(3) = yew8(iw3)
            zew8_temp(1) = zew8(iw1); zew8_temp(2) = zew8(iw2); zew8_temp(3) = zew8(iw3)

            ! For global domain, transform from sphere to PS plane
            raxis = sqrt(xebc ** 2 + yebc ** 2)
            raxisi = 1.0 / raxis

            sinwlat = zebc * eradi_r8
            coswlat = raxis * eradi_r8

            sinwlon = yebc * raxisi
            coswlon = xebc * raxisi

            ! Transform 3 W points to PS coordinates

            dxe = xew8_temp(1) - xebc
            dye = yew8_temp(1) - yebc
            dze = zew8_temp(1) - zebc
            call de_ps_r8(dxe, dye, dze, coswlat, sinwlat, coswlon, sinwlon, x1, y1)

            dxe = xew8_temp(2) - xebc
            dye = yew8_temp(2) - yebc
            dze = zew8_temp(2) - zebc
            call de_ps_r8(dxe, dye, dze, coswlat, sinwlat, coswlon, sinwlon, x2, y2)

            dxe = xew8_temp(3) - xebc
            dye = yew8_temp(3) - yebc
            dze = zew8_temp(3) - zebc
            call de_ps_r8(dxe, dye, dze, coswlat, sinwlat, coswlon, sinwlon, x3, y3)

            ! Compute intermediate quanties

            dx12 = x2 - x1
            dx13 = x3 - x1
            dx23 = x3 - x2

            s1 = x1**2 + y1**2
            s2 = x2**2 + y2**2
            s3 = x3**2 + y3**2

            ! Algebraic solution for circumcenter Y coordinate

            ycc = .5 * (dx13 * s2 - dx12 * s3 - dx23 * s1) &
                    / (dx13 * y2 - dx12 * y3 - dx23 * y1)

            ! Algebraic solution for circumcenter X coordinate

            if (abs(dx12) > abs(dx13)) then
                xcc = (s2 - s1 - ycc * 2. * (y2 - y1)) / (2. * dx12)
            else
                xcc = (s3 - s1 - ycc * 2. * (y3 - y1)) / (2. * dx13)
            endif

            ! For global domain, transform circumcenter from PS to earth coordinates

            call ps_de_r8(dxe, dye, dze, coswlat, sinwlat, coswlon, sinwlon, xcc, ycc)

            xem8(im) = dxe + xebc
            yem8(im) = dye + yebc
            zem8(im) = dze + zebc

        end do

        do im = 2, sjx_points, 1
            expansion = erad8 / sqrt(xem8(im) ** 2 &
                    + yem8(im) ** 2 &
                    + zem8(im) ** 2)

            xem8(im) = xem8(im) * expansion
            yem8(im) = yem8(im) * expansion
            zem8(im) = zem8(im) * expansion
        end do

    END SUBROUTINE circumcenter_spherical_calculation

    SUBROUTINE lonlat2xyz(len_temp, lon_deg, lat_deg, x, y, z)
        ! 将经纬度（角度）坐标转为笛卡尔坐标, 限制在单位圆上
        IMPLICIT NONE
        integer, intent(in) :: len_temp
        real(r8), dimension(:), intent(in) :: lon_deg, lat_deg
        real(r8), dimension(:), intent(out) :: x, y, z
        real(r8) :: lon_rad(len_temp), lat_rad(len_temp)

        lon_rad = lon_deg * pio180_r8
        lat_rad = lat_deg * pio180_r8
        x = cos(lat_rad) * cos(lon_rad)
        y = cos(lat_rad) * sin(lon_rad)
        z = sin(lat_rad)
        ! θ 是极角（范围通常为 0到 π，ϕ是方位角（范围通常为 0 到 2π）。
        ! 请检查角度范围到底对不对，而且MPAS中实际上会除以R

    END SUBROUTINE lonlat2xyz

    SUBROUTINE xyz2lonlat(v, lonlat_deg)
        ! 笛卡尔坐标转为经纬度坐标
        IMPLICIT NONE
        real(r8), intent(in) :: v(3)
        real(r8), intent(out) :: lonlat_deg(2)
        real(r8) :: norm, raxis, v1(3)
        
        norm = sqrt(v(1)**2 + v(2)**2 + v(3)**2)
        v1 = v / norm
        raxis = sqrt(v1(1) ** 2 + v1(2) ** 2)
        lonlat_deg(1) = atan2(v1(2), v1(1)) * piu180_r8
        lonlat_deg(2) = atan2(v1(3), raxis) * piu180_r8

    END SUBROUTINE xyz2lonlat

    SUBROUTINE TriMeshQuality(num_sjx, wp_f, ngrmw_f, adjust_sjx_flag, length_sjx_temp, angle_sjx_temp, Extr_sjx_temp, Eavg_sjx_temp, Savg_sjx_temp, angleless_temp, anglemore_temp)

        implicit none ! 定义的顺序最好是只读入的参数，内部参数，需要传出的参数
        integer, intent(in) :: num_sjx
        real(r8), dimension(:, :), intent(in) :: wp_f
        integer,  dimension(:, :), intent(in) :: ngrmw_f
        integer,  dimension(:), intent(in) :: adjust_sjx_flag
        integer :: i
        real(r8) :: length_temp(3), angle_temp(3), sjx(3, 2)
        real(r8) :: angle_regular, angle_regularless, angle_regularmore
        real(r8), dimension(:, :), intent(inout) :: length_sjx_temp, angle_sjx_temp
        real(r8), intent(inout) :: Extr_sjx_temp(2), Eavg_sjx_temp(2), Savg_sjx_temp
        integer,  intent(inout) :: angleless_temp(:), anglemore_temp(:)

        angle_regularless = 45
        angle_regularmore = 75
        do i = 2, num_sjx, 1 ! 从2开始，因为第一个不存在
            if (adjust_sjx_flag(i) /= 0) then ! 赋值为1的时候才需要更新数据
                sjx = wp_f(ngrmw_f(:, i), 1:2)
                CALL Get_Length_Angle(3, sjx, angle_temp, length_temp)! 计算多边形内角和
                angle_sjx_temp(i, :) = angle_temp
                length_sjx_temp(i, :) = length_temp 
            else
                angle_temp  = angle_sjx_temp(i, :)
                length_temp = length_sjx_temp(i, :)
            end if
            Eavg_sjx_temp(1) = Eavg_sjx_temp(1) + minval(angle_temp)
            Eavg_sjx_temp(2) = Eavg_sjx_temp(2) + maxval(angle_temp)
            Savg_sjx_temp    = Savg_sjx_temp    + sum((angle_temp - 60.)**2)
            if (minval(angle_temp) < angle_regularless) angleless_temp(i) = 1
            if (maxval(angle_temp) > angle_regularmore) anglemore_temp(i) = 1
        end do
        Extr_sjx_temp(1) = minval(angle_sjx_temp(2:num_sjx, :))
        Extr_sjx_temp(2) = maxval(angle_sjx_temp(2:num_sjx, :))
        Eavg_sjx_temp(:) = Eavg_sjx_temp(:) / (num_sjx - 1)
        Savg_sjx_temp    = sqrt(Savg_sjx_temp / (3 * num_sjx - 3))

    END SUBROUTINE TriMeshQuality

    ! 需要修改，增加length的输入输出
    SUBROUTINE PolyMeshQuality(num_edges, num_dbx, mp_f, ngrwm_f, n_ngrwm_f, adjust_dbx_flag, length_dbx_temp, angle_dbx_temp, Extr_dbx_temp, Eavg_dbx_temp, Savg_dbx_temp, angleless_temp, anglemore_temp)

        implicit none ! 定义的顺序最好是只读入的参数，内部参数，需要传出的参数
        integer, intent(in) :: num_edges, num_dbx
        real(r8), dimension(:, :), intent(in) :: mp_f
        integer,  dimension(:, :), intent(in) :: ngrwm_f
        integer,  dimension(:), allocatable, intent(in) :: n_ngrwm_f
        integer,  dimension(:), allocatable, intent(in) :: adjust_dbx_flag
        integer :: i, j
        real(r8) :: angle_regular, angle_regularless, angle_regularmore
        real(r8) :: length_temp(num_edges), angle_temp(num_edges), dbx(num_edges, 2)
        real(r8), dimension(:,:), intent(inout) :: length_dbx_temp, angle_dbx_temp
        real(r8), intent(inout) :: Extr_dbx_temp(2), Eavg_dbx_temp(2), Savg_dbx_temp
        integer,  intent(inout), optional :: angleless_temp(:), anglemore_temp(:)

        j = 0
        angle_regular = (num_edges-2) * 180. / num_edges !正多边形内角度数公式
        angle_regularless = angle_regular * 0.9
        angle_regularmore = angle_regular * 1.1
        do i = 2, num_dbx, 1 ! 为啥这里从2开始，而不是从1开始
            if (n_ngrwm_f(i) /= num_edges) cycle
            j = j + 1 ! 用于计数
            if (adjust_dbx_flag(i) /= 0) then ! 赋值为1的时候才需要更新数据
                dbx = mp_f(ngrwm_f(1:num_edges, i), :)
                CALL Get_Length_Angle(num_edges, dbx, angle_temp, length_temp)! 计算多边形内角和
                angle_dbx_temp(j, :)  = angle_temp
                length_dbx_temp(j, :) = length_temp
            else ! jump out if not change
                angle_temp  = angle_dbx_temp(j, :)
                length_temp = length_dbx_temp(j, :)
            end if
            Eavg_dbx_temp(1) = Eavg_dbx_temp(1) + minval(angle_temp)
            Eavg_dbx_temp(2) = Eavg_dbx_temp(2) + maxval(angle_temp)
            Savg_dbx_temp    = Savg_dbx_temp    + sum((angle_temp - angle_regular)**2)
            if (minval(angle_temp) < angle_regularless) angleless_temp(j) = 1
            if (maxval(angle_temp) > angle_regularmore) anglemore_temp(j) = 1
        end do
        Extr_dbx_temp(1) = minval(angle_dbx_temp)
        Extr_dbx_temp(2) = maxval(angle_dbx_temp)
        Eavg_dbx_temp(:) = Eavg_dbx_temp(:) / j
        Savg_dbx_temp    = sqrt(Savg_dbx_temp / (num_edges * j))

    END SUBROUTINE PolyMeshQuality   

    SUBROUTINE CheckLon(x)

        implicit none

        real(r8), intent(out) :: x

        if (x > 180._r8) then
            x = x - 360._r8
        else if (x < -180._r8) then
            x = x + 360._r8
        end if

    END SUBROUTINE CheckLon

    ! refence to function mpas_arc_length MPAS-Tools/mesh_tools/hex_projection/mpas_geometry_utilities.f90
    real(r8) function arc_length(ax, ay, az, bx, by, bz)
        ! Calculate arc length on unit sphere using Haversine formula
        ! EXACTLY matches MPAS-Tools pnt.h:250-255 including mixed precision
        ! Input: Cartesian coordinates (x, y, z) on unit sphere
        implicit none

        real(r8), intent(in) :: ax, ay, az, bx, by, bz
        real(r8) :: r_a, r_b, lon_a, lat_a, lon_b, lat_b
        real(r8) :: dlat_half, dlon_half, sin_dlat_half, sin_dlon_half
        real(r8) :: term1, term2, arg
        real(4) :: sin_dlat_half_f32, sin_dlon_half_f32  ! For powf emulation

        ! Convert Cartesian to spherical coordinates
        r_a = sqrt(ax*ax + ay*ay + az*az)
        r_b = sqrt(bx*bx + by*by + bz*bz)

        lon_a = atan2(ay, ax)
        lat_a = asin(az / r_a)
        lon_b = atan2(by, bx)
        lat_b = asin(bz / r_b)

        ! Haversine formula exactly matching MPAS-Tools pnt.h:253-255
        ! MPAS uses: arg = sqrt(powf(sin(0.5*(lat1-lat2)),2) +
        !                      cos(lat2)*cos(lat1)*powf(sin(0.5*(lon1-lon2)),2))
        ! powf is single-precision power, which we emulate with REAL(4)

        dlat_half = 0.5_r8 * (lat_a - lat_b)
        dlon_half = 0.5_r8 * (lon_a - lon_b)

        ! Compute sin values
        sin_dlat_half = sin(dlat_half)
        sin_dlon_half = sin(dlon_half)

        ! Emulate powf(x, 2) behavior: convert to float32, square, convert back
        sin_dlat_half_f32 = real(sin_dlat_half, kind=4)
        term1 = real(sin_dlat_half_f32 * sin_dlat_half_f32, kind=8)

        sin_dlon_half_f32 = real(sin_dlon_half, kind=4)
        term2 = cos(lat_b) * cos(lat_a) * real(sin_dlon_half_f32 * sin_dlon_half_f32, kind=8)

        arg = sqrt(term1 + term2)
        arc_length = r_a * 2.0_r8 * asin(arg)

    end function arc_length

    SUBROUTINE Get_Length_Angle(num_edges, dbx, angle_dbx, length_dbx) 
        ! 适用于三角形与五六七边形的球面边长和内角计算形式  
        implicit none
    
        integer, intent(in) :: num_edges
        real(r8), intent(in) :: dbx(num_edges, 2) 
        real(r8), intent(out) :: angle_dbx(num_edges)
        real(r8), intent(out), optional :: length_dbx(num_edges) ! 最后传入的边数一致
        integer :: i
        real(r8) :: length(3), sjx(3, 2), len, x(3), y(3), z(3)
        real(r8), allocatable :: dbx_cycle(:, :)

        sjx = 0.
        allocate(dbx_cycle(num_edges + 2, 2))
        dbx_cycle(1, :)             = dbx(num_edges, :)
        dbx_cycle(2:num_edges+1, :) = dbx(1:num_edges, :)
        dbx_cycle(num_edges+2, :)   = dbx(1, :)
        do i = 1, num_edges, 1
            sjx = dbx_cycle(i:i + 2, :)
            ! 先计算长度，再计算角度
            CALL lonlat2xyz(3, sjx(:, 1), sjx(:, 2), x, y, z)
            length(1) = arc_length(x(3), y(3), z(3), x(2), y(2), z(2))
            length(2) = arc_length(x(3), y(3), z(3), x(1), y(1), z(1))
            length(3) = arc_length(x(1), y(1), z(1), x(2), y(2), z(2))
            len = 0.5_r8 * sum(length)
            angle_dbx(i) = sqrt( sin(len-length(1))*sin(len-length(3)) / (sin(length(1)) * sin(length(3))) )
            angle_dbx(i) = 2 * asin(angle_dbx(i)) * piu180_r8 ! 弧度转为角度
            length_dbx(i)= length(1) * erad8 ! 只记录第一个， 实际长度返回在地球上的数值
        end do
        deallocate(dbx_cycle)

    END SUBROUTINE Get_Length_Angle

    ! same as routine-mpas_triangle_signed_area_sphere in the github ！
    ! max(0.0, tanqe) ! refence to function mpas_triangle_signed_area_sphere MPAS-Tools/mesh_tools/hex_projection/mpas_geometry_utilities.f90
    SUBROUTINE GetArea(num_sjx, num_dbx, num_edge, xem, yem, zem, xew, yew, zew, xev, yev, zev, cellsOnVertex, edgesOnVertex, cellsOnEdge, &
        verticesOnCell, nEdgesOnCell, kiteAreasOnVertex, areaTriangle, areaCell)

        implicit none
        integer,  intent(in) :: num_sjx, num_dbx, num_edge
        real(r8), intent(in) :: xem(:), yem(:), zem(:), xew(:), yew(:), zew(:), xev(:), yev(:), zev(:)
        integer,  intent(in) :: cellsOnVertex(:,:), edgesOnVertex(:,:), cellsOnEdge(:,:)
        integer,  intent(in) :: verticesOnCell(:, :), nEdgesOnCell(:)
        real(r8), intent(inout) :: kiteAreasOnVertex(:,:), areaTriangle(:), areaCell(:)
        integer :: i, j, k, num_edges, cell1, edge1, edge2, iCell, icv, vertexDegree, ie, iep1
        integer :: cell1Edge1, cell2Edge1, cell1Edge2, cell2Edge2
        integer :: num(3)
        real(r8) :: a, b, c, s, tanqe, x(3), y(3), z(3), temp, temp_relative
        real(r8) :: error_max, error_avg
        real(r8) :: xVertex(3), xEdge1(3), xEdge2(3), xCell(3), tri1_area, tri2_area

        ! MPAS-Tools algorithm: Loop through consecutive edge pairs to compute kiteAreasOnVertex
        ! Following implementation from MPAS-Tools/mesh_tools/hex_projection/ph_utils.f90:set_kiteAreasOnVertex
        vertexDegree = 3
        do i = 2, num_sjx, 1
            xVertex(1) = xem(i); xVertex(2) = yem(i); xVertex(3) = zem(i)

            ! DEBUG: Print debug info for vertex 81458
            if (i == 81458) then
                write(io6,*) "=== DEBUG: Processing vertex 81458 ==="
                write(io6,*) "cellsOnVertex:", cellsOnVertex(:,i)
                write(io6,*) "edgesOnVertex:", edgesOnVertex(:,i)
            end if

            ! Loop through consecutive edge pairs around this vertex
            do ie = 1, vertexDegree
                iep1 = ie + 1
                if (iep1 > vertexDegree) iep1 = 1  ! Wrap around

                edge1 = edgesOnVertex(ie, i)
                edge2 = edgesOnVertex(iep1, i)

                if ((edge1 > 0) .and. (edge2 > 0)) then
                    ! Find the cell shared by edge1 and edge2
                    ! MPAS-Tools checks all 4 combinations (ph_utils.f90:810-814)
                    cell1Edge1 = cellsOnEdge(1, edge1)
                    cell2Edge1 = cellsOnEdge(2, edge1)
                    cell1Edge2 = cellsOnEdge(1, edge2)
                    cell2Edge2 = cellsOnEdge(2, edge2)

                    iCell = 0
                    if (cell1Edge1 == cell1Edge2) iCell = max(cell1Edge1, iCell)
                    if (cell1Edge1 == cell2Edge2) iCell = max(cell1Edge1, iCell)
                    if (cell2Edge1 == cell1Edge2) iCell = max(cell2Edge1, iCell)
                    if (cell2Edge1 == cell2Edge2) iCell = max(cell2Edge1, iCell)

                    if (iCell > 0) then
                        ! Find the index of iCell in cellsOnVertex for this vertex
                        icv = 0
                        do k = 1, vertexDegree
                            if (cellsOnVertex(k, i) == iCell) then
                                icv = k
                                exit
                            end if
                        end do

                        if (icv > 0) then
                            ! Get coordinates for the two triangles forming the kite
                            xEdge1(1) = xev(edge1); xEdge1(2) = yev(edge1); xEdge1(3) = zev(edge1)
                            xEdge2(1) = xev(edge2); xEdge2(2) = yev(edge2); xEdge2(3) = zev(edge2)
                            xCell(1) = xew(iCell); xCell(2) = yew(iCell); xCell(3) = zew(iCell)

                            ! Compute kite area = abs(tri(V,E1,C)) + abs(tri(V,E2,C))
                            x(1:3) = xVertex(1:3)
                            y(1:3) = (/xEdge1(2), xEdge2(2), xCell(2)/)
                            z(1:3) = (/xEdge1(3), xEdge2(3), xCell(3)/)

                            ! Triangle 1: Vertex, Edge1, Cell
                            x(1) = xVertex(1); y(1) = xVertex(2); z(1) = xVertex(3)
                            x(2) = xEdge1(1); y(2) = xEdge1(2); z(2) = xEdge1(3)
                            x(3) = xCell(1); y(3) = xCell(2); z(3) = xCell(3)
                            tri1_area = abs(triangle_signed_area_sphere(x, y, z))

                            ! Triangle 2: Vertex, Edge2, Cell
                            x(2) = xEdge2(1); y(2) = xEdge2(2); z(2) = xEdge2(3)
                            tri2_area = abs(triangle_signed_area_sphere(x, y, z))

                            ! Assign to correct position in kiteAreasOnVertex
                            kiteAreasOnVertex(icv, i) = tri1_area + tri2_area

                            ! DEBUG: Print kite area details for vertex 81458
                            if (i == 81458) then
                                write(io6,*) "  ie=", ie, " edge1=", edge1, " edge2=", edge2
                                write(io6,*) "  iCell=", iCell, " icv=", icv
                                write(io6,*) "  tri1_area=", tri1_area
                                write(io6,*) "  tri2_area=", tri2_area
                                write(io6,*) "  kiteArea=", tri1_area + tri2_area
                            end if
                        end if
                    end if
                end if
            end do

            ! DEBUG: Print final kiteAreasOnVertex for vertex 81458
            if (i == 81458) then
                write(io6,*) "Final kiteAreasOnVertex:", kiteAreasOnVertex(:,i)
                write(io6,*) "=== END DEBUG vertex 81458 ==="
            end if
        end do

        do i = 2, num_sjx, 1
            areaTriangle(i) = sum(kiteAreasOnVertex(:, i)) ! 这样计算的结果与MPAS官方网格误差最小
        end do

        error_max = 0.0_r8
        error_avg = 0.0_r8
        do i = 2, num_sjx, 1
            num = cellsOnVertex(1:3, i)
            x = xew(num); y = yew(num); z = zew(num)
            temp = triangle_signed_area_sphere(x, y, z)
            temp_relative = abs(areaTriangle(i) - temp) / temp
            error_max = max(error_max, temp_relative)
            error_avg = error_avg + temp_relative
            ! areaTriangle(i) = temp
        end do

        error_avg = error_avg / (num_sjx-1)
        write(io6,*) " max reconstruction relative error areaTriangle ",error_max
        write(io6,*) " avg abs reconstruction relative error areaTriangle ",error_avg
        write(io6,*) ""

        do i = 2, num_dbx, 1
            num_edges = nEdgesOnCell(i)
            num(1) = verticesOnCell(1, i)
            do j = 1, num_edges-2, 1
                num(2:3) = verticesOnCell(j + 1:j + 2, i)
                x = xem(num); y = yem(num); z = zem(num)
                areaCell(i) = areaCell(i) + triangle_signed_area_sphere(x, y, z)
            end do
        end do

    END SUBROUTINE GetArea

    ! 为了不出现负值，请保证逆时针排序
    real(r8) function triangle_signed_area_sphere(x, y, z)

        real(r8), dimension(3), intent(in) :: x, y, z
        real(r8) :: a, b, c, s, tanqe

        a = arc_length(x(3), y(3), z(3), x(2), y(2), z(2))
        b = arc_length(x(3), y(3), z(3), x(1), y(1), z(1))
        c = arc_length(x(1), y(1), z(1), x(2), y(2), z(2))
        s = (a + b + c)/2.0_r8
        tanqe = tan(s/2.0_r8) * tan((s-a)/2.0_r8) * tan((s-b)/2.0_r8) * tan((s-c)/2.0_r8)
        triangle_signed_area_sphere = 4.0_r8 * atan(sqrt(max(0.0_r8, tanqe)))

    end function triangle_signed_area_sphere

    ! vp的计算精度不够
    SUBROUTINE GetEdge(num_sjx, wp, ngrmm, cellsOnVertex, cellsOnEdge, verticesOnEdge, edgesOnVertex, vp)

        implicit none
        integer, intent(in) :: num_sjx
        real(r8),allocatable, intent(in) :: wp(:,:)
        integer, allocatable, intent(in) :: ngrmm(:,:), cellsOnVertex(:,:)
        integer, allocatable, intent(inout) :: cellsOnEdge(:,:), verticesOnEdge(:,:), edgesOnVertex(:,:)
        real(r8),allocatable, intent(inout), optional :: vp(:,:)
        integer :: i, j, k, m, n, ij, ii, jj, temp(3), ref_temp, nEdge
        integer, allocatable :: sjxID(:) ! 用于标记三角形ID是否已经被使用了，0为未使用
        real(r8) :: a(2), b(2), lon_deg(2), lat_deg(2), centroid_deg(2) ! x(2), y(2), z(2), centroid(3)

        k = 1
        allocate(sjxID(num_sjx)); sjxID = 0
        do i = 2, num_sjx, 1
            do m = 1, 3, 1
                j = ngrmm(m, i) ! 获取三角形编号
                if (sjxID(j) == 1) then
                    do n = 1, 3, 1
                        if (ngrmm(n, j) == i) exit! 沿用边的旧编号, 找到合适的n退出
                    end do
                    edgesOnVertex(m, i) = edgesOnVertex(n, j)
                    cycle
                else
                    k = k + 1
                    edgesOnVertex(m, i) = k ! 获取边的新编号，从2开始，与三角形/多边形顶点编号起点类似
                end if
                verticesOnEdge(:, k) = [i, j]! 获取三角形中心点的编号
                ! 获取cellsOnEdge
                ij = IsNgrmm(cellsOnVertex(1:3, i), cellsOnVertex(1:3, j)) ! 剩下两个顶点就是相接的顶点编号
                if (ij == 1) then
                    ii = cellsOnVertex(2, i); jj = cellsOnVertex(3, i) ! 获取多边形中心点的编号
                else if (ij == 2) then
                    ii = cellsOnVertex(3, i); jj = cellsOnVertex(1, i) ! 获取多边形中心点的编号
                else if (ij == 3) then
                    ii = cellsOnVertex(1, i); jj = cellsOnVertex(2, i) ! 获取多边形中心点的编号
                else
                    write(io6, *) "ij = ", ij
                    STOP "ERROR!"
                end if

                ! 有必要的设置但是不是影响精度的主要原因
                ! 这是MPAS才有的要求，不一定适用于EarthMesh
                if (ii < jj) then ! make sure cellsOnEdge(1, k) less than cellsOnEdge(2,k)
                    cellsOnEdge(:, k) = [ii, jj]
                else
                    cellsOnEdge(:, k) = [jj, ii]
                end if
            end do
            sjxID(i) = 1 ! 使用结束后进行标记
        end do
        deallocate(sjxID)

        if (.not. present(vp)) return
        nEdge = size(cellsOnEdge, 2)
        do k = 2, nEdge, 1
            ! 求边的坐标的时候用的居然是相邻两个多边形中心坐标而不是三角形中心坐标
            a = wp(cellsOnEdge(1, k), :)!  * pio180_r8 ! 获取多边形中心经纬度
            b = wp(cellsOnEdge(2, k), :)!  * pio180_r8 ! 获取多边形中心经纬度
            lon_deg = [a(1), b(1)]
            lat_deg = [a(2), b(2)]
            CALL centroid_spherical_single(2, lon_deg, lat_deg, centroid_deg)
            vp(k, :) = centroid_deg
        end do

    END SUBROUTINE GetEdge

    SUBROUTINE GetSort_verticesOnEdge(num_edge, mp, wp, cellsOnEdge, verticesOnEdge)

        IMPLICIT NONE
        integer, intent(in) :: num_edge
        integer, allocatable, intent(in) :: cellsOnEdge(:,:)
        integer, allocatable, intent(inout) :: verticesOnEdge(:,:)
        real(r8), allocatable, intent(in) :: mp(:,:), wp(:,:)
        integer :: i, idx
        real(r8) :: tempa(2), tempb(2), tempc(2), tempd(2), res


        do i = 2, num_edge, 1
            tempa = wp(cellsOnEdge(1, i), :)
            tempb = wp(cellsOnEdge(2, i), :)
            tempc = mp(verticesOnEdge(1, i), :)
            tempd = mp(verticesOnEdge(2, i), :)
            tempb = tempb - tempa
            tempd = tempd - tempc

            if (tempb(1) > 180.) tempb(1) = tempb(1) - 360.
            if (tempb(1) <-180.) tempb(1) = tempb(1) + 360.
            if (tempd(1) > 180.) tempd(1) = tempd(1) - 360.
            if (tempd(1) <-180.) tempd(1) = tempd(1) + 360.

            res = tempb(1) * tempd(2) - tempb(2) * tempd(1)
            if (res > 0) cycle ! 计算叉乘方向
            idx                  = verticesOnEdge(1, i)
            verticesOnEdge(1, i) = verticesOnEdge(2, i)
            verticesOnEdge(2, i) = idx
        end do

    END SUBROUTINE GetSort_verticesOnEdge

    ! Order cellsOnVertex and edgesOnVertex counter-clockwise (matching MPAS-Tools)
    ! Reference: MPAS-Tools mpas_mesh_converter.cpp orderVertexArrays() lines 1320-1460
    SUBROUTINE orderVertexArrays(num_sjx, xem, yem, zem, xev, yev, zev, &
                                   verticesOnEdge, cellsOnEdge, edgesOnVertex, cellsOnVertex)

        IMPLICIT NONE
        integer, intent(in) :: num_sjx
        real(r8), intent(in) :: xem(:), yem(:), zem(:)
        real(r8), intent(in) :: xev(:), yev(:), zev(:)
        integer, intent(in) :: verticesOnEdge(:,:), cellsOnEdge(:,:)
        integer, intent(inout) :: edgesOnVertex(:,:), cellsOnVertex(:,:)

        integer :: iVertex, j, k, edge1, edge2, swp_idx, swp
        real(r8) :: vec1(3), vec2(3), normal(3), cross_prod(3)
        real(r8) :: mag1, mag2, angle, min_angle, dot_val, cross_mag, normal_mag
        real(r8), parameter :: pi2 = 2.0_r8 * pi_r8

        ! Loop over all vertices
        do iVertex = 2, num_sjx
            ! Normal vector for spherical surface = vertex position
            normal(1) = xem(iVertex)
            normal(2) = yem(iVertex)
            normal(3) = zem(iVertex)
            normal_mag = sqrt(normal(1)**2 + normal(2)**2 + normal(3)**2)

            ! Sort edgesOnVertex counter-clockwise
            do j = 1, 3
                if (edgesOnVertex(j, iVertex) <= 0) cycle

                edge1 = edgesOnVertex(j, iVertex)

                ! Vector from vertex to edge1
                vec1(1) = xev(edge1) - xem(iVertex)
                vec1(2) = yev(edge1) - yem(iVertex)
                vec1(3) = zev(edge1) - zem(iVertex)
                mag1 = sqrt(vec1(1)**2 + vec1(2)**2 + vec1(3)**2)

                min_angle = pi2
                swp_idx = -1

                ! Find next edge in CCW direction
                do k = j+1, 3
                    if (edgesOnVertex(k, iVertex) <= 0) cycle

                    edge2 = edgesOnVertex(k, iVertex)

                    ! Vector from vertex to edge2
                    vec2(1) = xev(edge2) - xem(iVertex)
                    vec2(2) = yev(edge2) - yem(iVertex)
                    vec2(3) = zev(edge2) - zem(iVertex)
                    mag2 = sqrt(vec2(1)**2 + vec2(2)**2 + vec2(3)**2)

                    ! Cross product vec1 × vec2
                    cross_prod(1) = vec1(2)*vec2(3) - vec1(3)*vec2(2)
                    cross_prod(2) = vec1(3)*vec2(1) - vec1(1)*vec2(3)
                    cross_prod(3) = vec1(1)*vec2(2) - vec1(2)*vec2(1)

                    cross_mag = sqrt(cross_prod(1)**2 + cross_prod(2)**2 + cross_prod(3)**2)

                    ! Check if edge2 is CCW from edge1
                    ! dot = cross.dot(normal) / (|cross| * |normal|)
                    if (cross_mag > 1.0e-15_r8 .and. normal_mag > 1.0e-15_r8) then
                        dot_val = (cross_prod(1)*normal(1) + cross_prod(2)*normal(2) + cross_prod(3)*normal(3)) / &
                                  (cross_mag * normal_mag)

                        if (dot_val > 0.0_r8) then  ! CCW direction
                            ! Compute angle between vec1 and vec2
                            angle = acos(max(-1.0_r8, min(1.0_r8, &
                                    (vec1(1)*vec2(1) + vec1(2)*vec2(2) + vec1(3)*vec2(3)) / (mag1 * mag2))))

                            if (angle < min_angle) then
                                min_angle = angle
                                swp_idx = k
                            end if
                        end if
                    end if
                end do

                ! Swap to put next edge in correct position
                if (swp_idx /= -1 .and. swp_idx /= j+1) then
                    swp = edgesOnVertex(j+1, iVertex)
                    edgesOnVertex(j+1, iVertex) = edgesOnVertex(swp_idx, iVertex)
                    edgesOnVertex(swp_idx, iVertex) = swp
                end if
            end do

            ! Rebuild cellsOnVertex based on ordered edgesOnVertex
            ! This ensures cellsOnVertex is also in CCW order
            do j = 1, 3
                if (edgesOnVertex(j, iVertex) <= 0) cycle

                edge1 = edgesOnVertex(j, iVertex)

                ! Get cell on correct side of edge
                ! If vertex is first vertex of edge, take first cell
                ! Otherwise, take second cell
                if (iVertex == verticesOnEdge(1, edge1)) then
                    cellsOnVertex(j, iVertex) = cellsOnEdge(1, edge1)
                else
                    cellsOnVertex(j, iVertex) = cellsOnEdge(2, edge1)
                end if
            end do
        end do

        ! Normalize rotation: rotate so minimum cell ID is at position 1
        ! VERIFIED: MPAS-Tools does NOT do rotation normalization!
        ! Enabling this makes errors WORSE (64% vs 52%)
        ! call normalizeRotation(num_sjx, cellsOnVertex, edgesOnVertex)

        write(io6,*) "orderVertexArrays: Completed CCW ordering of edgesOnVertex and cellsOnVertex"

    END SUBROUTINE orderVertexArrays

    ! Normalize rotation of cellsOnVertex and edgesOnVertex
    ! Rotate arrays so the minimum cell ID is at position 1
    SUBROUTINE normalizeRotation(num_sjx, cellsOnVertex, edgesOnVertex)

        IMPLICIT NONE
        integer, intent(in) :: num_sjx
        integer, intent(inout) :: cellsOnVertex(:,:), edgesOnVertex(:,:)

        integer :: iVertex, j, min_pos, min_cell
        integer :: cells_temp(3), edges_temp(3)

        do iVertex = 2, num_sjx
            ! Find position of minimum cell ID
            min_cell = cellsOnVertex(1, iVertex)
            min_pos = 1

            do j = 2, 3
                if (cellsOnVertex(j, iVertex) > 0) then
                    if (min_cell <= 0 .or. cellsOnVertex(j, iVertex) < min_cell) then
                        min_cell = cellsOnVertex(j, iVertex)
                        min_pos = j
                    end if
                end if
            end do

            ! If minimum is not at position 1, rotate
            if (min_pos /= 1 .and. min_cell > 0) then
                ! Save original arrays
                cells_temp = cellsOnVertex(:, iVertex)
                edges_temp = edgesOnVertex(:, iVertex)

                ! Rotate to put minimum at position 1
                if (min_pos == 2) then
                    ! [A, B, C] -> [B, C, A]
                    cellsOnVertex(1, iVertex) = cells_temp(2)
                    cellsOnVertex(2, iVertex) = cells_temp(3)
                    cellsOnVertex(3, iVertex) = cells_temp(1)

                    edgesOnVertex(1, iVertex) = edges_temp(2)
                    edgesOnVertex(2, iVertex) = edges_temp(3)
                    edgesOnVertex(3, iVertex) = edges_temp(1)
                else if (min_pos == 3) then
                    ! [A, B, C] -> [C, A, B]
                    cellsOnVertex(1, iVertex) = cells_temp(3)
                    cellsOnVertex(2, iVertex) = cells_temp(1)
                    cellsOnVertex(3, iVertex) = cells_temp(2)

                    edgesOnVertex(1, iVertex) = edges_temp(3)
                    edgesOnVertex(2, iVertex) = edges_temp(1)
                    edgesOnVertex(3, iVertex) = edges_temp(2)
                end if
            end if
        end do

    END SUBROUTINE normalizeRotation

    ! Reference: MPAS-Tools mpas_mesh_converter.cpp firstOrderingVerticesOnCell() lines 590-750
    SUBROUTINE orderVerticesOnCell(num_dbx, xew, yew, zew, xem, yem, zem, &
                                   verticesOnCell, nEdgesOnCell)
        ! Order vertices around each cell in CCW direction using MPAS-Tools algorithm
        ! Based on geometric angle sorting with cross product to determine CCW direction

        IMPLICIT NONE
        integer, intent(in) :: num_dbx
        real(r8), intent(in) :: xew(:), yew(:), zew(:)  ! cell coordinates
        real(r8), intent(in) :: xem(:), yem(:), zem(:)  ! vertex coordinates
        integer, intent(inout) :: verticesOnCell(:,:)
        integer, intent(in) :: nEdgesOnCell(:)

        integer :: iCell, j, k, iVertex1, iVertex2, swp_idx, swp, ne
        real(r8) :: vec1(3), vec2(3), cross(3), normal(3), cell_center(3)
        real(r8) :: vertex1(3), vertex2(3)
        real(r8) :: mag1, mag2, dot_product, angle, min_angle
        real(r8) :: dot_val_clamped
        real(r8), parameter :: PI = 3.141592653589793_r8

        write(io6,*) "orderVerticesOnCell: Starting MPAS-Tools style CCW ordering..."

        do iCell = 2, num_dbx
            ne = nEdgesOnCell(iCell)
            if (ne <= 0) cycle

            ! Cell center coordinates (for spherical mesh, this is the normal direction)
            cell_center(1) = xew(iCell)
            cell_center(2) = yew(iCell)
            cell_center(3) = zew(iCell)

            ! Normalize to get unit normal vector
            normal = cell_center / sqrt(cell_center(1)**2 + cell_center(2)**2 + cell_center(3)**2)

            ! Selection sort: find next CCW vertex at each step
            do j = 1, ne-1
                iVertex1 = verticesOnCell(j, iCell)
                if (iVertex1 <= 0) cycle

                vertex1(1) = xem(iVertex1)
                vertex1(2) = yem(iVertex1)
                vertex1(3) = zem(iVertex1)

                vec1 = vertex1 - cell_center
                mag1 = sqrt(vec1(1)**2 + vec1(2)**2 + vec1(3)**2)

                min_angle = 2.0_r8 * PI
                swp_idx = -1

                ! Find next CCW vertex among remaining unsorted vertices
                do k = j+1, ne
                    iVertex2 = verticesOnCell(k, iCell)
                    if (iVertex2 <= 0) cycle

                    vertex2(1) = xem(iVertex2)
                    vertex2(2) = yem(iVertex2)
                    vertex2(3) = zem(iVertex2)

                    vec2 = vertex2 - cell_center
                    mag2 = sqrt(vec2(1)**2 + vec2(2)**2 + vec2(3)**2)

                    ! Cross product: cross = vec1 × vec2
                    cross(1) = vec1(2)*vec2(3) - vec1(3)*vec2(2)
                    cross(2) = vec1(3)*vec2(1) - vec1(1)*vec2(3)
                    cross(3) = vec1(1)*vec2(2) - vec1(2)*vec2(1)

                    ! Dot product to check CCW direction: cross · normal
                    dot_product = cross(1)*normal(1) + cross(2)*normal(2) + cross(3)*normal(3)

                    ! Only consider CCW vertices (dot_product > 0)
                    if (dot_product > 0.0_r8) then
                        ! Calculate angle between vec1 and vec2
                        ! Clamp to avoid acos domain errors
                        dot_val_clamped = (vec1(1)*vec2(1) + vec1(2)*vec2(2) + vec1(3)*vec2(3)) / (mag1 * mag2)
                        if (dot_val_clamped > 1.0_r8) dot_val_clamped = 1.0_r8
                        if (dot_val_clamped < -1.0_r8) dot_val_clamped = -1.0_r8

                        angle = acos(dot_val_clamped)

                        if (angle < min_angle) then
                            min_angle = angle
                            swp_idx = k
                        end if
                    end if
                end do

                ! Swap vertex with minimum angle to position j+1
                if (swp_idx /= -1 .and. swp_idx /= j+1) then
                    swp = verticesOnCell(j+1, iCell)
                    verticesOnCell(j+1, iCell) = verticesOnCell(swp_idx, iCell)
                    verticesOnCell(swp_idx, iCell) = swp
                end if
            end do
        end do

        write(io6,*) "orderVerticesOnCell: CCW ordering complete"

    END SUBROUTINE orderVerticesOnCell

    ! Standardize verticesOnCell rotation to start with minimum vertex ID
    ! This reduces floating-point rounding differences in weightsOnEdge calculation
    ! by ensuring all meshes use the same starting vertex position
    SUBROUTINE standardizeVerticesOnCellRotation(num_dbx, verticesOnCell, nEdgesOnCell)
        ! Rotate each cell's verticesOnCell array to start with minimum vertex ID
        ! This ensures consistent accumulation order in weightsOnEdge calculation
        ! Reference: RESIDUAL_ERROR_ANALYSIS.md section 6.2

        IMPLICIT NONE
        integer, intent(in) :: num_dbx
        integer, intent(inout) :: verticesOnCell(:,:)
        integer, intent(in) :: nEdgesOnCell(:)

        integer :: iCell, j, ne, min_vertex_id, min_pos
        integer, allocatable :: vertices_temp(:)
        integer :: maxEdges

        write(io6,*) "standardizeVerticesOnCellRotation: Starting rotation standardization..."

        ! Get maximum number of edges for temp array allocation
        maxEdges = maxval(nEdgesOnCell)
        allocate(vertices_temp(maxEdges))

        do iCell = 2, num_dbx
            ne = nEdgesOnCell(iCell)
            if (ne <= 0) cycle

            ! Find position of minimum vertex ID
            min_vertex_id = HUGE(1)
            min_pos = 1

            do j = 1, ne
                if (verticesOnCell(j, iCell) > 0 .and. verticesOnCell(j, iCell) < min_vertex_id) then
                    min_vertex_id = verticesOnCell(j, iCell)
                    min_pos = j
                end if
            end do

            ! Rotate array if minimum is not at position 1
            if (min_pos /= 1) then
                ! Save current vertices to temp array
                vertices_temp(1:ne) = verticesOnCell(1:ne, iCell)

                ! Rotate: [min_pos:ne] goes to beginning, [1:min_pos-1] goes to end
                verticesOnCell(1:ne-min_pos+1, iCell) = vertices_temp(min_pos:ne)
                if (min_pos > 1) then
                    verticesOnCell(ne-min_pos+2:ne, iCell) = vertices_temp(1:min_pos-1)
                end if
            end if
        end do

        deallocate(vertices_temp)

        write(io6,*) "standardizeVerticesOnCellRotation: Rotation standardization complete"

    END SUBROUTINE standardizeVerticesOnCellRotation

    SUBROUTINE Get_ConnectOnCell(num_dbx, nEdgesOnCell, cellsOnEdge, edgesOnVertex, verticesOnCell, edgesOnCell, cellsOnCell)
        integer, intent(in) :: num_dbx
        integer, allocatable, intent(in) :: nEdgesOnCell(:), cellsOnEdge(:,:)
        integer, allocatable, intent(in) :: edgesOnVertex(:,:), verticesOnCell(:,:)
        integer, allocatable, intent(inout) :: edgesOnCell(:,:)
        integer, allocatable, intent(inout), optional :: cellsOnCell(:,:)
        integer :: i, j, k, n, vertex1, vertex2, edge1, cell1, cell2, edges_vertex1(3), edges_vertex2(3)

        ! 对edgesOnCell排序
        do i = 2, num_dbx, 1
            n = nEdgesOnCell(i)
            do j = 1, n, 1
                vertex1 = verticesOnCell(j, i)
                vertex2 = verticesOnCell(mod(j, n)+1 , i)
                edges_vertex1 = edgesOnVertex(:, vertex1)
                edges_vertex2 = edgesOnVertex(:, vertex2)
                do k = 1, 3, 1
                    if (any(edges_vertex1(k) == edges_vertex2)) exit
                end do
                if (k == 4) then
                    write(io6, *) "i = ", i
                    write(io6, *) "vertex1 = ", vertex1
                    write(io6, *) "vertex2 = ", vertex2
                    write(io6, *) "edges_vertex1 = ", edges_vertex1
                    write(io6, *) "edges_vertex2 = ", edges_vertex2
                    STOP "WARNNING! EDGES_VERTEX1 has value 4 which is greater than the upper bound of 3"
                else
                    edgesOnCell(j, i) = edges_vertex1(k)
                end if
            end do
        end do
        if (.not. present(cellsOnCell)) return

        ! 对cellsOnCell排序
        do i = 2, num_dbx, 1
            do j = 1, nEdgesOnCell(i), 1
                edge1 = edgesOnCell(j, i)
                cell1 = cellsOnEdge(1, edge1)
                cell2 = cellsOnEdge(2, edge1)
                if (cell1 == i) then
                    cellsOnCell(j, i) = cell2
                else if (cell2 == i) then
                    cellsOnCell(j, i) = cell1
                else
                    STOP "ERROR both cell1 and cell2 /= i"
                end if
            end do
        end do

    END SUBROUTINE Get_ConnectOnCell

    ! angleEdge计算精度不够
    SUBROUTINE Get_Edge_DIS_Angle(num_edge, cellsOnEdge, verticesOnEdge, &
                            xem, yem, zem, xew, yew, zew, xev, yev, zev, latVertex, lonEdge, latEdge, dcEdge, dvEdge, angleEdge)

        IMPLICIT NONE
        integer, intent(in) :: num_edge
        integer, intent(in) :: cellsOnEdge(:,:), verticesOnEdge(:,:)
        real(r8), intent(in) :: xem(:), yem(:), zem(:), xew(:), yew(:), zew(:), xev(:), yev(:), zev(:)
        real(r8), intent(in) :: latVertex(:), lonEdge(:), latEdge(:)
        real(r8), intent(inout) :: dcEdge(:), dvEdge(:), angleEdge(:)
        integer :: iv, im1, im2, iw1, iw2
        real(r8) :: dnv, dnc, x1, y1, z1, x2, y2, z2
        real(r8) :: angle, sign, xe, ye, ze, dlat, lonn, latn, xn, yn, zn, xv, yv, zv
        real(r8) :: pi_r8, norm

        pi_r8 = pi2_r8 * 0.5_r8
        do iv = 2, num_edge, 1
            im1 = verticesOnEdge(1, iv)
            im2 = verticesOnEdge(2, iv)
            iw1 = cellsOnEdge(1, iv)
            iw2 = cellsOnEdge(2, iv)

            ! vertex在MPAS与EarthMesh的定义相反！
            x1 = xem(im1); x2 = xem(im2)
            y1 = yem(im1); y2 = yem(im2)
            z1 = zem(im1); z2 = zem(im2)
            dvEdge(iv) = arc_length(x1, y1, z1, x2, y2, z2)

            x1 = xew(iw1); x2 = xew(iw2)
            y1 = yew(iw1); y2 = yew(iw2)
            z1 = zew(iw1); z2 = zew(iw2)
            dcEdge(iv) = arc_length(x1, y1, z1, x2, y2, z2)

            ! 计算angleEdge，传入三角形顶点的经纬度坐标就好了(相同的代码但是在matlab中结果不一样)
            angle = latVertex(im2) * pio180_r8 - latVertex(im1) * pio180_r8  ! 转为弧度再计算
            angle = angle / dvEdge(iv)
            angle = max(min(angle, 1.0_r8), -1.0_r8)
            angle = acos(angle)

            xe = xev(iv);  ye = yev(iv);  ze = zev(iv)
            xv = xem(im2); yv = yem(im2); zv = zem(im2)
            lonn = lonEdge(iv) * pio180_r8
            latn = (latEdge(iv) + 0.05_r8)  * pio180_r8
            xn = cos(latn)*cos(lonn); yn = cos(latn)*sin(lonn); zn = sin(latn)

            ! Pass edge position as normal vector (matches MPAS-Tools)
            sign = planeAngle(xe, ye, ze, xn, yn, zn, xv, yv, zv, xe, ye, ze)
            ! Normalize sign to ±1 (matches MPAS-Tools mpas_mesh_converter.cpp:2187-2191)
            if (abs(sign) > 1.0e-14_r8) then
                sign = sign / abs(sign)
            else
                sign = 1.0_r8
            end if
            angle = angle * sign
            if (angle > pi_r8) angle = angle - pi2_r8
            if (angle <-pi_r8) angle = angle + pi2_r8
            angleEdge(iv) = angle
        end do

    END SUBROUTINE Get_Edge_DIS_Angle

    real(r8) function planeAngle(ax, ay, az, bx, by, bz, cx, cy, cz, nx, ny, nz)
        ! Computes the angle between vectors AB and AC in a plane defined by normal vector n
        ! Matches MPAS-Tools pnt.h:444-502
        implicit none
        real(r8), intent(in) :: ax, ay, az, bx, by, bz, cx, cy, cz
        real(r8), intent(in) :: nx, ny, nz  ! Normal vector
        real(r8) :: ABx, ABy, ABz    ! The components of the vector AB
        real(r8) :: ACx, ACy, ACz    ! The components of the vector AC
        real(r8) :: Dx               ! The i-components of the cross product AB x AC
        real(r8) :: Dy               ! The j-components of the cross product AB x AC
        real(r8) :: Dz               ! The k-components of the cross product AB x AC
        real(r8) :: mAB, mAC
        real(r8) :: cos_angle

        ABx = bx - ax
        ABy = by - ay
        ABz = bz - az
        mAB = sqrt(ABx*ABx + ABy*ABy + ABz*ABz)

        ACx = cx - ax
        ACy = cy - ay
        ACz = cz - az
        mAC = sqrt(ACx*ACx + ACy*ACy + ACz*ACz)

        cos_angle = (ABx*ACx + ABy*ACy + ABz*ACz) / (mAB * mAC)
        cos_angle = max(min(cos_angle, 1.0_r8), -1.0_r8)

        Dx =   (ABy * ACz) - (ABz * ACy)
        Dy = -((ABx * ACz) - (ABz * ACx))
        Dz =   (ABx * ACy) - (ABy * ACx)
        ! Use normal vector n instead of point A (matches MPAS-Tools)
        if ((Dx*nx + Dy*ny + Dz*nz) >= 0.0_r8) then
            planeAngle =  acos(cos_angle)
        else
            planeAngle = -acos(cos_angle)
        end if

    end function planeAngle

    ! 计算边上的权重
    ! https://github.com/MPAS-Dev/MPAS-Tools/blob/5985d4f79111ce6b71ad7867f13759296980adf1/mesh_tools/hex_projection/ph_utils.f90#L967
    SUBROUTINE set_weightsOnEdge(num_sjx, num_dbx, num_edge, maxEdges, maxEdges2, vertexDegree, &
        areaCell, angleEdge, dcEdge, dvEdge, kiteAreasOnVertex, edgesOnCell, &
        cellsOnVertex, cellsOnEdge, verticesOnCell, verticesOnEdge, nEdgesOnCell, &
        weightsOnEdge, nEdgesOnEdge, edgesOnEdge, error_segment)

        IMPLICIT NONE
        integer, intent(in) :: num_sjx, num_dbx, num_edge, maxEdges, maxEdges2, vertexDegree
        real(r8), intent(in) :: areaCell(:), angleEdge(:), dcEdge(:), dvEdge(:), kiteAreasOnVertex(:,:)
        integer, intent(in) :: edgesOnCell(:,:), nEdgesOnCell(:)
        integer, intent(in) :: cellsOnVertex(:,:), cellsOnEdge(:,:), verticesOnCell(:,:), verticesOnEdge(:,:)
        integer, allocatable, intent(inout) :: edgesOnEdge(:,:), nEdgesOnEdge(:)
        real(r8), allocatable, intent(inout) :: weightsOnEdge(:,:)
        real(r8), allocatable, intent(out) :: error_segment(:)
        integer :: iEdge, iVertex, iCell, cell1, cell2, id, iv, ne, i, in, nw1, ie, ic, j
        integer :: vertexStart, edgeIndexCell
        integer :: vertexIndex1, kahan_i
        real(r8), dimension(maxEdges) :: RivCell
        real(r8), dimension(maxEdges2) :: RivWrap, weightsWrap
        integer, dimension(maxEdges2) :: edgeIndex

        real(r8), dimension(maxEdges2) :: u
        real(r8) :: vEdge, cosa, sina, pii, ve, tev2, error_abs_avg, error_max
        real(r8) :: kahan_sum, kahan_c, kahan_y, kahan_t
        real(r8), parameter :: uZonal = 1.0_r8, uMeridional = 0.0_r8
        ! 自由大气中环流主要是纬向zonal(东西方向)，经向meridional(南北方向)很弱
        allocate(error_segment(num_edge)); error_segment = 0.0_r8
        do iEdge = 2, num_edge, 1
            cell1 = cellsOnEdge(1, iEdge)
            cell2 = cellsOnEdge(2, iEdge)
            nEdgesOnEdge(iEdge) = 0

            do ic = 1, 2
                if (ic == 1) then
                    iCell = cell1
                    vertexStart = verticesOnEdge(2,iEdge)
                    tev2 = -1.0_r8
                    nw1 = 0
                else
                    iCell = cell2
                    vertexStart = verticesOnEdge(1,iEdge)
                    tev2 = 1.0_r8
                end if
                ne = nEdgesOnCell(iCell)

                RivCell(1:maxEdges) = 0.0_r8
                do iv = 1, nEdgesOnCell(iCell) ! use for vertex
                    iVertex = verticesOnCell(iv, iCell)

                    ! FIX: Find iCell in cellsOnVertex to get correct kite area index
                    ! This makes the lookup rotation-independent
                    id = 0
                    do j = 1, vertexDegree
                        if (cellsOnVertex(j, iVertex) == iCell) then
                            id = j
                            exit
                        end if
                    end do

                    if (id == 0) then
                        write(io6,*) "ERROR: Cell not found in cellsOnVertex!"
                        write(io6,*) "  iCell = ", iCell, " iVertex = ", iVertex
                        STOP
                    end if

                    RivCell(iv) = kiteAreasOnVertex(id, iVertex) / areaCell(iCell)
                end do

                ! calculate weights for cell
                ! counterclockwise summation around cell
                
                iv = findIndex(vertexStart, verticesOnCell(:,iCell), ne)
                vertexIndex1 = iv

                weightsWrap(1:MaxEdges2) = 0.0_r8
                RivWrap(1:ne) = RivCell(1:ne)
                RivWrap(ne+1:ne+ne) = RivCell(1:ne) ! why？？not one cell weight calculate

                ! Use Kahan compensated summation for higher precision
                ! This reduces floating-point rounding errors in cumulative sum
                do id = iv, iv+ne-2
                    ! Kahan summation algorithm
                    kahan_sum = 0.0_r8
                    kahan_c = 0.0_r8
                    do kahan_i = iv, id
                        kahan_y = RivWrap(kahan_i) - kahan_c
                        kahan_t = kahan_sum + kahan_y
                        kahan_c = (kahan_t - kahan_sum) - kahan_y
                        kahan_sum = kahan_t
                    end do

                    weightsWrap(id) = (kahan_sum - 0.5_r8)*tev2
                    weightsOnEdge(nw1+id-iv+1,iEdge) = weightsWrap(id)
                end do

                ! find iEdge index in edgesOnCell for cell
                edgeIndexCell = findIndex(iEdge, edgesOnCell(:,iCell), nEdgesOnCell(iCell))
                ne = nEdgesOnCell(iCell)
                edgeIndex(1:ne) = edgesOnCell(1:ne,iCell)
                edgeIndex(ne+1:2*ne) = edgeIndex(1:ne)
                do i = 1, ne-1
                    in = i + nw1
                    ie = edgeIndex(i+edgeIndexCell)
                    EdgesOnEdge(in,iEdge) = ie
                    weightsOnEdge(in,iEdge) = weightsOnEdge(in,iEdge)*dvEdge(ie)/dcEdge(iEdge)
                    ! check for inflow - switch sign on weight accordingly
                    if (cellsOnEdge(2, ie) .eq. iCell) then
                        weightsOnEdge(in, iEdge) = -weightsOnEdge(in, iEdge)
                    end if
                end do

                nw1 = ne - 1
                nEdgesOnEdge(iEdge) = nEdgesOnEdge(iEdge) + nw1

            end do
        end do

        !  test to see if weights are correct

        do iEdge = 2, num_edge

            vEdge = 0.0_r8
            do i=1, nEdgesOnEdge(iEdge)
                ! veolicities on edges contributing to iEdge.  U is norm component
                cosa = cos(angleEdge(EdgesOnEdge(i, iEdge)))
                sina = sin(angleEdge(EdgesOnEdge(i, iEdge)))
                u(i) =  uZonal*cosa + uMeridional*sina
                vEdge = vEdge + u(i)*weightsOnEdge(i,iEdge)
            end do

            cosa = cos(angleEdge(iEdge))
            sina = sin(angleEdge(iEdge))
            ve = -uZonal*sina + uMeridional*cosa
            error_segment(iEdge) = abs(vEdge - ve)

        end do

        error_abs_avg = sum(error_segment(2:num_edge))/ (num_edge-1)
        error_max = maxval(error_segment)
        write(io6,*) " max reconstruction error ", error_max
        write(io6,*) " avg abs reconstruction error ", error_abs_avg
        write(io6,*) ""

        contains

        integer function findIndex( index, indices, nIndices )

            ! find index in list of indices, return index location

            implicit none
            integer :: index, nIndices
            integer, dimension(num_edge) :: Indices
            integer :: i

            findIndex = 0
            do i=1, nIndices
            if(indices(i) .eq. index) findIndex = i
            end do

        end function findIndex

    END SUBROUTINE set_weightsOnEdge


    SUBROUTINE CheckCrossing(num_edges, points)

        implicit none

        integer, intent(in) :: num_edges
        real(r8), dimension(num_edges, 2), intent(inout) :: points
        ! real, dimension(num_edges, 2), intent(inout) :: points
        integer :: j
        do j = 1, num_edges, 1
            if(points(j, 1) < 0.)then
                points(j, 1) = points(j, 1) + 180.
            else
                points(j, 1) = points(j, 1) - 180.
            end if
        end do

    END SUBROUTINE CheckCrossing

    INTEGER FUNCTION IsNgrmm(a, b)
        ! 三角形中心点的相邻关系，0为不相邻，1，2，3为相邻的边不同
        IMPLICIT NONE
        integer, intent(in) :: a(3), b(3)! 三角形三个顶点的编号

        IsNgrmm = 0
        if (any(a(1) == b)) then ! 检查第一个顶点
            if (any(a(2) == b)) then
                IsNgrmm = 3 ! 说明是顶点3的对边相邻
                return
            else if (any(a(3) == b)) then
                IsNgrmm = 2 ! 说明是顶点2的对边相邻
                return
            end if
        else if (any(a(2) == b)) then ! 检查第二个顶点
            if (any(a(3) == b)) then
                IsNgrmm = 1 ! 说明是顶点1的对边相邻
                return
            end if
        end if

    END FUNCTION IsNgrmm


    SUBROUTINE GetSortNew(num_dbx, n_ngrwm, ngrmw, mp, ngrwm)
        USE consts_coms, only : pi_r8, pio180_r8
        implicit none
        integer, intent(in) :: num_dbx
        integer, allocatable, intent(in) :: n_ngrwm(:), ngrmw(:,:)
        real(r8),allocatable, intent(in) :: mp(:,:)
        integer, allocatable, intent(inout) :: ngrwm(:,:) ! 顶点w的相邻点m的逆时针排序输出
        integer :: i, j, k, mj, ik, ref_temp, num_inter, maxnum
        integer :: start_pos, next_pos
        real(r8) :: area 
        logical :: found
        integer, allocatable :: Isused(:), ngrwm_temp1(:), ngrwm_temp2(:)
        integer, allocatable :: neighbor_degree(:)
        real(r8),allocatable :: lon(:), lat(:)

        maxnum = maxval(n_ngrwm) ! 记录多边形最大的顶点个数
        allocate(Isused(maxnum)) ! 用于记录该三角形是否已经被使用
        allocate(ngrwm_temp1(maxnum)) ! 用于临时记录旧ngrwm(i)
        allocate(ngrwm_temp2(maxnum)) ! 用于临时存储新ngrwm(i)
        allocate(neighbor_degree(maxnum))
        allocate(lon(maxnum))
        allocate(lat(maxnum))

        do i = 2, num_dbx, 1
            num_inter = n_ngrwm(i) ! 获取单个多边形的顶点总数 ! use for sort points
            ngrwm_temp1 = ngrwm(:,i)
            ngrwm_temp2 = 1
            Isused = 0
            neighbor_degree = 0

            do j = 1, num_inter, 1
                do next_pos = 1, num_inter, 1
                    if (next_pos == j) cycle
                    ik = IsNgrmm(ngrmw(1:3, ngrwm_temp1(j)), ngrmw(1:3, ngrwm_temp1(next_pos)))
                    if (ik /= 0) neighbor_degree(j) = neighbor_degree(j) + 1
                end do
            end do

            start_pos = 1
            do j = 1, num_inter, 1
                if (neighbor_degree(j) == 1) then
                    start_pos = j
                    exit
                end if
            end do

            ref_temp = ngrwm_temp1(start_pos) ! 获取第一个三角形编号
            k = 1
            ngrwm_temp2(1) = ref_temp 
            Isused(start_pos) = 1 ! 对第一个三角形进行标记

            do while (k < num_inter)
                found = .FALSE.
                do j = 2, num_inter, 1
                    if (Isused(j) == 1) cycle ! 跳过已经处理的三角形
                    mj = ngrwm_temp1(j) ! 获取三角形编号
                    ik = IsNgrmm(ngrmw(1:3, ref_temp), ngrmw(1:3, mj))
                    if (ik == 0) cycle ! 跳过无连接关系的三角形

                    ref_temp = mj
                    k = k + 1
                    ngrwm_temp2(k) = ref_temp
                    Isused(j) = 1 ! 对第j个三角形进行标记
                    found = .TRUE.
                    exit
                end do 

                ! 如果从2到num_iter都没有找到符合条件的，则报错
                if (.not. found) then
                    write(io6, *) "WARNING! incomplete adjacency walk in SUBROUTINE GetSortNew", i, k, num_inter
                    do j = 1, num_inter, 1
                        if (Isused(j) == 1) cycle
                        ref_temp = ngrwm_temp1(j)
                        k = k + 1
                        ngrwm_temp2(k) = ref_temp
                        Isused(j) = 1
                        found = .TRUE.
                        exit
                    end do
                    if (.not. found) exit
                end if
            end do

            ! 判断ngrwm_temp2的方向是逆时针还是顺时针，并统一为逆时针
            lon = 0.0_r8; lat = 0.0_r8
            do j = 1, num_inter, 1
                lon(j) = mp(ngrwm_temp2(j), 1)
                lat(j) = mp(ngrwm_temp2(j), 2)
            end do
            area = robust_spherical_area(num_inter, pi_r8, pio180_r8, lon, lat)

            if (area < 0.0_r8) then ! 将顺时针存放的数据转为逆时针存放
                ngrwm_temp1 = ngrwm_temp2
                do j = 1, num_inter, 1
                    ngrwm_temp2(j) = ngrwm_temp1(num_inter-j+1)
                end do
            end if
            ngrwm(:, i) = ngrwm_temp2 ! 数据更新
        end do
        
        deallocate(Isused, ngrwm_temp1, ngrwm_temp2, neighbor_degree, lon, lat)

    END SUBROUTINE GetSortNew

    real(r8) FUNCTION robust_spherical_area(num_inter, pi_r8, pio180_r8, lon_deg, lat_deg)
        ! 健壮的球面多边形面积计算（支持凹多边形）而且已经考虑了跨越180经线的情况
        IMPLICIT NONE
        integer, intent(in) :: num_inter
        real(r8) :: pi_r8, pio180_r8
        real(r8), intent(in) :: lon_deg(num_inter), lat_deg(num_inter)
        integer :: i, j
        real(r8) :: area, delta_lon, lon_rad(num_inter), lat_rad(num_inter)

        lon_rad = lon_deg * pio180_r8
        lat_rad = lat_deg * pio180_r8
        area = 0.0_r8

        do i = 1, num_inter, 1
            j = mod(i, num_inter) + 1
            delta_lon = (lon_rad(j) - lon_rad(i))
            if (delta_lon > pi_r8) then
                delta_lon = delta_lon - 2.0_r8 * pi_r8
            else if (delta_lon < -pi_r8) then
                delta_lon = delta_lon + 2.0_r8 * pi_r8
            end if
            area = area + delta_lon * (2.0_r8 + sin(lat_rad(i)) + sin(lat_rad(j)))
        end do
        robust_spherical_area = area / 2.0_r8

    END FUNCTION robust_spherical_area

    SUBROUTINE IAP_Mesh_make(inputfile, outputfile)
        USE MOD_file_preprocess, only : IAP_Mesh_Read, Unstructured_Mesh_Save
        IMPLICIT NONE
        character(pathlen), intent(in) :: inputfile, outputfile
        integer :: sjx_points, lbx_points
        real(r8), allocatable :: mp(:, :), wp(:, :)
        real(r8), allocatable :: xem8(:), yem8(:), zem8(:), xew8(:), yew8(:), zew8(:)
        integer,  allocatable :: ngrmm(:,:), ngrmw(:, :), ngrwm_temp(:, :), ngrwm(:, :), n_ngrwm(:)
        integer :: i, j, k, maxnum
        real(r8) :: circum(3), circum_deg(2)

        CALL IAP_Mesh_Read(inputfile, sjx_points, lbx_points, wp, ngrmm, ngrmw)

        ! make mp 基于w点获取球面m点的重心经纬度坐标
        allocate(mp(sjx_points, 2)); mp = 0.0
        ! 基于w点获取球面m点的重心经纬度坐标
        CALL centroid_spherical_calculation(sjx_points, mp, wp, ngrmw) ! 获取mp经纬度坐标

        ! 计算三角形的球面外心经纬度坐标（基于球面重心经纬度坐标）
        allocate(xem8(sjx_points)); allocate(yem8(sjx_points)); allocate(zem8(sjx_points))
        allocate(xew8(lbx_points)); allocate(yew8(lbx_points)); allocate(zew8(lbx_points))
        CALL lonlat2xyz(sjx_points, mp(:, 1), mp(:, 2), xem8, yem8, zem8)
        CALL lonlat2xyz(lbx_points, wp(:, 1), wp(:, 2), xew8, yew8, zew8)
        xem8 = xem8 * erad8; yem8 = yem8 * erad8; zem8 = zem8 * erad8 ! 因为原本lonlat2xyz没有乘以erad8
        xew8 = xew8 * erad8; yew8 = yew8 * erad8; zew8 = zew8 * erad8
        CALL circumcenter_spherical_calculation(sjx_points, ngrmw, xem8, yem8, zem8, xew8, yew8, zew8)
        do i = 2, sjx_points, 1
            circum(1) = xem8(i); circum(2) = yem8(i); circum(3) = zem8(i); 
            CALL xyz2lonlat(circum, circum_deg)
            mp(i, :) = circum_deg
        end do

        ! make ngrwm and n_ngrwm
        allocate(ngrwm_temp(10, lbx_points)); ngrwm_temp = 1
        allocate(n_ngrwm(lbx_points)); n_ngrwm = 0
        do i = 2, sjx_points, 1 ! 三角形总数（含不存在的三角形），这个必须从2开始
            do j = 1, 3, 1
                k = ngrmw(j, i) ! 获取多边形中心点（或三角形顶点）编号信息
                n_ngrwm(k) = n_ngrwm(k) + 1 ! 累加顶点个数
                ngrwm_temp(n_ngrwm(k), k) = i
            end do
        end do

        maxnum = maxval(n_ngrwm)
        write(io6, *) "maxnum = ", maxnum
        if (maxnum <= 7) maxnum = 7
        allocate(ngrwm(maxnum, lbx_points)); ngrwm = ngrwm_temp(1:maxnum, :)
        CALL GetSortNew(lbx_points, n_ngrwm, ngrmw, mp, ngrwm)

        write(io6, *) "IAP_Mesh_make finish and save to unstructure file"
        CALL Unstructured_Mesh_Save(outputfile, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        deallocate(xem8, yem8, zem8, xew8, yew8, zew8)
        deallocate(mp, ngrwm, n_ngrwm)

    END SUBROUTINE IAP_Mesh_make

END Module MOD_grid_preprocess
