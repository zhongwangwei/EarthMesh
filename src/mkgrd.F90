!DESCRIPTION
!===========
!===============================================================================
!-- @brief Main program for unstructured mesh generation.
!-- @details This program generates unstructured meshes for Multiple components of
!-- earth system models (e.g., land surface model: CoLM, ocean model: FVCOM, 
!-- atmopheric model: MPAS-A)
!-- grid generation modes (hexagonal, triangular),
!-- and optional mesh refinement.
!===============================================================================

!REVISION HISTORY
!----------------
! 2025.06.11  Zhongwang Wei @ SYSU
! 2025.06.10  Rui Zhang @ SYSU
! 2023.02.21  Zhongwang Wei @ SYSU
! 2021.12.02  Zhongwang Wei @ SYSU 
! 2020.10.01  Zhongwang Wei @ SYSU

! The original code is from OLAM
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
    use consts_coms                                                          ! Module containing physical and mathematical constants
    use refine_vars                                                          ! Module containing variables related to mesh refinement
    use MOD_data_preprocess        , only : data_preprocess                  ! Module for preprocessing input data
    use MOD_grid_preprocess        , only : IAP_Mesh_Make,Grid_Quality_Check ! Module for preprocessing grid data
    use MOD_namelist               , only : read_nl                          ! Module for reading namelist files
    use MOD_grid_initialization    , only : init_consts, gridinit            ! Module for grid initialization
    use MOD_Area_judge             , only : Area_judge, Area_judge_refine    ! Module for judging areas for refinement/masking
    use MOD_GetContain             , only : Get_Contain                      ! Module to determine if points are contained in areas
    use MOD_GetRef                 , only : GetRef                           ! Module to get refinement flags
    use MOD_Refine                 , only : refine_loop                      ! Module for the main mesh refinement loop
    use MOD_mask_postproc          , only : mask_postproc                    ! Module for post-processing masks
    use MOD_utilities              , only : MPAS_Mesh_Read, FVCOM_Mesh_Read  ! Module for utility functions
    use MOD_utilities              , only : EarthMesh_Mesh_Check             ! Module for utility functions

    implicit none
    character(pathlen) :: nlfile = 'mkgrd.mnl'                               ! Namelist file name, default 'mkgrd.mnl'
    character(pathlen) :: finfolist                                          ! Path to save the namelist file
    character(pathlen) :: lndname                                            ! Land grid file name
    character(5)       :: stepc                                              ! Character representation of the current step
    character(5)       :: nxpc                                               ! Character representation of NXP (grid resolution parameter)
    logical            :: exit_loop                                          ! Flag to exit the refinement loop
    logical            :: fexists                                            ! Flag to check if a file exists
    integer            :: i                                                  ! Index
    
    ! Read namelist file
    CALL getarg(1, nlfile)                                                   ! Get the namelist file name from the command line
    CALL read_nl(nlfile)                                                     ! Read settings from the namelist file

    ! Save a copy of the namelist file
    finfolist = trim(file_dir)//'result/namelist.save'                       ! Define path for saving namelist
    CALL execute_command_line('cp '//trim(nlfile)//' '//trim(finfolist))     ! Copy namelist to result directory

    ! Check if the mesh type is valid
    mesh_type = trim(mesh_type)
    write(io6, *)  'mesh_type set as ', mesh_type
    if ((mesh_type /= 'atmosmesh') .and. &
        (mesh_type /= 'oceanmesh') .and. &
        (mesh_type /= 'landmesh' ) .and. &
        (mesh_type /= 'LOCmesh'  )) then
        STOP "ERROR! mesh_type mush be atmosmesh/landmesh/oceanmesh/LOCmesh"
    end if
        

    step = 1                                                                 ! Initialize current refinement step
    num_vertex = 1                                                           ! Initialize number of vertices
    num_center = 1                                                           ! Initialize number of center
    call init_consts()                                                       ! Initialize constants

    ! Grid generation logic based on mode_grid type
    mode_grid = trim(mode_grid)
    write(io6, *)  'mode_grid set as ', mode_grid   
    if ((mode_grid == 'hex') .or. &                                          ! Hexagonal grid
        (mode_grid == 'tri')) then                                           ! Triangular grid
        inquire(file = mode_file, exist = fexists)                           ! Check if the mode_file (initial grid file) exists
        if (.not. fexists) then ! 说明mode_file并不存在
            write(io6, *)  "mode_file is not exist!"
            write(io6, *)  "mode_file_description modify to 'EarthMesh'"
            mode_file_description = 'EarthMesh'
            call gridinit()                                                  ! Initialize the grid structure (e.g., icosahedron)
        else
            write(io6, *)  "read data from mode_file"
            write(io6, *)  "mode_file = ", trim(mode_file)
            write(nxpc, '(I4.4)') NXP
            write(stepc, '(I2.2)') step
            lndname = trim(file_dir)// 'gridfile/gridfile_NXP' // trim(nxpc) // '_'// trim(stepc) //'_'//trim(mode_grid)// '.nc4'
            ! check the file in the mode_file
            write(io6, *)  "mode_file_description = ", trim(mode_file_description)
            if (trim(mode_file_description) == 'EarthMesh') then
                CALL EarthMesh_Mesh_Check(mode_file, lndname) 
            else if (trim(mode_file_description) == 'MPAS') then
                CALL MPAS_Mesh_Read(mode_file, lndname) 
            else if (trim(mode_file_description) == 'FVCOM') then
                CALL FVCOM_Mesh_Read(mode_file, lndname)
            else if (trim(mode_file_description) == 'IAP-Ocean') then
                CALL IAP_Mesh_Make(mode_file, lndname)
            else
                stop "ERROR! Only EarthMesh / MPAS / FVCOM / IAP-Ocean can be used when mode_file exist !"
            end if
        end if

        ! mesh angle check
        write(io6, *)  'inital grid quality check start'
        CALL Grid_Quality_Check()
        write(io6, *)  'inital grid quality check finish'
    else
        stop 'ERROR mode_grid only tri/hex can choose !!!'
    end if 

    write(io6, *)  'data preporcess start'
    ! Preset some necessary data such as landtype 
    CALL data_preprocess()
    write(io6, *)  'data preporcess complete'
    write(io6, *) ""

    write(io6, *)  'area judge start'
    ! Multiple boundaries options available in the DmArea
    CALL Area_judge() ! Determine domain area (DmArea), handles mask-patch modifications
    write(io6, *)  'area judge complete'
    write(io6, *) ""

    ! Main refinement loop
    if (refine) then
        write(io6, *)  'make grid with variable-resolution mesh'
        write(io6, *)  ""
        write(io6, *)  'start do-while'
        exit_loop = .false. ! Initialize exit_loop flag
        do while(step <= max_iter) ! Loop through refinement steps
            write(io6, *)  'step = ',step, 'in the refine-circle'
            write(io6, *)  'Get ref_sjx start'
            ! Get refinement flags (ref_sjx) for triangles/polygons
            ! Only calculate for newly-generated triangles or polygons

            ! Threshold-based refinement
            if (refine_setting == 'calculate' .or. refine_setting == 'mixed') then
                if (step <= max_iter_cal) then
                    CALL Area_judge_refine(0)      ! Judge refinement area (0 indicates threshold-based)
                    CALL Get_Contain(0)            ! Determine containment for threshold-based
                    CALL GetRef(0)                 ! Get refinement flags for threshold-based
                end if
            end if

            ! Specified refinement (e.g., based on predefined regions)
            if (refine_setting == 'specified' .or. refine_setting == 'mixed') then
                if (step <= max_iter_spc) then
                    ! Update map for specified refinement
                    CALL Area_judge_refine(step)   ! Judge refinement area for specified (step indicates current iteration)
                    CALL Get_Contain(step)         ! Determine containment for specified
                    CALL GetRef(step)              ! Get refinement flags for specified
                end if
            end if
            write(io6, *)  'Get ref_sjx complete'
            write(io6, *)  ""

            write(io6, *)  'refine_loop start'
            CALL refine_loop(exit_loop) ! Perform the actual mesh refinement based on ref_sjx
            write(io6, *)  'refine_loop complete'
            write(io6, *)  ""
           
            if (exit_loop) then ! If GetRef indicated no more cells to refine
                write(io6, *)  'Exiting loop due to ref_sjx equal to zero! &
                                turn refine from to True to False !'
                exit ! Exit the DO WHILE loop
            end if
           
            step = step + 1 ! Increment refinement step
        end do
        write(io6, *)  'finish do-while'
        
        write(io6, *)  'final grid quality check start'
        CALL Grid_Quality_Check()
        write(io6, *)  'final grid quality check start'
    else
        write(io6, *)  'make grid with basic mesh'
    end if


    ! Final processing steps
    refine = .false. ! Ensure refine is false after the loop (or if it was never true)
                     ! This means no more cells need further calculation/refinement.
    CALL Get_Contain(0) ! Final containment check (purpose might need more context, 0 suggests a general pass)
    CALL mask_postproc(mesh_type) ! Post-process masks (e.g., apply land/sea mask)

    ! Success message
    write(io6, '(A)')  "--------------------------------"
    write(io6, '(A)')  ""
    write(io6, '(A)')  "!! Successfully Make Grid End !!"
    write(io6, '(A)')  ""
    write(io6, '(A)')  "--------------------------------"

end program main


  
