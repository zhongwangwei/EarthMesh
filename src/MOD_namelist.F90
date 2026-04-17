!-- @brief Namelist processing module
!-- @details This module contains subroutines for reading namelist files
module MOD_namelist
    implicit none
    
    public :: read_nl

contains

!-- @brief Reads namelist settings for grid generation and refinement.
    subroutine read_nl(nlfile)
        use consts_coms
        use refine_vars
        use MOD_mask_process, only : Mask_make
        implicit none

        character(*), intent(in) :: nlfile
        integer :: i, pos, iostat
        logical :: fexists
        character(pathlen) :: path, fprefix, filename, lndname

        namelist /mkgrd/ nl
        namelist /mkrefine/ rl
        ! OPEN THE NAMELIST FILE
        inquire(file = nlfile, exist = fexists)
        write(io6, *)  nlfile
        if (.not. fexists) then
            write(*, *) "The namelist file " // trim(nlfile) // " is missing."
            stop "Stopping model run."
        endif
        open(iunit, status = 'OLD', file = nlfile)
        ! READ GRID POINT, MODEL OPTIONS, AND PLOTTING INFORMATION FROM THE NAMELIST
        REWIND(iunit)

        read(iunit, nml = mkgrd)
        close(iunit)
        write(*, nml = mkgrd)
        write(io6, *)  ""

        !----------------------------------------------------------
        ! read from namelist
        expnme               = nl%expnme
        nxp                  = nl%nxp
        case_dir             = nl%case_dir
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
        mask_domain_global   = nl%mask_domain_global
        mask_domain_type     = nl%mask_domain_type
        mask_domain_fprefix  = nl%mask_domain_fprefix
        output_format        = nl%output_format
        mask_patch_on        = nl%mask_patch_on
        mask_patch_type      = nl%mask_patch_type
        mask_patch_fprefix   = nl%mask_patch_fprefix
        mask_sea_ratio       = nl%mask_sea_ratio 
        
        file_dir             = trim(case_dir) // trim(expnme) // '/'
       
        write(io6, *)  "gridnum_perdegree = ", gridnum_perdegree
        if (gridnum_perdegree == 240) then
            write(io6, *)  "landtype_igbp"
        else if (gridnum_perdegree == 120) then
            write(io6, *)  "landtype_usgs"
        else
            STOP "ERROR! gridnum_perdegree must 120 or 240 now!"
        end if

        if (mesh_type == 'landmesh') then
            if (output_format /= 'CoLM') then
                write(io6, *)  "ERROR! output_format should be CoLM when mesh_type is landmesh"
                STOP
            end if
        else if (mesh_type == 'oceanmesh') then
            ! output_format must match mesh_type
            if (output_format /= 'FVCOM') then
                write(io6, *)  "WARNING! output_format should be FVCOM when mesh_type is oceanmesh"
                write(io6, *)  "output_format = ", output_format
                STOP
            end if

            ! output_format must match mode_grid
            if (output_format == 'FVCOM') then
                if (mode_grid /= 'tri') then
                    write(io6, *)  "mode_grid = ", mode_grid
                    STOP "ERROR! mode_grid must be tri when output_format == 'FVCOM'"
                end if
            end if

        else if (mesh_type == 'atmosmesh') then
            if ((output_format == 'MPAS') .or. (output_format == 'MPAS-Simple')) then
                if (mode_grid /= 'hex') then
                    write(io6, *)  "mode_grid = ", mode_grid
                    STOP "ERROR! mode_grid must be hex when output_format == 'MPAS' or 'MPAS-Simple'"
                end if
            end if

            if ((output_format /= 'MPAS') .and. (output_format /= 'MPAS-Simple')) then
                write(io6, *)  "WARNING! output_format should be MPAS/MPAS-Simple when mesh_type is atmosmesh"
                write(io6, *)  "output_format = ", output_format
                stop
            end if

        else if (mesh_type == 'LOCmesh') then
            STOP "ERROR! can not use now! under development"
            if (output_format /= 'CoLM') then
                write(io6, *)  "WARNING! output_format should be CoLM when mesh_type is LOCmesh"
                stop
            end if
        else
            STOP "ERROR! mesh_type mush be landmesh/oceanmesh/atmosmesh/LOCmesh"
        end if


        CALL execute_command_line('rm -rf '//trim(file_dir)) ! rm old filedir
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"contain/") ! use for step
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"gridfile/") ! use for step
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"result/") ! final mesh file
        CALL execute_command_line('mkdir -p '//trim(file_dir)//"tmpfile/")
        CALL execute_command_line('ls *_filelist.txt 2>/dev/null | grep -q .', exitstat=iostat)
        if (iostat == 0) CALL execute_command_line('rm *_filelist.txt')
        
        if ((mesh_type == 'landmesh') .or. &
            (mesh_type == 'LOCmesh')) then 
            CALL execute_command_line('mkdir -p '//trim(file_dir)//"patchtype/")
        end if

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
            halo                  = rl%halo
            max_transition_row    = rl%max_transition_row
            niter_refine          = rl%niter_refine
            SpringGlobal_type     = rl%SpringGlobal_type
            num_rc                = rl%num_rc
            set_dis_type          = rl%set_dis_type
            

            if (Istransition .eqv. .false.) then
                if (mode_grid /= 'tri') STOP "ERROR! not Istransition can only use in the tri"
                SpringGlobal_type = 0
                write(io6, *)  "Istransition = .false. SpringGlobal_type modify to zero !"
            else
                if ((SpringGlobal_type < 0) .or. (SpringGlobal_type > 1)) STOP "ERROR! SpringGlobal_type must 0,1"
                
                if (SpringGlobal_type > 0) then
                    if (niter_refine < 1000) then
                        write(io6, *)  "when SpringGlobal_type > 0, niter_refine = ", niter_refine
                        write(io6, *)  "WARNING! The number of iterations is relatively small, it can be increased to over 1000"
                    end if
                end if
            end if
        
            refine_spc            = rl%refine_spc
            refine_cal            = rl%refine_cal
            if (refine_spc) max_iter_spc          = rl%max_iter_spc ! 默认为0，开关打开才读取
            if (refine_cal) max_iter_cal          = rl%max_iter_cal ! 默认为0，开关打开才读取

            max_iter = max(max_iter_cal, max_iter_spc) ! Determine maximum iterations from calculated and specified values
            write(io6, *)  "max_iter_spc = ", max_iter_spc ! Max iterations for specified refinement (read from namelist)
            write(io6, *)  "max_iter_cal = ", max_iter_cal ! Max iterations for calculated (threshold-based) refinement (read from namelist)
            write(io6, *)  "max_iter = ", max_iter
            if (max_iter <= 0) stop 'Error! max_iter must more than zero'

            ! Validate halo vs. max_transition_row settings
            do i = 1, max_iter, 1
                if (halo(i) < max_transition_row(i)) then
                    write(io6, *)  'i = ', i
                    write(io6, *)  'halo(i) = ', halo(i)
                    write(io6, *)  'max_transition_row(i) = ', max_transition_row(i)
                    stop "ERROR! halo must larger than max_transition_row!"
                end if   
                if (halo(i) <= 0) stop 'Error! halo(i) must more than zero'
                if (max_transition_row(i) <= 0) stop 'Error! max_transition_row(i) must more than zero'
            end do

            ! Check for max_transition_row (must use the same values)
            do i = 2, max_iter, 1
                if (max_transition_row(i) /= max_transition_row(i-1)) then
                    STOP "ERROR! max_transition_row(i) must equal to max_transition_row(i-1)"
                end if
            end do
            if (refine_cal) then
                if (mesh_type == 'atmosmesh') STOP "ERROR! atmosmesh can not use in refine_cal"
            end if

            ! Check for weak_concav_eliminate (must use the same values)
            do i = 1, max_iter, 1
                if ((weak_concav_eliminate(i) /= 1) .and. &
                    (weak_concav_eliminate(i) /= 0)) then
                    STOP "ERROR! weak_concav_eliminate(i) must be one or zero"
                end if
            end do
            do i = 2, max_iter, 1
                if (weak_concav_eliminate(i) /= weak_concav_eliminate(i-1)) then
                    STOP "ERROR! weak_concav_eliminate(i) must equal to weak_concav_eliminate(i-1)"
                end if
            end do

            if ((refine_spc .eqv. .TRUE.) .and. (refine_cal .eqv. .TRUE.)) then
                refine_setting = 'mixed'
            else if ((refine_spc .eqv. .TRUE.)  .and. (refine_cal .eqv. .FALSE.)) then
                refine_setting = 'specified'
            else if ((refine_spc .eqv. .FALSE.) .and. (refine_cal .eqv. .TRUE.)) then
                refine_setting = 'calculate'
            else
                stop "ERROR! MUst one of TRUE in the refine_spc and refine_cal when refine is TRUE"
            end if
            write(io6, *)  "refine_setting = ", refine_setting
            
            ! 指定细化/混合细化
            if (refine_setting == 'specified' .or. refine_setting == 'mixed') then
                mask_refine_spc_type       = RL%mask_refine_spc_type
                mask_refine_spc_fprefix    = RL%mask_refine_spc_fprefix
                CALL Mask_make('mask_refine', mask_refine_spc_type, mask_refine_spc_fprefix) 
                if (mask_refine_ndm(max_iter_spc) == 0) then
                    write(io6, *)  "max_iter_spc = ", max_iter_spc
                    write(io6, *)  "mask_refine_ndm(max_iter_spc) = ", mask_refine_ndm(max_iter_spc)
                    stop "ERROR! mask_refine_ndm(max_iter_spc) must larger then one, please modify max_iter_spc"
                end if
            end if

            ! 阈值细化/混合细化
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

                if (mesh_type == 'atmosmesh') then
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
                            (all(refine_onelayer_Ocn  .eqv. .false.))) then
                            write(io6, *)  "refine_num_landtypes = ", refine_num_landtypes
                            write(io6, *)  "refine_area_mainland = ", refine_area_mainland
                            write(io6, *)  "refine_sea_ratio = ", refine_sea_ratio
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

end module MOD_namelist 
