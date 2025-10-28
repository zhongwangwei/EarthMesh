!===============================================================================
! OLAM was originally developed at Duke University by Robert Walko, Martin Otte,
! and David Medvigy in the project group headed by Roni Avissar.  Development
! has continued by the same team working at other institutions (University of
! Miami (rwalko@rsmas.miami.edu), the Environmental Protection Agency, and
! Princeton University), with significant contributions from other people.

! Portions of this software are copied or derived from the RAMS software
! package.  The following copyright notice pertains to RAMS and its derivatives

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
Module consts_coms

    implicit none

    ! Single (real*4) and double (real*8) precision real kinds:
    integer, parameter :: r4 = selected_real_kind(6, 37)
    integer, parameter :: r8 = selected_real_kind(13, 300)
    real, parameter :: pio180 = atan(1.0_r8)/45.0_r8   ! radians per degree
    real, parameter :: piu180 = 45.0_r8/atan(1.0_r8)   ! degrees per radian
    real, parameter :: pi2 = 8.0_r8 * atan(1.0_r8) 
    
    real(r8), parameter :: pio180_r8 = atan(1.0_r8)/45.0_r8   ! radians per degree
    real(r8), parameter :: piu180_r8 = 45.0_r8/atan(1.0_r8)   ! degrees per radian
    real(r8), parameter :: pi2_r8 = 8.0_r8 * atan(1.0_r8) 
    real(r8), parameter :: pi_r8  = 4.0_r8 * atan(1.0_r8) 
    real(r8)  :: erad8 ! Earth radius [m]
    real(r8)  :: erad2_r8
    real(r8) :: eradi_r8
    real(r8) :: erad2sq_r8
    real :: erad                          ! Earth radius [m]
    real :: erad2
    real :: erador5
    real :: eradi
    real :: erad2sq
    integer, parameter :: maxremote = 30   ! Max # of remote send/recv processes
    integer, parameter :: pathlen = 256  ! Max length of character strings for file paths

    character(64) :: expnme
    character(pathlen) :: base_dir
    character(pathlen) :: file_dir
    character(pathlen) :: mode_file
    character(pathlen) :: landtype_file
    character(pathlen) :: mask_domain_fprefix
    character(pathlen) :: mask_patch_fprefix
    character(16) :: mode_file_description
    character(16) :: mesh_type
    character(16) :: mask_domain_type
    character(16) :: mask_patch_type
    character(16) :: mode_grid
    character(16) :: output_format

    real :: rinit = 0.0
    real(r8) :: rinit8 = 0.0_r8
    integer :: impent(12) = 0  ! Scratch array for storing 12 pentagonal IM indices
    integer :: iunit = 10
    integer :: step = 1
    integer :: io6
    integer :: nxp
    integer :: openmp
    integer :: niter
    integer :: mask_domain_ndm
    integer :: gridnum_perdegree
    logical :: refine
    logical :: Isolated_Ocean
    logical :: mask_domain_global
    logical :: mask_restart
    logical :: mask_patch_on
    integer :: nlons_source
    integer :: nlats_source
    integer :: maxlc
    integer :: num_vertex
    integer :: num_center
    integer :: mask_patch_ndm(0:9) = 0
    integer :: num_mp_step(0:9) = 1 ! 
    integer :: num_wp_step(0:9) = 1 ! 
    real(r8) :: mask_sea_ratio
    real :: beta
    real :: relax

    Type oname_vars
        !!    RUNTYPE/NAME
        character(64) :: expnme = '/tmp'
        integer :: nxp = 0
        character(pathlen) :: base_dir = ' /tmp'
        character(16) :: mesh_type = '/tmp'
        character(16) :: mode_grid = '/tmp'
        character(16) :: mode_file_description = '/tmp'
        character(pathlen) :: mode_file = ' /tmp'
        logical :: refine = .FALSE.
        integer :: openmp = 16
        integer :: niter  = 5000
        integer :: gridnum_perdegree = 120
        real(r8) :: mask_sea_ratio             =    0.5
        real :: beta = 1.2
        real :: relax = 0.04
        logical :: Isolated_Ocean = .FALSE.
        logical :: mask_restart = .FALSE.
        character(16) :: mask_domain_type = '/tmp'
        character(pathlen) :: landtype_file = '/tmp'
        character(pathlen) :: mask_domain_fprefix = '/tmp'
        logical :: mask_domain_global = .true.
        logical :: mask_patch_on = .false.
        character(16) :: mask_patch_type = '/tmp'
        character(pathlen) :: mask_patch_fprefix = '/tmp'
        character(16) :: output_format = '/tmp'
    End Type oname_vars
    type (oname_vars) :: nl

End Module
!===============================================================================

! Add by Rui Zhang for lonlatmesh read from namelist
!===============================================================================
Module lonlatmesh_coms

    use consts_coms, only : r8 
    implicit none

    character(16) :: definition
    real(r8)      :: lon_start
    real(r8)      :: lon_end
    real(r8)      :: lon_grid_interval
    integer       :: lon_points
    real(r8)      :: lat_start     
    real(r8)      :: lat_end         
    real(r8)      :: lat_grid_interval
    integer       :: lat_points

    Type mesh_vars
        character(16) :: definition        = 'center'
        real(r8)      :: lon_start         = 0.
        real(r8)      :: lon_end           = 359. 
        real(r8)      :: lon_grid_interval = 0.0625
        integer       :: lon_points        = 2880
        real(r8)      :: lat_start         = 0. 
        real(r8)      :: lat_end           = 0.
        real(r8)      :: lat_grid_interval = 0.
        integer       :: lat_points        = 1440
    End Type mesh_vars
    type (mesh_vars) :: mesh
    namelist /lonlatmesh/ mesh
End Module
!===============================================================================

! Add by Rui Zhang for fvcommesh_coms
!===============================================================================
Module fvcommesh_coms

    use consts_coms, only : r8, pathlen
    implicit none

    character(pathlen) :: casename
    character(pathlen) :: Dem_file
    character(16)      :: lon_name
    character(16)      :: lat_name
    character(16)      :: dep_name
    real(r8)           :: mindepth
    real(r8)           :: maxdepth 
    real(r8)           :: limslope
    integer            :: nlons_Dem
    integer            :: nlats_Dem

    Type mesh_vars
        character(pathlen) :: casename = 'CASENAME'
        character(pathlen) :: Dem_file = '/tmp'
        character(16)      :: lon_name = '/tmp'
        character(16)      :: lat_name = '/tmp'
        character(16)      :: dep_name = '/tmp'
        real(r8)           :: mindepth = 1.
        real(r8)           :: maxdepth = 300. 
        real(r8)           :: limslope = 0.02
    End Type mesh_vars
    type (mesh_vars) :: mesh
End Module
!===============================================================================

Module mem_grid
    use consts_coms, only : r8
    implicit none

    private :: r8

    integer :: &  ! N values for full-domain reference on any process
            nma, nua, nva, nwa  ! Horiz number of all M,U,V,W points in full domain

    integer :: &
            mma, mua, mva, mwa  ! Horiz number of all M,U,V,W points in sub-process

    real, allocatable, dimension(:) :: &

              xem, yem, zem    &  ! XE,YE,ZE coordinates of M point
            , xew, yew, zew    &  ! XE,YE,ZE coordinates of W point
            , glatm, glonm     &  ! Latitude/longitude at M point
            , glatw, glonw        ! Latitude/longitude at W point

    ! integer :: impent(12)  ! Scratch array for storing 12 pentagonal IM indices

Contains

    !===============================================================================

    subroutine alloc_xyzem(lma)

        implicit none

        integer, intent(in) :: lma

        allocate (xem(lma));  xem(1:lma) = 0.
        allocate (yem(lma));  yem(1:lma) = 0.
        allocate (zem(lma));  zem(1:lma) = 0.

    end subroutine alloc_xyzem

    !===============================================================================

    subroutine alloc_xyzew(lwa)

        implicit none

        integer, intent(in) :: lwa

        allocate (xew(lwa));  xew(1:lwa) = 0.
        allocate (yew(lwa));  yew(1:lwa) = 0.
        allocate (zew(lwa));  zew(1:lwa) = 0.

    end subroutine alloc_xyzew

    !===============================================================================

    subroutine alloc_grid_lonlatmw(lma, lva, lwa)

        use consts_coms, only : io6
        implicit none

        integer, intent(in) :: lma, lva, lwa

        ! Allocate and initialize arrays (xem, yem, zem are already allocated)
        allocate (glatw(lwa));  glatw(1:lwa) = 0.
        allocate (glonw(lwa));  glonw(1:lwa) = 0.
        allocate (glatm(lma));  glatm(1:lma) = 0.
        allocate (glonm(lma));  glonm(1:lma) = 0.

        write(io6, *) 'finishing alloc_grid_lonlatmw'

    end subroutine alloc_grid_lonlatmw

    !===============================================================================

End Module mem_grid

Module mem_ijtabs

    use consts_coms, only : maxremote

    implicit none

    private :: maxremote

    integer, parameter :: mloops = 7 ! max # non-para DO loops for M,V,W pts

    integer, parameter :: nloops_m = mloops + maxremote ! # M DO loops incl para
    integer, parameter :: nloops_v = mloops + maxremote ! # V DO loops incl para
    integer, parameter :: nloops_w = mloops + maxremote ! # W DO loops incl para

    ! M, U, V, and W loop indices. The last 4 letters of these indices have the
    ! following meanings:
    !
    ! *_grid is for setting up grid parameters in the ctrlvols subroutines
    ! *_init is for initialization of ATM fields (in ohhi, olhi, fldsisan,
    !        hurricane_init, omic_init)
    ! *_prog is for points where primary quantities such as VMC or THIL are
    !        prognosed
    ! *_wadj is for U, V, or W points that are adjacent to W points where primary
    !        quantities are prognosed
    ! *_wstn is for all U, V, or W points in the stencil of a W point where
    !        primary quantities are prognosed
    ! *_lbcp is for lateral boundary points whose values are copied from an
    !        interior prognostic point
    ! *_vadj is for M points that are adjacent to a prognostic V point, while
    !        jtw_vadj is the representation of jtm_vadj in the pre-hex-grid stage
    !        of model grid initialization)

    integer, parameter :: jtm_grid = 1, jtu_grid = 1, jtv_grid = 1, jtw_grid = 1
    integer, parameter :: jtm_init = 2, jtu_init = 2, jtv_init = 2, jtw_init = 2
    integer, parameter :: jtm_prog = 3, jtu_prog = 3, jtv_prog = 3, jtw_prog = 3
    integer, parameter :: jtm_wadj = 4, jtu_wadj = 4, jtv_wadj = 4, jtw_wadj = 4
    integer, parameter :: jtm_wstn = 5, jtu_wstn = 5, jtv_wstn = 5, jtw_wstn = 5
    integer, parameter :: jtm_lbcp = 6, jtu_lbcp = 6, jtv_lbcp = 6, jtw_lbcp = 6
    integer, parameter :: jtm_vadj = 7, jtu_wall = 7, jtv_wall = 7, jtw_vadj = 7

    Type itab_m_vars             ! data structure for M pts (individual rank)
        logical, allocatable :: loop(:) ! flag to perform each DO loop at this M pt
        integer :: npoly = 0       ! number of V/W neighbors of this M pt
        integer :: imp = 1         ! M point from which to copy this M pt's values
        integer :: imglobe = 1     ! global index of this M pt (in parallel case)
        integer :: mrlm = 0        ! mesh refinement level of this M pt
        integer :: mrlm_orig = 0   ! original MRL of this M pt (hex only)
        integer :: mrow = 0        ! Full row number outside nest
        integer :: ngr = 0         ! Grid number
        integer :: iv(3) = 1       ! array of V neighbors of this M pt
        integer :: iw(3) = 1       ! array of W neighbors of this M pt
    End Type itab_m_vars

    Type itab_v_vars             ! data structure for V pts (individual rank)
        logical, allocatable :: loop(:) ! flag to perform each DO loop at this V pt
        integer :: ivp = 1       ! V pt from which to copy this V pt's values
        integer :: irank = -1    ! rank of parallel process at this V pt
        integer :: ivglobe = 1   ! global index of this V pt (in parallel case)
        integer :: mrlv = 0      ! mesh refinement level of this V pt
        integer :: im(6) = 1     ! neighbor M pts of this V pt
        integer :: iw(4) = 1     ! neighbor W pts of this V pt
        integer :: iv(4) = 1     ! neighbor V pts

    End Type itab_v_vars

    Type itab_w_vars             ! data structure for W pts (individual rank)
        logical, allocatable :: loop(:) ! flag to perform each DO loop at this W pt
        integer :: npoly = 0     ! number of M/V neighbors of this W pt
        integer :: iwp = 1       ! W pt from which to copy this W pt's values
        integer :: irank = -1    ! rank of parallel process at this W pt
        integer :: iwglobe = 1   ! global index of this W pt (in parallel run)
        integer :: mrlw = 0      ! mesh refinement level of this W pt
        integer :: mrlw_orig = 0 ! original MRL of this W pt
        integer :: ngr = 0       ! Grid number
        integer :: im(7) = 1     ! neighbor M pts
        integer :: iv(7) = 1     ! neighbor V pts
        integer :: iw(7) = 1     ! neighbor W pts
        real :: dirv(7) = 0.     ! pos direction of V neighbors

    End Type itab_w_vars

    type (itab_m_vars), allocatable, target :: itab_m(:)
    type (itab_v_vars), allocatable, target :: itab_v(:)
    type (itab_w_vars), allocatable, target :: itab_w(:)

Contains

    !===============================================================================

    subroutine alloc_itabs(mma, mva, mwa, input)

        implicit none

        integer, intent(in) :: mma, mva, mwa, input
        integer :: im, iv, iw

        allocate (itab_m(mma))
        allocate (itab_v(mva))
        allocate (itab_w(mwa))

        do im = 1, mma
            allocate(itab_m(im)%loop(mloops))
            itab_m(im)%loop(1:mloops) = .false.
        enddo

        do iv = 1, mva
            allocate(itab_v(iv)%loop(mloops))
            itab_v(iv)%loop(1:mloops) = .false.
        enddo

        do iw = 1, mwa
            allocate(itab_w(iw)%loop(mloops))
            itab_w(iw)%loop(1:mloops) = .false.
        enddo

    end subroutine alloc_itabs

    !===============================================================================

End Module mem_ijtabs

Module mem_delaunay

    use mem_ijtabs, only : mloops

    implicit none

    private :: mloops

    Type itab_md_vars             ! data structure for M pts (individual rank)
        ! on the Delaunay mesh
        logical :: loop(mloops) = .false. ! flag to perform each DO loop at this M pt
        integer :: npoly = 0       ! number of V/W neighbors of this M pt
        integer :: imp = 1         ! M point from which to copy this M pt's values
        integer :: mrlm = 0        ! mesh refinement level of this M pt
        integer :: mrlm_orig = 0   ! original MRL of this M pt (hex only)
        integer :: ngr = 0         ! Grid number
        integer :: im(7) = 1       ! array of M neighbors of this M pt
        integer :: iu(7) = 1       ! array of U neighbors of this M pt
        integer :: iw(7) = 1       ! array of W neighbors of this M pt
    End Type itab_md_vars

    Type itab_ud_vars             ! data structure for U pts (individual rank)
        ! on the Delaunay mesh
        logical :: loop(mloops) = .false. ! flag to perform each DO loop at this M pt
        integer :: iup = 1       ! U pt from which to copy this U pt's values
        integer :: mrlu = 0      ! mesh refinement level of this U pt
        integer :: im(2) = 1     ! neighbor M pts of this U pt
        integer :: iu(12) = 1    ! neighbor U pts
        integer :: iw(6) = 1     ! neighbor W pts
    End Type itab_ud_vars

    Type itab_wd_vars             ! data structure for W pts (individual rank)
        ! on the Delaunay mesh
        logical :: loop(mloops) = .false. ! flag to perform each DO loop at this W pt
        integer :: npoly = 0     ! number of M/V neighbors of this W pt
        integer :: iwp = 1       ! W pt from which to copy this W pt's values
        integer :: mrlw = 0      ! mesh refinement level of this W pt
        integer :: mrlw_orig = 0 ! original MRL of this W pt
        integer :: mrow = 0      ! Full row number outside nest
        integer :: ngr = 0       ! Grid number
        integer :: im(3) = 1     ! neighbor M pts
        integer :: iu(3) = 1     ! neighbor U pts
        integer :: iw(9) = 1     ! neighbor W pts
    End Type itab_wd_vars

    Type nest_ud_vars        ! temporary U-pt data structure for spawning nested grids
        integer :: im = 0, iu = 0 ! new M/U pts attached to this U pt
    End Type nest_ud_vars

    Type nest_wd_vars        ! temporary W-pt data structure for spawning nested grids
        integer :: iu(3) = 0  ! new U pts attached to this W pt
        integer :: iw(3) = 0  ! new W pts attached to this W pt
    End Type nest_wd_vars

    type (itab_md_vars), allocatable :: itab_md(:)
    type (itab_ud_vars), allocatable :: itab_ud(:)
    type (itab_wd_vars), allocatable :: itab_wd(:)

    type (itab_md_vars), allocatable :: itab_md_copy(:)
    type (itab_ud_vars), allocatable :: itab_ud_copy(:)
    type (itab_wd_vars), allocatable :: itab_wd_copy(:)

    real, allocatable :: xemd(:), yemd(:), zemd(:)

    real, allocatable :: xemd_copy(:), yemd_copy(:), zemd_copy(:)

    integer :: nmd, nud, nwd

    integer :: nmd_copy = 0
    integer :: nud_copy = 0
    integer :: nwd_copy = 0

    integer, allocatable :: iwdorig(:), iwdorig_temp(:)

Contains

    !===============================================================================

    subroutine alloc_itabsd(mma, mua, mwa)

        use consts_coms, only : rinit
        use mem_ijtabs, only : mloops

        implicit none

        integer, intent(in) :: mma, mua, mwa

        allocate (itab_md(mma))
        allocate (itab_ud(mua))
        allocate (itab_wd(mwa))

        allocate(xemd(mma)) ; xemd = rinit
        allocate(yemd(mma)) ; yemd = rinit
        allocate(zemd(mma)) ; zemd = rinit

        xemd(1) = 0.
        yemd(1) = 0.
        zemd(1) = 0.

    end subroutine alloc_itabsd

    !===============================================================================

End Module mem_delaunay

Module refine_vars

    use consts_coms, only : r8

    implicit none
    
    character(16)  :: refine_setting           = '/tmp'
    character(16)  :: mask_refine_spc_type     = '/tmp'
    character(256) :: mask_refine_spc_fprefix  = '/tmp'
    character(16)  :: mask_refine_cal_type     = '/tmp'
    character(256) :: mask_refine_cal_fprefix  = '/tmp'
    character(256) :: threshold_dir            = '/tmp'
    character(16)  :: set_dis_type             = '/tmp'
    integer :: mask_refine_ndm(0:9)   =  0
    integer :: max_iter               =  0
    integer :: max_iter_spc           =  0
    integer :: max_iter_cal           =  0
    integer :: halo(10)               =  0
    integer :: max_transition_row(10) =  0
    integer :: SpringGlobal_type      =  1
    integer :: SpringRegional_type    =  1
    integer :: num_rc                 =  0
    integer :: vertex_pretect_layers     =  1
    integer :: niter_refine           =  100
    integer :: th_num_landtypes       =  12

    real(r8) :: th_area_mainland      =  0.6
    real(r8) :: th_sea_ratio(2)       =  0.5
    real(r8) :: th_onelayer_Lnd(4)    =  999.
    real(r8) :: th_onelayer_Ocn(8)    =  999.
    real(r8) :: th_onelayer_Atmos(2)  =  999.
    real(r8) :: th_twolayer_Lnd(10, 2)=  999.

    logical :: weak_concav_eliminate  = .FALSE.
    logical :: Istransition           = .FALSE.
    logical :: iterD                  = .FALSE.
    logical :: refine_spc             = .FALSE.
    logical :: refine_cal             = .FALSE.
    logical :: refine_num_landtypes   = .FALSE.
    logical :: refine_area_mainland   = .FALSE.
    logical :: refine_sea_ratio       = .FALSE.
    logical :: refine_onelayer_Lnd(4)     = .FALSE.
    logical :: refine_onelayer_Ocn(8)     = .FALSE.
    logical :: refine_onelayer_Atmos(2)   = .FALSE.
    logical :: refine_twolayer_Lnd(10)    = .FALSE.
    logical :: exit_loop_step(10)         = .FALSE.

    Type threshold_vars

        character(16)  :: mask_refine_spc_type     = '/tmp'
        character(256) :: mask_refine_spc_fprefix  = '/tmp'
        character(16)  :: mask_refine_cal_type     = '/tmp'
        character(256) :: mask_refine_cal_fprefix  = '/tmp'
        character(256) :: threshold_dir            = '/tmp'
        character(16)  :: set_dis_type             = '/tmp'
        integer  :: max_iter_spc
        integer  :: max_iter_cal
        integer  :: halo(10)
        integer  :: max_transition_row(10)
        integer  :: SpringGlobal_type
        integer  :: SpringRegional_type
        integer  :: num_rc
        integer  :: vertex_pretect_layers
        integer  :: niter_refine
        integer  :: th_num_landtypes
        logical  :: weak_concav_eliminate = .FALSE.  
        logical  :: Istransition          = .FALSE.
        logical  :: iterD                 = .FALSE.
        logical  :: refine_spc            = .FALSE.
        logical  :: refine_cal            = .FALSE.

        ! use for landmesh
        logical  :: refine_num_landtypes  = .FALSE.
        logical  :: refine_area_mainland  = .FALSE.
        logical  :: refine_lai_m          = .FALSE.
        logical  :: refine_lai_s          = .FALSE.
        logical  :: refine_slope_m        = .FALSE.
        logical  :: refine_slope_s        = .FALSE.
        logical  :: refine_k_s_m          = .FALSE.
        logical  :: refine_k_s_s          = .FALSE.
        logical  :: refine_k_solids_m     = .FALSE.
        logical  :: refine_k_solids_s     = .FALSE.
        logical  :: refine_tkdry_m        = .FALSE.
        logical  :: refine_tkdry_s        = .FALSE.
        logical  :: refine_tksatf_m       = .FALSE.
        logical  :: refine_tksatf_s       = .FALSE.
        logical  :: refine_tksatu_m       = .FALSE.
        logical  :: refine_tksatu_s       = .FALSE.
        ! use for oceanmesh
        logical  :: refine_sea_ratio      = .FALSE.
        logical  :: refine_sst_m          = .FALSE.
        logical  :: refine_sst_s          = .FALSE.
        logical  :: refine_ssh_m          = .FALSE.
        logical  :: refine_ssh_s          = .FALSE.
        logical  :: refine_eke_m          = .FALSE.
        logical  :: refine_eke_s          = .FALSE.
        logical  :: refine_sea_slope_m    = .FALSE.
        logical  :: refine_sea_slope_s    = .FALSE.

        ! use for earthmesh/airmesh
        logical  :: refine_typhoon_m      = .FALSE.
        logical  :: refine_typhoon_s      = .FALSE.

        real(r8) :: th_area_mainland
        real(r8) :: th_lai_m
        real(r8) :: th_lai_s
        real(r8) :: th_slope_m
        real(r8) :: th_slope_s
        real(r8) :: th_k_s_m(2)
        real(r8) :: th_k_s_s(2)
        real(r8) :: th_k_solids_m(2)
        real(r8) :: th_k_solids_s(2)
        real(r8) :: th_tkdry_m(2)
        real(r8) :: th_tkdry_s(2)
        real(r8) :: th_tksatf_m(2)
        real(r8) :: th_tksatf_s(2)
        real(r8) :: th_tksatu_m(2)
        real(r8) :: th_tksatu_s(2)

        real(r8) :: th_sea_ratio(2)
        real(r8) :: th_sst_m
        real(r8) :: th_sst_s
        real(r8) :: th_ssh_m
        real(r8) :: th_ssh_s
        real(r8) :: th_eke_m
        real(r8) :: th_eke_s
        real(r8) :: th_sea_slope_m
        real(r8) :: th_sea_slope_s

        real(r8) :: th_typhoon_m
        real(r8) :: th_typhoon_s


    End Type threshold_vars
    type (threshold_vars) :: rl


End Module refine_vars
