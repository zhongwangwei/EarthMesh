module MOD_refine
    USE consts_coms
    USE refine_vars
    USE NETCDF
    USE MOD_GetContain, only: CheckCrossing
    USE MOD_GetRef, only : ref_sjx
    use MOD_utilities, only : Unstructured_Mesh_Save, Unstructured_Mesh_Read, ref_sjx_save ! Add by Rui Zhang
    use MOD_grid_preprocess, only : IsNgrmm, GetSortNew, set_ngrmm, refine_sjx_regional_make, Grid_Quality_Check_Global
    use MOD_utilities, only : CHECK
    implicit none
    Contains

    SUBROUTINE refine_loop(exit_loop)

        implicit none
        logical, intent(inout) :: exit_loop ! use for exit refine
        integer :: set_dis, set_dis_in                    ! halo(step), max_transition_row(step) 
        integer :: TransitionRow_iter 
        integer :: dist_len ! 用于限制细化三角形的区域    

        integer :: i, j, k, m, n, num, num_edges
        integer :: num_sjx_ref, num_edge, num_ref_last
        integer :: num_lop                                     ! 记录需要lop变换的三角形总数用于更新num_vertex
        integer :: num_ref                                     ! 细化三角形数
        integer :: num_tranrow_sjx                             ! halo三角形个数
        integer :: num_mp(800), num_wp(800)                    ! 记录每次细化后的m，w点数量
        integer :: iter                                        ! 网格细化次数
        integer :: num_sjx, num_dbx                            ! 细化后三角形、多边形数量
        integer :: sjx_points, lbx_points
        integer :: num_closed_curve_refine ! 闭合边界总数
        integer :: num_bdy_refine_segment ! 分段总数
        integer :: num_ref_weak_concav, num_weak_concav_segment, num_weak_concav_pair
        integer :: num_end ! tran_degree, 
        integer,  allocatable :: ref_lbx(:)                 ! 多边形相邻的三角形是否存在被细化的情况
        real(r8), allocatable :: mp(:, :), wp(:, :)            ! 三角形、多边形网格中心点起始数据(上一步细化的结果)
        real(r8), allocatable :: mp_new(:, :), wp_new(:, :)    ! 三角形、多边形网格中心点更新数据
        real(r8), allocatable :: mp_f(:, :), wp_f(:, :)        ! 三角形、多边形网格中心点最终数据 
        integer,  allocatable :: n_ngrwm(:) ! 
        integer,  allocatable :: ngrmw(:, :), ngrwm(:, :) ! 用zero and one 表示顶点是否存在
        integer,  allocatable :: ngrmw_new(:, :), ngrwm_new(:, :)  ! m/w点相邻的w/m点索引(细化后)
        integer,  allocatable :: ngrmw_f(:, :), ngrwm_f(:, :)      ! m/w点相邻的w/m点索引(最终)
        integer,  allocatable :: n_ngrwm_f(:) ! n_ngrwm define in the MOD_utilities.F90
        integer,  allocatable :: ngrmm(:, :)        ! m点相邻的m点索引(细化前)
        integer,  allocatable :: mrl_new(:)         ! 三角形网格细化程度(细化后) 
        integer,  allocatable :: mrl_bk(:)          ! 三角形是否处于细化区域内部
        integer,  allocatable :: weak_concav_pair(:,:)
        integer,  allocatable :: weak_concav_segment(:,:), weak_concav_segment_old(:,:) ! 记录弱凹左右两侧所属的分段编号，以及进行LOP的tran数
        integer,  allocatable :: n_weak_concav_segment(:)
        integer,  allocatable :: ref_sjx_segment(:) ! 用于记录需要细化的三角形
        integer,  allocatable :: ref_sjx_lop(:), ref_sjx_lop_temp(:, :), n_ref_sjx_lop_temp(:) ! 用于存储与LOP变换相关的三角形编号与个数
        integer,  allocatable :: close_curve_refine(:,:), n_close_curve_refine(:) ! 闭合曲线点位存储
        integer,  allocatable :: isbdy_refine(:) ! 细化边界标记
        integer,  allocatable :: isbdy_array(:) 
        integer,  allocatable :: bdy_refine(:), bdy_refine_tran(:) ! 细化区域/细化+过渡区域边界点位标记
        integer,  allocatable :: bdy_refine_segment(:,:), bdy_refine_segment_old(:,:) ! 存储分段中待细化三角形的编号
        integer,  allocatable :: n_bdy_refine_segment(:), n_bdy_refine_segment_old(:) ! 存储分段中待细化三角形个数
        integer,  allocatable :: sjx_child(:,:) ! 用于存储过渡细化中去除的父三角形与生成的子三角形的关系
        integer,  allocatable :: num_bdy_refine_segment_curve(:) ! 记录每一个闭合曲线上的最后一个分段编号
        character(pathlen) :: lndname
        character(LEN = 5) :: nxpc, stepc, TransitionRow_iterc, numiterc
        logical :: iterA                  ! 当迭代B与迭代C同时一次性通过，迭代A通过
        logical :: iterB                  ! 从三角形网格进行判断
        logical :: iterC                  ! 从多边形网格进行判断
        logical :: iterG                  ! 弱凹点判断
        logical :: isreverse
        logical :: iswrite
        logical :: weak_concav_eliminate_in

        ! read unstructure mesh
        write(io6, *)  "start to read unstructure mesh data in the Module MOD_refine in Line 55"
        ! 读取未细化初始网格数据
        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') step
        lndname = trim(file_dir) // 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_' // trim(mode_grid) // '.nc4'
        ! write(io6, *)  lndname
        CALL Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *)  "The unstructured grid data reading have done "
        write(io6, *)  ""
        write(io6, *)  "In total, triangular mesh number: ", sjx_points, "polygon mesh number: ", lbx_points
        write(io6, *)  ""
        CALL execute_command_line('cp '//trim(lndname)//' '//trim(trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc) // "_"// trim(stepc) //"_ori.nc4"))

        iter = 1                                 ! 本次细化中的迭代次数
        num_mp(1) = sjx_points ! 后面涉及sjx_points与num_mp，直接用num_mp(1)代替sjx_points
        num_wp(1) = lbx_points ! 后面涉及lbx_points与num_wp，直接用num_wp(1)代替lbx_points
        ! 存放原始数据，就是直接对读入数据cp就好了

        !----------------------------------------------------------------
        ! ngrmm, mrl_new 数据的初始化与更新
        !----------------------------------------------------------------
        ! Triangle mesh refinement degree (三角形网格细化程度/方式) 1为不细化，2,4为细化, 0为三角形不存在
        allocate(mrl_new(sjx_points));                  mrl_new = 1     ! 反映三角形自身细化与否（0或者1），以及细化方法（2或者4）

        ! 构建ngrmm数组 : mp adjacent mp initial index table (m点相邻m点的初始索引表)
        allocate(ngrmm(3, sjx_points)); ngrmm = 1   ! 反映三角形的相邻三角形的邻域关系(0表示没有相邻三角形)
        CALL set_ngrmm(sjx_points, ngrmw, ngrwm, n_ngrwm, ngrmm)
        write(io6, *)  "set ngrmm finish"
        
        allocate(ref_lbx(lbx_points));      ref_lbx = 0 ! 表示该多边形相邻三角形是否存在被细化的情况，分为0, 1
        
        set_dis_in = max_transition_row(step) ! 过渡行行数设置
        set_dis = halo(step)
        if (SpringGlobal_type /= 0) dist_len = set_dis+num_rc
        
        write(io6, *)  "set_dis_in = ", set_dis_in
        write(io6, *)  "set_dis = ", set_dis
        write(io6, *)  "dist_len = ", dist_len
        write(io6, *)  ""
        num_ref = INT(sum(ref_sjx))                  ! 需要细化的三角形个数

        ! 如果出现num_rc不为0的适合，是否需要额外的处理，来保证连续性
        ! 这部分的重点是往回去找
        if (step > 1) then
            write(io6, *)  "before halo protect = ", num_ref
            ! 标记细化三角形
            allocate(mrl_bk(sjx_points))
            CALL refine_sjx_regional_make(step-1, sjx_points, mp, mrl_bk)
            mrl_bk(1:num_vertex) = 0 ! 进一步保证N+1级细化出现在N级细化内

            if (sum(mrl_bk(1:num_vertex)) > 0) then
                write(io6, *)  "sum(mrl_bk(1:num_vertex)) = ", sum(mrl_bk(1:num_vertex))
                write(io6, *)  "Warning! Please check for SUBROUTINE refine_loop"
                mrl_bk(1:num_vertex) = 0 ! 进一步保证N+1级细化出现在N级细化内
            end if

            ! 将处于细化区域过渡带的三角形标记去除，不可细化
            allocate(isbdy_array(lbx_points)); isbdy_array = 0
            m = 0
            do while(m < dist_len)
                ! 先确认边界点位
                isbdy_array = 0
                do i = num_center + 1, lbx_points, 1
                    num_edges = n_ngrwm(i)
                    num = sum(mrl_bk(ngrwm(1:num_edges, i)))
                    if (num == 0) cycle ! 多边形内没有细化三角形，跳过
                    if (num == num_edges) cycle ! 多边形内全是细化三角形，跳过
                    isbdy_array(i) = 1
                end do

                ! 确认需要边界内的三角形
                do i = num_center + 1, lbx_points, 1
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

            do i = num_vertex + 1, sjx_points, 1
                if (ref_sjx(i) == 0) cycle ! 跳过不细化的三角形
                if (mrl_bk(i) == 1) cycle ! 跳过处于细化区域非过渡区域的三角形
                ref_sjx(i) = 0 ! 找到位于过渡区域的细化三角形，并取消细化
                num_ref = num_ref - 1
            end do
            deallocate(mrl_bk, isbdy_array)
            write(io6, *)  "after  halo protect = ", num_ref
            write(io6, *)  ""
        end if

        lndname = trim(file_dir) // "tmpfile/ref_sjx_after_halo_protect_NXP" // trim(nxpc)  // "_"//  trim(stepc) // ".nc4"
        CALL ref_sjx_save(lndname, sjx_points, ref_sjx)

        do i = num_vertex + 1, sjx_points, 1
            if (ref_sjx(i) /= 1) cycle ! 跳过不需要细化的三角形
            if (sum(ref_sjx(ngrmm(:, i))) > 0) cycle ! 只去除孤立三角形网格
            ref_sjx(i) = 0
            num_ref = num_ref - 1
        end do
        write(io6, *)  "去除孤立细化三角形后，需要细化的三角形：", num_ref
        write(io6, *)  ""
        lndname = trim(file_dir) // "tmpfile/ref_sjx_isolated_remove_NXP" // trim(nxpc)  // "_"//  trim(stepc) // ".nc4"
        CALL ref_sjx_save(lndname, sjx_points, ref_sjx)

        if (num_ref == 0) then
            exit_loop = .true.   
            return
        end if
        
        !--------------------------------------------------
        ! 1.2 Preliminary refinement (one into four) 【初步细化（一分为四）】初步细化就是阈值细化
        !--------------------------------------------------
        write(io6, *)  "Start to refine"
        write(io6, *)  "iter =", iter, "num =", num_ref ! iter：迭代次数 num_ref：需要细化的三角形个数
        allocate(ref_sjx_segment(sjx_points)); ref_sjx_segment = ref_sjx ! 在这个之后才从ref_sjx 改为 ref_sjx_segment 使用
        write(numiterc, '(I3.3)') iter
        lndname = trim(file_dir) // "tmpfile/ref_sjx_segment_NXP" // trim(nxpc)  // "_"//  trim(stepc) //"_"//  trim(numiterc) // ".nc4"
        ! write(io6, *)  lndname
        CALL ref_sjx_save(lndname, sjx_points, ref_sjx_segment)

        iter = iter + 1 ! 相对于FHW的代码，这是加一之后的结果
        num_mp(iter) = num_mp(iter - 1) + 4 * num_ref        ! 记录每次迭代后三角形数，每细化（一分为四）一个三角形，增加4个小三角形的中心点，
        num_wp(iter) = num_wp(iter - 1) + 3 * num_ref        ! 记录每次迭代后多边形数，每细化（一分为四）一个三角形，增加三个中点作为三角形的顶点
        CALL OnedivideFour_connection(iter, sjx_points, ngrmw, ngrmm, ref_lbx, mrl_new)
        
        !--------------------------------------------------
        ! 2.1 进行迭代（防止细化交汇带出现冲突，一分为四）! 也是采用一分四算法 iterB 和 iterC 用三角形与多边形的角度去防止细化带的出现
        !--------------------------------------------------
        if (weak_concav_eliminate(step) == 1) then
            weak_concav_eliminate_in = .TRUE.
        else
            weak_concav_eliminate_in = .FALSE.
        end if

        iterA = .false.
        write(io6, *)  "iterA start"
        write(io6, *)  ""
        do while(iterA .eqv. .false.) ! 当iterA为true时，该步骤完成
            iterA = .true.    ! 判断所有迭代是否都已满足条件
            iterB = .false.   ! 从三角形网格的外包络线进行判断
            iterC = .false.   ! 从多边形网格进行判断
            iterG = .false.   ! 从弱凹点处进行判断
            
            write(io6, *)  "    iterB start" ! 从三角形网格进行判断
            do while (iterB .eqv. .false.)
                CALL iterB_judge(set_dis_in, sjx_points, ngrmm, mrl_new)
                CALL num_ref_cal(sjx_points, num_ref, ref_sjx_segment)
                if (num_ref == 0) then
                    write(io6, *)  "    No need to add new refine sjx in the iterB"
                    iterB = .true.
                else
                    write(io6, *)  "    iter =", iter, "num =", num_ref
                    ! if (step > 1) then
                        write(numiterc, '(I3.3)') iter
                        lndname = trim(file_dir) // "tmpfile/ref_sjx_segment_NXP" // trim(nxpc)  // "_"//  trim(stepc) //"_"//  trim(numiterc) // ".nc4"
                        ! write(io6, *)  lndname
                        CALL ref_sjx_save(lndname, sjx_points, ref_sjx_segment)
                    ! end if
                    iterA = .false.
                    iter = iter + 1
                    num_mp(iter) = num_mp(iter - 1) + 4 * num_ref
                    num_wp(iter) = num_wp(iter - 1) + 3 * num_ref
                    CALL OnedivideFour_connection(iter, sjx_points, ngrmw, ngrmm, ref_lbx, mrl_new)
                end if
            end do ! iterB
            write(io6, *)  "    iterB end"
            write(io6, *)  ""

            write(io6, *)  "    iterC start"! 从多边形形网格进行判断
            do while (iterC .eqv. .false.)
                CALL iterC_judge(sjx_points, lbx_points, ngrmm, ngrwm, n_ngrwm, mrl_new, ref_lbx)
                CALL num_ref_cal(sjx_points, num_ref, ref_sjx_segment)
                if (num_ref == 0) then
                    write(io6, *)  "    No need to add new refine sjx in the iterC"
                    iterC = .true.
                else
                    write(io6, *)  "    iter =", iter, "num =", num_ref
                    !if (step > 1) then
                        write(numiterc, '(I3.3)') iter
                        lndname = trim(file_dir) // "tmpfile/ref_sjx_segment_NXP" // trim(nxpc)  // "_"//  trim(stepc) //"_"//  trim(numiterc) // ".nc4"
                        ! write(io6, *)  lndname
                        CALL ref_sjx_save(lndname, sjx_points, ref_sjx_segment)
                    !end if
                    iterA = .false.
                    iter = iter + 1
                    num_mp(iter) = num_mp(iter - 1) + 4 * num_ref
                    num_wp(iter) = num_wp(iter - 1) + 3 * num_ref
                    CALL OnedivideFour_connection(iter, sjx_points, ngrmw, ngrmm, ref_lbx, mrl_new)
                end if

            end do ! iterC
            write(io6, *)  "    iterC end"
            write(io6, *)  ""
            if (iterA .eqv. .false.) cycle

            write(io6, *)  "    iterG start" ! 从三角形网格进行判断
            do while(iterG .eqv. .false.) ! 
                ! 寻找弱凹点，有没有一种可能第一次细化就没有了弱凹点呢？
                ! New(五边形没有弱凹点，七边形也没有，只有六边形有)
                CALL iterG_judge(lbx_points, ngrwm, n_ngrwm, mrl_new)
                num_ref = INT(sum(ref_sjx)) ! 获取需要细化的三角形个数
                if (num_ref == 0) then
                    write(io6, *)  "    no 弱凹点 in iterG"
                    num_ref_weak_concav = 0
                    num_weak_concav_pair = 0
                    num_weak_concav_segment = 0
                    iterG = .true.
                else
                    if (.not. weak_concav_eliminate_in) then
                        write(numiterc, '(I3.3)') iter
                        lndname = trim(file_dir) // "tmpfile/ref_sjx_weak_concav_NXP" // trim(nxpc)  // "_"//  trim(stepc) // ".nc4"
                        write(io6, *)  lndname
                        CALL ref_sjx_save(lndname, sjx_points, ref_sjx)
                        num_ref_weak_concav = num_ref !!!!!pair类型的弱凹!!!!
                        exit
                    end if
                    write(io6, *)  "    iter =", iter, "num =", num_ref
                    ref_sjx_segment(num_vertex+1:sjx_points) = ref_sjx_segment(num_vertex+1:sjx_points) + ref_sjx(num_vertex+1:sjx_points)
                    iterA = .false.
                    iter = iter + 1
                    num_mp(iter) = num_mp(iter - 1) + 4 * num_ref
                    num_wp(iter) = num_wp(iter - 1) + 3 * num_ref
                    CALL OnedivideFour_connection(iter, sjx_points, ngrmw, ngrmm, ref_lbx, mrl_new)
                end if
            end do ! iterG
            write(io6, *)  "    iterG end"
            write(io6, *)  ""
            if (.not. weak_concav_eliminate_in) exit
        end do ! iterA
        write(io6, *)  "iterA end"
        write(io6, *)  ""

        !--------------------------------------------------
        ! 2.2 对一分四细化的网格处理并储存
        !--------------------------------------------------
        num_sjx_ref = INT(sum(ref_sjx_segment)) ! 获取需要一分四细化的三角形个数
        write(io6, *)  "需要一分四细化的三角形个数：", num_sjx_ref

        ! 确定halo所需要的网格个数，并确认最终*_new的数组长度
        num_tranrow_sjx = num_sjx_ref
        write(io6, *)  "Array length calculation Start"
        CALL Array_length_calculation(set_dis_in, sjx_points, lbx_points, wp, mrl_new, ngrmm, ngrmw, ngrwm, n_ngrwm, num_tranrow_sjx, &
            num_closed_curve_refine, close_curve_refine, n_close_curve_refine, isbdy_refine, bdy_refine, bdy_refine_tran)
        write(io6, *)  "细化的三角形+外围halo三角形个数:", num_tranrow_sjx
        write(io6, *)  "Array length calculation Finish"
        write(io6, *)  ""

        !----------------------------------------------------------------
        ! 完成mp_new, wp_new, ngrmw_new, ngrwm_new 数据的初始化与更新
        !----------------------------------------------------------------
        ! num_tranrow_sjx 本质上是为三角形设计的，但是六边形增加的个数少于三角形增加个数，所以也可用于多边形
        allocate(mp_new(sjx_points + (num_tranrow_sjx) * 4, 2))    ; mp_new = 0.   ! The center point of the triangular grid updates the data (三角形网格中心点更新数据)
        allocate(wp_new(lbx_points + (num_tranrow_sjx) * 4, 2))    ; wp_new = 0.   ! Update data at center point of polygon mesh (多边形网格中心点更新数据)
        allocate(ngrmw_new(3, sjx_points + (num_tranrow_sjx) * 4)) ; ngrmw_new = 1 ! The adjacent wp points of mp update the index table (mp的相邻wp点更新索引表)，
        allocate(ngrwm_new(7, lbx_points + (num_tranrow_sjx) * 4)) ; ngrwm_new = 1 ! wp's adjacent mp points update the index table (wp的相邻mp点更新索引表)
        mp_new(1:sjx_points, :) = mp ! 三角形中心的的经，纬度
        wp_new(1:lbx_points, :) = wp ! 多边形中心的经，纬度
        ngrmw_new(:, 1:sjx_points) = ngrmw(1:3, 1:sjx_points) ! 三角形顶点的的经，纬度
        ngrwm_new(:, 1:lbx_points) = ngrwm(1:7, 1:lbx_points) ! 多边形顶点的的经，纬度

        CALL OnedivideFour_renew(iter, ngrmw, ref_sjx_segment, num_mp, num_wp, mp_new, wp_new, ngrmw_new)
        lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc)  // "_"//  trim(stepc) // "_withref.nc4"
        ! write(io6, *)  lndname
        CALL Unstructured_Mesh_Save(lndname, num_mp(iter), num_wp(iter), mp_new, wp_new, ngrmw_new, ngrwm_new)
        deallocate(ref_sjx_segment)

        num_end = max(4, 4 * (set_dis_in-1))
        write(io6, *)  "max value of num_end = ", num_end, "ref_sjx_lop_temp"
        ! write(io6, *)  " "
        if (Istransition) then
            ! 创建bdy_refine_segment实现数据分段（这里并没有考虑弱凹的情况）
            iswrite = .TRUE.
            allocate(num_bdy_refine_segment_curve(0:num_closed_curve_refine)); num_bdy_refine_segment_curve = 0
            CALL bdy_refine_segment_make(iswrite, set_dis_in, num_closed_curve_refine, close_curve_refine, n_close_curve_refine, ngrwm, n_ngrwm, mrl_new, bdy_refine_segment, n_bdy_refine_segment, num_bdy_refine_segment, num_bdy_refine_segment_curve)
            write(io6, *)  "分段个数num_bdy_refine_segment为:", num_bdy_refine_segment
            write(io6, *)  ""

            if (num_ref_weak_concav == 0) weak_concav_eliminate_in = .TRUE.
            if (.not. weak_concav_eliminate_in) then ! 消除弱凹就肯定是拓展为六边形
                ! 在bdy_refine_segment分段中提取出弱凹分段
                ! 更新分段情况，考虑存在弱凹的分段方式（原本pair的弱凹可能要和其他分段数据合并，部分pair弱凹变为1+n或者n+n类型的弱凹）
                write(io6, *)  "weak concav segment make start"
                CALL weak_concav_segment_make(set_dis_in, num_bdy_refine_segment, num_ref_weak_concav, ngrmw, num_bdy_refine_segment_curve, bdy_refine_segment, n_bdy_refine_segment, num_weak_concav_segment, num_weak_concav_pair, weak_concav_segment, n_weak_concav_segment, weak_concav_pair)
                write(io6, *)  "num_ref_weak_concav = ", num_ref_weak_concav
                write(io6, *)  "num_weak_concav_segment(n+n)or(1+n) = ", num_weak_concav_segment
                write(io6, *)  "num_weak_concav_pair(1+1) = ", num_weak_concav_pair
                write(io6, *)  "weak concav segment make finish"
                write(io6, *)  ""
                if (set_dis_in == 1) then
                    if (num_weak_concav_pair /= 0) STOP "ERROR! num_weak_concav_pair must equal to zero when set_dis_in == 1"
                end if
            end if

            allocate(sjx_child(2, num_mp(1))); sjx_child = 0
            TransitionRow_iter = 1
            num_lop = 0
            do while(TransitionRow_iter <= set_dis_in)
                write(io6, *)  "TransitionRow_iter = ", TransitionRow_iter
                write(TransitionRow_iterc, '(I1)') TransitionRow_iter
                !--------------------------------------------------
                ! 4.1 记录需要正向一分二的三角形编号与个数
                !-------------------------------------------------- 
                ref_sjx = 0
                ! 针对强凹
                if (num_bdy_refine_segment /= 0) then
                    allocate(bdy_refine_segment_old(set_dis_in, num_bdy_refine_segment)); bdy_refine_segment_old = bdy_refine_segment
                    allocate(n_bdy_refine_segment_old(num_bdy_refine_segment)); n_bdy_refine_segment_old = n_bdy_refine_segment
                    do i = 1, num_bdy_refine_segment, 1
                        if (n_bdy_refine_segment(i) == 0) cycle ! 跳过不符合的过渡等级 
                        do j = 1, set_dis_in, 1
                            if (bdy_refine_segment(j, i) == 1) exit ! 三角形不存在就跳过
                            ref_sjx(bdy_refine_segment(j, i)) = 1
                        end do
                        n_bdy_refine_segment(i) = n_bdy_refine_segment(i) - 1 ! 完成分段长度的缩减，这个放在这里好不好，或者放在其他地方呢？
                    end do
                end if

                ! 针对弱凹（融合非两端都是1和两端都是1两种情况）
                if (num_ref_weak_concav /= 0) then
                    allocate(weak_concav_segment_old(set_dis_in, num_ref_weak_concav)); weak_concav_segment_old = weak_concav_segment
                    do i = 1, num_ref_weak_concav, 1
                        if (n_weak_concav_segment(i) == 0) cycle ! 跳过不符合的过渡等级 
                        do j = 1, set_dis_in, 1
                            if (weak_concav_segment(j, i) == 1) exit ! 三角形不存在就跳过
                            ref_sjx(weak_concav_segment(j, i)) = 1
                        end do
                        n_weak_concav_segment(i) = n_weak_concav_segment(i) - 1 ! 完成分段长度的缩减，这个放在这里好不好，或者放在其他地方呢？
                    end do
                end if

                num_ref = INT(sum(ref_sjx))
                if (num_ref == 0) then
                    if (TransitionRow_iter == 1) then
                        stop "ERROR! impossible for NO 相邻三角形中只有一个三角形被细化"
                    else
                        write(io6, *)  "TransitionRow iter finish and exit!"
                        exit
                    end if
                else
                    write(io6, *)  "iter =", iter, "num =", num_ref, "in the ODT step"
                    iter = iter + 1
                    isreverse = .false. ! 正向一分为二
                    num_mp(iter) = num_mp(iter - 1) + 2 * num_ref
                    num_wp(iter) = num_wp(iter - 1) + num_ref
                    CALL OnedivideTwo(iter, isreverse, ngrmw, ngrmm, ngrwm, num_mp, num_wp, mp_new, wp_new, mrl_new, ngrmw_new, sjx_child)
                    do i = num_vertex + 1, num_mp(1), 1 ! 放在外面更新，要不然容易出错
                        if (ref_sjx(i) == 1) mrl_new(i) = 4
                    end do
                    lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc) // "_"// trim(stepc) //"_ODT_"//trim(TransitionRow_iterc) // ".nc4"
                    ! write(io6, *)  lndname
                    CALL Unstructured_Mesh_Save(lndname, num_mp(iter), num_wp(iter), mp_new, wp_new, ngrmw_new, ngrwm_new)
                    ! write(io6, *)  "the ODT step finish"
                end if
                ! write(io6, *)  ""

                ! 这个需要小心，如果出现左右两侧分段长度的弱凹的话，这就会产生八边形，所有要合理的避免这种情况
                ! 如果出现set_dis_in=2而且有1+1弱凹的情况的话，需要把1+1弱凹进行LOP变换先
                TransitionRow_iter = TransitionRow_iter + 1
                if ((TransitionRow_iter > set_dis_in) .and. (weak_concav_eliminate_in .eqv. .TRUE.)) cycle ! 跳过后面内容
                if (TransitionRow_iter == 3) then
                    num_weak_concav_pair = 0 
                    write(io6, *)  "num_weak_concav_pair turn to zero"
                end if

                !--------------------------------------------------
                ! 4.4 记录强凹与弱凹中需要反向一分二的三角形，并完成bdy_refine_segment/weak_concav_segment的更新
                !--------------------------------------------------    
                ! 这里的TransitionRow_iter已经加过1了

                ref_sjx = 0
                ! 专门处理三角形所在分段中需要反向一分二而且确定下一轮需要正向一分二的三角形(强凹与弱凹都适应)
                CALL ref_sjx_isreverse_judge(set_dis_in, num_bdy_refine_segment, ngrmm, mrl_new, bdy_refine_segment, n_bdy_refine_segment)
                write(io6, *)  "weak_concav_eliminate_in:", weak_concav_eliminate_in
                if (.not. weak_concav_eliminate_in) CALL ref_sjx_isreverse_judge(set_dis_in, num_weak_concav_segment, ngrmm, mrl_new, weak_concav_segment, n_weak_concav_segment)
                ! 到这步时候，bdy_refine_segment, n_bdy_refine_segment和weak_concav_segment, n_weak_concav_segment已经更新为下一次正向一分二的数据

                ! 要求在弱凹中，这里只针对弱凹两端都是1的情况(而且是针对过渡行数量大于1的时候)
                if (num_weak_concav_pair /= 0) then
                    write(io6, *)  "weak concav pair special start"
                    ! 需要更新weak_concav_segment， n_weak_concav_segment,并确认需要反向细化的三角形
                    CALL weak_concav_pair_special(num_weak_concav_pair, num_ref_weak_concav, ngrmm, ngrmw, mrl_new, weak_concav_pair, weak_concav_segment, n_weak_concav_segment)
                    write(io6, *)  "weak concav pair special finish"
                    write(io6, *)  ""
                end if

                num_ref = INT(sum(ref_sjx))
                if (num_ref == 0) then
                    write(io6, *)  "NO 相邻三角形之间的反向一分二细化"
                else
                    write(io6, *)  "iter =", iter, "num =", num_ref, "in the ODTR step"
                    iter = iter + 1
                    isreverse = .true. ! 反向一分二
                    num_mp(iter) = num_mp(iter - 1) + 2 * num_ref
                    num_wp(iter) = num_wp(iter - 1) + num_ref
                    CALL OnedivideTwo(iter, isreverse, ngrmw, ngrmm, ngrwm, num_mp, num_wp, mp_new, wp_new, mrl_new, ngrmw_new, sjx_child)
                    do i = num_vertex + 1, num_mp(1), 1 ! 放在外面更新，要不然容易出错
                        if (ref_sjx(i) == 1) mrl_new(i) = 4
                    end do
                    lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc) // "_"// trim(stepc) //"_ODTR_"//trim(TransitionRow_iterc) // ".nc4"
                    ! write(io6, *)  lndname
                    CALL Unstructured_Mesh_Save(lndname, num_mp(iter), num_wp(iter), mp_new, wp_new, ngrmw_new, ngrwm_new)
                    ! write(io6, *)  "the ODTR step finish"
                end if
                ! write(io6, *)  ""

                !--------------------------------------------------
                ! 4.5 记录需要对角变换的三角形并交换(针对强凹与弱凹有不同的处理方式)
                !-------------------------------------------------- 
                ! ref_sjx_lop_temp: 将强凹/弱凹的对角变换一起处理
                ! n_ref_sjx_lop_temp:
                ! 需要确认需要对角变换的三角形的个数
                num_ref = 0 ! 再根据num_ref的大小确定ref_sjx_segment
                allocate(ref_sjx_lop_temp(num_end, num_bdy_refine_segment+num_ref_weak_concav)); ref_sjx_lop_temp = 1 ! 三角形初始编号为1
                allocate(n_ref_sjx_lop_temp(num_bdy_refine_segment+num_ref_weak_concav)); n_ref_sjx_lop_temp = 0 ! 含有三角形的个数，初始为0
                n_ref_sjx_lop_temp(1:num_bdy_refine_segment) = n_bdy_refine_segment ! 已经在前面进行过减一的处理 ！！！！！！
                
                CALL sharp_concav_lop_judge(set_dis_in, num_ref, num_bdy_refine_segment, mrl_new, ngrmm, ngrmw_new, sjx_child, bdy_refine_segment, bdy_refine_segment_old, n_bdy_refine_segment, &
                                            ref_sjx_lop_temp, n_ref_sjx_lop_temp)
                
                if (.not. weak_concav_eliminate_in) CALL weak_concav_lop_judge(set_dis_in, num_ref, num_bdy_refine_segment, num_ref_weak_concav, num_weak_concav_segment, num_weak_concav_pair, &
                                                mrl_new, ngrmm, ngrmw_new, sjx_child, weak_concav_segment, weak_concav_segment_old, n_weak_concav_segment, weak_concav_pair, ref_sjx_lop_temp, n_ref_sjx_lop_temp)

                if (num_ref == 0) then
                    write(io6, *)  "不需要对角交换"
                else
                    write(io6, *)  "iter =", iter, "num =", num_ref, "in the LOP step"
                    iter = iter + 1
                    allocate(ref_sjx_lop(num_ref)); ref_sjx_lop = 1! 获取细化三角形的索引编号
                    m = 0 ! 用于推进
                    do i = 1, num_bdy_refine_segment + num_ref_weak_concav, 1
                        if (n_ref_sjx_lop_temp(i) == 0) cycle ! 跳过不需要进一步处理的分段
                        ! if (n_ref_sjx_lop_temp(i) > num_end) write(io6, *)  "i = ", i, "n_ref_sjx_lop_temp(i) = ", n_ref_sjx_lop_temp(i)
                        ref_sjx_lop(m+1:m+n_ref_sjx_lop_temp(i)) = ref_sjx_lop_temp(1:n_ref_sjx_lop_temp(i), i) ! 第一个是个数
                        m = m + n_ref_sjx_lop_temp(i)
                    end do

                    num_mp(iter) = num_mp(iter - 1) + num_ref
                    num_wp(iter) = num_wp(iter - 1) ! 每次去掉两个三角形，而生成两个新三角形，不认为有新的多边形生成
                    CALL Delaunay_Lop(iter, num_ref, num_mp, num_wp, mp_new, wp_new, ngrmw_new, ref_sjx_lop)  
                    deallocate(ref_sjx_lop)
                    num_lop = num_lop + num_ref
                    lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc) // "_"// trim(stepc) //"_LOP_"//trim(TransitionRow_iterc) // ".nc4"
                    ! write(io6, *)  lndname
                    CALL Unstructured_Mesh_Save(lndname, num_mp(iter), num_wp(iter), mp_new, wp_new, ngrmw_new, ngrwm_new)
                    ! write(io6, *)  "the LOP step finish"
                end if
                ! write(io6, *)  ""

                deallocate(bdy_refine_segment_old, n_bdy_refine_segment_old)
                if (allocated(weak_concav_segment_old)) deallocate(weak_concav_segment_old)
                deallocate(ref_sjx_lop_temp, n_ref_sjx_lop_temp)
                sjx_child = 0

                ! 将前面弱凹相关的需要一分二的三角形激活
                if (num_weak_concav_pair /= 0) then
                    do i = num_ref_weak_concav-num_weak_concav_pair+1, num_ref_weak_concav, 1 ! 有值但是个数为0，激活
                        if (weak_concav_segment(1, i) == 1) cycle 
                        n_weak_concav_segment(i) = 1
                    end do
                    if (num_weak_concav_segment == 0) then
                        num_weak_concav_segment = num_weak_concav_pair ! 针对原本只有1+1的情况
                    end if
                    write(io6, *)  "弱凹相关的需要一分二的三角形激活 完成"
                end if

                if (num_weak_concav_segment == 0) cycle
                if (sum(n_weak_concav_segment) == 0) then
                    write(io6, *)  "num_weak_concav_segment turn to zero"
                    num_weak_concav_segment = 0
                end if

                if (weak_concav_eliminate_in) cycle
                if (num_weak_concav_pair + num_weak_concav_segment == 0) then
                    write(io6, *)  " weak_concav_eliminate_in turn to TRUE"
                    weak_concav_eliminate_in = .TRUE.
                end if

            end do
            write(io6, *)  "过渡构建finish"
        end if
        write(io6, *)  "细化后共有", num_wp(iter), "个多边形网格"
        write(io6, *)  "细化后共有", num_mp(iter), "个三角形网格" 
        write(io6, *)  ""

        !--------------------------------------------------
        ! 5.5 存储网格数据
        !--------------------------------------------------
        CALL NGR_RENEW(iter, num_mp, num_wp, mp_new, wp_new, ngrmw_new, num_sjx, num_dbx, mp_f, wp_f, ngrmw_f, ngrwm_f, n_ngrwm_f, bdy_refine, bdy_refine_tran)
        !lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc) // "_"// trim(stepc) //"_5.nc4"
        !write(io6, *)  lndname
        !CALL Unstructured_Mesh_Save(lndname, num_sjx, num_dbx, mp_f, wp_f, ngrmw_f, ngrwm_f, n_ngrwm_f)

        ! 更新num_mp_step(step)和num_wp_step(step) 但是也可能范围不够，不准确
        ! 循环起点num_vertex和num_center更新与存储（便于后续distsOnEdge的设置）
        ! 需要注意的是num_center是对于set_dis而言，最小的三角形顶点编号（这句话也有一定的问题）
        ! 需要注意的是num_vertex是针对新生成三角形而言的，并不适用于set_dis范围内的三角形中心点最小编号
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        write(io6, *)  "step =", step
        write(io6, *)  "before num_mp_step(step) = ", num_mp_step(step)
        write(io6, *)  "before num_wp_step(step) = ", num_wp_step(step)

        num_vertex = sjx_points - (num_mp(iter) - num_sjx) + num_lop ! num_vertex+1：新三角形最小编号
        num_mp_step(step) = num_vertex

        ! 我觉得这里的上限可以从num_sjx改为num_vertex + 4*num_sjx_ref
        ! 因为假设是n+1级三角形只在n级的一分四三角形
        num_center = lbx_points
        do i = num_vertex + 1, num_sjx, 1
            do j = 1, 3, 1
                k = ngrmw_f(j, i)
                if (k < num_center) num_center = k
            end do 
        end do
        num_wp_step(step) = num_center

        write(io6, *)  "after num_mp_step(step) = ", num_mp_step(step)
        write(io6, *)  "after num_wp_step(step) = ", num_wp_step(step)
        write(io6, *)  ""
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

        ! 规定180°经线圈上的经度为180° (限制经度范围),可能很鸡肋作用不大
        ! 目前需要从2开始，因为是对全局调整的
        do i = num_vertex + 1, num_sjx, 1 ! mp is the center of sjx
            if (mp_f(i, 1) == -180.) mp_f(i, 1) = 180.
        end do
        do i = num_center + 1,  num_dbx, 1 ! wp is the center of lbx
            if (wp_f(i, 1) == -180.) wp_f(i, 1) = 180.
        end do

        !--------------------------------------------------
        ! 6.7 存储最终网格数据
        !--------------------------------------------------

        write(stepc, '(I2.2)') step + 1
        lndname = trim(file_dir) // 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_' // trim(mode_grid) // '.nc4'
        write(io6, *)  lndname
        CALL Unstructured_Mesh_Save(lndname, num_sjx, num_dbx, mp_f, wp_f, ngrmw_f, ngrwm_f, n_ngrwm_f)
        
        ! deallocate
        deallocate(ref_sjx, ref_lbx)
        deallocate(mp, wp, mp_new, wp_new, mp_f, wp_f)
        deallocate(ngrmm, ngrmw, ngrwm, n_ngrwm)
        deallocate(ngrmw_new, ngrwm_new, ngrmw_f, ngrwm_f, n_ngrwm_f)
        deallocate(mrl_new, sjx_child)
        deallocate(num_bdy_refine_segment_curve)

    END SUBROUTINE refine_loop

    SUBROUTINE iterB_judge(set_dis_in, sjx_points, ngrmm, mrl_new)
        ! 要继承之前已经算好的距离矩阵，减少计算量
        implicit none
        integer, intent(in)    :: set_dis_in, sjx_points
        integer, allocatable, intent(in) :: ngrmm(:,:), mrl_new(:)
        integer :: i, j, k, m1, m2, m3, hhh(5), num_vertex_in
        integer, allocatable :: mrl_in(:), mrl_bk(:)

        !num_vertex_in = num_vertex
        num_vertex_in = 1
        hhh = [1,2,3,1,2] 
        allocate(mrl_in(sjx_points)); mrl_in = 0 ! 用于标记三角形是否需要进行细化
        allocate(mrl_bk(sjx_points)); mrl_bk = 0
        do i = num_vertex_in + 1, sjx_points, 1
            if (mrl_new(i) /= 4) cycle ! 跳过未细化的三角形
            do j = 1, 3, 1
                k = ngrmm(j, i)
                if (mrl_new(k) == 4) cycle
                mrl_in(ngrmm(j, i)) = mrl_in(ngrmm(j, i)) + 2 ! 这一步就可能产生需要细化的三角形
            end do
        end do

        k = 1
        do while(k < set_dis_in)
            mrl_bk = mrl_in ! 用于do-while循环迭代
            do i = num_vertex_in + 1, sjx_points, 1
                if (mrl_new(i) == 4) cycle ! 跳过已经细化的三角形
                if (mrl_in(i) /= 0) cycle
                if (sum(mrl_in(ngrmm(:, i))) < 4) cycle
                ! 走到这一步说明是本身没细化，而且相邻三角形中有两个“一分二”，一个为未细化
                do j = 1, 3, 1
                    m1 = ngrmm(hhh(j),   i)
                    m2 = ngrmm(hhh(j+1), i)
                    m3 = ngrmm(hhh(j+2), i)
                    if ((mrl_in(m1) /= 2) .or. (mrl_in(m2) /= 2)) cycle
                    ! 很关键！只对mrl_bk赋值，避免影响当前结果
                    mrl_bk(i) = mrl_bk(i) + 2
                    mrl_bk(m3)= mrl_bk(m3)+ 2
                    exit ! jump 
                end do
            end do 
            k = k + 1
            mrl_in = mrl_bk
        end do 

        ref_sjx = 0
        do i = num_vertex_in + 1, sjx_points, 1
            if (mrl_new(i) == 4) cycle 
            if (mrl_in(i)  >= 4) ref_sjx(i) = 1 ! 说明tran区域重叠需要进一步细化
        end do
        
        ! 在过渡区域产生孤立细化三角形后,细化周围三角形
        do i = num_vertex_in + 1, sjx_points, 1
            if (ref_sjx(i) /= 1) cycle ! 跳过不需要细化的三角形
            if (sum(ref_sjx(ngrmm(:, i))) > 0) cycle ! 
            do j = 1, 3, 1
                k = ngrmm(j, i)
                if (mrl_in(k) == 0) cycle
                ref_sjx(k) = 1
                exit
            end do

            if (ref_sjx(k) == 0) cycle
            do j = 1, 3, 1
                m1 = ngrmm(j, k)
                ref_sjx(m1) = 1
            end do

        end do

    END SUBROUTINE iterB_judge

    SUBROUTINE iterC_judge(sjx_points, lbx_points, ngrmm, ngrwm, n_ngrwm, mrl_new, ref_lbx)

        implicit none
        integer,  intent(in) :: sjx_points, lbx_points
        integer,  allocatable, intent(in) :: ngrmm(:,:), ngrwm(:,:), n_ngrwm(:)
        integer,  allocatable, intent(in) :: mrl_new(:), ref_lbx(:)
        integer :: num_edges, i, j, k, m1, m2, hhh(5), num_center_in, num_vertex_in
        integer,  allocatable :: mrl_in(:), ref_lbx_in(:,:)
        real(r8) :: num_ref_lbx(7)

        num_center_in = num_center
        num_vertex_in = num_vertex
        hhh = [1,2,3,1,2]
        allocate(mrl_in(sjx_points)); mrl_in = 0
        allocate(ref_lbx_in(7, lbx_points)); ref_lbx_in = 0 ! 用于记录多边形顶点相邻的三角形是否射入
        ref_sjx = 0

        ! 标记过渡行为1的情况下，哪些网格已经需要细化

        ! 针对多边形中已经含有的被细化的三角形个数进行细化（利用ref_lbx和mrl_new进行判断）
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        do i = num_center_in + 1,  lbx_points, 1
            if (ref_lbx(i) == 0) cycle ! 跳过编号为i的多边形（w点）的相邻三角形（m点）均未被细化       
            num_edges = n_ngrwm(i) ! 用于统计多边形网格边数

            ! 当多边形中心点相邻的三角形存在细化，即ref_lbx(i) /= 0 可能存在弱凹点
            if (num_edges == 5) then ! 五边形没有弱凹点！！！
                ! 将存在连续两个或者三个已被细化三角形的五边形的剩下三角形都细化
                if (sum( mrl_new(ngrwm(1:num_edges, i)) ) > 10 ) then
                    do j = 1, num_edges, 1
                        if (mrl_new(ngrwm(j, i)) == 1) ref_sjx(ngrwm(j, i)) = 1
                    end do 
                end if

            else if (num_edges == 6) then 
                ! 可能1：是连续一个或两个或三个或四个连续三角形已被细化，不需要处理
                if (sum(mrl_new(ngrwm(1:num_edges, i))) == 12) then! 两个三角形被细化 存在两种情况，相邻，隔两个（相对，同顶点）
                    ! 可能2：两个对角三角形被细化，中间都相隔两个没有被细化的三角形 -> 变为连续四个三角形已被细化的情况
                    do j = 1, 3, 1
                        if ((mrl_new(ngrwm(j, i)) == 4) .and. &
                            (mrl_new(ngrwm(j + 3, i)) == 4)) then! 两个被细化三角形是相对位置的
                            if ((mrl_new(ngrwm(j + 1, i)) == 1) .and. &
                                (mrl_new(ngrwm(j + 2, i)) == 1)) then
                                ref_sjx(ngrwm(j+1:j+2, i)) = 1
                            end if
                        end if ! 细化一侧的三角形，将另一侧视作弱凹
                    end do
                end if

            end if ! num_edges == 6 或者 5 的问题

        end do ! i = num_center_in + 1, lbx_points, 1 循环
        ! 经过这一次处理后，五边形中只能含有一个细化三角形
        ! 六边形可以是含有一个/两个/三个/四个连续的细化三角形
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

        ! 考虑外部射线对于多边形的影响
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        ! 利用mrl_new 标记 mrl_in
        do i = num_vertex_in + 1, sjx_points, 1
            if (mrl_new(i) /= 4) cycle ! 跳过非细化三角形
            ! 细化三角形向三个方向做射线，要求跳过相邻的细化三角形
            do j = 1, 3, 1
                k = ngrmm(j, i)
                if (mrl_new(k) == 4) cycle
                mrl_in(ngrmm(j, i)) = 2 ! 这里应该只存在0和2两种情况，因为大于2的情况在iterB中已经被细化了
            end do
        end do

        ! 利用mrl_in标记ref_lbx_in
        do i = num_center_in + 1, lbx_points, 1
            num_edges = n_ngrwm(i)
            do j = 1, num_edges, 1
                k = ngrwm(j, i)
                if (mrl_in(k) == 2) ref_lbx_in(j, i) = 1
            end do
        end do

        do i = num_center_in + 1,  lbx_points, 1
            num_edges = n_ngrwm(i) ! 用于统计多边形网格边数
            if (ref_lbx(i) /= 0) then  ! 当多边形中含有细化三角形的情况
                if (sum(mrl_new(ngrwm(1:num_edges, i))) == 18) cycle ! 排除弱凹的情况（即六边形中含有两个连续的未细化三角形）
                ! 这种不允许有相邻的射线的判断适用范围：1）含有一个细化三角形的五边形
                ! 2）含有一个/两个/三个细化三角形的六边形
                do j = 1, num_edges, 1
                    m1 = ngrwm(j,   i)
                    if (mrl_in(m1) /= 2) cycle
                    m2 = ngrwm(mod(j, num_edges) + 1, i) ! 为了实现首尾相连
                    if (mrl_in(m2) == 2) then
                        ref_sjx(m1) = 1
                        ref_sjx(m2) = 1
                    end if
                end do

                if (sum(mrl_new(ngrwm(1:num_edges, i))) == 9) then ! 针对只含有一个细化三角形的六边形
                    if (sum(ref_lbx_in(1:num_edges, i)) < 3) cycle
                    do j = 1, num_edges, 1
                        m1 = ngrwm(j,   i)
                        if (mrl_new(m1) == 1) ref_sjx(m1) = 1
                    end do
                end if

            else ! 当多边形中不含有细化三角形的时候
                num_ref_lbx = ref_lbx_in(1:7, i) ! 获取射线的位置，避免因为对原来数值的修改影响结果
                do j = 1, num_edges, 1
                    m1 = j
                    m2 = mod(j, num_edges) + 1
                    if ((ref_lbx_in(m1, i) == 1) .and. &
                        (ref_lbx_in(m2, i) == 1)) then ! 针对两个相邻的三角形，因此它们组成弱凹
                        num_ref_lbx(m1) = 0.5 
                        num_ref_lbx(m2) = 0.5
                    end if! 两个0.5合计1，表示要增加1条边
                end do
                if (sum(num_ref_lbx(1:num_edges)) + num_edges > 7.) then 
                    do j = 1, num_edges, 1 !New 应该不会循环到7
                        m1 = ngrwm(j, i)
                        if ((mrl_in(m1) == 2) .and. &
                            (mrl_new(m1) == 1)) then
                            ref_sjx(m1) = 1 ! 说明该三角形要细化（一分四那种）
                        end if
                    end do
                end if
            end if
        end do
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

        deallocate(mrl_in, ref_lbx_in)
        return

    END SUBROUTINE iterC_judge

    SUBROUTINE iterG_judge(lbx_points, ngrwm, n_ngrwm, mrl_new)
        ! 需要弱凹点的算法，前提弱凹点只出现在六边形中
        implicit none
        integer, intent(in)    :: lbx_points
        integer, allocatable, intent(in) :: ngrwm(:,:), n_ngrwm(:), mrl_new(:)
        integer :: i, j, num_edges

        ref_sjx = 0
        do i = num_center + 1, lbx_points, 1
            num_edges = n_ngrwm(i)
            if (num_edges /= 6) cycle
            if (sum( mrl_new(ngrwm(1:num_edges, i)) ) /= 18) cycle 
            do j = 1, num_edges, 1
                if (mrl_new(ngrwm(j, i)) == 1) then
                    ref_sjx(ngrwm(j, i)) = 1
                end if
            end do 
        end do

    END SUBROUTINE iterG_judge

    ! 确保计算获得的ref_sjx不会重复出现
    SUBROUTINE num_ref_cal(sjx_points, num_ref, ref_sjx_segment)
        IMPLICIT NONE
        integer, intent(in) :: sjx_points
        integer, intent(out) :: num_ref
        integer,  dimension(:), allocatable, intent(inout) :: ref_sjx_segment
        integer :: i

        num_ref = 0
        do i = num_vertex + 1, sjx_points, 1
            if (ref_sjx(i) == 0) cycle
            if (ref_sjx_segment(i) == 0) then
                num_ref = num_ref + 1
                ref_sjx_segment(i) = 1
            end if
        end do

    END SUBROUTINE num_ref_cal

    SUBROUTINE OnedivideFour_connection(iter, sjx_points, ngrmw, ngrmm, ref_lbx, mrl_new)
        ! 根据ref_sjx跟新ref_lbx, mrl_new
        IMPLICIT NONE
        integer :: i, j, k
        integer,  intent(in) :: iter, sjx_points
        integer,  dimension(:, :), allocatable, intent(in) :: ngrmw, ngrmm
        integer,  dimension(:),    allocatable, intent(inout) :: ref_lbx
        integer,  dimension(:),    allocatable, intent(inout) :: mrl_new

        ! 需要建立refed_iter 与第几个三角形的映射关系
        do i = num_vertex + 1, sjx_points, 1
            if ((ref_sjx(i) == 0) .or. (mrl_new(i) /= 1)) cycle ! 若三角形需要细化而且还没别细化
            !!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 连接关系调整 更新ref_lbx, mrl_new!!!!!!!!!!!!!!!!!!!!!!!!!
            ref_lbx(ngrmw(1:3, i)) = 1 ! 作用在iterC_judge，说明这个多边形相连接的三角形被细化

            ! 更新三角形的自身细化状态mrl_new(分为两个部分，一个是自身三角形，一个是细化生成的三角形)
            mrl_new(i) = 4 ! 原三角形网格被平均分为四份 ! 作用在iterB/C/D_judge
        end do

    END SUBROUTINE OnedivideFour_connection

    SUBROUTINE OnedivideFour_renew(iter, ngrmw, ref_sjx_segment, num_mp, num_wp, mp_new, wp_new, ngrmw_new)

        IMPLICIT NONE
        integer :: i, j, k, refed_iter
        integer :: icl, m0, w0      ! 三角形和多边形中心点序号起始索引
        real(r8) :: sjx(3, 2), newsjx(4, 2), newdbx(3, 2)
        integer,  intent(in) :: iter
        integer,  dimension(:, :), allocatable, intent(in) :: ngrmw
        integer,  dimension(:),    allocatable, intent(in) :: ref_sjx_segment
        integer,  dimension(:), intent(inout) :: num_mp, num_wp
        real(r8), dimension(:, :), allocatable, intent(inout) :: mp_new, wp_new
        integer,  dimension(:, :), allocatable, intent(inout) :: ngrmw_new

        sjx = 0.; newdbx = 0.; newsjx = 0.
        refed_iter = 0
        ! 需要建立refed_iter 与第几个三角形的映射关系
        do i = num_vertex + 1, num_mp(1), 1
            if (ref_sjx_segment(i) == 0) cycle ! 找到需要细化的三角形更新相关的网格信息
            ! write(io6, *)  "i = ", i, "OnedivideFour_renew"
            !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 新的三角形与多边形顶点坐标, ngrmw_new !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            icl = 0
            sjx = wp_new(ngrmw(:, i), :) 
            if (maxval(sjx(:, 1)) - minval(sjx(:, 1)) > 180.) then ! need to modify sjx first
                icl = 1
                CALL CheckCrossing(3, sjx)
            end if

            ! 生成新的多边形
            newdbx(1, 1:2) = (sjx(2, 1:2) + sjx(3, 1:2)) / 2. ! 顶点1对边中点为第一个三角形顶点
            newdbx(2, 1:2) = (sjx(1, 1:2) + sjx(3, 1:2)) / 2. !  顺序逆时针，
            newdbx(3, 1:2) = (sjx(1, 1:2) + sjx(2, 1:2)) / 2. ! 计算新生成的中间小三角形顶点（原三角形中点） 
            ! 生成新的三角形
            newsjx(1, 1:2) = (sjx(1, :) + newdbx(2, :) + newdbx(3, :)) / 3.
            newsjx(2, 1:2) = (sjx(2, :) + newdbx(1, :) + newdbx(3, :)) / 3.
            newsjx(3, 1:2) = (sjx(3, :) + newdbx(1, :) + newdbx(2, :)) / 3.
            newsjx(4, 1:2) = (newdbx(3, :) + newdbx(1, :) + newdbx(2, :)) / 3.

            if (icl /= 0) then ! 经度跨越修正! 将新生成m点、w点大于180°的经度减小360° 
                CALL CheckCrossing(4, newsjx)
                CALL CheckCrossing(3, newdbx)
            end if

            m0 = num_mp(1) + refed_iter * 4 ! 新三角形中心点编号基准
            w0 = num_wp(1) + refed_iter * 3 ! 新多边形中心点编号基准
            mp_new(m0 + 1 : m0 + 4, 1:2) = newsjx(:, 1:2)! 新三角形中心点经纬度(四个)
            wp_new(w0 + 1 : w0 + 3, 1:2) = newdbx(:, 1:2)! 新多边形中心点经纬度(三个)
            !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 新的三角形与多边形顶点坐标 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

            ! 增加第一，二，三个新三角形的剩下两个顶点的编号信息,ngrmw_new作用在ngr_renew中
            ngrmw_new(2, m0 + 1) = w0 + 3 !w3
            ngrmw_new(3, m0 + 1) = w0 + 2 !w2
            ngrmw_new(2, m0 + 2) = w0 + 1 !w1
            ngrmw_new(3, m0 + 2) = w0 + 3 !w3
            ngrmw_new(2, m0 + 3) = w0 + 2 !w2
            ngrmw_new(3, m0 + 3) = w0 + 1 !w1
            ! 增加第四个小三角形顶点编号信息
            ngrmw_new(1, m0 + 4) = w0 + 1
            ngrmw_new(2, m0 + 4) = w0 + 2
            ngrmw_new(3, m0 + 4) = w0 + 3
            ! 更新原三角形与新三角形的ngrmw_new
            do k = 1, 3, 1
                ngrmw_new(k, i) = 1 ! 1. 去掉原三角形的顶点编号信息
                ngrmw_new(1, m0 + k) = ngrmw(k, i)! 2. 增加新三角形的顶点编号信息
            end do
            refed_iter = refed_iter + 1
    
            !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            !write(io6, *)  "sjx = ", sjx
            !write(io6, *)  "newsjx = ", newsjx
            !write(io6, *)  "newdbx = ", newdbx
            !write(io6, *)  "m0 = ", m0
            !write(io6, *)  "w0 = ", w0
            !write(io6, *)  "mp_new(m1:m4, 1:2) = ", mp_new(m0+1:m0+4, 1:2)
            !write(io6, *)  "wp_new(w1:w3, 1:2) = ", wp_new(w0+1:w0+3, 1:2)
            !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        end do

        ! 针对可能压着180经线的情况
        CALL crossline_check(iter, mp_new, wp_new, num_mp, num_wp)

    END SUBROUTINE OnedivideFour_renew

    SUBROUTINE Array_length_calculation(set_dis_in, sjx_points, lbx_points, wp, mrl_new, ngrmm, ngrmw, ngrwm, n_ngrwm, num_tranrow_sjx, &
        num_closed_curve_refine, close_curve_refine, n_close_curve_refine, isbdy_refine, bdy_refine, bdy_refine_tran)
        ! 确定halo所需要的网格个数，并确认最终*_new的数组长度
        USE MOD_utilities, only : close_Mesh_Save ! Add by Rui Zhang
        implicit none
        integer, intent(in) :: set_dis_in, sjx_points, lbx_points
        real(r8),allocatable, intent(in) :: wp(:, :)
        integer, allocatable, intent(in) :: mrl_new(:)
        integer, allocatable, intent(in) :: ngrmm(:,:), ngrmw(:,:)
        integer, allocatable, intent(in) :: ngrwm(:,:), n_ngrwm(:)
        integer, intent(inout) :: num_tranrow_sjx
        integer, intent(out) :: num_closed_curve_refine
        integer, allocatable, intent(out) :: close_curve_refine(:,:), n_close_curve_refine(:)
        integer, allocatable, intent(out) :: isbdy_refine(:)
        integer, allocatable, intent(out) :: bdy_refine(:), bdy_refine_tran(:)
        integer :: i, j, k, m, num_edges, num, close_num
        real(r8), allocatable :: close_points(:,:)
        logical :: iswrite
        integer, allocatable :: isbdy_array(:), mrl_in(:)
        character(pathlen) :: lndname
        character(LEN = 5) :: nxpc, stepc, numc, refinec

        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') step

        ! 如果该顶点（即该六边形是存在而且不完整，则认为是边界点，非边界点位必须满秩）
        allocate(mrl_in(sjx_points)); mrl_in = mrl_new
        allocate(isbdy_array(lbx_points)); isbdy_array = 0 ! 1表示是边界点位 ! 只适用于第一次循环中

        do i = num_center + 1, lbx_points, 1
            num_edges = n_ngrwm(i)
            num = sum(mrl_in(ngrwm(1:num_edges, i)))
            if (num == num_edges) cycle ! 多边形内没有细化三角形，跳过
            if (num == num_edges * 4) cycle ! 多边形内全是细化三角形，跳过
            isbdy_array(i) = 1
        end do
        allocate(isbdy_refine(lbx_points)); isbdy_refine = isbdy_array

        ! use for 统计num_tranrow_sjx个数
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        m = 0
        do while(m < set_dis_in)
            ! 这里应该只需要对mrl_in处理就好了
            do i = num_center + 1, lbx_points, 1
                if (isbdy_array(i) /= 1) cycle
                num_edges = n_ngrwm(i) ! 获取相连的三角形个数
                do j = 1, num_edges ,1
                    k = ngrwm(j, i) ! 获取对应的center网格编号
                    if (mrl_in(k) == 4) cycle ! 跳过细化三角形
                    mrl_in(k) = 4 ! 向外找到非细化网格标记为细化网格，便于下一次循环
                    num_tranrow_sjx = num_tranrow_sjx + 1 ! 统计num_tranrow_sjx个数
                end do
            end do

            isbdy_array = 0 ! 1表示是边界点位 ! 只适用于第一次循环中
            do i = num_center + 1, lbx_points, 1
                num_edges = n_ngrwm(i)
                num = sum(mrl_in(ngrwm(1:num_edges, i)))
                if (num == num_edges) cycle ! 多边形内没有细化三角形，跳过
                if (num == num_edges * 4) cycle ! 多边形内全是细化三角形，跳过
                isbdy_array(i) = 1
            end do
            m = m + 1
        end do
        !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

        write(refinec, '(I1)') step
        ! 计算isbdy_refine的连接关系并将连接关系当作close类型写入nc文件中，当成是1级细化
        ! 计算会在尾部空出一个位置
        iswrite = .TRUE.
        CALL bdy_connection_make(iswrite, sjx_points, lbx_points, mrl_new, ngrmm, ngrmw, num_closed_curve_refine, close_curve_refine, n_close_curve_refine)
        mask_patch_ndm(step) = num_closed_curve_refine
        do i = 1, num_closed_curve_refine, 1
            close_num = n_close_curve_refine(i) - 1 ! 因为最后一个位置没有实际顶点编号
            allocate(close_points(close_num, 2))
            do j = 1, close_num, 1
                close_points(j, :) = wp(close_curve_refine(j, i), :)
            end do

            write(numc, '(I2.2)') i
            lndname = trim(file_dir)// 'tmpfile/mask_patch_close_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL close_Mesh_Save(lndname, close_num, close_points)
            deallocate(close_points)
        end do

        ! 判断num_closed_curve_refine是否为1，如果不为1，可能需要注意halo区域重叠的问题
        ! if (num_closed_curve_refine > 1) STOP "please make sure halo region without shaping"

        ! 保存细化区域边界点位信息
        close_num = sum(isbdy_refine) 
        allocate(bdy_refine(close_num)); bdy_refine = 0
        m = 0
        do i = num_center + 1, lbx_points, 1
            if (isbdy_refine(i) /= 1) cycle
            m = m + 1
            bdy_refine(m) = i
        end do

        ! 保存细化+过渡区域边界点位信息
        close_num = sum(isbdy_array) 
        allocate(bdy_refine_tran(close_num)); bdy_refine_tran = 0
        m = 0
        do i = num_center + 1, lbx_points, 1
            if (isbdy_array(i) /= 1) cycle
            m = m + 1
            bdy_refine_tran(m) = i
        end do
        deallocate(isbdy_array, mrl_in)

    END SUBROUTINE Array_length_calculation

    SUBROUTINE bdy_connection_make(iswrite, sjx_points, lbx_points, mrl_bk, ngrmm, ngrmw, num_closed_curve, close_curve, n_close_curve)

        implicit none
        logical, intent(in) :: iswrite
        integer, intent(in) :: sjx_points, lbx_points
        integer, allocatable, intent(in) :: mrl_bk(:) ! 因为传入的数据有可能是mrl_new或者mrl_in
        integer, allocatable, intent(in) :: ngrmm(:,:), ngrmw(:,:)
        integer :: i, j, im, w0, w1, w2
        integer :: bdy_num_in, num_bdy_long, bdy_num_in_save, hhh(5)
        integer, allocatable :: ngrvv(:, :), n_ngrvv(:)
        integer, allocatable :: bdy_order(:)
        integer, allocatable :: bdy_ngr(:, :)
        integer, intent(out) :: num_closed_curve
        integer, allocatable, intent(out) :: close_curve(:,:), n_close_curve(:)

        hhh = [1,2,3,1,2]
        ! 建立细化网格边界vertex-vertex的连接关系数组ngrvv
        ! 通过理论分析，一个海洋网格边界理论上最多有两个走向
        allocate(ngrvv(2, lbx_points));  ngrvv = 1
        allocate(n_ngrvv(lbx_points)); n_ngrvv = 0
        bdy_num_in = 1 ! 第一个位置空出来！！！
        do i = num_vertex + 1, sjx_points, 1
            if (mrl_bk(i) /= 1) cycle ! 跳过细化三角形
            if (sum(mrl_bk(ngrmm(:,i))) /= 6) cycle ! 跳过非细化三角形的相连三角形的细化个数不为1的情况
            ! 找到非细化三角形的对偶三角形是细化的
            do j = 1, 3, 1
                im = ngrmm(j, i)
                if (mrl_bk(im) == 4) exit
            end do
            bdy_num_in = bdy_num_in + 1 ! 记录符合条件的三角形个数

            do j = 1, 3, 1
                w0 = ngrmw(hhh(j), i)
                if (all(w0 /= ngrmw(:, im))) exit
            end do
            ! 找到对偶三角形的相接的两个三角形顶点
            w1 = ngrmw(hhh(j+1), i)
            w2 = ngrmw(hhh(j+2), i)
            n_ngrvv(w1) = n_ngrvv(w1) + 1
            n_ngrvv(w2) = n_ngrvv(w2) + 1
            ngrvv(n_ngrvv(w1), w1) = w2
            ngrvv(n_ngrvv(w2), w2) = w1
        end do
        bdy_num_in_save = bdy_num_in
        if (iswrite) write(io6, *)  "bdy_num_in_save(空出第一个位置) = ", bdy_num_in_save
        if (iswrite) write(io6, *)  ""

        ! check for n_ngrvv and adjust ifneed
        do i = num_center + 1, lbx_points, 1
            if (n_ngrvv(i) == 1) then
                write(io6, *)  "i = ", i, "n_ngrvv(i) = ", n_ngrvv(i)
                write(io6, *)  "ngrvv(1:2, i) = ", ngrvv(1:2, i)
                STOP "ERROR in the SUBROUTINE bdy_connection_make! ngrvv(1;2, i) must larger than one"
            end if
        end do

        ! 对于每一个边界vertex，真正有效的连接关系与网格形状无关，应该有却只有两个连接的vertex
        allocate(bdy_order(bdy_num_in)); bdy_order = 1
        allocate(bdy_ngr(2, lbx_points))
        bdy_ngr = ngrvv(1:2, :)
        bdy_num_in = 1
        do i = num_center + 1, lbx_points, 1
            if (n_ngrvv(i) /= 2) cycle
            bdy_num_in = bdy_num_in + 1 !获取边界顶点信息，也就是多边形个数
            bdy_order(bdy_num_in) = i
        end do
        ! if (iswrite) write(io6, *)  "bdy_order(空出第一个位置)", bdy_order
        deallocate(ngrvv, n_ngrvv)


        ! 因为是闭合图形，所以在边界上的三角形个数与多边形个数一致
        if (bdy_num_in_save /= bdy_num_in) then
            write(io6, *)  "bdy_num_in_save(空出第一个位置) = ", bdy_num_in_save
            write(io6, *)  "bdy_num_in(空出第一个位置) = ", bdy_num_in
            stop "ERROR! bdy_num_in_save /= bdy_num_in"
        end if

        ! 获取num_closed_curve, num_bdy_long信息
        CALL bdy_connection_closed_curve(iswrite, bdy_num_in, bdy_order, bdy_ngr, num_closed_curve, num_bdy_long)
        
        ! 重新遍历，获取bdy_queue信息并保留！！！！
        if (iswrite) write(io6, *)  "get close_curve and n_close_curve start"
        allocate(close_curve(num_bdy_long, num_closed_curve)); close_curve = 1
        allocate(n_close_curve(num_closed_curve)); n_close_curve = 0
        CALL bdy_connection_closed_curve(iswrite, bdy_num_in, bdy_order, bdy_ngr, num_closed_curve, num_bdy_long, close_curve, n_close_curve)
        if (iswrite) write(io6, *)  "get close_curve and n_close_curve finish"
        deallocate(bdy_order, bdy_ngr)

    END SUBROUTINE bdy_connection_make

    SUBROUTINE bdy_connection_closed_curve(iswrite, bdy_num_in, bdy_order, bdy_ngr, num_closed_curve, num_bdy_long, close_curve, n_close_curve)
        ! 进行vertex的前后连接, 形成闭合曲线
        logical, intent(in) :: iswrite
        integer, intent(in) :: bdy_num_in ! 细化边界点位总数（第一个位置是空出来的）
        integer, allocatable, intent(in) :: bdy_order(:), bdy_ngr(:, :)
        integer :: i, j
        integer :: num_points, bdy_end, ngr_select
        integer, allocatable :: bdy_queue(:), bdy_alternate(:) ! 
        integer, intent(out) :: num_closed_curve, num_bdy_long ! 闭合曲线个数，闭合曲线最大长度
        integer, allocatable, intent(inout), optional :: close_curve(:,:), n_close_curve(:) ! 闭合曲线点位与闭合曲线长度
        
        allocate(bdy_queue(bdy_num_in))
        allocate(bdy_alternate(bdy_num_in)); bdy_alternate = 1 ! 1表示可以使用，0表示已经使用
        num_closed_curve = 0 ! 记录闭合曲线个数
        num_bdy_long = 0 ! 记录闭合曲线最大长度
        do while(sum(bdy_alternate) > 1)
            ! 寻找闭合曲线的起点
            num_points = 1 ! 数据初始化
            num_closed_curve = num_closed_curve + 1
            bdy_queue = 1 ! 数据初始化
            do j = 2, bdy_num_in, 1
                if (bdy_alternate(j) == 1) then ! the start of queue
                    bdy_queue(num_points) = bdy_order(j)
                    bdy_alternate(j) = 0
                    exit
                end if
            end do

            ! 开始进行vertex连接使其成为闭合曲线
            bdy_end    = bdy_ngr(2, bdy_order(j)) ! the end of queue
            ngr_select = bdy_ngr(1, bdy_order(j)) ! 获取编号, 还需要知道这个编号对应的顺序编号
            ! write(io6, *)  "start from : ", bdy_order(j)
            ! write(io6, *)  "end at : ", bdy_end
            ! write(io6, *)  "ngr_select = ", ngr_select

            do while(ngr_select /= bdy_end)
                num_points = num_points + 1
                bdy_queue(num_points)  = ngr_select
                do j = 2, bdy_num_in, 1 ! ngr_select 实际在bdy_order中的位置
                    if (bdy_order(j) == ngr_select) exit
                end do
                ! write(io6, *)  "j = ", j
                bdy_alternate(j) = 0
                do i = 1, 2, 1
                    if (bdy_ngr(i, ngr_select) == bdy_queue(num_points-1)) cycle
                    ngr_select = bdy_ngr(i, ngr_select)
                    exit ! aviod ngr_select change twice!
                end do
            end do
            ! write(io6, *)  ""

            num_points = num_points + 1
            bdy_queue(num_points)  = bdy_end
            do j = 2, bdy_num_in, 1
                if (bdy_order(j) == bdy_end) exit
            end do
            bdy_alternate(j) = 0
            if (num_points < 3) stop "ERROR! num_points < 3 !"

            ! bdy_queue是有顺序的，不可以随便处理，需要谨慎！
            ! 如果存在某一个参数，则执行这部分内容
            if (present(n_close_curve)) then
                n_close_curve(num_closed_curve) = num_points + 1
                close_curve(1:num_points, num_closed_curve) = bdy_queue(1:num_points)
                num_points = num_points + 1
                if (iswrite) write(io6, '(A, I3, A, I3)')  "num_closed_curve = ", num_closed_curve, ", num points of closed curves(尾部空1): ", num_points
            end if

            ! num_bdy_long 更新
            if (num_points > num_bdy_long) num_bdy_long = num_points ! num_points变为最长
        end do ! do while(sum(bdy_alternate) > 1)

        if (.not. present(n_close_curve)) then
            num_bdy_long = num_bdy_long + 1
            if (iswrite) write(io6, *)  "num_bdy_long = ", num_bdy_long, "start from two"
        end if

    END SUBROUTINE bdy_connection_closed_curve

    SUBROUTINE bdy_refine_segment_make(iswrite, set_dis_in, num_closed_curve, close_curve, n_close_curve, ngrwm, n_ngrwm, mrl_new, bdy_refine_segment, n_bdy_refine_segment, num_bdy_refine_segment, num_bdy_refine_segment_curve)
        ! 根据mrl和ngrwm一起判断是弱凹（四个细化三角形）还是强凹（两个细化三角形），直线（三个细化三角形）和转折（一个细化三角形）
        IMPLICIT NONE
        logical, intent(in) :: iswrite
        integer, intent(in) :: set_dis_in, num_closed_curve
        integer, allocatable, intent(in) :: ngrwm(:,:), n_ngrwm(:), mrl_new(:)
        integer, allocatable, intent(inout) :: close_curve(:,:), n_close_curve(:)
        integer, allocatable, intent(out) :: bdy_refine_segment(:,:) ! 存储细化三角形分组情况
        integer, allocatable, intent(out) :: n_bdy_refine_segment(:) ! 存储每一个分段中三角形个数
        integer, intent(out) :: num_bdy_refine_segment
        integer, allocatable, intent(inout), optional :: num_bdy_refine_segment_curve(:) ! 
        integer :: i, j, k, w, num_edges, num, num_segement, num_sum
        integer :: m, m1, m2, m3, num_edges1, num_edges2
        logical :: isexist
        integer, allocatable :: bdy_closed_curve_temp(:) ! 临时存储闭合曲线，便于索引
        integer, allocatable :: segement_start_end(:, :) ! 临时存储闭合曲线分段信息起点与终点
        integer, allocatable :: bdy_refine_segment_temp(:, :), n_bdy_refine_segment_temp(:)
        ! 需要对边界相对顺序进行适当的调整，要求闭合曲线的起点左右两侧不在直线上

        allocate(bdy_refine_segment_temp(set_dis_in, sum(n_close_curve))); bdy_refine_segment_temp = 1 ! 初始三角形编号
        allocate(n_bdy_refine_segment_temp(sum(n_close_curve))); n_bdy_refine_segment_temp = 0 ! 标记每一分段含有的非细化三角形的个数
        num_bdy_refine_segment = 0 ! 标记分段个数
        do i = 1, num_closed_curve, 1
            allocate(segement_start_end(n_close_curve(i), 2)); segement_start_end = 0 ! 初始为0
            close_curve(n_close_curve(i), i) = close_curve(1, i) ! 第一个数值赋值给最后一个形成闭环,默认不需要调整
            if (set_dis_in == 1) then
                segement_start_end(1:n_close_curve(i)-1, 1) = [(j, j=1, n_close_curve(i)-1)]
                segement_start_end(1:n_close_curve(i)-1, 2) = [(j, j=2, n_close_curve(i))]
            else
                ! 需要对边界相对顺序进行适当的调整，要求闭合曲线的起点不是直线，闭合曲线
                do j = 1, n_close_curve(i)-1, 1 ! 因为最后一个数据还没有赋值
                    k = close_curve(j, i) ! 获取对应的三角形坐标编号
                    num_edges = n_ngrwm(k) ! 获取边界点位对应的三角形个数
                    num = sum(mrl_new(ngrwm(1:num_edges, k)))
                    if (INT((num - num_edges)/3) /= 3) exit ! 找到转弯位置就好了
                end do

                ! 调整起点与终点，便于后续的分段
                if (j /= 1) then
                    ! 先考虑首尾重复，再移动，便于验证
                    ! if (iswrite) write(io6, *)  "j = ", j, "need to modify order of close_curve(1:n_close_curve(i), i)"
                    close_curve(n_close_curve(i), i) = close_curve(j, i) ! 第一个数值赋值给最后一个形成闭环

                    ! 根据获取的j调整close_curve和bdy_closed_curve_temp, 数据范围是1到n_close_curve(i)-1
                    allocate(bdy_closed_curve_temp(n_close_curve(i)-1))
                    bdy_closed_curve_temp = close_curve(1:n_close_curve(i)-1, i)
                    close_curve(n_close_curve(i)-j+1:n_close_curve(i)-1, i) = bdy_closed_curve_temp(1:j-1)
                    close_curve(1:n_close_curve(i)-j, i) = bdy_closed_curve_temp(j:n_close_curve(i)-1)
                    deallocate(bdy_closed_curve_temp)

                    if (close_curve(n_close_curve(i), i) /= close_curve(1, i)) then
                        write(io6, *)  "close_curve(n_close_curve(i), i) = ", close_curve(n_close_curve(i), i)
                        write(io6, *)  "close_curve(1, i) = ", close_curve(1, i)
                        write(io6, *)  "ERROR! close_curve(n_close_curve(i), i) /= close_curve(1, i)"
                        stop
                    end if
                else
                    ! if (iswrite) write(io6, *)  "j = ", 1, " No need to modify order of close_curve(1:n_close_curve(i), i)"
                end if

                m = 1
                segement_start_end(m, 1) = 1
                do j = 2, n_close_curve(i)-1, 1 ! start from 2, end at n_close_curve(i)-1
                    k = close_curve(j, i) ! 获取对应的三角形坐标编号
                    num_edges = n_ngrwm(k) ! 获取边界点位对应的三角形个数
                    num = sum(mrl_new(ngrwm(1:num_edges, k)))
                    if (INT((num - num_edges)/3) == 3) cycle
                    segement_start_end(m, 2) = j ! 是当前segement的终点
                    segement_start_end(j, 1) = j ! 也是另外一个segement的起点
                    m = j ! 标记分段起点
                end do
                segement_start_end(m, 2) = n_close_curve(i)

                ! 根据set_dis_in进一步划分
                do j = 1, n_close_curve(i)-1, 1
                    if (segement_start_end(j, 1) == 0) cycle ! 跳过
                    num = segement_start_end(j, 2) - segement_start_end(j, 1) ! 获取分段含有细化三角形的个数
                    if (num <= set_dis_in) then
                        if (num < INT((set_dis_in+1)/2)) then
                            if (iswrite) write(io6, *)  "Warning! num less than half of set_dis_in! defective!" 
                        end if
                        cycle ! 如果比set_dis_in短或者一样长，跳过
                    end if
                    num_segement = INT((num+1)/real(set_dis_in)) ! 确定分段个数, 至少有两个，不适合set_dis_in=1的情况
                    !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                    if (mod(num+1, set_dis_in) /= 0) num_segement = num_segement + 1
                    if (mod(num,   set_dis_in) == 0) num_segement = num_segement - 1 ! 不知道用num还是num+1
                    !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                    ! if (iswrite) write(io6, *)  "num = ", num, "num_segement = ", num_segement
                    segement_start_end(j+num_segement-1, 2) = segement_start_end(j, 2)
                    do k = 1, num_segement - 1, 1
                        segement_start_end(j+k-1, 2) = segement_start_end(j+k-1, 1) + set_dis_in
                        segement_start_end(j+k,   1) = segement_start_end(j+k-1, 2)
                    end do
                    if (set_dis_in < 3) cycle
                    k = num_segement - 1 ! 因为在fdortran循环中do k = 1, num_segement - 1, 1，结束后得到的k是num_segement
                    num = segement_start_end(j+k, 2) - segement_start_end(j+k, 1)
                    !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 后面再考虑如何合理分配 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                    ! 如果最后一个区间内的细化三角形个数小于最小数量的限制则重新分配
                    if (num < INT((set_dis_in+1)/2)) then ! 长度4，最大区间3，分为2+2
                        segement_start_end(j+k, 1)   = segement_start_end(j+k, 2) - INT((set_dis_in+1)/2) ! 修改分段起点
                        segement_start_end(j+k-1, 2) = segement_start_end(j+k, 1) ! 修改上一个分段的终点
                        if (.not. iswrite) cycle
                        write(io6, *)  ""
                        write(io6, *)  "refine sjx in the segement :", segement_start_end(j+k, 2) - segement_start_end(j+k, 1)
                        write(io6, *)  "j = ", j, "k = ", k, "j+k = ", j+k
                        write(io6, *)  "segement_start_end(j+k, 2) = ", segement_start_end(j+k, 2)
                        write(io6, *)  "segement_start_end(j+k, 1) = ", segement_start_end(j+k, 1)
                        write(io6, *)  "不满足最小区间要求",INT((set_dis_in+1)/2),"，数组修改"
                        write(io6, *)  ""
                    end if
                    !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!! 后面再考虑如何合理分配 !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                end do

                num_sum = 0
                do j = 1, n_close_curve(i)-1, 1
                    if (segement_start_end(j, 1) == 0) cycle ! 跳过
                    num = segement_start_end(j, 2) - segement_start_end(j, 1)
                    num_sum = num_sum + num
                end do
                if (num_sum /= n_close_curve(i)-1) then
                    write(io6, *)  "ERROR! num_sum must same as n_close_curve(i)-1"
                    write(io6, *)  "num_sum = ", num_sum
                    write(io6, *)  "n_close_curve(i)-1 = ", n_close_curve(i)-1
                    write(io6, *)  ""
                    stop
                end if
            end if

            ! 根据segement_start_end找到对应的三角形编号并存入bdy_refine_segment_temp
            ! 这里num不应该出现负数，而且最后一个num为1才对，说明前面segement_start_end有误
            do j = 1, n_close_curve(i)-1, 1
                if (segement_start_end(j, 1) == 0) cycle ! 跳过
                num = segement_start_end(j, 2) - segement_start_end(j, 1) ! 获取分段含有细化三角形的个数
                num_bdy_refine_segment = num_bdy_refine_segment + 1 ! 向前推进
                n_bdy_refine_segment_temp(num_bdy_refine_segment) = num
                do k = 1, num, 1
                    m1 = close_curve(segement_start_end(j, 1) + k - 1, i) ! 获取边界点位编号1
                    m2 = close_curve(segement_start_end(j, 1) + k, i) ! 获取边界点位编号2
                    num_edges1 = n_ngrwm(m1) ! 获取边界点位1对应的三角形个数
                    isexist = .false.
                    do m = 1, num_edges1, 1
                        if (mrl_new(ngrwm(m, m1)) == 4) cycle ! 跳过已经细化的三角形
                        m3 = ngrwm(m, m1) ! 获取非细化的三角形编号
                        num_edges2 = n_ngrwm(m2) ! 获取边界点位1对应的三角形个数
                        do w = 1, num_edges2, 1
                            if (mrl_new(ngrwm(w, m2)) == 4) cycle ! 跳过已经细化的三角形
                            if (m3 == ngrwm(w, m2)) then
                                isexist = .true.
                                exit
                            end if
                        end do
                        if (isexist) exit
                    end do
                    bdy_refine_segment_temp(k, num_bdy_refine_segment) = m3
                end do
            end do
            deallocate(segement_start_end)
            write(io6, *)  ""
            if (present(num_bdy_refine_segment_curve)) num_bdy_refine_segment_curve(i) = num_bdy_refine_segment
        end do

        allocate(bdy_refine_segment(set_dis_in, num_bdy_refine_segment))
        allocate(n_bdy_refine_segment(num_bdy_refine_segment))
        bdy_refine_segment = bdy_refine_segment_temp(:, 1:num_bdy_refine_segment)
        n_bdy_refine_segment = n_bdy_refine_segment_temp(1:num_bdy_refine_segment)
        deallocate(bdy_refine_segment_temp, n_bdy_refine_segment_temp)

    END SUBROUTINE bdy_refine_segment_make

    ! 目前有1+1和1+n和n+n三种种情况
    SUBROUTINE weak_concav_segment_make(set_dis_in, num_bdy_refine_segment, num_ref_weak_concav, ngrmw, num_bdy_refine_segment_curve, bdy_refine_segment, n_bdy_refine_segment, num_weak_concav_segment, num_weak_concav_pair, weak_concav_segment, n_weak_concav_segment, weak_concav_pair)
        ! 实现弱凹三角形的标记，弱凹三角形所在分段的标记，弱凹三角形分段中均只有一个三角形的标记与处理
        IMPLICIT NONE
        ! input
        integer, intent(in) :: set_dis_in, num_bdy_refine_segment, num_ref_weak_concav ! 最大分段长度，细化分段个数，弱凹三角形个数
        integer, allocatable, intent(in) :: ngrmw(:,:), num_bdy_refine_segment_curve(:)
        integer, allocatable, intent(inout) :: bdy_refine_segment(:,:) ! 存储细化三角形分组情况
        integer, allocatable, intent(inout) :: n_bdy_refine_segment(:) ! 存储每一个分段中三角形个数
        ! ouput
        integer, intent(out) :: num_weak_concav_segment, num_weak_concav_pair ! 弱凹分段个数， 弱凹pair个数
        integer, allocatable, intent(out) :: weak_concav_segment(:,:) ! 存储细化三角形分组情况
        integer, allocatable, intent(out) :: n_weak_concav_segment(:) ! 存储每一个分段中三角形个数
        integer, allocatable, intent(out) :: weak_concav_pair(:, :)
        integer :: i, j, m1, m2, ik, ii
        integer :: num_max, num_min, num_diff ! 两段中的最大长度，最短长度，长度差异
        integer :: num_bdy_refine_segment_temp
        integer, allocatable :: bdy_refine_segment_temp(:, :), n_bdy_refine_segment_temp(:)
        integer, allocatable :: weak_concav_segment_temp(:,:), n_weak_concav_segment_temp(:)
        integer, allocatable :: weak_concav_pair_temp(:)

        ! 先专门存储好，因为最后的数据范围可能会发生变化（再考虑了弱凹之后）
        allocate(bdy_refine_segment_temp(set_dis_in, num_bdy_refine_segment)); bdy_refine_segment_temp = bdy_refine_segment
        allocate(n_bdy_refine_segment_temp(num_bdy_refine_segment)); n_bdy_refine_segment_temp = n_bdy_refine_segment
        num_bdy_refine_segment_temp = num_bdy_refine_segment

        ! num_ref_weak_concav是弱凹总数，含1+1（pair）和1+n和n+n（segment）三种种情况
        allocate(weak_concav_segment_temp(set_dis_in, num_ref_weak_concav)); weak_concav_segment_temp = 1 ! 弱凹三角形所在分段的三角形编号，初始化为1 
        allocate(n_weak_concav_segment_temp(num_ref_weak_concav)); n_weak_concav_segment_temp = 0 ! 计算分段中三角形个数，初始化为0
        allocate(weak_concav_pair_temp(num_ref_weak_concav)); weak_concav_pair_temp = 1
        num_weak_concav_segment = 0 ! 用于记录弱凹左右两侧长度相同，而且不为1的情况
        num_weak_concav_pair = 0 ! 用于记录弱凹左右两侧长度为1的情况

        ii = 1
        do i = 1, num_bdy_refine_segment, 1
            j = i + 1
            ! 如果i是闭合曲线的最后一个分段，则j变为该曲线上第一个分段
            ! 确保每一次都在自己的闭合曲线内进行首尾连接
            if (i == num_bdy_refine_segment_curve(ii)) then
                j = num_bdy_refine_segment_curve(ii-1) + 1
                ! write(io6, '(A, I3, A, I3, A, I3)')  "start = ", j, ", end = ", i, ", num_closed_curve = ", ii
                ii = ii + 1
            end if

            m1 = bdy_refine_segment(n_bdy_refine_segment(i), i) ! 获取前一个分段最后一个三角形
            m2 = bdy_refine_segment(1, j) ! 获取后一个分段第一个三角形
            ik = IsNgrmm(ngrmw(1:3, m1), ngrmw(1:3, m2))
            if (ik == 0) cycle ! 说明三角形不是对偶弱凹
            num_max = max(n_bdy_refine_segment(i), n_bdy_refine_segment(j))
            num_min = min(n_bdy_refine_segment(i), n_bdy_refine_segment(j))
            num_diff = num_max - num_min ! 计算两者差异

            if (num_diff == 0) then
                if (set_dis_in == 1) then ! 针对过度行只有一行的情况
                    weak_concav_segment_temp(:, num_weak_concav_segment+1) = bdy_refine_segment(:, i)
                    weak_concav_segment_temp(:, num_weak_concav_segment+2) = bdy_refine_segment(:, j)
                    n_weak_concav_segment_temp(num_weak_concav_segment+1:num_weak_concav_segment+2) = n_bdy_refine_segment(i)
                    num_weak_concav_segment = num_weak_concav_segment + 2 ! 针对两侧长度一致且不为1的处理
                else
                    if (n_bdy_refine_segment(i) == 1) then ! 两段长度都为1
                        weak_concav_pair_temp(num_weak_concav_pair + 1) = m1 ! 记录弱凹三角形编号   
                        weak_concav_pair_temp(num_weak_concav_pair + 2) = m2 ! 记录弱凹三角形编号  
                        num_weak_concav_pair = num_weak_concav_pair + 2 ! 针对长度为1的情况进行特殊处理
                    else ! 两段长度都为n
                        weak_concav_segment_temp(:, num_weak_concav_segment+1) = bdy_refine_segment(:, i)
                        weak_concav_segment_temp(:, num_weak_concav_segment+2) = bdy_refine_segment(:, j)
                        n_weak_concav_segment_temp(num_weak_concav_segment+1:num_weak_concav_segment+2) = n_bdy_refine_segment(i)
                        num_weak_concav_segment = num_weak_concav_segment + 2 ! 针对两侧长度一致且不为1的处理
                    end if
                end if
                bdy_refine_segment_temp(:, [i,j]) = 1
                n_bdy_refine_segment_temp([i,j]) = 0

            else if (num_diff == 1) then
                ! STOP "ERROR! only 1+1 and n+n HERE!"
                if (num_min < 3) then ! 1+2或者2+3情况
                    weak_concav_segment_temp(1, num_weak_concav_segment+1) = bdy_refine_segment(n_bdy_refine_segment(i), i)
                    weak_concav_segment_temp(1, num_weak_concav_segment+2) = bdy_refine_segment(1, j)
                    n_weak_concav_segment_temp(num_weak_concav_segment+1:num_weak_concav_segment+2) = 1
                    num_weak_concav_segment = num_weak_concav_segment + 2
                    if (num_min == 2) then ! 2+3 = 1+2 and 2
                        if (n_bdy_refine_segment(i) > n_bdy_refine_segment(j)) then
                            bdy_refine_segment_temp(n_bdy_refine_segment(i), i) = 1
                            n_bdy_refine_segment_temp(i) = n_bdy_refine_segment_temp(i) - 1
                        else
                            bdy_refine_segment_temp(1:n_bdy_refine_segment(j)-1, j) = bdy_refine_segment(2:n_bdy_refine_segment(j), j)
                            bdy_refine_segment_temp(n_bdy_refine_segment(j), j) = 1
                            n_bdy_refine_segment_temp(j) = n_bdy_refine_segment_temp(j) - 1
                        end if
                    end if
                else ! n1+n2(n1,n2>2) = 1+1 and n1-1 and n2-1
                    weak_concav_pair_temp(num_weak_concav_pair + 1) = m1 ! 记录弱凹三角形编号   
                    weak_concav_pair_temp(num_weak_concav_pair + 2) = m2 ! 记录弱凹三角形编号  
                    num_weak_concav_pair = num_weak_concav_pair + 2 ! 针对长度为1的情况进行特殊处理
                    bdy_refine_segment_temp(n_bdy_refine_segment(i), i) = 1
                    if (n_bdy_refine_segment(j) /= 1) then
                        bdy_refine_segment_temp(1:n_bdy_refine_segment(j)-1, j) = bdy_refine_segment(2:n_bdy_refine_segment(j), j)
                    end if
                    bdy_refine_segment_temp(n_bdy_refine_segment(j), j) = 1
                    n_bdy_refine_segment_temp(i) = n_bdy_refine_segment_temp(i) - 1
                    n_bdy_refine_segment_temp(j) = n_bdy_refine_segment_temp(j) - 1
                end if

            else ! num_diff >=2
                ! STOP "ERROR! only 1+1 and n+n HERE!"
                if (num_min == 1) then ! 1+n（n>2） ! 情况A2 1+n(n>=3) 分为1+1和n-1
                    weak_concav_pair_temp(num_weak_concav_pair + 1) = m1 ! 记录弱凹三角形编号   
                    weak_concav_pair_temp(num_weak_concav_pair + 2) = m2 ! 记录弱凹三角形编号  
                    num_weak_concav_pair = num_weak_concav_pair + 2 ! 针对长度为1的情况进行特殊处理
                    bdy_refine_segment_temp(n_bdy_refine_segment(i), i) = 1
                    if (n_bdy_refine_segment(j) /= 1) then
                        bdy_refine_segment_temp(1:n_bdy_refine_segment(j)-1, j) = bdy_refine_segment(2:n_bdy_refine_segment(j), j)
                    end if
                    bdy_refine_segment_temp(n_bdy_refine_segment(j), j) = 1
                    n_bdy_refine_segment_temp(i) = n_bdy_refine_segment_temp(i) - 1
                    n_bdy_refine_segment_temp(j) = n_bdy_refine_segment_temp(j) - 1
                else ! num_min > 2 n1+n2(n1,n2>2) ---> num_min+min_min and num_diff
                    bdy_refine_segment_temp(num_diff+1:n_bdy_refine_segment(i), i) = 1
                    if (n_bdy_refine_segment(j) /= num_min) then
                        bdy_refine_segment_temp(1:num_diff, j) = bdy_refine_segment(n_bdy_refine_segment(j)-num_diff+1:n_bdy_refine_segment(j), j)
                    end if
                    bdy_refine_segment_temp(num_diff+1:n_bdy_refine_segment(j), j) = 1
                    n_bdy_refine_segment_temp(i) = n_bdy_refine_segment_temp(i) - num_min
                    n_bdy_refine_segment_temp(j) = n_bdy_refine_segment_temp(j) - num_min
                    weak_concav_segment_temp(:, num_weak_concav_segment+1) = bdy_refine_segment_temp(:, i)
                    weak_concav_segment_temp(:, num_weak_concav_segment+2) = bdy_refine_segment_temp(:, j)
                    n_weak_concav_segment_temp(num_weak_concav_segment+1:num_weak_concav_segment+2) = n_bdy_refine_segment(i)
                    num_weak_concav_segment = num_weak_concav_segment + 2 ! 针对两侧长度一致且不为1的处理
                end if
            end if
        end do

        if (num_ref_weak_concav /= (num_weak_concav_segment + num_weak_concav_pair)) then
            write(io6, *)  "num_ref_weak_concav = ", num_ref_weak_concav
            write(io6, *)  "num_weak_concav_segment(n+n)or(1+n) = ", num_weak_concav_segment
            write(io6, *)  "num_weak_concav_pair(1+1) = ", num_weak_concav_pair
            stop "ERROR! num_ref_weak_concav /= (num_weak_concav_segment + num_weak_concav_pair) in SUBROUTINE weak_concav_segment_make"
        end if
        bdy_refine_segment = bdy_refine_segment_temp
        n_bdy_refine_segment = n_bdy_refine_segment_temp

        allocate(weak_concav_segment(set_dis_in, num_ref_weak_concav))
        allocate(n_weak_concav_segment(num_ref_weak_concav))
        weak_concav_segment = weak_concav_segment_temp
        n_weak_concav_segment = n_weak_concav_segment_temp

        ! weak_concav_pair的数据也会存放在weak_concav_segment中，存放在后面
        if (num_weak_concav_pair /= 0) then
            allocate(weak_concav_pair(2, num_weak_concav_pair))
            weak_concav_pair(1, 1:num_weak_concav_pair) = weak_concav_pair_temp(1:num_weak_concav_pair)
            weak_concav_segment(1, num_weak_concav_segment+1:num_ref_weak_concav) = weak_concav_pair(1, 1:num_weak_concav_pair)
            n_weak_concav_segment(num_weak_concav_segment+1:num_ref_weak_concav) = 1
        end if
        deallocate(bdy_refine_segment_temp, n_bdy_refine_segment_temp, weak_concav_segment_temp, n_weak_concav_segment_temp, weak_concav_pair_temp)

    END SUBROUTINE weak_concav_segment_make

    SUBROUTINE OnedivideTwo(iter, isreverse, ngrmw, ngrmm, ngrwm, num_mp, num_wp, mp_new, wp_new, mrl_new, ngrmw_new, sjx_child)
        ! 一分二算法（针对细化向非细化的过渡）
        IMPLICIT NONE
        ! 内部自变量
        integer :: i, j, k, icl, num_ref, refed_iter 
        integer :: m1, m2, w1, w2, w3, w4     
        integer :: hhh(5)
        real(r8) :: sjx(3, 2), tempa(1,2),tempb(1,2), tempc(1,2) 
        ! 外部读入变量
        integer,  intent(in) :: iter
        logical,  intent(in) :: isreverse
        integer,  dimension(:, :), allocatable, intent(in) :: ngrwm, ngrmm, ngrmw
        integer,  dimension(:), intent(inout) :: num_mp, num_wp
        real(r8), dimension(:, :), allocatable, intent(inout) :: mp_new, wp_new
        integer,  dimension(:),    allocatable, intent(inout) :: mrl_new
        integer,  dimension(:, :), allocatable, intent(inout) :: ngrmw_new
        integer,  dimension(:, :), allocatable, intent(inout) :: sjx_child

        ! 开始细化
        hhh = [1,2,3,1,2]
        refed_iter = 0
        do i = num_vertex + 1, num_mp(1), 1
            if (ref_sjx(i) == 0) cycle
            k = 0
            if (.not. isreverse) then ! 找到邻域中唯一一个被细化的三角形
                do j = 1, 3, 1
                    if (mrl_new(ngrmm(j, i)) == 4) k = ngrmm(j, i)
                end do
            else ! 找到邻域中唯一一个没有被细化的三角形(反向一分二)
                do j = 1, 3, 1
                    if (mrl_new(ngrmm(j, i)) == 1) k = ngrmm(j, i)
                end do
            end if
            if (k == 0) stop "k==0 in Line 1714"
            ! 确定k的目的在于找到两个三角形相接的边

            do j = 1, 3, 1
                if( (ngrmw_new(j, i) /= ngrmw(1, k)) .and. & 
                    (ngrmw_new(j, i) /= ngrmw(2, k)) .and. &
                    (ngrmw_new(j, i) /= ngrmw(3, k)) )then ! 找到三角形i与细化三角公共边不相交的顶点
                    ! 根据公共边的位置，获取顶点编号，不建议用w1到w3应该w1和m1都要新增加的含义，
                    ! 但是这里只有w4是新增加的
                    w1 = ngrmw_new(hhh(j), i)
                    w2 = ngrmw_new(hhh(j+1), i)
                    w3 = ngrmw_new(hhh(j+2), i)
                    sjx(1, 1:2) = wp_new(w1, 1:2)
                    sjx(2, 1:2) = wp_new(w2, 1:2)
                    sjx(3, 1:2) = wp_new(w3, 1:2)
                    !!!!!!!!!!!!!!!!!!!!!!!! 
                    ! write(io6, *)  "w1 = ", w1
                    ! write(io6, *)  "w2 = ", w2
                    ! write(io6, *)  "w3 = ", w3
                    !!!!!!!!!!!!!!!!!!!!!!!!
                end if       
            end do
            icl = 0
            if (maxval(sjx(:, 1)) - minval(sjx(:, 1)) > 180.) then
                icl = 1
                CALL CheckCrossing(3, sjx)
            end if
            tempc(1, :) = (sjx(2, :) + sjx(3, :)) / 2.! 获取公共边中点的经纬度

            m1 = num_mp(iter - 1) + refed_iter * 2 + 1
            m2 = num_mp(iter - 1) + refed_iter * 2 + 2
            tempa(1, :) = (sjx(1, :) + tempc(1, :) + sjx(2, :)) / 3.! 新增加两个三角形的中心点经纬度
            tempb(1, :) = (sjx(1, :) + tempc(1, :) + sjx(3, :)) / 3.

            w4 = num_wp(iter - 1) + refed_iter + 1
            
            ngrmw_new(1, m1) = w1! 增加第一个新三角形的顶点信息
            ngrmw_new(2, m1) = w2
            ngrmw_new(3, m1) = w4
            ngrmw_new(1, m2) = w1! 增加第二个新三角形的顶点信息
            ngrmw_new(2, m2) = w3
            ngrmw_new(3, m2) = w4

            if (icl /= 0) then! 经度复原
                Call CheckCrossing(1, tempa)
                Call CheckCrossing(1, tempb)
                Call CheckCrossing(1, tempc)
            end if
            mp_new(m1, 1:2) = tempa(1, :)
            mp_new(m2, 1:2) = tempb(1, :)
            wp_new(w4, 1:2) = tempc(1, :)
            ngrmw_new(:, i) = 1
            refed_iter = refed_iter + 1
            sjx_child(:, i) = [m1, m2]
        end do
        CALL crossline_check(iter, mp_new, wp_new, num_mp, num_wp)

    END SUBROUTINE OnedivideTwo

    SUBROUTINE ref_sjx_isreverse_judge(set_dis_in, num_segment, ngrmm, mrl_new, segment, n_segment)
        ! 专门处理三角形所在分段中需要反向一分二而且确定下一轮需要正向一分二的三角形(强凹与弱凹都适应)
        IMPLICIT NONE
        integer :: set_dis_in, num_segment
        integer, allocatable, intent(in) :: ngrmm(:,:), mrl_new(:)
        integer, allocatable, intent(inout) :: segment(:,:), n_segment(:)
        integer :: i, j, m0, w0, m, w, m1, w1
        logical :: isexist
        integer, allocatable :: segment_select(:)

        allocate(segment_select(set_dis_in))
        do i = 1, num_segment, 1
            if (n_segment(i) == 0) cycle ! 跳过不符合的过渡等级 
            segment_select = segment(:, i) ! 临时存储
            segment(:, i) = 1 ! 重新初始化，存储下一轮需要细化的三角形
            do j = 1, set_dis_in-1, 1
                if (segment_select(j+1) == 1) exit ! 三角形不存在就直接结束，退出
                m0 = segment_select(j)
                w0 = segment_select(j+1)
                isexist = .false.
                do m = 1, 3, 1
                    m1 = ngrmm(m, m0) ! 获取对应的相邻三角形
                    do w = 1, 3, 1
                        w1 = ngrmm(w, w0) ! 获取对应的相邻三角形
                        if (m1 == w1) then
                            isexist = .true.
                            exit
                        end if
                    end do
                    if (isexist) exit
                end do
                ref_sjx(m1) = 1 ! 获取需要反向一分为二的三角形

                ! 获取下一轮需要细化的三角形，确保只有一个三角形被细化，所以一分二的细化标记放在外部进行
                do m = 1, 3, 1
                    if (mrl_new(ngrmm(m, m1)) == 4) cycle
                    segment(j, i) = ngrmm(m, m1)
                end do
            end do
        end do
        deallocate(segment_select)

    END SUBROUTINE ref_sjx_isreverse_judge

    SUBROUTINE weak_concav_pair_special(num_weak_concav_pair, num_ref_weak_concav, ngrmm, ngrmw, mrl_new, weak_concav_pair, weak_concav_segment, n_weak_concav_segment)
        ! 专门处理弱凹三角形所在分段中三角形个数为1的情况，确定弱凹所对应的配对三角形并确定下一次迭代中需要正向一分二的三角形
        IMPLICIT NONE
        integer :: num_weak_concav_pair, num_ref_weak_concav
        integer, allocatable, intent(in) :: ngrmm(:,:), ngrmw(:,:)
        integer, allocatable, intent(in) :: n_weak_concav_segment(:)
        integer, allocatable, intent(inout) :: mrl_new(:)
        integer, allocatable, intent(inout) :: weak_concav_pair(:, :)
        integer, allocatable, intent(inout) :: weak_concav_segment(:, :)
        integer :: k, m, m1, m2, m3, m4, mm, w1, n
        integer, allocatable :: mrl_renew(:)
        logical :: isexist

        allocate(mrl_renew(num_weak_concav_pair)); mrl_renew = 1
        do k = 1, num_weak_concav_pair, 1
            m1 = weak_concav_pair(1, k) ! 获取弱凹三角形
            ! 获取对偶弱凹三角形
            if (mod(k, 2) == 0) then
                m2 = weak_concav_pair(1, k-1)
            else
                m2 = weak_concav_pair(1, k+1)
            end if

            do m = 1, 3, 1
                m3 = ngrmm(m, m1) ! 获取弱凹中指向外侧的三角形
                if (mrl_new(m3) == 4) cycle
                exit 
            end do
            weak_concav_pair(2, k) = m3 ! 说明该弱凹指向外侧的三角形需要LOP变换
            ref_sjx(m3) = 1 ! 该三角形细化反向一分二 ! 这个三角形可能在强凹区已经确认要一分二细化
            
            ! 获取下一轮需要细化的三角形，有两个非细化的三角形，将其中一个标记为已经细化
            do m = 1, 3, 1
                m4 = ngrmm(m, m3) ! 指向非细化的三角形
                if (mrl_new(m4) == 4) cycle ! 说明指向了自身
                isexist = .true. ! 说明多边形顶点不相接
                do n = 1, 3, 1
                    w1 = ngrmw(n, m4)
                    if (any(w1 == ngrmw(:,m2))) then
                        isexist = .false.
                        exit
                    end if
                end do
                if (isexist) then ! 找到三角形i与细化三角公共边不相交的顶点
                    ! 记录下来，最后统一更新
                    mrl_renew(k) = m4
                else
                    mm = num_ref_weak_concav - num_weak_concav_pair + k
                    weak_concav_segment(1, mm) = m4
                    ! write(io6, *)  "mm = ", mm, "m4 = ", m4
                end if
            end do
        end do

        do k = 1, num_weak_concav_pair, 1
            mrl_new(mrl_renew(k)) = 4
        end do
        deallocate(mrl_renew)

    END SUBROUTINE weak_concav_pair_special

    ! 利用bdy_refine_segment_old 和 bdy_refine_segment,这个才是为强凹设计的
    SUBROUTINE sharp_concav_lop_judge(set_dis_in, num_ref, num_bdy_refine_segment, mrl_new, ngrmm, ngrmw_new, sjx_child, bdy_refine_segment, bdy_refine_segment_old, n_bdy_refine_segment, ref_sjx_lop_temp, n_ref_sjx_lop_temp)
        
        IMPLICIT NONE
        integer, intent(in) :: set_dis_in, num_bdy_refine_segment
        integer, intent(inout) :: num_ref
        integer, allocatable, intent(in) :: mrl_new(:), ngrmm(:, :), ngrmw_new(:, :)
        integer, allocatable, intent(in) :: sjx_child(:, :)
        integer, allocatable, intent(in) :: bdy_refine_segment(:,:), bdy_refine_segment_old(:,:)
        integer, allocatable, intent(in) :: n_bdy_refine_segment(:)
        integer, allocatable, intent(inout) :: ref_sjx_lop_temp(:,:), n_ref_sjx_lop_temp(:)
        integer :: i, j, k, w0, w1, m, m1, m2, m11, w11, m22, w12, k2 
        integer :: num_end, tran_degree
        logical :: isexist

        do i = 1, num_bdy_refine_segment, 1
            tran_degree = n_ref_sjx_lop_temp(i) + 1 ! 获取当前过渡行等级，临时加一，便于后续操作
            if (tran_degree == 1) cycle ! 跳过不符合的过渡等级 
            do j = 1, tran_degree-1, 1 ! tran_degree == 1 说明是本轮原分段中还有两个三角形
                ! 开始按顺序存储两两配对的对边三角形
                m1 = bdy_refine_segment_old(j, i)
                !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                w0 = bdy_refine_segment(j, i)
                ! w0是下一轮需要正向一分二的三角形，他的对偶三角形才是我们需要的
                do m = 1, 3, 1
                    if (mrl_new(ngrmm(m, w0)) == 1) cycle
                    w1 = ngrmm(m, w0) ! 这才是反向一分二的三角形
                    exit
                end do
                !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                m2 = bdy_refine_segment_old(j+1, i)

                ! 获取m11, w11并赋值
                CALL m1w1_to_m11w11(m1, w1, sjx_child, ngrmw_new, m11, w11)

                ! 获取w12, m22并赋值
                w12 = sjx_child(1, w1)
                if (w12 == w11) w12 = sjx_child(2, w1)
                do k2 = 1, 2, 1
                    m22 = sjx_child(k2, m2)
                    if (IsNgrmm(ngrmw_new(1:3, w12), ngrmw_new(1:3, m22)) /= 0) exit
                end do
                ref_sjx_lop_temp(4*j-3:4*j, i) = [m11, w11, w12, m22] ! 每次放四个三角形，两两配对
            end do
            num_end = 4*(tran_degree-1) ! 定制化处理
            n_ref_sjx_lop_temp(i) = INT(tran_degree/2) * 4 ! 获取num_ref的长度
            num_ref = num_ref + n_ref_sjx_lop_temp(i)
            if (tran_degree == 2) cycle
            ! write(io6, *)  "n_ref_sjx_lop_temp(i) = ", n_ref_sjx_lop_temp(i)
            do k = 1, n_ref_sjx_lop_temp(i), 4
                ! 在相邻位置获取另一端的数据
                ref_sjx_lop_temp(k+2:k+3, i) =  ref_sjx_lop_temp(num_end-k:num_end-k+1, i)
            end do
            ! write(io6, *)  "after ref_sjx_lop_temp(1:num_end, i) = ", ref_sjx_lop_temp(1:num_end, i)
            ! write(io6, *)  ""
        end do

    END SUBROUTINE sharp_concav_lop_judge

    ! 专门针对弱凹三角形的处理，需要分为（1+1和n+n）两种情况去讨论，这部分代码还需要进一步修改
    SUBROUTINE weak_concav_lop_judge(set_dis_in, num_ref, num_bdy_refine_segment, num_ref_weak_concav, num_weak_concav_segment, num_weak_concav_pair, mrl_new, ngrmm, ngrmw_new, sjx_child, &
                                    weak_concav_segment, weak_concav_segment_old, n_weak_concav_segment, weak_concav_pair, ref_sjx_lop_temp, n_ref_sjx_lop_temp)

        IMPLICIT NONE
        integer, intent(in) :: set_dis_in, num_bdy_refine_segment
        integer, intent(in) :: num_ref_weak_concav, num_weak_concav_segment, num_weak_concav_pair
        integer, intent(inout) :: num_ref
        integer, allocatable, intent(in) :: mrl_new(:), ngrmm(:, :), ngrmw_new(:, :)
        integer, allocatable, intent(in) :: sjx_child(:, :)
        integer, allocatable, intent(inout) :: weak_concav_segment(:,:)
        integer, allocatable, intent(in) :: weak_concav_segment_old(:,:)
        integer, allocatable, intent(in) :: n_weak_concav_segment(:)
        integer, allocatable, intent(in) :: weak_concav_pair(:,:)
        integer, allocatable, intent(inout) :: ref_sjx_lop_temp(:,:), n_ref_sjx_lop_temp(:)
        integer :: i, j, k, w0, w1, m, m1, m11, w11, kk
        integer :: num_end

        ! 针对弱凹而且左右两侧长度均以1的情况
        if (num_weak_concav_pair /= 0) then
            do i = 1, num_weak_concav_pair, 1
                m1 = weak_concav_pair(1, i)
                w1 = weak_concav_pair(2, i)
                CALL m1w1_to_m11w11(m1, w1, sjx_child, ngrmw_new, m11, w11) ! 获取m11, w11并赋值
                m = num_bdy_refine_segment+num_weak_concav_segment+i
                n_ref_sjx_lop_temp(m) = 2 ! 获取num_ref的长度
                num_ref = num_ref + n_ref_sjx_lop_temp(m)
                ref_sjx_lop_temp(1:2, m) = [m11, w11]
            end do
            num_end = num_weak_concav_segment
        else
            num_end = num_ref_weak_concav
        end if

        ! 这个可以考虑修改为多对多的弱凹细化处理哈哈哈哈(存在两个方向的问题，一个是分段内部，一个是分段与分段之间)
        if (num_weak_concav_segment /= 0) then
            do i = 1, num_end, 1
                if (weak_concav_segment(1, i) == 1) cycle ! 跳过已经不存在的三角形
                ! write(io6, *)  "i = ", i, "in Line 1889"
                m = i + num_bdy_refine_segment
                kk = 0
                ! 分段之间
                if (mod(i, 2) /= 0) then ! 将去除八边形的LOP变换的三角形放在弱凹左侧
                    m1 = weak_concav_segment_old(n_weak_concav_segment(i)+1, i) ! 弱凹左侧
                    w1 = weak_concav_segment_old(1, i+1) ! 弱凹右侧
                    CALL m1w1_to_m11w11(m1, w1, sjx_child, ngrmw_new, m11, w11) ! 获取m11, w11并赋值
                    n_ref_sjx_lop_temp(m) = 2 ! 获取num_ref的长度
                    num_ref = num_ref + 2
                    ref_sjx_lop_temp(kk+1:kk+2, m) = [m11, w11]
                    kk = kk + 2
                    if (n_weak_concav_segment(i) == 0) then
                        weak_concav_segment(:, i:i+1) = 1
                        cycle
                    end if
                end if
                ! if (n_weak_concav_segment(i) == 0) cycle
                ! write(io6, *)  "n_weak_concav_segment(i) = ", n_weak_concav_segment(i), "说明存在两侧长度大于1的弱凹 in SUBROUTINE weak_concav_lop_judge"

                ! 分段内部
                do j = 1, n_weak_concav_segment(i), 1 ! 已经进行了减一操作
                    m1 = weak_concav_segment_old(j-mod(i, 2)+1, i) ! 理论上左右两侧都适用
                    !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                    w0 = weak_concav_segment(j, i)
                    ! w0是下一轮需要正向一分二的三角形，他的对偶三角形才是我们需要的
                    do k = 1, 3, 1
                        if (mrl_new(ngrmm(k, w0)) == 1) cycle
                        w1 = ngrmm(k, w0) ! w1这才是反向一分二的三角形
                        exit
                    end do
                    !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                    CALL m1w1_to_m11w11(m1, w1, sjx_child, ngrmw_new, m11, w11) ! 获取m11, w11并赋值
                    n_ref_sjx_lop_temp(m) = n_ref_sjx_lop_temp(m) + 2 ! 获取num_ref的长度
                    num_ref = num_ref + 2
                    ref_sjx_lop_temp(kk+1:kk+2, m) = [m11, w11]
                    kk = kk + 2
                end do
            end do
        end if

    END SUBROUTINE weak_concav_lop_judge

    SUBROUTINE m1w1_to_m11w11(m1, w1, sjx_child, ngrmw_new, m11, w11)

        IMPLICIT NONE
        integer, intent(in) :: m1, w1
        integer, allocatable, intent(in) :: sjx_child(:,:), ngrmw_new(:,:)
        integer, intent(out) :: m11, w11
        integer :: k1, k2
        logical :: isexist

        isexist = .false.
        do k1 = 1, 2, 1
            do k2 = 1, 2, 1
                m11 = sjx_child(k1, m1)
                w11 = sjx_child(k2, w1)
                if (IsNgrmm(ngrmw_new(1:3, w11), ngrmw_new(1:3, m11)) /= 0) then
                    isexist = .true.
                    exit
                end if
            end do
            if (isexist) exit
        end do
        if (isexist .eqv. .false.) stop "ERROR! isexist .eqv. .false. in SUBROUTINE m1w1_to_m11w11"

    END SUBROUTINE m1w1_to_m11w11

    SUBROUTINE Delaunay_Lop(iter, num_ref, num_mp, num_wp, mp_new, wp_new, ngrmw_new, ref_sjx_lop)            
        ! 对角变换
        IMPLICIT NONE
        ! 内部自变量
        integer :: i, j, k, x, icl, refed_iter
        integer :: m, m1, m2
        integer :: w, w1, w2, w3, w4      ! 三角形和多边形中心点序号起始索引
        real(r8) :: newdbx(4, 2), newsjx(2, 2)  ! 数组大小是不一样的
        ! 外部读入变量
        integer,  intent(in) :: iter, num_ref
        integer,  dimension(:), intent(inout) :: num_mp, num_wp
        real(r8), dimension(:, :), allocatable, intent(inout) :: mp_new, wp_new
        integer,  dimension(:, :), allocatable, intent(inout) :: ngrmw_new
        integer,  dimension(:),    allocatable, intent(in) :: ref_sjx_lop

        ! 开始细化弱凹点 : two adjacent triangle in a polygon need to refine 
        refed_iter = 0
        do k = 1, num_ref/2, 1
            ! if (mod(k, 2) == 1) cycle ! 只部分执行看看效果
            i = ref_sjx_lop(2*k-1)
            j = ref_sjx_lop(2*k)
            if (i==0 .or. j==0) then
                write(io6, *)  "i = ", i, "j = ", j, "in Line 1971 SUBROUTINE Delaunay_Lop"
                cycle ! 不应该出现zero，暂时不知道为什么
            end if

            do x = 1, 3, 1
                ! 判断顶点w1到w3的位置 判断顶点是否在三角形j的顶点上
                ! 只有当所有这三个条件同时满足时，整个条件语句的结果才为 true
                if ((ngrmw_new(x, i) /= ngrmw_new(1, j)) .and. &
                    (ngrmw_new(x, i) /= ngrmw_new(2, j)) .and. & 
                    (ngrmw_new(x, i) /= ngrmw_new(3, j)) ) then 
                    w1 = ngrmw_new(x, i) ! 判断为真，说明x是三角形i is relative to triangle j 的游离的顶点
                end if
            end do

            do x = 1, 3, 1
                if (w1 /= ngrmw_new(x, i)) then 
                    w2 = ngrmw_new(x, i)
                    exit ! 找到一个就退出，
                end if
            end do

            do x = 1, 3, 1
                ! 只有当所有条件同时满足时，整个条件语句的结果才为 true
                if ((w1 /= ngrmw_new(x, i)) .and. (w2 /= ngrmw_new(x, i)) ) w4 = ngrmw_new(x, i)
            end do

            do x = 1, 3, 1
                ! 判断顶点w3的位置
                if ((ngrmw_new(x, j) /= ngrmw_new(1, i)) .and. &
                    (ngrmw_new(x, j) /= ngrmw_new(2, i)) .and. & 
                    (ngrmw_new(x, j) /= ngrmw_new(3, i)) ) then 
                    w3 = ngrmw_new(x, j) ! 判断为真，说明是三角形j is relative to triangle i的游离的顶点
                end if
            end do
            !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

            m1 = num_mp(iter - 1) + refed_iter * 2 + 1
            m2 = num_mp(iter - 1) + refed_iter * 2 + 2

            ! m1
            ngrmw_new(1, m1) = w1
            ngrmw_new(2, m1) = w2
            ngrmw_new(3, m1) = w3
            ! m2
            ngrmw_new(1, m2) = w1
            ngrmw_new(2, m2) = w4
            ngrmw_new(3, m2) = w3

            ! 赋值，尽量不适用原来的数据
            newdbx(1, :) = wp_new(w1, :)
            newdbx(2, :) = wp_new(w2, :)
            newdbx(3, :) = wp_new(w3, :)
            newdbx(4, :) = wp_new(w4, :)
            icl = 0
            if (maxval(newdbx(1:4, 1)) - minval(newdbx(1:4, 1)) > 180.) then ! need to modify sjx first
                icl = 1! 判断是否存在跨越180经线，有则修正，并返回状态变量icl
                CALL CheckCrossing(4, newdbx)
            end if

            ! 两个相连三角形对边交换形成新的三角形
            newsjx(1, 1:2) = (newdbx(1, 1:2) + newdbx(2, 1:2) + newdbx(3, 1:2)) / 3.
            newsjx(2, 1:2) = (newdbx(1, 1:2) + newdbx(4, 1:2) + newdbx(3, 1:2)) / 3.
            ! 经度修正(对新生成的m或者w点的经度进行矫正)
            if (icl /= 0) CALL CheckCrossing(2, newsjx)
            mp_new(m1:m2,:) = newsjx
            ! 将旧三角形的信息去掉
            ngrmw_new(:, i) = 1
            ngrmw_new(:, j) = 1
            refed_iter = refed_iter + 1
        end do
        CALL crossline_check(iter, mp_new, wp_new, num_mp, num_wp)

    END SUBROUTINE Delaunay_Lop

    SUBROUTINE crossline_check(iter, mp_new, wp_new, num_mp, num_wp)

        IMPLICIT NONE
        integer :: i
        integer,  intent(in) :: iter
        real(r8), dimension(:, :), allocatable, intent(inout) :: mp_new, wp_new
        integer,  dimension(:), intent(in) :: num_mp, num_wp
        do i = num_mp(iter - 1) + 1, num_mp(iter), 1 ! mp_new is the center of sjx
            if (mp_new(i, 1) == -180.) mp_new(i, 1) = 180.
        end do
        do i = num_wp(iter - 1) + 1, num_wp(iter), 1 ! wp_new is the center of lbx
            if (wp_new(i, 1) == -180.) wp_new(i, 1) = 180.
        end do

    END SUBROUTINE crossline_check

    SUBROUTINE NGR_RENEW(iter, num_mp, num_wp, mp_new, wp_new, ngrmw_new, num_sjx, num_dbx, mp_f, wp_f, ngrmw_f, ngrwm_f, n_ngrwm_f, bdy_refine, bdy_refine_tran)
        ! 更新 mp_f, wp_f, ngrmw_f, ngrwm_f, n_ngrwm_f, bdy_refine, bdy_refine_tran
        implicit none
        integer, intent(in) :: iter
        integer,  dimension(:), intent(in) :: num_mp, num_wp
        real(r8), dimension(:, :), allocatable, intent(in) :: mp_new, wp_new
        integer,  dimension(:, :), allocatable, intent(in) :: ngrmw_new
        integer, intent(out) :: num_sjx, num_dbx
        real(r8), dimension(:, :), allocatable, intent(inout) :: mp_f, wp_f
        integer,  dimension(:, :), allocatable, intent(inout) :: ngrmw_f, ngrwm_f
        integer,  dimension(:),    allocatable, intent(inout) :: n_ngrwm_f
        integer,  dimension(:),    allocatable, intent(inout) :: bdy_refine, bdy_refine_tran
        integer :: i, j, k, num_ref
        integer :: ncid, spDimID, lpDimID, dimaID, dimbID, ncvarid(2) 
        logical :: isexist                ! 判断细化后是否存在重复w点
        integer,  allocatable :: vertex_mapping(:)  ! 新旧顶点之间的映射关系
        character(pathlen) :: lndname
        character(LEN = 5) :: nxpc, stepc
        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') step
        ! m点 增加，不会重复；减少（只在原来的范围内）m点具有唯一性
        ! w点 增加，会重复，不同编号对应同一个点位；不会减少 w点没有唯一性


        write(io6, *)  "wp_f start"
        allocate(wp_f(num_wp(iter), 2)); wp_f(:, 1:2) = 9999. ! 初始化
        allocate(vertex_mapping(num_wp(iter))); vertex_mapping = 0
        num_dbx = num_wp(1) ! 初始化
        wp_f(1:num_wp(1), :) = wp_new(1:num_wp(1), :)
        vertex_mapping(1:num_wp(1)) = [(j, j=1, num_wp(1))]

        do i = num_wp(1) + 1, num_wp(iter), 1
            isexist = .false.
            do j = num_wp(1) + 1, num_dbx + 1, 1 ! 新点是否与原始顶点有映射关系 
                if ((wp_f(j, 1) == wp_new(i, 1)) .and. &
                    (wp_f(j, 2) == wp_new(i, 2)) ) then
                    isexist = .true.
                    exit
                end if
            end do
            if (isexist .eqv. .false.) then
                num_dbx = num_dbx + 1
                wp_f(num_dbx, 1:2) = wp_new(i, 1:2)
                vertex_mapping(i) = num_dbx
            else
                vertex_mapping(i) = j
            end if
        end do

        write(io6, *)  "max(vertex_mapping) = ", maxval(vertex_mapping)
        if (maxval(vertex_mapping) /= num_dbx) stop "maxval(vertex_mapping) /= num_dbx"

        write(io6, *)  "细化前共有", num_wp(1), "个多边形网格"
        write(io6, *)  "细化后共有", num_wp(iter), "个多边形网格"
        write(io6, *)  "去除重复点后，还剩", num_dbx, "个多边形网格"
        write(io6, *)  "wp_f finish"
        write(io6, *)  ""

        lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc)  // "_" // trim(stepc) // "_wp_f.nc4"
        ! write(io6, *)  lndname
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncID, "lbx_points", num_wp(iter), lpDimID))
        CALL CHECK(NF90_DEF_DIM(ncID, "dim_a", 2, DimaID))
        CALL CHECK(NF90_DEF_VAR(ncID, "vertex_mapping", NF90_INT, (/ lpDimID /), ncVarID(1)))
        CALL CHECK(NF90_DEF_VAR(ncID, "wp_f", NF90_DOUBLE, (/ lpDimID, DimaID /), ncVarID(2)))
        CALL CHECK(NF90_ENDDEF(ncID))
        CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(1), vertex_mapping))
        CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(2), wp_f))
        CALL CHECK(NF90_CLOSE(ncID))


        ! 更新 mp_f ! 如果ngrmw_new不存在，则这个三角形不存在 
        write(io6, *)  "重新计算ngrmw_new and mp_f，并储存mp_f" 
        ! m点的特点是会增加，也是减少（只在原来的范围内），但是不会重复！！！！！！！！！！！
        ! 统计初始三角形中被细化的个数，此时的ngrmw_new 还没有进行重新编号
        write(io6, *)  "mp_f start"
        num_ref = 0
        ! 这里可能需要修改，因为对角变换的时候会删去新生成的三角形
        ! do i = num_vertex + 1, num_mp(1), 1
        do i = num_vertex + 1, num_mp(iter), 1
            if (ngrmw_new(1, i) == 1) num_ref = num_ref + 1 ! 当三角顶点不存在的时候，跳过
        end do

        num_sjx = num_mp(iter) - num_ref ! 获取三角形总数
        allocate(mp_f(num_sjx, 2)); mp_f = 0. ! 经纬度
        allocate(ngrmw_f(3, num_sjx)); ngrmw_f = 1 ! 顶点编号
        ngrmw_f(:, 1:num_vertex) = ngrmw_new(:, 1:num_vertex)
        mp_f(1:num_vertex, :) = mp_new(1:num_vertex, :)
        k = num_vertex
        do i = num_vertex + 1, num_mp(iter), 1 ! 因为细化的时候会把旧的三角形去除，所有还是从2还是比较稳妥
            if (ngrmw_new(1, i) == 1) cycle ! 跳过原来三角形中被细化的那部分三角形，
            k = k + 1 ! 累加，用于进位
            mp_f(k, :) = mp_new(i, :)
            !!!!!!!!!!!!!!!!!!!!!!!! add by RuiZhang !!!!!!!!!!!!!!!!
            ! ref_tr_f(k, :) = ref_tr(i, :)
            !!!!!!!!!!!!!!!!!!!!!!!! add by RuiZhang !!!!!!!!!!!!!!!!
            ngrmw_f(:, k) = ngrmw_new(:, i)
        end do
        write(io6, *)  "细化前共有", num_mp(1), "个三角形网格"
        write(io6, *)  "细化后共有", num_mp(iter), "个三角形网格"
        write(io6, *)  "去除重复点后，还剩", num_sjx, "个三角形网格"
        write(io6, *)  "mp_f finish"
        write(io6, *)  ""
        lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc)  // "_" // trim(stepc) // "_mp_f.nc4"
        ! write(io6, *)  lndname
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncID, "sjx_points", num_sjx, spDimID))
        CALL CHECK(NF90_DEF_DIM(ncID, "dim_a", 2, DimaID))
        CALL CHECK(NF90_DEF_DIM(ncID, "dim_b", 3, DimbID))
        CALL CHECK(NF90_DEF_VAR(ncID, "mp_f", NF90_DOUBLE, (/ spDimID, DimaID /), ncVarID(1)))
        CALL CHECK(NF90_DEF_VAR(ncID, "ngrmw_f_orial", NF90_INT, (/ DimbID, spDimID /), ncVarID(2)))
        CALL CHECK(NF90_ENDDEF(ncID))
        CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(1), mp_f))
        CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(2), ngrmw_f))
        CALL CHECK(NF90_CLOSE(ncID))
        
        ! 更新ngrmw_f和ngrwm_f
        allocate(ngrwm_f(7, num_dbx)); ngrwm_f   = 1 ! 记录相邻三角形编号，初始化为1
        allocate(n_ngrwm_f(num_dbx));  n_ngrwm_f = 0 ! 记录相邻三角形
        do i = 2, num_sjx, 1 ! 三角形总数（含不存在的三角形），这个必须从2开始
            do j = 1, 3, 1
                ngrmw_f(j, i) = vertex_mapping(ngrmw_f(j, i))
                k = ngrmw_f(j, i) ! 获取多边形中心点（或三角形顶点）编号信息
                n_ngrwm_f(k) = n_ngrwm_f(k) + 1 ! 累加顶点个数
                ngrwm_f(n_ngrwm_f(k), k) = i
            end do
        end do
        
        !write(io6, *)  "n_ngrwm_f(93404) = ", n_ngrwm_f(93404)
        !if (n_ngrwm_f(93404) == 0) STOP "ERROR! n_ngrwm_f(i) = 0"

        ! 基于边行走的多边形排序方法，适用于球面凹/凸多边形
        CALL GetSortNew(num_dbx, n_ngrwm_f, ngrmw_f, mp_f, ngrwm_f)

        write(io6, *)  "ngrmw_f和ngrwm_f finish"
        write(io6, *)  ""
        lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc)  // "_" // trim(stepc) // "_ngrwm_f.nc4"
        ! write(io6, *)  lndname
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncID, "lbx_points", num_wp(iter), lpDimID))
        CALL CHECK(NF90_DEF_DIM(ncID, "dim_b", 7, DimbID))
        CALL CHECK(NF90_DEF_VAR(ncID, "ngrwm_f", NF90_INT, (/ DimbID, lpDimID /), ncVarID(1)))
        CALL CHECK(NF90_DEF_VAR(ncID, "n_ngrwm_f", NF90_INT, (/ lpDimID /), ncVarID(2)))
        CALL CHECK(NF90_ENDDEF(ncID))
        CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(1), ngrwm_f))
        CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(2), n_ngrwm_f))
        CALL CHECK(NF90_CLOSE(ncID))

        ! 更新边界点位bdy_refine/bdy_refine_tran并保存
        num_ref = size(bdy_refine)
        do i = 1, num_ref, 1
            bdy_refine(i) = vertex_mapping(bdy_refine(i))
        end do

        num_ref = size(bdy_refine_tran)
        do i = 1, num_ref, 1
            bdy_refine_tran(i) = vertex_mapping(bdy_refine_tran(i))
        end do

        lndname = trim(file_dir) // "tmpfile/gridfile_NXP" // trim(nxpc)  // "_" // trim(stepc) // "_bdy_refine.nc4"
        ! write(io6, *)  lndname
        CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
        CALL CHECK(NF90_DEF_DIM(ncID, "dim_a", size(bdy_refine), DimaID))
        CALL CHECK(NF90_DEF_DIM(ncID, "dim_b", size(bdy_refine_tran), DimbID))
        CALL CHECK(NF90_DEF_VAR(ncID, "bdy_refine", NF90_INT, (/ DimaID /), ncVarID(1)))
        CALL CHECK(NF90_DEF_VAR(ncID, "bdy_refine_tran", NF90_INT, (/ DimbID /), ncVarID(2)))
        CALL CHECK(NF90_ENDDEF(ncID))
        CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(1), bdy_refine))
        CALL CHECK(NF90_PUT_VAR(ncID, ncvarid(2), bdy_refine_tran))
        CALL CHECK(NF90_CLOSE(ncID))

    END SUBROUTINE NGR_RENEW

END module MOD_refine
