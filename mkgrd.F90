!DESCRIPTION
!===========
! This program is the unstructure mesh generation tool for land surface models (e.g.,CoLM).

!ANCILLARY FUNCTIONS AND SUBROUTINES
!-------------------
!* :SUBROUTINE:"colm_CaMa_init" :  Initialization of the coupler


!REVISION HISTORY
!----------------
! 2023.02.21  Zhongwang Wei @ SYSU
! 2021.12.02  Zhongwang Wei @ SYSU 
! 2020.10.01  Zhongwang Wei @ SYSU


!===============================================================================
! OLAM was originally developed at Duke University by Robert Walko, Martin Otte,
! and David Medvigy in the project group headed by Roni Avissar.  Development
! has continued by the same team working at other institutions (University of
! Miami (rwalko@rsmas.miami.edu), the Environmental Protection Agency, and
! Princeton University), with significant contributions from other people.

! Portions of this software are copied or derived from the RAMS software
! package.  The following copyright notice pertains to RAMS and its derivatives,
! including OLAM:  

!----------------------------------------------------------------------------
! Copyright (C) 1991-2006  ; All Rights Reserved ; Colorado State University;
! Colorado State University Research Foundation ; ATMET, LLC

! This software is free software; you can redistribute it and/or modify it
! under the terms of the GNU General Public License as published by the Free
! Software Foundation; either version 2 of the License, or (at your option)
! any later version.

! This software is distributed in the hope that it will be useful, but
! WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
! or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
! for more details.

! You should have received a copy of the GNU General Public License along
! with this program; if not, write to the Free Software Foundation, Inc.,
! 59 Temple Place - Suite 330, Boston, MA 02111-1307, USA
! (http://www.gnu.org/licenses/gpl.html)
!----------------------------------------------------------------------------

!===============================================================================
program main
    use netcdf
    use consts_coms
    use refine_vars
    use MOD_file_preprocess , only : MPAS_Mesh_Read, FVCOM_Mesh_Read
    use MOD_grid_preprocess , only : IAP_Mesh_Make
    use MOD_data_preprocess , only : data_preprocess
    use MOD_Area_judge      , only : Area_judge, Area_judge_refine
    use MOD_GetContain      , only : Get_Contain
    use MOD_GetRef          , only : GetRef
    use MOD_Refine          , only : refine_loop
    use MOD_mask_postproc   , only : mask_postproc

    implicit none
    character(pathlen) :: nlfile = 'mkgrd.mnl'
    character(pathlen) :: finfolist, lndname
    character(5) :: stepc, nxpc
    logical :: exit_loop, fexists
    integer :: i, ncid, dimID_sjx, sjx_points, length
    io6 = 6! If run is sequential, default choice is to set io6 to standard output unit 6.
    
    ! Initialize HDF5 library

    !call h5open_f(hdferr)
    CALL GET_COMMAND_ARGUMENT(1, nlfile)
    ! get nml name from command line. For example: if execute ./mkgrd.x ../mkgrd.nml ! Add comment by Rui Zhang
    ! nlfile = ../mkgrd.nml ! Add comment by Rui Zhang
    ! Read Fortran namelist
    call read_nl(nlfile)
    
    step = 1
    num_vertex = 1
    num_center = 1
    if (mask_restart) then
        call init_consts()
        refine = .false.
        step = max_iter + 1
        if ((mesh_type == 'oceanmesh') .and. (.not. mask_patch_on)) then
            ! 这是针对只调整mask_sea_ratio的情况，如果要补丁的话，那就要从后面的流程
            write(io6, *) "Remask_restart start"
            CALL mask_postproc(mesh_type) 
            write(io6, '(A)') "--------------------------------"
            write(io6, '(A)') ""
            write(io6, '(A)') "!! Successfully Make Grid End !!"
            write(io6, '(A)') ""
            write(io6, '(A)') "--------------------------------"
            stop "Remask_restart finish"
        end if
    end if

    finfolist = trim(file_dir)//'result/namelist.save' !
    CALL execute_command_line('cp '//trim(nlfile)//' '//trim(finfolist)) ! cp ../mkgrd.nml finfolist

    inquire(file = mode_file, exist = fexists)
    if ((mode_grid == 'hex') .or. &
        (mode_grid == 'tri')) then

        call init_consts() ! Const Initialize
        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') step
        lndname = trim(file_dir)// 'gridfile/gridfile_NXP' // trim(nxpc) // '_'// trim(stepc) //'_'//trim(mode_grid)// '.nc4'
        
        if (.not. fexists) then ! 说明mode_file并不存在
            write(io6, *) "mode_file is not exist!"
            write(io6, *) "mode_file_description modify to 'EarthMesh'"
            mode_file_description = 'EarthMesh'
            call gridinit()
        else
            write(io6, *) "read data from mode_file"
            write(io6, *) "mode_file = ", trim(mode_file)! sdfafdsa
            if (trim(mode_file_description) == 'EarthMesh') then
                ! check the file in the mode_file
                CALL CHECK(NF90_OPEN(trim(mode_file), nf90_nowrite, ncid))
                CALL CHECK(NF90_INQ_DIMID(ncid, "sjx_points", dimID_sjx))!
                CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_sjx, len = sjx_points))
                CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
                if (int((sjx_points-1)/20) /= int(nxp*nxp)) then
                    write(io6, *) "WARNNING! nxp read from namelist diff from nxp in the mode_file"
                end if
                CALL execute_command_line('cp '//trim(mode_file)//' '//trim(lndname))

            else if (trim(mode_file_description) == 'MPAS') then
                write(io6, *) "MPAS"
                CALL MPAS_Mesh_Read(mode_file, lndname) 
            else if (trim(mode_file_description) == 'FVCOM') then
                write(io6, *) "FVCOM"
                CALL FVCOM_Mesh_Read(mode_file, lndname)
            else if (trim(mode_file_description) == 'IAP-Ocean') then
                write(io6, *) "IAP-Ocean"
                CALL IAP_Mesh_Make(mode_file, lndname)
            else
                write(io6, *) "Only EarthMesh / MPAS / FVCOM / IAP-Ocean can use now!"
                stop "ERROR!"
            end if
        end if

        ! mesh angle check
        write(io6, *) 'inital grid quality check start'
        CALL Inital_Grid_Quality_Check()
        write(io6, *) 'inital grid quality check finish'
        write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
        write(io6, *) 'grid_write: opening file:', trim(lndname)
        write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
        
    else if ((mode_grid == 'lonlat' ) .or. &
             (mode_grid == 'lambert')) then
        stop "ERROR! can not use now! only tri/hex can choose"
        if (fexists) then
            write(io6, *) 'mode4mesh_make start'
            inquire(file = mode_file, exist = fexists)
            if (.not. fexists) then
                write(*, *) "The input file " // trim(mode_file) // " is missing."
                stop "Stopping model run."
            endif
            CALL mode4mesh_make(mode_file, mode_grid)
            write(io6, *) 'mode4mesh_make complete'
            write(io6, *) ""
        else
            mode_file_description = 'none'
            write(io6, *) "ERROR! mode_file must fexists when mode_grid as ", mode_grid
            stop
        end if

        if (refine) then
            write(io6, *) "turn refine from TRUE to FALSE"
            refine = .false.
        else
            write(io6, *) "refine is FALSE"
        end if

    else if ((mode_grid == 'dbx') .or. &
             (mode_grid == 'cubical')) then
        stop "ERROR! can not use now! only tri/hex can choose"
        ! 未来的设置是dbx用于直接读入数据，只能有nc/nc4结尾的形式
        stop "ERROR! mode_grid == dbx/cubical can not use now!"
        length = len_trim(mode_file)
        if ('nml' == mode_file(length-2:length)) stop 'ERROR! can not use now in the dbx'
        if (refine) then
            write(io6, *) "turn refine from TRUE to FALSE when choose dbx"
            refine = .false.
         end if
    else
        stop 'ERROR mode_grid !!!'
    end if
    write(io6, *) 'mode_grid set as ', mode_grid    

    write(io6, *) 'data preporcess start'
    CALL data_preprocess()! Preset some necessary data such as landtype 
    write(io6, *) 'data preporcess complete'
    write(io6, *) ""

    write(io6, *) 'area judge start'
    ! Multiple boundaries options available in the DmArea(RfArea)
    CALL Area_judge() ! Determination of DmArea(RfArea) / mask-patch-modify
    write(io6, *) 'area judge complete'
    write(io6, *) ""


    if (refine) then
        write(io6, *) 'make grid with refine mesh'
        write(io6, *) ""
 
        max_iter = max(max_iter_cal, max_iter_spc)
        write(io6, *) "max_iter_spc = ", max_iter_spc ! 从namelist中读入
        write(io6, *) "max_iter_cal = ", max_iter_cal ! 从namelist中读入
        write(io6, *) "max_iter = ", max_iter
        if (max_iter <= 0) stop 'Error! max_iter must more than zero'

        do i = 1, max_iter, 1
            if (halo(i) < max_transition_row(i)) then
                write(io6, *) 'i = ', i
                write(io6, *) 'halo(i) = ', halo(i)
                write(io6, *) 'max_transition_row(i) = ', max_transition_row(i)
                stop "ERROR! halo must larger than max_transition_row!"
            end if   
            if (halo(i) <= 0) stop 'Error! halo(i) must more than zero'
            if (max_transition_row(i) <= 0) stop 'Error! max_transition_row(i) must more than zero'
        end do

        write(io6, *) 'start do-while'
        exit_loop = .false.
        do while(step <= max_iter)
            write(io6, *) 'step = ',step, 'in the refine-circle'
            write(io6, *) 'Get ref_sjx start'
            ! only calculate for newly-generated tri or polygon'
            ! 用于阈值细化
            if (refine_setting == 'calculate' .or. refine_setting == 'mixed') then
                if (step <= max_iter_cal) then
                    CALL Area_judge_refine(0)
                    CALL Get_Contain(0)
                    CALL GetRef(0)
                end if
            end if

            ! 用于指定细化
            if (refine_setting == 'specified' .or. refine_setting == 'mixed') then
                if (step <= max_iter_spc) then
                    ! 更换指定细化的地图
                    CALL Area_judge_refine(step)
                    CALL Get_Contain(step)
                    CALL GetRef(step)
                end if
            end if
            write(io6, *) 'Get ref_sjx complete'

            write(io6, *) 'refine_loop start'
            CALL refine_loop(exit_loop) ! 读取ref_sjx进行操作
            write(io6, *) 'refine_loop complete'
            write(io6, *) ""
           
            if (exit_loop) then
                write(io6, *) 'Exiting loop due to ref_sjx equal to zero! &
                          turn refine from to True to False !'
                exit_loop_step(step) = .TRUE.
                exit ! 退出外部的 DO WHILE 循环
            end if
           
            step = step + 1
        end do

        ! calculate for newly-generated tri or polygon
        CALL Final_Grid_Quality_Check() ! 自行判断是否需要全局网格调整
        write(io6, *) 'finish do-while'
    else
        write(io6, *) 'make grid with basic mesh'
    end if

    refine = .false. ! 当不存在需要继续计算的网格后，turn refine from to True to False !'dadadssa
    CALL Get_Contain(0)
    CALL mask_postproc(mesh_type)
    write(io6, '(A)') "--------------------------------"
    write(io6, '(A)') ""
    write(io6, '(A)') "!! Successfully Make Grid End !!"
    write(io6, '(A)') ""
    write(io6, '(A)') "--------------------------------"

end program main

    ! 这是对于初始网格进行全局网格质量检查
    SUBROUTINE Inital_Grid_Quality_Check()
        USE NETCDF
        USE consts_coms , only : pathlen, r8, nxp, step, file_dir, mode_grid, io6
        USE MOD_file_preprocess, only : Unstructured_Mesh_Read
        USE MOD_grid_preprocess, only : Grid_Quality_Check_Global
        IMPLICIT NONE
        integer :: sjx_points, lbx_points
        real(r8),allocatable :: mp(:, :), wp(:, :)
        integer, allocatable :: ngrmw(:, :), ngrwm(:, :), n_ngrwm(:)
        character(pathlen) :: lndname, inputfile
        character(LEN = 5) :: nxpc, stepc

        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') step
        write(io6, *) "start to read unstructure mesh data in the SUBROUTINE Inital_Grid_Quality_Check in Line 288"
        ! read unstructure mesh ! 读取未细化初始网格数据
        lndname = trim(file_dir) // 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_' // trim(mode_grid) // '.nc4'
        write(io6, *) lndname
        CALL Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *) "The unstructured grid data reading have done "
        write(io6, *) ""
        write(io6, *) "In total, triangular mesh number: ", sjx_points, "polygon mesh number: ", lbx_points
        write(io6, *) ""

        write(io6, *) "Grid_Quality_Check_Global start"
        lndname = trim(file_dir) // "result/quality_NXP" // trim(nxpc) // '_' // trim(stepc) // "_global_orial.nc4"
        call Grid_Quality_Check_Global(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *) "Grid_Quality_Check_Global finish"

    END SUBROUTINE Inital_Grid_Quality_Check

    SUBROUTINE Final_Grid_Quality_Check()
        USE NETCDF
        USE consts_coms, only : pathlen, r8, nxp, step, file_dir, mode_grid, io6
        USE refine_vars, only : SpringGlobal_type, SpringRegional_type, HALO
        USE MOD_file_preprocess, only : Unstructured_Mesh_Read, Unstructured_Mesh_Save
        USE MOD_grid_preprocess, only : Springjustment_global, Springjustment_regional_step, Grid_Quality_Check_Global
        IMPLICIT NONE
        integer :: sjx_points, lbx_points, num_edge, set_dis, num_sjx_ref
        real(r8), allocatable :: mp(:, :), wp(:, :)
        integer,  allocatable :: ngrmw(:, :), ngrwm(:, :), n_ngrwm(:)
        character(pathlen) :: lndname, inputfile
        character(LEN = 5) :: nxpc, stepc

        if ((SpringGlobal_type == 0) .and. (SpringRegional_type == 0)) then
            write(io6, *) "no need to springGlobal and springRegional! exit"
            return
        end if

        if (SpringRegional_type == 1) then
            write(io6, *) "SpringRegional in the each step is not here! exit"
            return
        end if

        ! 只有全局调整和在最后的局部调整才需要在这里进行。
        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') step
        write(io6, *) "start to read unstructure mesh data in the SUBROUTINE Final_Grid_Quality_Check"
        ! read unstructure mesh ! 读取未细化初始网格数据
        lndname = trim(file_dir) // 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_' // trim(mode_grid) // '.nc4'
        write(io6, *) lndname
        CALL Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        write(io6, *) "The unstructured grid data reading have done "
        write(io6, *) ""
        write(io6, *) "In total, triangular mesh number: ", sjx_points, "polygon mesh number: ", lbx_points
        write(io6, *) ""

        ! 保存原始无弹性调整的网格
        inputfile = lndname
        lndname = trim(file_dir) // 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_' // trim(mode_grid) // '_orial.nc4'
        CALL execute_command_line('cp '//trim(inputfile)//' '//trim(lndname))

        write(io6, *) "need Springjustment and check Global Grid Quality before and after Springjustment"
        write(io6, *) "Before Springjustment "
        lndname = trim(file_dir) // "result/quality_NXP" // trim(nxpc) // '_' // trim(stepc) // "_global_beforeSpring.nc4"
        write(io6, *) lndname
        CALL Grid_Quality_Check_Global(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)

        num_edge = int((sjx_points-1)/2*3 + 1) ! 只适用于全局网格 
        if (SpringGlobal_type == 1) then
            write(io6, *) "Springjustment_global start"
            CALL Springjustment_global(sjx_points, lbx_points, num_edge, mp, wp, ngrmw, ngrwm, n_ngrwm)
            write(io6, *) "Springjustment_global finish"
        else
            write(io6, *) "Springjustment_regional finish"
            ! CALL Springjustment_regional_final(sjx_points, lbx_points, num_edge, mp, wp, ngrmw, ngrwm, n_ngrwm)
            set_dis = HALO(1)
            num_sjx_ref = 0
            CALL Springjustment_regional_step(set_dis, num_sjx_ref, sjx_points, lbx_points, num_edge, mp, wp, ngrmw, ngrwm, n_ngrwm)
            write(io6, *) "Springjustment_regional finish"
        end if

        write(io6, *) "After Springjustment "
        lndname = trim(file_dir) // "result/quality_NXP" // trim(nxpc) // '_' // trim(stepc) // "_global.nc4"
        write(io6, *) lndname
        CALL Grid_Quality_Check_Global(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)

        write(io6, *) "start to save unstructure mesh data in the SUBROUTINE Final_Grid_Quality_Check"
        ! save unstructure mesh ! 读取未细化初始网格数据
        lndname = trim(file_dir) // 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_' // trim(mode_grid) // '.nc4'
        write(io6, *) lndname
        CALL Unstructured_Mesh_Save(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)

    END SUBROUTINE Final_Grid_Quality_Check

    subroutine mode4mesh_make(inputfile, grid_select)

        use consts_coms, only : r8, pathlen, io6, file_dir, EXPNME, NXP, refine, step
        use netcdf
        use lonlatmesh_coms, only : mesh, lonlatmesh
        use MOD_file_preprocess, only : Mode4_Mesh_Save ! Add by Rui Zhang

        implicit none
        character(pathlen), intent(in) :: inputfile
        character(*), intent(in) :: grid_select
        character(pathlen) :: lndname
        logical :: fexists
        integer :: i, j, idx, length
        integer :: nlon_cal, nlat_cal
        integer :: ncid, dimID_lon, dimID_lat
        integer :: lon_points, lat_points, bound_points, mode_points
        integer, dimension(10) :: varid
        real(r8) :: lon_start, lat_start, lon_end, lat_end, lon_grid_interval, lat_grid_interval
        real(r8), dimension(:),  allocatable :: lon_center, lat_center, lon_bound, lat_bound
        real(r8), dimension(:, :), allocatable :: lon_vert, lat_vert, lonlat_bound
        integer,  dimension(:, :), allocatable :: ngr_bound
        integer,  dimension(:),    allocatable :: n_ngr
        character(5) :: nxpc, stepc, numc
        character(16) :: definition

        ! loop start from here 
        write(io6, *) "file_dir : ", file_dir
        lndname = inputfile
        length = len_trim(lndname)
        if (trim(adjustl(grid_select)) == 'lonlat') then
            if (('.nc' == lndname(length-2:length)) .or. &
                ('nc4' == lndname(length-2:length))) then
                stop 'ERROR! can not use now in the lonlat'

            else if ('nml' == lndname(length-2:length)) then
                ! OPEN THE NAMELIST FILE
                open(10, status = 'OLD', file = lndname)
                ! READ GRID POINT, MODEL OPTIONS, AND PLOTTING INFORMATION FROM THE NAMELIST
                REWIND(10)
                !write(io6, *)nl
                read(10, nml = lonlatmesh)
                close(10)
                write(*, nml = lonlatmesh)

                definition             = mesh%definition
                lon_start              = mesh%lon_start
                lon_end                = mesh%lon_end
                lon_grid_interval      = mesh%lon_grid_interval
                lon_points             = mesh%lon_points
                lat_start              = mesh%lat_start
                lat_end                = mesh%lat_end
                lat_grid_interval      = mesh%lat_grid_interval
                lat_points             = mesh%lat_points
                ! make sure Data self consistency
                nlon_cal = int( (lon_end - lon_start) / lon_grid_interval) + 1
                nlat_cal = int( (lat_end - lat_start) / lat_grid_interval) + 1
                if (nlon_cal /= lon_points) then
                    write(io6, *) "nlon_cal = ", nlon_cal
                    write(io6, *) "lon_points = ", lon_points
                    stop 'ERROR nlon_cal /= lon_points'
                else if (nlat_cal /= lat_points) then
                    write(io6, *) "nlat_cal = ", nlat_cal
                    write(io6, *) "lat_points = ", lat_points
                    stop 'ERROR nlat_cal /= lat_points'
                end if
                
                if (definition == 'center') then
                    allocate(lon_center(lon_points))
                    allocate(lat_center(lat_points))
                    allocate(lon_bound(lon_points + 1))
                    allocate(lat_bound(lat_points + 1))
                    lon_center = lon_start + lon_grid_interval * ([(i, i=1, lon_points)] - 1)
                    lat_center = lat_start + lat_grid_interval * ([(i, i=1, lat_points)] - 1)
                    lon_bound(1 : lon_points) = lon_center - lon_grid_interval / 2.0
                    lon_bound(1 + lon_points) = lon_end    + lon_grid_interval / 2.0
                    lat_bound(1 : lat_points) = lat_center - lat_grid_interval / 2.0
                    lat_bound(1 + lat_points) = lat_end    + lat_grid_interval / 2.0
                else if (definition == 'bound') then
                    allocate(lon_bound(lon_points))
                    allocate(lat_bound(lat_points))
                    ! 因为读入是边界值，网格数量比边界个数少一个
                    lon_points = lon_points - 1
                    lat_points = lat_points - 1
                    allocate(lon_center(lon_points))
                    allocate(lat_center(lat_points))
                    lon_bound(1 : lon_points) = lon_start + lon_grid_interval * ([(i, i=1, lon_points)] - 1)
                    lon_bound(1 + lon_points) = lon_end
                    lat_bound(1 : lat_points) = lat_start + lat_grid_interval * ([(i, i=1, lat_points)] - 1)
                    lat_bound(1 + lat_points) = lat_end
                    lon_center = (lon_bound(1 : lon_points) + lon_bound(2 : 1 + lon_points)) / 2.0
                    lat_center = (lat_bound(1 : lat_points) + lat_bound(2 : 1 + lat_points)) / 2.0
                else
                    stop "error definition mismatch"
                end if
            else
                stop "error datatype must choose 'ncfile' or 'namelist' please check!"
            end if
        
            write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
            write(io6, *) "lonlat bound range"
            write(io6, *) "lon_bound(1) = ", lon_bound(1)
            write(io6, *) "lon_bound(1 + lon_points) = ", lon_bound(1 + lon_points)
            write(io6, *) "lat_bound(1) = ", lat_bound(1)
            write(io6, *) "lat_bound(1 + lat_points) = ", lat_bound(1 + lat_points)
            write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
            write(io6, *) ""
            
            write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
            write(io6, *) "lon_points = ", lon_points
            write(io6, *) "lat_points = ", lat_points
            write(io6, *) "lonlat start end interval"
            write(io6, *) "lon_center start from :", lon_center(1)
            write(io6, *) "lon_center start end  :", lon_center(lon_points)
            write(io6, *) "lon_grid_interval     :", lon_grid_interval
            write(io6, *) "lat_center start from :", lat_center(1)
            write(io6, *) "lat_center start end  :", lat_center(lat_points)
            write(io6, *) "lat_grid_interval     :", lat_grid_interval
            write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
            write(io6, *) ""
            where(lon_center > 180.)  lon_center = lon_center - 360. ! between -180. and 180.
            where(lon_bound  > 180.)  lon_bound  = lon_bound  - 360. ! between -180. and 180.
            NXP = int( abs(360. / lon_grid_interval) / 5 )! compare with NXP in triangle or polygon   

            ! turn lon_bound/lat_bound(1D) to lon_vert.lat_vert(2D)
            write(io6, *) "turn lon_bound/lat_bound(1D) to lon_vert/lat_vert(2D) start"
            allocate(lon_vert(lon_points + 1, lat_points + 1)); lon_vert = 0.
            allocate(lat_vert(lon_points + 1, lat_points + 1)); lat_vert = 0.
            do j = 1, lat_points + 1, 1
                lon_vert(:, j) = lon_bound
            end do 
            do i = 1, lon_points + 1, 1
                lat_vert(i, :) = lat_bound 
            end do
            write(io6, *) "turn lon_bound/lat_bound(1D) to lon_vert/lat_vert(2D) finish"
            write(io6, *) ""

        else if (trim(adjustl(grid_select)) == 'lambert') then
            if (('.nc' == lndname(length-2:length))  .or. &
                ('nc4' == lndname(length-2:length))) then
                CALL CHECK(NF90_OPEN(lndname, nf90_nowrite, ncid))
                CALL CHECK(NF90_INQ_DIMID(ncid, "xi_vert",  dimID_lon))
                CALL CHECK(NF90_INQ_DIMID(ncid, "eta_vert", dimID_lat))
                CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lon, len = lon_points))
                CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lat, len = lat_points))
                allocate(lon_vert(lon_points, lat_points))
                allocate(lat_vert(lon_points, lat_points))
                CALL CHECK(NF90_INQ_VARID(ncid, "lon_vert", varid(1)))
                CALL CHECK(NF90_INQ_VARID(ncid, "lat_vert", varid(2)))
                CALL CHECK(NF90_GET_VAR(ncid, varid(1), lon_vert))
                CALL CHECK(NF90_GET_VAR(ncid, varid(2), lat_vert))
                CALL CHECK(NF90_CLOSE(ncid))
                ! 因为读入是边界值，网格数量比边界个数少一个
                lon_points = lon_points - 1
                lat_points = lat_points - 1
                where(lon_vert > 180.)  lon_vert = lon_vert - 360. ! between -180. and 180.
            else if ('nml' == lndname(length-2:length)) then
                stop 'ERROR! nml can not use in lambert now'
            end if

        else if (trim(adjustl(grid_select)) == 'cubical') then
            stop "error lambert or cubical can not use now!"
        else
            stop "error grid_select must choose 'lonlat' or 'lambert' or 'cubical' please check!"  
        end if

        ! turn 1D to 2D
        write(io6, *) "lonlat_bound and ngr_bound calculate start"
        write(io6, *) "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++"
        bound_points = (lon_points + 1) * (lat_points + 1) + 1
        mode_points = lon_points * lat_points + 1
        allocate(lonlat_bound(bound_points, 2)); lonlat_bound = -999.
        allocate(ngr_bound(4, mode_points)); ngr_bound = 1
        allocate(n_ngr(mode_points)); n_ngr = 4

        idx = 1
        do j = 1, lat_points + 1, 1
            do i = 1, lon_points + 1, 1
                idx = idx + 1
                lonlat_bound(idx, :) = [lon_vert(i, j), lat_vert(i, j)]
            end do
        end do
        
        ! ngr_bound calculate, never start from 1 !
        idx = 1
        do j = 1, lat_points, 1
            do i = 1, lon_points, 1
                idx = idx + 1
                ngr_bound(:, idx) = [i + (j - 1) * (lon_points + 1),     &
                                    i + (j - 1) * (lon_points + 1) + 1, &
                                    i +  j      * (lon_points + 1) + 1, &
                                    i +  j      * (lon_points + 1)]
            end do
        end do
        ngr_bound = ngr_bound + 1 ! never start from 1 !

        write(io6, *) "lon_points : ", lon_points
        write(io6, *) "lat_points : ", lat_points
        write(io6, *) "bound_points : ", bound_points
        write(io6, *) "mode_points  : ", mode_points
        write(io6, *) "minval(lonlat_bound) : ", minval(lonlat_bound(2:bound_points, 1))
        write(io6, *) "maxval(lonlat_bound) : ", maxval(lonlat_bound(2:bound_points, 1))
        write(io6, *) "maxval(lonlat_bound) : ", maxval(lonlat_bound(2:bound_points, 2))
        write(io6, *) "minval(lonlat_bound) : ", minval(lonlat_bound(2:bound_points, 2))
        write(io6, *) "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++"
        write(io6, *) "lonlat_bound and ngr_bound calculate finish"
        write(io6, *) ""

        write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
        write(io6, *) 'grid_write: opening file:', lndname
        write(io6, *) 'mode_points : ', mode_points
        write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
        ! initial file without any refine
        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') step
        lndname = trim(file_dir)// 'gridfile/gridfile_NXP' // trim(nxpc) // '_'// trim(stepc) //'_'//trim(grid_select)//'.nc4'
        write(io6, *) lndname
        CALL Mode4_Mesh_Save(lndname, bound_points, mode_points, lonlat_bound, ngr_bound, n_ngr)
        write(io6, *) "mode4mesh_make finish"

    end subroutine mode4mesh_make

    ! 需要把refine的文件编号全部弄好来
    subroutine read_nl(nlfile)
        use consts_coms
        use refine_vars
        implicit none

        character(*), intent(in) :: nlfile
        integer :: i, pos, iostat
        logical :: fexists
        character(pathlen) :: path, fprefix, filename, lndname

        namelist /mkgrd/ nl
        namelist /mkrefine/ rl
        ! OPEN THE NAMELIST FILE
        inquire(file = nlfile, exist = fexists)
        write(io6, *) nlfile
        if (.not. fexists) then
            write(*, *) "The namelist file " // trim(nlfile) // " is missing."
            stop "Stopping model run."
        endif
        open(iunit, status = 'OLD', file = nlfile)
        ! READ GRID POINT, MODEL OPTIONS, AND PLOTTING INFORMATION FROM THE NAMELIST
        REWIND(iunit)
        !write(io6, *)nl
        read(iunit, nml = mkgrd)
        close(iunit)
        write(*, nml = mkgrd)
        write(io6, *) ""

        !----------------------------------------------------------
        ! Variables in the following section either must not be changed on a history
        ! restart or changing them would be irrelevant.  Thus, they are only copied
        ! from the namelist if a history file is not being read.
        expnme               = nl%expnme
        nxp                  = nl%nxp
        base_dir             = nl%base_dir
        landtype_file        = nl%landtype_file
        mesh_type            = nl%mesh_type
        mode_grid            = nl%mode_grid
        mode_file            = nl%mode_file
        mode_file_description= nl%mode_file_description
        refine               = nl%refine
        gridnum_perdegree    = nl%gridnum_perdegree
        niter                = nl%niter
        beta                 = nl%beta
        relax                = nl%relax
        openmp               = nl%openmp
        mask_sea_ratio       = nl%mask_sea_ratio
        mask_restart         = nl%mask_restart
        mask_domain_global   = nl%mask_domain_global
        mask_domain_type     = nl%mask_domain_type
        mask_domain_fprefix  = nl%mask_domain_fprefix
        mask_patch_on        = nl%mask_patch_on
        mask_patch_type      = nl%mask_patch_type
        mask_patch_fprefix   = nl%mask_patch_fprefix
        output_format        = nl%output_format
        file_dir             = trim(base_dir) // trim(expnme) // '/'
        
        write(io6, *) "gridnum_perdegree = ", gridnum_perdegree
        if (gridnum_perdegree == 240) then
            write(io6, *) "landtype_igbp"
        else if (gridnum_perdegree == 120) then
            write(io6, *) "landtype_usgs"
        else
            STOP "ERROR! gridnum_perdegree must 120 or 240 now!"
        end if

        if (mesh_type == 'landmesh') then
            if (output_format /= 'CoLM') then
                write(6, *) "ERROR! output_format should be CoLM when mesh_type is landmesh"
                STOP
            end if
        else if (mesh_type == 'oceanmesh') then
            if (output_format /= 'FVCOM') then
                write(6, *) "WARNING! output_format should be FVCOM when mesh_type is oceanmesh"
                write(6, *) "output_format = ", output_format
                STOP
            end if
        else if (mesh_type == 'atmosmesh') then
            if ((output_format /= 'MPAS') .and. (output_format /= 'MPAS-Simple')) then
                write(6, *) "WARNING! output_format should be MPAS/MPAS-Simple when mesh_type is atmosmesh"
                write(6, *) "output_format = ", output_format
                stop
            end if
        else if (mesh_type == 'LOCmesh') then
            if (output_format /= 'CoLM') then
                write(6, *) "WARNING! output_format should be CoLM when mesh_type is LOCmesh"
                stop
            end if
        else
            STOP "ERROR! mesh_type mush be landmesh/oceanmesh/atmosmesh/LOCmesh"
        end if

        if (mask_restart) then
            if (mask_patch_on) CALL Mask_make('mask_patch', mask_patch_type, mask_patch_fprefix)
            return
        end if

        CALL execute_command_line('rm -rf '//trim(file_dir)) ! rm old filedir
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"contain/") ! use for step
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"gridfile/") ! use for step
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"patchtype/")
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"result/") ! final mesh file
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"tmpfile/")
        CALL execute_command_line('rm *_filelist.txt')
        ! mask_domain
        if (.not. mask_domain_global) CALL Mask_make('mask_domain', mask_domain_type, mask_domain_fprefix)
        ! mask_patch
        if (mask_patch_on) CALL Mask_make('mask_patch', mask_patch_type, mask_patch_fprefix)

        if (refine) then
            CALL execute_command_line('mkdir -p '//trim(file_dir)//"threshold/")
            open(iunit, status = 'OLD', file = nlfile)
            REWIND(iunit)
            read(iunit, nml = mkrefine)
            close(iunit)
            write(*, nml = mkrefine)
            weak_concav_eliminate = rl%weak_concav_eliminate
            Istransition          = rl%Istransition
            iterD                 = rl%iterD
            halo                  = rl%halo
            max_transition_row    = rl%max_transition_row
            SpringGlobal_type     = rl%SpringGlobal_type
            num_rc                = rl%num_rc
            set_dis_type          = rl%set_dis_type
            SpringRegional_type   = rl%SpringRegional_type
            vertex_pretect_layers    = rl%vertex_pretect_layers
            niter_refine          = rl%niter_refine

            if (Istransition .eqv. .false.) then
                if (mode_grid /= 'tri') STOP "ERROR! not Istransition can only use in the tri"
                SpringGlobal_type = 0
                SpringRegional_type = 0
                write(io6, *) "Istransition = .false. SpringGlobal_type/SpringRegional_type modify to zero !"
            else
                if ((SpringGlobal_type < 0) .or. (SpringGlobal_type > 1)) STOP "ERROR! SpringGlobal_type must 0,1"
                if ((SpringRegional_type < 0) .or. (SpringRegional_type > 2)) STOP "ERROR! SpringRegional_type must 0,1,2"
                
                if ((SpringGlobal_type > 0) .and. (SpringRegional_type > 0)) then
                    write(io6, *) "SpringGlobal_type = ", SpringGlobal_type
                    write(io6, *) "SpringRegional_type = ", SpringRegional_type
                    STOP "ERROR! only one of (SpringGlobal_type and SpringRegional_type) can larger than zero"
                end if

                if (SpringGlobal_type > 0) then
                    if (niter_refine < 1000) then
                        write(io6, *) "when SpringGlobal_type > 0, niter_refine = ", niter_refine
                        write(io6, *) "WARNING! The number of iterations is relatively small, it can be increased to over 1000"
                    end if
                end if

                if (SpringRegional_type > 0) then
                    if (niter_refine > 100) then
                        write(io6, *) "when SpringRegional_type > 0, niter_refine = ", niter_refine
                        write(io6, *) "WARNING! The number of iterations is relatively larger, it can be reduced to below 100"
                    end if
                end if
            end if

            if (SpringGlobal_type > 0)  vertex_pretect_layers = 0
            if (vertex_pretect_layers < 0) STOP "ERROR! vertex_pretect_layers must >= 0"
        
            refine_spc            = rl%refine_spc
            refine_cal            = rl%refine_cal
            if (refine_spc) max_iter_spc          = rl%max_iter_spc ! 默认为0，开关打开才读取
            if (refine_cal) max_iter_cal          = rl%max_iter_cal ! 默认为0，开关打开才读取
            if (refine_cal) then
            if (mesh_type == 'atmosmesh') STOP "ERROR! atmosmesh can not use in refine_cal"
            end if

            if ((refine_spc .eqv. .TRUE.) .and. (refine_cal .eqv. .TRUE.)) then
                refine_setting = 'mixed'
            else if ((refine_spc .eqv. .TRUE.)  .and. (refine_cal .eqv. .FALSE.)) then
                refine_setting = 'specified'
            else if ((refine_spc .eqv. .FALSE.) .and. (refine_cal .eqv. .TRUE.)) then
                refine_setting = 'calculate'
            else
                stop "ERROR! MUst one of TRUE in the refine_spc and refine_cal when refine is TRUE"
            end if
            write(io6, *) "refine_setting = ", refine_setting
            
            ! 指定细化
            if (refine_setting == 'specified' .or. refine_setting == 'mixed') then
                mask_refine_spc_type       = RL%mask_refine_spc_type
                mask_refine_spc_fprefix    = RL%mask_refine_spc_fprefix
                CALL Mask_make('mask_refine', mask_refine_spc_type, mask_refine_spc_fprefix) 
                if (mask_refine_ndm(max_iter_spc) == 0) then
                    write(io6, *) "max_iter_spc = ", max_iter_spc
                    write(io6, *) "mask_refine_ndm(max_iter_spc) = ", mask_refine_ndm(max_iter_spc)
                    stop "ERROR! mask_refine_ndm(max_iter_spc) must larger then one, please modify max_iter_spc"
                end if
            end if

            ! 阈值细化
            if (refine_setting == 'calculate' .or. refine_setting == 'mixed') then
                threshold_dir              = RL%threshold_dir 

                if ((mesh_type == 'landmesh') .or. (mesh_type == 'LOCmesh')) then
                    refine_num_landtypes      = rl%refine_num_landtypes
                    refine_area_mainland      = rl%refine_area_mainland
                    refine_onelayer_Lnd( 1)   = rl%refine_lai_m
                    refine_onelayer_Lnd( 2)   = rl%refine_lai_s
                    refine_onelayer_Lnd( 3)   = rl%refine_slope_m
                    refine_onelayer_Lnd( 4)   = rl%refine_slope_s
                    refine_twolayer_Lnd( 1)   = rl%refine_k_s_m
                    refine_twolayer_Lnd( 2)   = rl%refine_k_s_s
                    refine_twolayer_Lnd( 3)   = rl%refine_k_solids_m
                    refine_twolayer_Lnd( 4)   = rl%refine_k_solids_s
                    refine_twolayer_Lnd( 5)   = rl%refine_tkdry_m
                    refine_twolayer_Lnd( 6)   = rl%refine_tkdry_s
                    refine_twolayer_Lnd( 7)   = rl%refine_tksatf_m
                    refine_twolayer_Lnd( 8)   = rl%refine_tksatf_s
                    refine_twolayer_Lnd( 9)   = rl%refine_tksatu_m
                    refine_twolayer_Lnd(10)   = rl%refine_tksatu_s

                    th_num_landtypes          = rl%th_num_landtypes
                    th_area_mainland          = rl%th_area_mainland
                    th_onelayer_Lnd( 1)       = rl%th_lai_m
                    th_onelayer_Lnd( 2)       = rl%th_lai_s
                    th_onelayer_Lnd( 3)       = rl%th_slope_m
                    th_onelayer_Lnd( 4)       = rl%th_slope_s
                    th_twolayer_Lnd( 1, 1:2)  = rl%th_k_s_m
                    th_twolayer_Lnd( 2, 1:2)  = rl%th_k_s_s
                    th_twolayer_Lnd( 3, 1:2)  = rl%th_k_solids_m
                    th_twolayer_Lnd( 4, 1:2)  = rl%th_k_solids_s
                    th_twolayer_Lnd( 5, 1:2)  = rl%th_tkdry_m
                    th_twolayer_Lnd( 6, 1:2)  = rl%th_tkdry_s
                    th_twolayer_Lnd( 7, 1:2)  = rl%th_tksatf_m
                    th_twolayer_Lnd( 8, 1:2)  = rl%th_tksatf_s
                    th_twolayer_Lnd( 9, 1:2)  = rl%th_tksatu_m
                    th_twolayer_Lnd(10, 1:2)  = rl%th_tksatu_s
                end if

                if ((mesh_type == 'oceanmesh') .or. (mesh_type == 'LOCmesh')) then
                    refine_sea_ratio          = rl%refine_sea_ratio
                    refine_onelayer_Ocn(1)    = rl%refine_sst_m
                    refine_onelayer_Ocn(2)    = rl%refine_sst_s
                    refine_onelayer_Ocn(3)    = rl%refine_ssh_m
                    refine_onelayer_Ocn(4)    = rl%refine_ssh_s
                    refine_onelayer_Ocn(5)    = rl%refine_eke_m
                    refine_onelayer_Ocn(6)    = rl%refine_eke_s
                    refine_onelayer_Ocn(7)    = rl%refine_sea_slope_m
                    refine_onelayer_Ocn(8)    = rl%refine_sea_slope_s

                    th_sea_ratio              = rl%th_sea_ratio
                    th_onelayer_Ocn(1)        = rl%th_sst_m
                    th_onelayer_Ocn(2)        = rl%th_sst_s
                    th_onelayer_Ocn(3)        = rl%th_ssh_m
                    th_onelayer_Ocn(4)        = rl%th_ssh_s
                    th_onelayer_Ocn(5)        = rl%th_eke_m
                    th_onelayer_Ocn(6)        = rl%th_eke_s
                    th_onelayer_Ocn(7)        = rl%th_sea_slope_m
                    th_onelayer_Ocn(8)        = rl%th_sea_slope_s
                end if

                if ((mesh_type == 'atmosmesh') .or. (mesh_type == 'LOCmesh')) then
                    refine_onelayer_Atmos( 1) = rl%refine_typhoon_m
                    refine_onelayer_Atmos( 2) = rl%refine_typhoon_s
                    th_onelayer_Atmos( 1)     = rl%th_typhoon_m
                    th_onelayer_Atmos( 2)     = rl%th_typhoon_s
                end if

                ! 开启阈值细化就一定要开启阈值具体的阈值细化开关
                if (refine_setting == 'calculate' .or. refine_setting == 'mixed') then
                    if (mesh_type == 'landmesh') then
                        if ((refine_num_landtypes .eqv. .false.) .and. &
                            (refine_area_mainland .eqv. .false.) .and. &
                            (all(refine_onelayer_Lnd  .eqv. .false.)).and. &
                            (all(refine_twolayer_Lnd  .eqv. .false.))) then
                            stop "Error! MUst one of TRUE in the refine_num_landtypes or &
                                    refine_area_mainland or refine_onelayer_Lnd or refine_twolayer_Lnd &
                                    when refine is TRUE and meshtype = landmesh"
                        end if

                    else if (mesh_type == 'oceanmesh') then
                        if ((refine_sea_ratio .eqv. .false.) .and. &
                            (all(refine_onelayer_Ocn .eqv. .false.))) then
                            stop "ERROR! MUst one of TRUE in the refine_sea_ratio or refine_onelayer_Ocn when refine is TRUE and meshtype = oceanmesh"
                        end if

                    else if (mesh_type == 'atmosmesh') then
                        if (all(refine_onelayer_Atmos .eqv. .false.)) then
                            stop "ERROR! MUst one of TRUE in the refine_onelayer_Atmos when refine is TRUE and meshtype = atmosmesh"
                        end if
                        
                    else if (mesh_type == 'LOCmesh') then
                        if ((refine_num_landtypes .eqv. .false.) .and. &
                            (refine_area_mainland .eqv. .false.) .and. &
                            (refine_sea_ratio     .eqv. .false.) .and. &
                            (all(refine_onelayer_Lnd  .eqv. .false.)) .and. &
                            (all(refine_twolayer_Lnd  .eqv. .false.)) .and. &
                            (all(refine_onelayer_Ocn  .eqv. .false.)) .and. &
                            (all(refine_onelayer_Atmos .eqv. .false.))) then
                            write(io6, *) "refine_num_landtypes = ", refine_num_landtypes
                            write(io6, *) "refine_area_mainland = ", refine_area_mainland
                            write(io6, *) "refine_sea_ratio = ", refine_sea_ratio
                            stop "Error! MUst one of TRUE in the refine_sea_ratio  or &
                                    refine_num_landtypes or &
                                    refine_area_mainland or refine_onelayer_Lnd or refine_twolayer_Lnd or &
                                    refine_onelayer_Ocn or refine_onelayer_Atmos &
                                    when refine is TRUE and meshtype = LOCmesh"
                        end if
                    end if
                end if
                mask_refine_cal_type       = RL%mask_refine_cal_type
                mask_refine_cal_fprefix    = RL%mask_refine_cal_fprefix
                CALL Mask_make('mask_refine', mask_refine_cal_type, mask_refine_cal_fprefix)
            end if
            
            ! onelayer_Lnd
            if ((refine_onelayer_Lnd( 1) .eqv. .true.) .and. (th_onelayer_Lnd( 1) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Lnd( 1) and   th_onelayer_Lnd( 1) "
            if ((refine_onelayer_Lnd( 2) .eqv. .true.) .and. (th_onelayer_Lnd( 2) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Lnd( 2) and   th_onelayer_Lnd( 2) "
            if ((refine_onelayer_Lnd( 3) .eqv. .true.) .and. (th_onelayer_Lnd( 3) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Lnd( 3) and   th_onelayer_Lnd( 3) "
            if ((refine_onelayer_Lnd( 4) .eqv. .true.) .and. (th_onelayer_Lnd( 4) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Lnd( 4) and   th_onelayer_Lnd( 4) "

            ! twolayer_Lnd
            if ((refine_twolayer_Lnd( 1) .eqv. .true.) .and. any(th_twolayer_Lnd( 1, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 1)  and      th_twolayer_Lnd( 1, 1:2) "
            if ((refine_twolayer_Lnd( 2) .eqv. .true.) .and. any(th_twolayer_Lnd( 2, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 2)  and      th_twolayer_Lnd( 2, 1:2) "
            if ((refine_twolayer_Lnd( 3) .eqv. .true.) .and. any(th_twolayer_Lnd( 3, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 3)  and      th_twolayer_Lnd( 3, 1:2) "
            if ((refine_twolayer_Lnd( 4) .eqv. .true.) .and. any(th_twolayer_Lnd( 4, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 4)  and      th_twolayer_Lnd( 4, 1:2) "
            if ((refine_twolayer_Lnd( 5) .eqv. .true.) .and. any(th_twolayer_Lnd( 5, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 5)  and      th_twolayer_Lnd( 5, 1:2) "
            if ((refine_twolayer_Lnd( 6) .eqv. .true.) .and. any(th_twolayer_Lnd( 6, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 6)  and      th_twolayer_Lnd( 6, 1:2) "
            if ((refine_twolayer_Lnd( 7) .eqv. .true.) .and. any(th_twolayer_Lnd( 7, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 7)  and      th_twolayer_Lnd( 7, 1:2) "
            if ((refine_twolayer_Lnd( 8) .eqv. .true.) .and. any(th_twolayer_Lnd( 8, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 8)  and      th_twolayer_Lnd( 8, 1:2) "
            if ((refine_twolayer_Lnd( 9) .eqv. .true.) .and. any(th_twolayer_Lnd( 9, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd( 9)  and      th_twolayer_Lnd( 9, 1:2) "
            if ((refine_twolayer_Lnd(10) .eqv. .true.) .and. any(th_twolayer_Lnd(10, 1:2) == 999.))  stop "stop for &
            mismatch between refine_twolayer_Lnd(10)  and      th_twolayer_Lnd(10, 1:2) "

            ! onelayer_Ocn
            if ((refine_onelayer_Ocn( 1) .eqv. .true.) .and. (th_onelayer_Ocn( 1) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Ocn( 1) and   th_onelayer_Ocn( 1) "
            if ((refine_onelayer_Ocn( 2) .eqv. .true.) .and. (th_onelayer_Ocn( 2) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Ocn( 2) and   th_onelayer_Ocn( 2) "
            if ((refine_onelayer_Ocn( 3) .eqv. .true.) .and. (th_onelayer_Ocn( 3) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Ocn( 3) and   th_onelayer_Ocn( 3) "
            if ((refine_onelayer_Ocn( 4) .eqv. .true.) .and. (th_onelayer_Ocn( 4) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Ocn( 4) and   th_onelayer_Ocn( 4) "
            if ((refine_onelayer_Ocn( 5) .eqv. .true.) .and. (th_onelayer_Ocn( 5) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Ocn( 5) and   th_onelayer_Ocn( 5) "
            if ((refine_onelayer_Ocn( 6) .eqv. .true.) .and. (th_onelayer_Ocn( 6) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Ocn( 6) and   th_onelayer_Ocn( 6) "
            if ((refine_onelayer_Ocn( 7) .eqv. .true.) .and. (th_onelayer_Ocn( 7) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Ocn( 7) and   th_onelayer_Ocn( 7) "
            if ((refine_onelayer_Ocn( 8) .eqv. .true.) .and. (th_onelayer_Ocn( 8) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Ocn( 8) and   th_onelayer_Ocn( 8) "

            ! onelayer_Atmos
            if ((refine_onelayer_Atmos( 1) .eqv. .true.) .and. (th_onelayer_Atmos( 1) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Atmos( 1) and   th_onelayer_Atmos( 1) "
            if ((refine_onelayer_Atmos( 2) .eqv. .true.) .and. (th_onelayer_Atmos( 2) == 999.) ) stop "stop for &
                mismatch between refine_onelayer_Atmos( 2) and   th_onelayer_Atmos( 2) "

        end if

    end subroutine read_nl

    SUBROUTINE Mask_make(mask_select, type_select, mask_fprefix)
        ! 将domain/refine/patch的mask前处理共用部分放在一起
        ! 但是目前还不支持nc文件的读写
        use consts_coms
        use refine_vars
        use netcdf
        implicit none
        character(*), intent(in) :: mask_select, type_select, mask_fprefix
        integer :: pos, i, iostat
        logical :: fexists
        character(pathlen) :: path, fprefix, filename, lndname, command
        
        pos = 0
        do i = len_trim(mask_fprefix), 1, -1
            if (mask_fprefix(i:i) == '/') then
                pos = i
                exit
            end if
        end do
        if (pos > 0) then
            path = mask_fprefix(1:pos)   ! 提取路径部分
            fprefix = mask_fprefix(pos+1:)  ! 提取文件名部分
            write(io6, *) 'path: ',    trim(adjustl(path))
            write(io6, *) 'fprefix: ', trim(adjustl(fprefix))
            fexists = .false.

            ! 使用系统命令列出目录中的所有文件
            lndname = trim(mask_select) // '_filelist.txt'
            command = 'ls ' // trim(mask_fprefix) // '* > ' // trim(lndname)
            call execute_command_line(command)

            ! 打开临时文件读取文件列表
            open(unit=111, file=lndname, status='old')
            REWIND(111)
            ! 遍历文件列表
            do while (.true.)
                read(111,'(A)', iostat=iostat) filename
                write(io6, *) "iostat = ", iostat, "mask_select = ", mask_select
                if (iostat < 0) exit
                if (iostat > 0) then
                    write(io6, *) "文件读取错误 in the Mask_make in the mkgrd.F90"
                    stop
                end if
                ! 如果文件名中包含特定的字符串，设置found为.true.
                if (index(trim(adjustl(filename)), trim(adjustl(fprefix)) ) > 0) then
                    write(io6, *) "filename : ", trim(adjustl(filename))
                    fexists = .true.
                    if (type_select == 'bbox') then
                        CALL bbox_mask_make(filename, mask_select)
                    else if (type_select == 'lambert') then
                        CALL lamb_mask_make(filename, mask_select)
                    else if (type_select == 'circle') then
                        CALL circle_mask_make(filename, mask_select) 
                    else if (type_select == 'close') then
                        CALL close_mask_make(filename, mask_select)
                    else
                        write(io6, *) "ERROR! ", type_select, " must be bbox, lambert, circle, close"
                        stop
                    end if
                end if
            end do

            ! 关闭文件
            close(111)

            ! 输出是否找到匹配的文件
            if (.not. fexists) stop '未找到匹配的文件 路径格式错误 in the mask_refine_fprefix'
        else
            write(io6, *) "路径格式错误 in the mask_fprefix", trim(mask_fprefix)
            stop
        end if

    END SUBROUTINE Mask_make

    subroutine bbox_mask_make(inputfile, mask_select)
        ! 文件类型然后转为nc文件便于读取，如果是nc就直接赋值
        use netcdf
        use consts_coms, only : r8, pathlen, file_dir, mask_domain_ndm, mask_patch_ndm, io6
        use refine_vars, only : mask_refine_ndm, max_iter_spc
        use MOD_file_preprocess, only : bbox_Mesh_Save ! Add by Rui Zhang
        implicit none
        character(pathlen), intent(in) :: inputfile
        character(*), intent(in) :: mask_select
        integer :: ncid, varid
        character(pathlen) :: lndname, line
        logical :: fexists
        integer :: i, bbox_num, refine_degree, length, io_stat
        real(r8), allocatable :: bbox_points(:,:)
        character(5) :: numc, refinec

        length = len_trim(inputfile)
        if ('nml' == inputfile(length-2:length)) then
            ! nml：第一个行说明点的个数
            open(unit=10, file=inputfile, status='old', action='read')
            REWIND(10)
            ! * 是自由格式，Fortran 会跳过所有非数字字符，直到找到数字部分
            read(10, '(A)', iostat=io_stat) line  ! 读取整行
            if (io_stat /= 0) then
                write(io6, *) "Error: Failed to read bbox_num"
                stop
            end if
            read(line(index(line, '=')+1:), *) bbox_num  ! 解析 bbox_num

            read(10, '(A)', iostat=io_stat) line  ! 读取整行
            if (io_stat /= 0) then
                write(io6, *) "Error: Failed to read circle_refine"
                stop
            end if
            read(line(index(line, '=')+1:), *) refine_degree  ! 解析 bbox_refine
            if (refine_degree > max_iter_spc) then
                write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
                return ! 返回，不进行后续操作
                close(10)
            end if

            write(io6, *) "bbox_points must be 1.West 2.East 3.North and 4.South"
            allocate(bbox_points(bbox_num, 4))
            do i = 1, bbox_num, 1
                ! must make sure bbox_points in the range from -180. to 180. and -90. to 90.
                read(10, *) bbox_points(i, 1), bbox_points(i, 2), bbox_points(i, 3), bbox_points(i, 4)
                if (bbox_points(i, 1) > bbox_points(i, 2)) stop "ERROR! bbox_points(i, 1) > bbox_points(i, 2)"
                if (bbox_points(i, 3) < bbox_points(i, 4)) stop "ERROR! bbox_points(i, 3) < bbox_points(i, 4)"
            end do
            close(10)

            write(refinec, '(I1)') refine_degree
            if (mask_select == 'mask_domain') then
                mask_domain_ndm = mask_domain_ndm + 1
                write(numc, '(I2.2)') mask_domain_ndm
            else if (mask_select == 'mask_refine') then
                mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
            else if (mask_select == 'mask_patch') then
                mask_patch_ndm(refine_degree) = mask_patch_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_patch_ndm(refine_degree)
            end if

            lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_bbox_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL bbox_Mesh_Save(lndname, bbox_num, bbox_points)
            deallocate(bbox_points)

        else if (('.nc' == inputfile(length-2:length)) .or. &
                ('nc4' == inputfile(length-2:length))) then
            CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件
            CALL CHECK(NF90_INQ_VARID(ncid, 'bbox_refine', varid))
            CALL CHECK(NF90_GET_VAR(ncid, varid, refine_degree))
            CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_bbox关闭文件
            if (refine_degree > max_iter_spc) then
                write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
                return ! 返回，不进行后续操作
            end if

            write(refinec, '(I1)') refine_degree
            if (mask_select == 'mask_domain') then
                mask_domain_ndm = mask_domain_ndm + 1
                write(numc, '(I2.2)') mask_domain_ndm
            else if (mask_select == 'mask_refine') then
                mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
            else if (mask_select == 'mask_patch') then
                mask_patch_ndm(refine_degree) = mask_patch_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_patch_ndm(refine_degree)
            end if

            lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_bbox_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL execute_command_line('cp '//trim(inputfile)//' '//trim(lndname))
        else
            stop "ERROR! must choose 'ncfile' or 'nml' please check!"
        end if 
        write(io6, *) lndname

    end subroutine bbox_mask_make

    ! 目前只可以用于nc文件，而且只能用于阈值计算中
    subroutine lamb_mask_make(inputfile, mask_select)
        use netcdf
        use consts_coms, only : r8, pathlen, file_dir, mask_domain_ndm, mask_patch_ndm, io6
        use refine_vars, only : mask_refine_ndm, max_iter_spc
        use MOD_file_preprocess, only : Mode4_Mesh_Save ! Add by Rui Zhang

        implicit none
        character(pathlen), intent(in) :: inputfile
        character(*), intent(in) :: mask_select
        character(pathlen) :: lndname
        logical :: fexists
        integer :: i, j, idx, length
        integer :: nlon_cal, nlat_cal
        integer :: ncid, dimID_lon, dimID_lat
        integer :: lon_points, lat_points, bound_points, mode_points, refine_degree
        integer, dimension(10) :: varid
        real(r8), dimension(:, :), allocatable :: lon_vert, lat_vert, lonlat_bound
        integer,  dimension(:, :), allocatable :: ngr_bound
        integer,  dimension(:),    allocatable :: n_ngr
        character(5) :: nxpc, stepc, numc, refinec

        length = len_trim(inputfile)
        if ('nml' == inputfile(length-2:length)) then
            stop 'ERROR! nml can not use in lambert now'
        else if (('.nc' == inputfile(length-2:length))  .or. &
                ('nc4' == inputfile(length-2:length))) then
            CALL CHECK(NF90_OPEN(inputfile, nf90_nowrite, ncid))
            ! CALL CHECK(NF90_INQ_VARID(ncid, 'lamb_refine', varid))
            ! CALL CHECK(NF90_GET_VAR(ncid, varid, refine_degree))
            ! if (refine_degree > max_iter_spc) then
            !     write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
            !     CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_bbox关闭文件
            !     return ! 返回，不进行后续操作
            ! end if
            CALL CHECK(NF90_INQ_DIMID(ncid, "xi_vert",  dimID_lon))
            CALL CHECK(NF90_INQ_DIMID(ncid, "eta_vert", dimID_lat))
            CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lon, len = lon_points))
            CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lat, len = lat_points))
            allocate(lon_vert(lon_points, lat_points))
            allocate(lat_vert(lon_points, lat_points))
            CALL CHECK(NF90_INQ_VARID(ncid, "lon_vert", varid(1)))
            CALL CHECK(NF90_INQ_VARID(ncid, "lat_vert", varid(2)))
            CALL CHECK(NF90_GET_VAR(ncid, varid(1), lon_vert))
            CALL CHECK(NF90_GET_VAR(ncid, varid(2), lat_vert))
            CALL CHECK(NF90_CLOSE(ncid))
            ! 因为读入是边界值，网格数量比边界个数少一个
            lon_points = lon_points - 1
            lat_points = lat_points - 1
            where(lon_vert > 180.)  lon_vert = lon_vert - 360. ! between -180. and 180.
        else
            stop "ERROR! must choose 'ncfile' or 'nml' please check!"
        end if

        ! turn 1D to 2D
        write(io6, *) "lonlat_bound and ngr_bound calculate start"
        write(io6, *) "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++"
        bound_points = (lon_points + 1) * (lat_points + 1) + 1
        mode_points = lon_points * lat_points + 1
        allocate(lonlat_bound(bound_points, 2)); lonlat_bound = -999.
        allocate(ngr_bound(4, mode_points)); ngr_bound = 1
        allocate(n_ngr(mode_points)); n_ngr = 4

        idx = 1
        do j = 1, lat_points + 1, 1
            do i = 1, lon_points + 1, 1
                idx = idx + 1
                lonlat_bound(idx, :) = [lon_vert(i, j), lat_vert(i, j)]
            end do
        end do
        
        ! ngr_bound calculate, never start from 1 !
        idx = 1
        do j = 1, lat_points, 1
            do i = 1, lon_points, 1
                idx = idx + 1
                ngr_bound(:, idx) = [i + (j - 1) * (lon_points + 1),     &
                                    i + (j - 1) * (lon_points + 1) + 1, &
                                    i +  j      * (lon_points + 1) + 1, &
                                    i +  j      * (lon_points + 1)]
            end do
        end do
        ngr_bound = ngr_bound + 1 ! never start from 1 !

        write(io6, *) "lon_points : ", lon_points
        write(io6, *) "lat_points : ", lat_points
        write(io6, *) "bound_points : ", bound_points
        write(io6, *) "mode_points  : ", mode_points
        write(io6, *) "minval(lonlat_bound) : ", minval(lonlat_bound(2:bound_points, 1))
        write(io6, *) "maxval(lonlat_bound) : ", maxval(lonlat_bound(2:bound_points, 1))
        write(io6, *) "maxval(lonlat_bound) : ", maxval(lonlat_bound(2:bound_points, 2))
        write(io6, *) "minval(lonlat_bound) : ", minval(lonlat_bound(2:bound_points, 2))
        write(io6, *) "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++"
        write(io6, *) "lonlat_bound and ngr_bound calculate finish"
        write(io6, *) ""

        refine_degree = 0
        write(refinec, '(I1)') refine_degree
        if (mask_select == 'mask_domain') then
            mask_domain_ndm = mask_domain_ndm + 1
            write(numc, '(I2.2)') mask_domain_ndm
        else if (mask_select == 'mask_refine') then
            mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
            write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
        else if (mask_select == 'mask_patch') then
            mask_patch_ndm(refine_degree) = mask_patch_ndm(refine_degree) + 1
            write(numc, '(I2.2)') mask_patch_ndm(refine_degree)
        end if
        lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_lambert_'//trim(refinec)//'_'//trim(numc)//'.nc4'
        write(io6, *) lndname 
        CALL Mode4_Mesh_Save(lndname, bound_points, mode_points, lonlat_bound, ngr_bound, n_ngr)
        write(io6, *) "lamb_mask_make finish"

    end subroutine lamb_mask_make

    subroutine circle_mask_make(inputfile, mask_select)
        ! 文件类型然后转为nc文件便于读取，，如果是nc就直接赋值
        use netcdf
        use consts_coms, only : r8, pathlen, file_dir, mask_domain_ndm, mask_patch_ndm, io6
        use refine_vars, only : mask_refine_ndm, max_iter_spc
        use MOD_file_preprocess, only : circle_Mesh_Save ! Add by Rui Zhang
        implicit none
        character(pathlen), intent(in) :: inputfile
        character(*), intent(in) :: mask_select
        integer :: ncid, varid
        character(pathlen) :: lndname, line
        logical :: fexists
        integer :: i, circle_num, refine_degree, length, io_stat
        real(r8), allocatable :: circle_points(:,:), circle_radius(:)
        character(5) :: numc, refinec

        length = len_trim(inputfile)
        if ('nml' == inputfile(length-2:length)) then
            ! nml：第一行说明点的个数，第二行说明细化等级，三列的要求，第一列是经度，纬度，半径（km）
            open(unit=10, file=inputfile, status='old', action='read')
            REWIND(10)
            ! 读取第一行（circle_num）
            read(10, '(A)', iostat=io_stat) line  ! 读取整行
            if (io_stat /= 0) then
                write(io6, *) "Error: Failed to read circle_num"
                close(10)
                stop
            end if
            read(line(index(line, '=')+1:), *) circle_num  ! 解析 circle_num

            read(10, '(A)', iostat=io_stat) line  ! 读取整行
            if (io_stat /= 0) then
                write(io6, *) "Error: Failed to read circle_refine"
                close(10)
                stop
            end if
            read(line(index(line, '=')+1:), *) refine_degree  ! 解析 close_refine
            if (refine_degree > max_iter_spc) then
                write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
                close(10)
                return ! 返回，不进行后续操作
            end if

            allocate(circle_points(circle_num, 2))
            allocate(circle_radius(circle_num))
            do i = 1, circle_num, 1
                read(10, *) circle_points(i,1), circle_points(i,2), circle_radius(i)
            end do
            close(10)

            ! 存储为nc文件后deallocate
            write(refinec, '(I1)') refine_degree
            if (mask_select == 'mask_domain') then
                mask_domain_ndm = mask_domain_ndm + 1
                write(numc, '(I2.2)') mask_domain_ndm
            else if (mask_select == 'mask_refine') then
                mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
            else if (mask_select == 'mask_patch') then
                mask_patch_ndm(refine_degree) = mask_patch_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_patch_ndm(refine_degree)
            end if
            lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_circle_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL circle_Mesh_Save(lndname, circle_num, circle_points, circle_radius)
            deallocate(circle_points, circle_radius)

        else if (('.nc' == inputfile(length-2:length)) .or. &
                ('nc4' == inputfile(length-2:length))) then
            CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件
            CALL CHECK(NF90_INQ_VARID(ncid, 'circle_refine', varid))
            CALL CHECK(NF90_GET_VAR(ncid, varid, refine_degree))
            CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_bbox关闭文件
            if (refine_degree > max_iter_spc) then
                write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
                return ! 返回，不进行后续操作
            end if

            write(refinec, '(I1)') refine_degree
            if (mask_select == 'mask_domain') then
                mask_domain_ndm = mask_domain_ndm + 1
                write(numc, '(I2.2)') mask_domain_ndm
            else if (mask_select == 'mask_refine') then
                mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
            else if (mask_select == 'mask_patch') then
                mask_patch_ndm(refine_degree) = mask_patch_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_patch_ndm(refine_degree)
            end if
            lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_circle_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL execute_command_line('cp '//trim(inputfile)//' '//trim(lndname))
        else
            stop "ERROR! must choose 'ncfile' or 'nml' please check!"
        end if
        write(io6, *) lndname 

    end subroutine circle_mask_make

    subroutine close_mask_make(inputfile, mask_select)
        ! 文件类型然后转为nc文件便于读取，，如果是nc就直接赋值
        use netcdf
        use consts_coms, only : r8, pathlen, file_dir, mask_domain_ndm, mask_patch_ndm, io6
        use refine_vars, only : mask_refine_ndm, max_iter_spc
        use MOD_file_preprocess, only : close_Mesh_Save ! Add by Rui Zhang
        implicit none
        character(pathlen), intent(in) :: inputfile
        character(*), intent(in) :: mask_select
        integer :: ncid, varid
        character(pathlen) :: lndname, line
        logical :: fexists
        integer :: i, close_num, refine_degree, length, io_stat
        real(r8), allocatable :: close_points(:,:)
        character(5) :: numc, refinec

        ! 判断线段是否自交，然后对线段进行编号与排序
        length = len_trim(inputfile)
        if ('nml' == inputfile(length-2:length)) then
            ! nml：第一个行说明点的个数，四列的要求，第一列是经度，纬度，半径（km），细化等级（不细化为0）
            open(unit=10, file=inputfile, status='old', action='read')
            REWIND(10)
            ! * 是自由格式，Fortran 会跳过所有非数字字符，直到找到数字部分
            read(10, '(A)', iostat=io_stat) line  ! 读取整行
            if (io_stat /= 0) then
                write(io6, *) "Error: Failed to read circle_num"
                close(10)
                stop
            end if
            read(line(index(line, '=')+1:), *) close_num  ! 解析 close_num
            
            read(10, '(A)', iostat=io_stat) line  ! 读取整行
            if (io_stat /= 0) then
                write(io6, *) "Error: Failed to read close_refine"
                close(10)
                stop
            end if
            read(line(index(line, '=')+1:), *) refine_degree  ! 解析 close_refine
            if (refine_degree > max_iter_spc) then
                write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
                return ! 返回，不进行后续操作
                close(10)
            end if

            allocate(close_points(close_num, 2))
            do i = 1, close_num, 1
                read(10, *) close_points(i, 1), close_points(i, 2)
            end do
            close(10)

            write(refinec, '(I1)') refine_degree
            if (mask_select == 'mask_domain') then
                mask_domain_ndm = mask_domain_ndm + 1
                write(numc, '(I2.2)') mask_domain_ndm
            else if (mask_select == 'mask_refine') then
                mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
            else if (mask_select == 'mask_patch') then
                mask_patch_ndm(refine_degree) = mask_patch_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_patch_ndm(refine_degree)
            end if
            lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_close_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL close_Mesh_Save(lndname, close_num, close_points)
            deallocate(close_points)

        else if (('.nc' == inputfile(length-2:length)) .or. &
                ('nc4' == inputfile(length-2:length))) then
            CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件
            CALL CHECK(NF90_INQ_VARID(ncid, 'close_refine', varid))
            CALL CHECK(NF90_GET_VAR(ncid, varid, refine_degree))
            CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_close关闭文件
            if (refine_degree > max_iter_spc) then
                write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
                return ! 返回，不进行后续操作
            end if
            
            write(refinec, '(I1)') refine_degree
            if (mask_select == 'mask_domain') then
                mask_domain_ndm = mask_domain_ndm + 1
                write(numc, '(I2.2)') mask_domain_ndm
            else if (mask_select == 'mask_refine') then
                mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
            else if (mask_select == 'mask_patch') then
                mask_patch_ndm(refine_degree) = mask_patch_ndm(refine_degree) + 1
                write(numc, '(I2.2)') mask_patch_ndm(refine_degree)
            end if
            lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_close_'//trim(refinec)//'_'//trim(numc)//'.nc4'
            CALL execute_command_line('cp '//trim(inputfile)//' '//trim(lndname))
        else
            stop "ERROR! must choose 'ncfile' or 'nml' please check!"
        end if
        write(io6, *) lndname
    
    end subroutine close_mask_make


    subroutine init_consts()
        use consts_coms
        implicit none
        ! Standard (Earth) values
        ! erad = 6371.22e3          ! Earth radius [m]
        erad = 6371229 ! [m] same as MPAS
        write(io6, *) "erad = ", erad, "in the subroutine init_consts() in the mkgrd.F90" 
        ! Secondary values
        erad2 = erad * 2.
        erador5 = erad / sqrt(5.)
        eradi = 1.0 / erad
        erad2sq = erad2 * erad2

        erad8 = 6371229
        erad2_r8 = erad8 * 2.
        eradi_r8 = 1.0 / erad8
        erad2sq_r8 = erad2_r8 * erad2_r8

    end subroutine init_consts


    subroutine gridinit()

        use consts_coms, only : io6, nxp
        use mem_delaunay, only :nmd, nud, nwd
        use mem_grid, only : nma, nua, nva, nwa, alloc_grid_lonlatmw
        implicit none

        write(io6, '(/,a)') 'gridinit calling icosahedron'
        call icosahedron(nxp)  ! global spherical domain; calls 2 allocs
        write(io6, '(/,a)') 'gridinit after icosahedron'
        write(io6, '(a,i0)')    ' nmd = ', nmd
        write(io6, '(a,i0)')    ' nud = ', nud
        write(io6, '(a,i0)')    ' nwd = ', nwd

        call voronoi() ! 获取三角形重心坐标，更新连接关系？？？从xemd到xem
        call pcvt() ! 获取三角形的外心坐标！！！
        write(io6, '(/,a)') 'gridinit after voronoi'
        write(io6, '(a,i8)')   ' nma = ', nma
        write(io6, '(a,i8)')   ' nua = ', nua
        write(io6, '(a,i8)')   ' nwa = ', nwa

        ! Allocate remaining GRID FOOTPRINT arrays for full domain
        write(io6, '(/,a)') 'gridinit calling alloc_grid_lonlatmw for full domain'
        call alloc_grid_lonlatmw(nma, nva, nwa)

        ! Fill remaining GRID FOOTPRINT geometry for full domain
        write(io6, '(/,a)') 'gridinit calling grid_grid_xyz2lonlat'
        CALL grid_xyz2lonlat()

        write(io6, '(/,a)') 'gridinit calling gridfile_write'
        call gridfile_write()

        write(io6, '(/,a)') 'gridinit completed'

    end subroutine gridinit


    subroutine gridfile_write()

        use netcdf
        use consts_coms, only : r8, pathlen, io6, file_dir, EXPNME, NXP, mode_grid, refine, step
        use mem_ijtabs, only : mloops, itab_m, itab_w! , itab_v
        use mem_grid, only : nma, nwa, glatw, glonw, glatm, glonm! , nva, glatv, glonv
        use mem_grid, only : xew, yew, zew
        use MOD_file_preprocess, only : Unstructured_Mesh_Save ! Add by Rui Zhang
        implicit none

        ! This routine writes the grid variables to the gridfile.
        integer :: im, iw, sjx_points, lbx_points! , iv, edge_points
        real(r8), allocatable :: mp(:,:), wp(:,:)
        integer, allocatable :: ngrmw(:,:), ngrwm(:,:), n_ngrwm(:)
        character(pathlen) :: lndname
        character(5) :: nxpc, stepc

        allocate(ngrmw(3, nma)) ; ngrmw = 0
        allocate(ngrwm(7, nwa)) ; ngrwm = 0
        do im = 1, nma
        ngrmw(1:3, im) = itab_m(im)%iw(1:3)
        enddo
        do iw = 1, nwa
        ngrwm(1:7, iw) = itab_w(iw)%im(1:7)
        enddo


        allocate(n_ngrwm(nwa))
        n_ngrwm(1) = 1
        n_ngrwm(2:nwa) = 6
        do iw = 2, nwa, 1
            if (ngrwm(6, iw) == 1) n_ngrwm(iw) = 5
        end do 
    
        allocate(mp(nma,2)); mp(:, 1) = GLONM; mp(:, 2) = GLATM
        allocate(wp(nwa,2)); wp(:, 1) = GLONW; wp(:, 2) = GLATW


        ! Execute the command
        write(nxpc, '(I4.4)') NXP
        write(stepc, '(I2.2)') step
        ! Open gridfile
        lndname = trim(file_dir)// 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_'// trim(mode_grid)// '.nc4'

        write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
        write(io6, *) 'grid_write: opening file:', lndname
        write(io6, *) 'sjx_points:', nma
        write(io6, *) 'lbx_points:', nwa
        write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
        sjx_points = nma
        lbx_points = nwa
        ! initial file without any refine
        CALL Unstructured_Mesh_Save(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
        deallocate(mp, wp, ngrmw, ngrwm, n_ngrwm)

    END SUBROUTINE gridfile_write

    SUBROUTINE CHECK(STATUS)
        use netcdf
        INTEGER, intent (in) :: STATUS
        if  (STATUS .NE. NF90_NOERR) then ! nf_noerr=0 表示没有错误
            print *, NF90_STRERROR(STATUS)
            stop 'stopped'
        endif
    END SUBROUTINE CHECK

    SUBROUTINE voronoi()

        use mem_ijtabs, only : mloops, itab_m, itab_v, itab_w, alloc_itabs

        use mem_delaunay, only : itab_md, itab_ud, itab_wd, &
                xemd, yemd, zemd, nmd, nud, nwd

        use mem_grid, only : nma, nua, nva, nwa, mma, mua, mva, mwa, &
                xem, yem, zem, xew, yew, zew, &
                alloc_xyzem, alloc_xyzew

        use consts_coms, only : erad, r8, io6, nxp, file_dir, step
        implicit none

        integer :: im1, im2
        integer :: iw1, iw2, iw3, im, iw
        integer :: iwd, iv, iud, iud1, iud2, imd, npoly, j, j1
        real :: expansion
        ! Interchange grid dimensions

        nma = nwd
        nua = nud
        nva = nud
        nwa = nmd

        mma = nma
        mua = nua
        mva = nva
        mwa = nwa

        ! Allocate Voronoi set of itabs

        call alloc_itabs(nma, nva, nwa, 0)

        ! Allocate XEW,YEW,ZEW arrays, and fill their values from XEMD,YEMD,ZEMD, which
        ! still have the OLD nmad dimension which is the NEW nwa dimension

        call move_alloc(xemd, xew) ! 说明之前在弹性调整时候对于xemd的操作是对Delaunay triangles vertex的操作
        call move_alloc(yemd, yew)
        call move_alloc(zemd, zew)

        ! Allocate XEM,YEM,ZEM to NEW nma dimension

        call alloc_xyzem(nma)

        ! Since XEM,YEM,ZEM have just been re-allocated, initialize their values to be
        ! barycenters of Delaunay triangles whose vertices are at XEW,YEW,ZEW

        do iwd = 2, nwd
            im = iwd

            ! Indices of 3 M points surrounding WD point

            if (any(itab_wd(iwd)%im(1:3) < 2)) cycle

            iw1 = itab_wd(iwd)%im(1)
            iw2 = itab_wd(iwd)%im(2)
            iw3 = itab_wd(iwd)%im(3)

            xem(im) = (xew(iw1) + xew(iw2) + xew(iw3)) / 3.
            yem(im) = (yew(iw1) + yew(iw2) + yew(iw3)) / 3.
            zem(im) = (zew(iw1) + zew(iw2) + zew(iw3)) / 3.

            ! push M point coordinates out to earth radius
            expansion = erad / sqrt(xem(im) ** 2  &
                    + yem(im) ** 2  &
                    + zem(im) ** 2)

            xem(im) = xem(im) * expansion
            yem(im) = yem(im) * expansion
            zem(im) = zem(im) * expansion

        enddo

        ! Loop over V points

        do iv = 2, nva
            iud = iv

            itab_v(iv)%loop(1:mloops) = itab_ud(iud)%loop(1:mloops)

            itab_v(iv)%ivp = itab_ud(iud)%iup
            itab_v(iv)%ivglobe = iv
            itab_v(iv)%mrlv = itab_ud(iud)%mrlu

            itab_v(iv)%im(1:6) = itab_ud(iud)%iw(1:6)
            itab_v(iv)%iw(1:2) = itab_ud(iud)%im(1:2)

            itab_v(iv)%iv(1:4) = itab_ud(iud)%iu(1:4)
            ! itab_v(iv)%iv(1:12) = itab_ud(iud)%iu(1:12)

            ! For periodic Cartesian hex domain, compute coordinates for outer M points

            im1 = itab_v(iv)%im(1)
            im2 = itab_v(iv)%im(2)
            iw1 = itab_v(iv)%iw(1)
            iw2 = itab_v(iv)%iw(2)

            if (itab_wd(im1)%npoly < 3) then ! itab_m(im1)%npoly not filled yet
                xem(im1) = xew(iw1) + xew(iw2) - xem(im2)
                yem(im1) = yew(iw1) + yew(iw2) - yem(im2)
                zem(im1) = 0.
            elseif (itab_wd(im2)%npoly < 3) then ! itab_m(im2)%npoly not filled yet
                xem(im2) = xew(iw1) + xew(iw2) - xem(im1)
                yem(im2) = yew(iw1) + yew(iw2) - yem(im1)
                zem(im2) = 0.
            endif

            ! Extract information from IMD1 neighbor

            imd = itab_ud(iud)%im(1)
            npoly = itab_md(imd)%npoly

            do j = 1, npoly
                j1 = j + 1
                if (j == npoly) j1 = 1

                iud1 = itab_md(imd)%iu(j)
                iud2 = itab_md(imd)%iu(j1)

                ! IW(3) and IW(4) neighbors of IV

                if (iud2 == iv) then
                    iw1 = itab_ud(iud1)%im(1)
                    iw2 = itab_ud(iud1)%im(2)

                    if (iw1 == imd) then
                        itab_v(iv)%iw(3) = iw2
                    else
                        itab_v(iv)%iw(3) = iw1
                    endif
                endif

                if (iud1 == iv) then
                    iw1 = itab_ud(iud2)%im(1)
                    iw2 = itab_ud(iud2)%im(2)

                    if (iw1 == imd) then
                        itab_v(iv)%iw(4) = iw2
                    else
                        itab_v(iv)%iw(4) = iw1
                    endif
                endif

            enddo

        enddo

        ! Loop over W points

        do iw = 2, nwa
            imd = iw

            itab_w(iw)%loop(1:mloops) = itab_md(imd)%loop(1:mloops)

            itab_w(iw)%iwp = iw

            itab_w(iw)%npoly = itab_md(imd)%npoly
            itab_w(iw)%iwglobe = iw

            itab_w(iw)%mrlw = itab_md(imd)%mrlm
            itab_w(iw)%mrlw_orig = itab_md(imd)%mrlm_orig
            itab_w(iw)%ngr = itab_md(imd)%ngr

            npoly = itab_w(iw)%npoly

            ! Loop over IM/IV neighbors of IW

            do j = 1, itab_w(iw)%npoly
                im = itab_md(imd)%iw(j)
                iwd = im
                iv = itab_md(imd)%iu(j)

                iw1 = itab_v(iv)%iw(1)
                iw2 = itab_v(iv)%iw(2)

                itab_w(iw)%im(j) = im
                itab_w(iw)%iv(j) = iv

                if (iw1 == iw) then
                    itab_w(iw)%iw(j) = iw2
                    itab_w(iw)%dirv(j) = -1.
                else
                    itab_w(iw)%iw(j) = iw1
                    itab_w(iw)%dirv(j) = 1.
                endif

            enddo

        enddo

        ! Loop over M points

        do im = 2, nma
            iwd = im

            itab_m(im)%loop(1:mloops) = itab_wd(iwd)%loop(1:mloops)

            itab_m(im)%imp = im

            itab_m(im)%npoly = itab_wd(iwd)%npoly
            itab_m(im)%imglobe = im

            itab_m(im)%mrlm = itab_wd(iwd)%mrlw
            itab_m(im)%ngr = itab_wd(iwd)%ngr

            itab_m(im)%mrlm_orig = itab_wd(iwd)%mrlw_orig
            itab_m(im)%mrow = itab_wd(iwd)%mrow

            itab_m(im)%iv(1:3) = itab_wd(iwd)%iu(1:3)
            itab_m(im)%iw(1:3) = itab_wd(iwd)%im(1:3)
        enddo

        deallocate(itab_md, itab_ud, itab_wd)

    END SUBROUTINE voronoi


    SUBROUTINE pcvt()

        ! Iterative procedure for defining centroidal voronoi cells

        use mem_ijtabs, only : itab_m
        use mem_grid, only : nma, xem, yem, zem, xew, yew, zew
        use consts_coms, only : erad, eradi, r8, pathlen, io6, file_dir, nxp, step
        implicit none

        integer :: im, iw1, iw2, iw3
        real :: raxis, raxisi, expansion
        real :: sinwlat, coswlat, sinwlon, coswlon
        real :: dxe, dye, dze
        real :: xebc, yebc, zebc
        real :: x1, x2, x3, y1, y2, y3
        real :: dx12, dx13, dx23
        real :: s1, s2, s3
        real :: xcc, ycc

        ! Compute XEM,YEM,ZEM location as circumcentric coordinates of 3 W points.
        ! This establishes W cell as voronoi.

        ! Loop over all M points

        !$omp parallel
        !$omp do private(iw1,iw2,iw3,xebc,yebc,zebc,raxis,raxisi, &
        !$omp            sinwlat,coswlat,sinwlon,coswlon,dxe,dye,dze,x1,y1, &
        !$omp            x2,y2,x3,y3,dx12,dx13,dx23,s1,s2,s3,ycc,xcc)
        do im = 2, nma

            ! Indices of 3 W points surrounding M point

            if (any(itab_m(im)%iw(1:3) < 2)) cycle

            iw1 = itab_m(im)%iw(1)
            iw2 = itab_m(im)%iw(2)
            iw3 = itab_m(im)%iw(3)
            
            ! These were initialized to be the barycenter of each triangle

            xebc = xem(im)
            yebc = yem(im)
            zebc = zem(im)


            ! For global domain, transform from sphere to PS plane

            raxis = sqrt(xebc ** 2 + yebc ** 2)
            raxisi = 1.0 / raxis

            sinwlat = zebc * eradi
            coswlat = raxis * eradi

            sinwlon = yebc * raxisi
            coswlon = xebc * raxisi

            ! Transform 3 W points to PS coordinates

            dxe = xew(iw1) - xebc
            dye = yew(iw1) - yebc
            dze = zew(iw1) - zebc
            call de_ps(dxe, dye, dze, coswlat, sinwlat, coswlon, sinwlon, x1, y1)

            dxe = xew(iw2) - xebc
            dye = yew(iw2) - yebc
            dze = zew(iw2) - zebc
            call de_ps(dxe, dye, dze, coswlat, sinwlat, coswlon, sinwlon, x2, y2)

            dxe = xew(iw3) - xebc
            dye = yew(iw3) - yebc
            dze = zew(iw3) - zebc
            call de_ps(dxe, dye, dze, coswlat, sinwlat, coswlon, sinwlon, x3, y3)

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

            call ps_de(dxe, dye, dze, coswlat, sinwlat, coswlon, sinwlon, xcc, ycc)

            xem(im) = dxe + xebc
            yem(im) = dye + yebc
            zem(im) = dze + zebc

        enddo
        !$omp end do nowait

        ! Adjust each M point to the Earth's radius for global domain

        !$omp do private(expansion)
        do im = 2, nma

            expansion = erad / sqrt(xem(im) ** 2 &
                    + yem(im) ** 2 &
                    + zem(im) ** 2)

            xem(im) = xem(im) * expansion
            yem(im) = yem(im) * expansion
            zem(im) = zem(im) * expansion

        enddo
        !$omp end do nowait


        !$omp end parallel

    END SUBROUTINE pcvt


    subroutine grid_xyz2lonlat()

        use mem_ijtabs, only : itab_m, itab_v, itab_w
        use mem_grid, only : nma, nwa, xem, yem, zem, &
                xew, yew, zew, glonw, glatw, glonm, glatm
        use consts_coms, only : erad, piu180
        implicit none
        integer :: im, iw
        real :: raxis

        !$omp parallel
        !$omp do private(raxis)
        do im = 2, nma
            ! Latitude and longitude at M points
            raxis = sqrt(xem(im) ** 2 + yem(im) ** 2)  ! dist from earth axis
            glatm(im) = atan2(zem(im), raxis) * piu180
            glonm(im) = atan2(yem(im), xem(im)) * piu180
        enddo
        !$omp end do
        !$omp end parallel


        !$omp parallel
        !$omp do private(raxis)
        do iw = 2, nwa
            ! Fill outward unit vector components and latitude and longitude of W point
            raxis = sqrt(xew(iw) ** 2 + yew(iw) ** 2)
            glatw(iw) = atan2(zew(iw), raxis) * piu180
            glonw(iw) = atan2(yew(iw), xew(iw)) * piu180
        enddo
        !$omp end do
        !$omp end parallel

    end subroutine grid_xyz2lonlat
  
