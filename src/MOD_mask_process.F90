! Mask processing module
! This module contains subroutines for creating and processing
! different types of masks (domain, refine, patch)
module MOD_mask_process
    use consts_coms
    use MOD_utilities, only : CHECK
    implicit none
    
    public :: Mask_make, bbox_mask_make, lamb_mask_make, circle_mask_make, close_mask_make

contains

! Manages the creation of different types of masks (domain, refine, patch).
SUBROUTINE Mask_make(mask_select, type_select, mask_fprefix)
    use refine_vars     
    use netcdf          
    implicit none
    character(*), intent(in) :: mask_select   
    character(*), intent(in) :: type_select   
    character(*), intent(in) :: mask_fprefix  
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
        path = mask_fprefix(1:pos)       
        fprefix = mask_fprefix(pos+1:)   
        print *, 'path: ',    trim(adjustl(path))
        print *, 'fprefix: ', trim(adjustl(fprefix))
        fexists = .false. 

        lndname = trim(mask_select) // '_filelist.txt' 
        command = 'ls ' // trim(mask_fprefix) // '* > ' // trim(lndname) 
        call execute_command_line(command) 

        open(unit=111, file=lndname, status='old')
        REWIND(111)
        do while (.true.)
            read(111,'(A)', iostat=iostat) filename 
            write(io6, *) "iostat = ", iostat, "mask_select = ", mask_select
            if (iostat < 0) exit 
            if (iostat > 0) then
                print *, "File read error in Mask_make in mkgrd.F90"
                stop
            end if
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

        close(111)

        if (.not. fexists) stop 'No matching files found. Incorrect path format in mask_refine_fprefix.'
    else
        write(io6, *) "Incorrect path format in mask_fprefix: ", trim(mask_fprefix)
        stop
    end if

END SUBROUTINE Mask_make

! Creates a bounding box (bbox) mask.
subroutine bbox_mask_make(inputfile, mask_select)
    use netcdf
    use consts_coms,   only : r8, pathlen, file_dir, mask_domain_ndm, mask_patch_ndm
    use refine_vars,   only : mask_refine_ndm, max_iter_spc
    use MOD_utilities, only : bbox_Mesh_Save 

    implicit none
    character(pathlen), intent(in) :: inputfile   
    character(*), intent(in)       :: mask_select 
    integer :: ncid, varid                        
    character(pathlen) :: lndname, line           
    logical :: fexists                            
    integer :: i, bbox_num, refine_degree, length, io_stat
    real(r8), allocatable :: bbox_points(:,:)     
    character(5) :: numc, refinec                

    length = len_trim(inputfile) 

    if ('nml' == inputfile(length-2:length)) then 
        open(unit=10, file=inputfile, status='old', action='read')
        REWIND(10)
        read(10, '(A)', iostat=io_stat) line  
        if (io_stat /= 0) then
            print *, "Error: Failed to read bbox_num"
            stop
        end if
        read(line(index(line, '=')+1:), *) bbox_num  

        read(10, '(A)', iostat=io_stat) line  
        if (io_stat /= 0) then
            print *, "Error: Failed to read refine_degree for bbox" 
            stop
        end if
        read(line(index(line, '=')+1:), *) refine_degree  
        if (refine_degree > max_iter_spc) then 
            write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
            close(10)
            return 
        end if

        write(io6, *) "bbox_points must be 1.West 2.East 3.North and 4.South"
        allocate(bbox_points(bbox_num, 4)) 
        do i = 1, bbox_num, 1
            read(10, *) bbox_points(i, 1), bbox_points(i, 2), bbox_points(i, 3), bbox_points(i, 4) 
            if (bbox_points(i, 1) > bbox_points(i, 2)) stop "ERROR! bbox_points(i, 1) > bbox_points(i, 2) (West > East)"
            if (bbox_points(i, 3) < bbox_points(i, 4)) stop "ERROR! bbox_points(i, 3) < bbox_points(i, 4) (North < South)"
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
            mask_patch_ndm = mask_patch_ndm + 1 
            write(numc, '(I2.2)') mask_patch_ndm
        end if

        lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_bbox_'//trim(refinec)//'_'//trim(numc)//'.nc4'
        CALL bbox_Mesh_Save(lndname, bbox_num, bbox_points) 
        deallocate(bbox_points)

    else if (('.nc' == inputfile(length-2:length)) .or. &  
             ('nc4' == inputfile(length-2:length))) then
        CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid)) 
        CALL CHECK(NF90_INQ_VARID(ncid, 'bbox_refine', varid))    
        CALL CHECK(NF90_GET_VAR(ncid, varid, refine_degree))      
        CALL CHECK(NF90_CLOSE(ncid))                              
        if (refine_degree > max_iter_spc) then
            write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
            return 
        end if

        write(refinec, '(I1)') refine_degree
        if (mask_select == 'mask_domain') then
            mask_domain_ndm = mask_domain_ndm + 1
            write(numc, '(I2.2)') mask_domain_ndm
        else if (mask_select == 'mask_refine') then
            mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
            write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
        else if (mask_select == 'mask_patch') then
            mask_patch_ndm = mask_patch_ndm + 1
            write(numc, '(I2.2)') mask_patch_ndm
        end if

        lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_bbox_'//trim(refinec)//'_'//trim(numc)//'.nc4'
        CALL execute_command_line('cp '//trim(inputfile)//' '//trim(lndname)) 
    else
        stop "ERROR! must choose 'ncfile' or 'nml' please check!" 
    end if
    write(io6, *) lndname 

end subroutine bbox_mask_make

!-- @brief Creates a Lambert projection mask.
subroutine lamb_mask_make(inputfile, mask_select)
    use netcdf
    use consts_coms,   only : r8, pathlen, file_dir, mask_domain_ndm, mask_patch_ndm
    use refine_vars,   only : mask_refine_ndm, max_iter_spc
    use MOD_utilities, only : lambert_Mesh_Save 

    implicit none
    character(pathlen), intent(in) :: inputfile   
    character(*), intent(in)       :: mask_select 
    character(pathlen)             :: lndname       
    logical                        :: fexists       
    integer :: i, j, idx, length                  
    integer :: nlon_cal, nlat_cal                 
    integer :: ncid, dimID_lon, dimID_lat         
    integer :: lon_points, lat_points, bound_points, mode_points, refine_degree
    integer, dimension(10)         :: varid         
    real(r8), dimension(:, :), allocatable :: lon_vert, lat_vert, lonlat_bound
    integer,  dimension(:, :), allocatable :: ngr_bound     
    integer,  dimension(:),    allocatable :: n_ngr         
    character(5)                   :: nxpc, stepc, numc, refinec

    length = len_trim(inputfile) 

    if (('.nc' == inputfile(length-2:length))  .or. &  
        ('nc4' == inputfile(length-2:length))) then
        CALL CHECK(NF90_OPEN(inputfile, nf90_nowrite, ncid)) 
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
        lon_points = lon_points - 1 
        lat_points = lat_points - 1 
        where(lon_vert > 180.)  lon_vert = lon_vert - 360. 
    else
        stop "ERROR! must choose 'ncfile', please check!" 
    end if

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
    ngr_bound = ngr_bound + 1 

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
        mask_patch_ndm = mask_patch_ndm + 1
        write(numc, '(I2.2)') mask_patch_ndm
    end if
    lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_lambert_'//trim(refinec)//'_'//trim(numc)//'.nc4'
    write(io6, *) lndname
    CALL lambert_Mesh_Save(lndname, bound_points, mode_points, lonlat_bound, ngr_bound, n_ngr) 
    write(io6, *) "lamb_mask_make finish"

end subroutine lamb_mask_make

!-- @brief Creates a circular mask.
subroutine circle_mask_make(inputfile, mask_select)
    use netcdf
    use consts_coms, only : r8, pathlen, file_dir, mask_domain_ndm, mask_patch_ndm
    use refine_vars, only : mask_refine_ndm, max_iter_spc
    use MOD_utilities, only : circle_Mesh_Save 

    implicit none
    character(pathlen), intent(in) :: inputfile   
    character(*), intent(in)       :: mask_select 
    integer :: ncid, varid                        
    character(pathlen) :: lndname, line           
    logical :: fexists                            
    integer :: i, circle_num, refine_degree, length, io_stat
    real(r8), allocatable :: circle_points(:,:), circle_radius(:)
    character(5) :: numc, refinec                

    length = len_trim(inputfile) 

    if ('nml' == inputfile(length-2:length)) then 
        open(unit=10, file=inputfile, status='old', action='read')
        REWIND(10)
        read(10, '(A)', iostat=io_stat) line  
        if (io_stat /= 0) then
            print *, "Error: Failed to read circle_num"
            close(10)
            stop
        end if
        read(line(index(line, '=')+1:), *) circle_num  

        read(10, '(A)', iostat=io_stat) line  
        if (io_stat /= 0) then
            print *, "Error: Failed to read circle_refine" 
            close(10)
            stop
        end if
        read(line(index(line, '=')+1:), *) refine_degree  
        if (refine_degree > max_iter_spc) then
            write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
            close(10)
            return 
        end if

        allocate(circle_points(circle_num, 2)) 
        allocate(circle_radius(circle_num))   
        do i = 1, circle_num, 1
            read(10, *) circle_points(i,1), circle_points(i,2), circle_radius(i) 
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
            mask_patch_ndm = mask_patch_ndm + 1
            write(numc, '(I2.2)') mask_patch_ndm
        end if
        lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_circle_'//trim(refinec)//'_'//trim(numc)//'.nc4'
        CALL circle_Mesh_Save(lndname, circle_num, circle_points, circle_radius)
        deallocate(circle_points, circle_radius)

    else if (('.nc' == inputfile(length-2:length)) .or. &  
             ('nc4' == inputfile(length-2:length))) then
        CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid)) 
        CALL CHECK(NF90_INQ_VARID(ncid, 'circle_refine', varid))  
        CALL CHECK(NF90_GET_VAR(ncid, varid, refine_degree))    
        CALL CHECK(NF90_CLOSE(ncid))                            
        if (refine_degree > max_iter_spc) then
             write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
            return 
        end if

        write(refinec, '(I1)') refine_degree
        if (mask_select == 'mask_domain') then
            mask_domain_ndm = mask_domain_ndm + 1
            write(numc, '(I2.2)') mask_domain_ndm
        else if (mask_select == 'mask_refine') then
            mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
            write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
        else if (mask_select == 'mask_patch') then
            mask_patch_ndm = mask_patch_ndm + 1
            write(numc, '(I2.2)') mask_patch_ndm
        end if
        lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_circle_'//trim(refinec)//'_'//trim(numc)//'.nc4'
        CALL execute_command_line('cp '//trim(inputfile)//' '//trim(lndname)) 
    else
        stop "ERROR! must choose 'ncfile' or 'nml' please check!" 
    end if
   write(io6, *) lndname 

end subroutine circle_mask_make

!-- @brief Creates a closed polygon mask.
subroutine close_mask_make(inputfile, mask_select)
    use netcdf
    use consts_coms, only : r8, pathlen, file_dir, mask_domain_ndm, mask_patch_ndm
    use refine_vars, only : mask_refine_ndm, max_iter_spc
    use MOD_utilities, only : close_Mesh_Save 

    implicit none
    character(pathlen), intent(in) :: inputfile   
    character(*), intent(in)       :: mask_select 
    integer :: ncid, varid                        
    character(pathlen) :: lndname, line           
    logical :: fexists                            
    integer :: i, close_num, refine_degree, length, io_stat
    real(r8), allocatable :: close_points(:,:)     
    character(5) :: numc, refinec                

    length = len_trim(inputfile) 

    if ('nml' == inputfile(length-2:length)) then 
        open(unit=10, file=inputfile, status='old', action='read')
        REWIND(10)
        read(10, '(A)', iostat=io_stat) line  
        if (io_stat /= 0) then
            print *, "Error: Failed to read close_num" 
            close(10)
            stop
        end if
        read(line(index(line, '=')+1:), *) close_num  

        read(10, '(A)', iostat=io_stat) line  
        if (io_stat /= 0) then
            print *, "Error: Failed to read close_refine"
            close(10)
            stop
        end if
        read(line(index(line, '=')+1:), *) refine_degree  
        if (refine_degree > max_iter_spc) then
            write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
            close(10)
            return 
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
            mask_patch_ndm = mask_patch_ndm + 1
            write(numc, '(I2.2)') mask_patch_ndm
        end if
        lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_close_'//trim(refinec)//'_'//trim(numc)//'.nc4'
        CALL close_Mesh_Save(lndname, close_num, close_points)
        deallocate(close_points)

    else if (('.nc' == inputfile(length-2:length)) .or. &  
             ('nc4' == inputfile(length-2:length))) then
        CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid)) 
        CALL CHECK(NF90_INQ_VARID(ncid, 'close_refine', varid))   
        CALL CHECK(NF90_GET_VAR(ncid, varid, refine_degree))     
        CALL CHECK(NF90_CLOSE(ncid))                             
        if (refine_degree > max_iter_spc) then
            write(io6, *) "refine_degree > max_iter_spc :", refine_degree, ">", max_iter_spc
            return 
        end if

        write(refinec, '(I1)') refine_degree
        if (mask_select == 'mask_domain') then
            mask_domain_ndm = mask_domain_ndm + 1
            write(numc, '(I2.2)') mask_domain_ndm
        else if (mask_select == 'mask_refine') then
            mask_refine_ndm(refine_degree) = mask_refine_ndm(refine_degree) + 1
            write(numc, '(I2.2)') mask_refine_ndm(refine_degree)
        else if (mask_select == 'mask_patch') then
            mask_patch_ndm = mask_patch_ndm + 1
            write(numc, '(I2.2)') mask_patch_ndm
        end if
        lndname = trim(file_dir)// 'tmpfile/'//trim(mask_select)// '_close_'//trim(refinec)//'_'//trim(numc)//'.nc4'
        CALL execute_command_line('cp '//trim(inputfile)//' '//trim(lndname)) 
    else
        stop "ERROR! must choose 'ncfile' or 'nml' please check!" 
    end if
    write(io6, *) lndname 

end subroutine close_mask_make

end module MOD_mask_process 
