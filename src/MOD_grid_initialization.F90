! Grid initialization module
! This module contains subroutines for initializing physical constants
! and basic grid structure generation
module MOD_grid_initialization
    implicit none
    
    public :: init_consts, gridinit, gridfile_write

contains

    ! Initializes physical constants, primarily related to Earth's geometry.
    ! Sets values for Earth's radius and derived quantities.
    subroutine init_consts()
        use consts_coms ! Module containing constants like erad, pio180
        implicit none

        ! Standard (Earth) values
        ! erad = 6371.22e3          ! Earth radius [m]
        erad = 6371229 ! [m] same as MPAS
        write(io6, *) "erad = ", erad, "in the subroutine init_consts() in the MOD_grid_initialization.F90"

        ! Secondary values (derived from erad)
        erad2 = erad * 2.0_r8       ! Twice Earth's radius
        erador5 = erad / sqrt(5.0_r8) ! Earth's radius divided by sqrt(5)
        eradi = 1.0_r8 / erad       ! Inverse of Earth's radius
        erad2sq = erad2 * erad2     ! Square of twice Earth's radius

        erad8 = 6371229.0_r8 
        erad2_r8 = erad8 * 2.0_r8
        eradi_r8 = 1.0_r8 / erad8
        erad2sq_r8 = erad2_r8 * erad2_r8

    end subroutine init_consts

    ! Initializes the base grid structure, typically an icosahedral grid.
    ! This subroutine performs several steps to set up the initial global grid:
    ! 1. Calls `icosahedron` to generate a base Delaunay triangular grid on a sphere.
    ! 2. Calls `voronoi` to compute the Voronoi dual of the Delaunay grid.
    ! 3. Calls `pcvt` (Lloyd's algorithm/iterations) to optimize the Voronoi cells towards Centroidal Voronoi Tessellation.
    ! 4. Allocates memory for grid arrays (`alloc_grid_lonlatmw`).
    ! 5. Computes grid geometry details (`grid_xyz2lonlat`).
    ! 6. Writes the initial grid to a file (`gridfile_write`).
    !===============================================================================
    subroutine gridinit()

        use consts_coms, only : io6, nxp                                         ! Constants (output unit, grid resolution parameter)
        use mem_delaunay, only : nmd, nud, nwd                                   ! Delaunay grid variables and routines
                                                                                ! nmd: number of Delaunay cells (triangles)
                                                                                ! nud: number of Delaunay unique edges
                                                                                ! nwd: number of Delaunay vertices
        use mem_grid, only : nma, nua, nva, nwa, alloc_grid_lonlatmw            ! Voronoi/Hex grid variables and allocation routines
                                                                                ! nma: number of Voronoi cells (polygons/hexagons)
                                                                                ! nua: number of Voronoi unique edges (same as nva)
                                                                                ! nva: number of Voronoi unique edges
                                                                                ! nwa: number of Voronoi vertices (same as nmd)
        implicit none

        ! Horizontal grid setup

        ! Now generate global atmospheric grid
        write(io6, '(/,a)') 'gridinit calling icosahedron'
        call icosahedron(nxp)  ! Generate global spherical Delaunay triangular grid; calls 2 allocs within
        write(io6, '(/,a)') 'gridinit after icosahedron'
        write(io6, '(a,i0)')    ' nmd (Delaunay triangles) = ', nmd
        write(io6, '(a,i0)')    ' nud (Delaunay unique edges) = ', nud
        write(io6, '(a,i0)')    ' nwd (Delaunay vertices) = ', nwd

        ! Compute Voronoi dual and optimize it
        call voronoi() ! Computes the Voronoi diagram from the Delaunay triangulation
                    ! After this, nma (Voronoi cells) = nwd (Delaunay vertices), etc.
        call pcvt()    ! Performs Lloyd's algorithm (PCVT iterations) to relax the Voronoi mesh
                    ! towards a Centroidal Voronoi Tessellation, making cells more regular.
        write(io6, '(/,a)') 'gridinit after voronoi and pcvt'
        write(io6, '(a,i8)')   ' nma (Voronoi cells) = ', nma
        write(io6, '(a,i8)')   ' nua (Voronoi unique edges A) = ', nua
        write(io6, '(a,i8)')   ' nwa (Voronoi vertices) = ', nwa

        ! Allocate remaining GRID FOOTPRINT arrays for full domain
        write(io6, '(/,a)') 'gridinit calling alloc_grid_lonlatmw for full domain'
        call alloc_grid_lonlatmw(nma, nva, nwa)

        ! Fill remaining GRID FOOTPRINT geometry for full domain
        write(io6, '(/,a)') 'gridinit calling grid_grid_xyz2lonlat'
        CALL grid_xyz2lonlat()

        ! Write GRIDFILE (and potentially SFCGRILE - surface grid file, though not explicitly shown here)
        write(io6, '(/,a)') 'gridinit calling gridfile_write'
        call gridfile_write() ! Writes the generated grid to a NetCDF file

        write(io6, '(/,a)') 'gridinit completed'

    end subroutine gridinit


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

    ! Writes the generated grid data to a NetCDF file.
    ! This subroutine takes the primary grid information (cell centers, vertex coordinates,
    ! and connectivity tables for both Delaunay triangles/polygons and Voronoi cells/vertices)
    ! and saves it into a NetCDF file. The filename includes NXP, step, and mode_grid.
    ! Note: The comments mention "sjx_points" (triangles/polygons from Delaunay-like structure)
    ! and "lbx_points" (vertices of these polygons, or centers of Voronoi cells).
    ! It appears to save the primary cell structure (e.g., triangles if `mode_grid` is `tri`,
    ! or polygons if `mode_grid` is `hex` after Voronoi generation).
    !===============================================================================
    subroutine gridfile_write()
        ! Does not calculate dismm and disww because they are not used.
        use netcdf
        use consts_coms, only : r8, pathlen, io6, file_dir, EXPNME, NXP, mode_grid, refine, step ! Global constants and parameters
        use mem_ijtabs, only : mloops, itab_m, itab_w                                            ! Neighbor tables for M (cell centers) and W (vertices) points
                                                                                                ! itab_m(im)%iw(1:3) are vertices of triangle 'im'
                                                                                                ! itab_w(iw)%im(1:7) are cells surrounding vertex 'iw' (up to 7 for hex)
        use mem_grid, only : nma, nwa, glatw, glonw, glatm, glonm                                ! Grid dimensions and coordinate arrays
                                                                                                ! nma: number of main cells (e.g., triangles or hexagons)
                                                                                                ! nwa: number of vertices
                                                                                                ! glonm, glatm: longitude/latitude of cell centers (M points)
                                                                                                ! glonw, glatw: longitude/latitude of cell vertices (W points)
        use mem_grid, only : xew, yew, zew
        use MOD_utilities, only : Unstructured_Mesh_Save                                         ! Subroutine to handle NetCDF writing

        implicit none

        ! This routine writes the grid variables to the gridfile.
        integer :: im, iw                           ! Loop counters for cells (im) and vertices (iw)
        integer :: sjx_points, lbx_points           ! Number of "sjx_points" (polygons/triangles) and "lbx_points" (vertices defining them)
                                                    ! Typically, sjx_points = nma, lbx_points = nwa for this context.
        real(r8), allocatable :: mp(:,:),wp(:,:)    ! Arrays to hold cell center (mp) and vertex (wp) coordinates (lon, lat)
        integer, allocatable :: ngrmw(:,:),ngrwm(:,:)! Connectivity arrays:
                                                    ! ngrmw: for each cell (M), its vertices (W)
                                                    ! ngrwm: for each vertex (W), its surrounding cells (M)
        integer, allocatable :: n_ngrwm(:)          ! Number of neighboring cells for each vertex (not explicitly used in Unstructured_Mesh_Save call shown)
        character(pathlen) :: lndname               ! Output NetCDF filename
        character(5) :: nxpc,stepc                  ! Character representation of NXP and step for filename

        ! Populate ngrmw: for each cell (M point, index im), list its vertices (W points)
        ! For a triangular mesh (mode_grid='tri'), each cell im has 3 vertices.
        ! For a hexagonal mesh (mode_grid='hex'), itab_m would store neighbors differently if it were primary.
        ! Here, it seems to assume a triangular primary structure (Delaunay like) where itab_m lists 3 vertices.
        allocate (ngrmw(3, nma)) ; ngrmw = 0
        do im = 1, nma
        ngrmw(1:3, im) = itab_m(im)%iw(1:3) ! Get the 3 vertices for triangle 'im'
        enddo

        ! Populate ngrwm: for each vertex (W point, index iw), list its surrounding cells (M points)
        ! A vertex can be shared by up to 7 cells in a hexagonal grid (less for boundaries/pentagons).
        
        allocate (ngrwm(7, nwa)) ; ngrwm = 0
        !@RuiZhang: should 7 be replace by itab_w(iw)%ngr ?
        ! itab_w(iw)%ngr may be 5 or 6 but we need 7
        do iw = 1, nwa
        ngrwm(1:7, iw) = itab_w(iw)%im(1:7) 
        !ngrwm(1:itab_w(iw)%ngr, iw) = itab_w(iw)%im(1:itab_w(iw)%ngr) ! Get surrounding cells for vertex 'iw'
                                                                    ! itab_w(iw)%ngr is the actual number of neighbors
        enddo

        ! add by Rui Zhang 20250604
        ! Populate n_ngrwm: the number of surrounding cells for each vertex
        allocate(n_ngrwm(nwa)); n_ngrwm = 6
        n_ngrwm(1) = 1
        do iw = 2, nwa
        if (ngrwm(6, iw) == 1) n_ngrwm(iw) = 5
        enddo

        ! Prepare coordinate arrays for saving
        allocate(mp(nma,2)); mp(:,1) = GLONM; mp(:,2) = GLATM ! Cell centers: (lon, lat)
        allocate(wp(nwa,2)); wp(:,1) = GLONW; wp(:,2) = GLATW ! Vertices: (lon, lat)

        ! Construct the output filename
        write(nxpc, '(I4.4)') NXP    ! Format NXP (e.g., 0030)
        write(stepc, '(I2.2)') step ! Format step (e.g., 01)
        ! Open gridfile
        lndname = trim(file_dir)// 'gridfile/gridfile_NXP' // trim(nxpc) // '_'//trim(stepc)// '_'// trim(mode_grid)// '.nc4'

        ! Print information and set point counts for saving
        sjx_points = nma ! Number of polygons/triangles (M points)
        lbx_points = nwa ! Number of vertices (W points)
        write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'
        write(io6, *) 'grid_write: opening file:', lndname
        write(io6, *) 'sjx_points (cells):', sjx_points
        write(io6, *) 'lbx_points (vertices):', lbx_points
        write(io6, *) '++++++++++++++++++++++++++++++++++++++++++++++++++++++'

        ! Save the unstructured mesh data to NetCDF
        ! Initial file without any refinement (step usually 1 here)
        CALL Unstructured_Mesh_Save(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)

        ! Deallocate temporary arrays
        deallocate(mp, wp, ngrmw, ngrwm, n_ngrwm)

    END SUBROUTINE gridfile_write

end module MOD_grid_initialization 
