module MOD_file_preprocess
   ! 可以考虑把labdtype数据存在这里也是挺好的，还有一个问题
   USE consts_coms, only: r8, pathlen, nxp, piu180_r8, file_dir, mask_patch_on
   USE netcdf
   implicit none

   contains

   SUBROUTINE distsOnEdge_save(inputfile, num_edge, vp, distsOnEdge)

      IMPLICIT NONE
      character(pathlen), intent(in)  :: inputfile
      integer, intent(in) :: num_edge
      real(r8), allocatable, intent(in) :: vp(:,:), distsOnEdge(:)
      integer :: ncid, dimID_edge, varid(3)
 
      CALL CHECK(NF90_CREATE(trim(inputfile), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "num_edge", num_edge, DimID_edge))
      CALL CHECK(NF90_DEF_VAR(ncid, "lonv", NF90_DOUBLE, (/ DimID_edge /), varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncid, "latv", NF90_DOUBLE, (/ DimID_edge /), varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncid, "distsOnEdge", NF90_DOUBLE, (/ DimID_edge /), varid(3)))
      CALL CHECK(NF90_ENDDEF(ncid))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), vp(:,1)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2), vp(:,2)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3), distsOnEdge))
      CALL CHECK(NF90_CLOSE(ncid))
      
   END SUBROUTINE distsOnEdge_save

   SUBROUTINE data_read(inputfile, cellsOnEdge_reference, vp) ! verticesOnEdge, edgesOnVertex, vp

      IMPLICIT NONE
      character(pathlen), intent(in) :: inputfile
      integer :: ncid, dimID_sjx, dimID_edge
      integer :: varid(5), num_sjx, num_edge
      integer,  dimension(:, :), allocatable, intent(out) :: cellsOnEdge_reference
      real(r8), dimension(:, :), allocatable, intent(out), optional :: vp

      CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "nEdges", dimID_edge))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_edge, len = num_edge))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      num_edge = num_edge + 1
      allocate(cellsOnEdge_reference(2, num_edge)); cellsOnEdge_reference = 0
      allocate(vp(num_edge, 2)); vp = 0.
      CALL CHECK(NF90_INQ_VARID(ncid, 'cellsOnEdge', varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'lonEdge', varid(2)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'latEdge', varid(3)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), cellsOnEdge_reference(:, 2:num_edge)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(2), vp(2:num_edge, 1)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(3), vp(2:num_edge, 2)))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
      cellsOnEdge_reference = cellsOnEdge_reference + 1
      vp = vp * piu180_r8 
      where(vp(:, 1) > 180._r8) vp(:, 1) = vp(:, 1) - 360._r8
   
   END SUBROUTINE data_read

   SUBROUTINE FVCOM_Mesh_Read(inputfile, outputfile)
      ! change FVCOM mesh to EarthMesh 目前只支持读入的文件是nc文件，但是如果
      IMPLICIT NONE
      character(pathlen), intent(in) :: inputfile, outputfile
      integer :: i, ncid, dimID_sjx, dimID_lbx, dimID_maxelem
      integer :: spDimID, lpDimID, thDimID, seDimID
      integer, dimension(7) :: varid
      integer  :: sjx_points, lbx_points, maxelem
      integer,  dimension(:), allocatable :: n_ngrwm
      integer,  dimension(:, :), allocatable :: ngrmw, ngrwm, ngrwm_temp
      real(r8), dimension(:, :), allocatable :: mp, wp

      CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "maxelem", dimID_maxelem))
      CALL CHECK(NF90_INQ_DIMID(ncid, "node", dimID_lbx))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQ_DIMID(ncid, "nele", dimID_sjx))! 三个参数依次是：文件编号，NC文件中的维度名，维度ID
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_maxelem, len = maxelem))
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_sjx, len = sjx_points))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lbx, len = lbx_points))! 三个参数依次是：文件编号，维度ID，维度名
      sjx_points = sjx_points + 1
      lbx_points = lbx_points + 1
      allocate(mp(sjx_points, 2)); mp = 0.
      allocate(wp(lbx_points, 2)); wp = 0.            
      allocate(ngrmw(3, sjx_points)); ngrmw = 0
      allocate(ngrwm_temp(maxelem, lbx_points)); ngrwm_temp = 0 
      allocate(ngrwm(7, lbx_points)); ngrwm = 0    
      allocate(n_ngrwm( lbx_points)); n_ngrwm = 0

      CALL CHECK(NF90_INQ_VARID(ncid, 'lonc', varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'latc', varid(2)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'lon', varid(3)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'lat', varid(4)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'nv', varid(5))) ! 注意方向
      CALL CHECK(NF90_INQ_VARID(ncid, 'nbve', varid(6)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'ntve', varid(7)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), mp(2:sjx_points, 1)))! 6. NF90_GET_VAR：文件编号，变量ID，程序中的变量名
      CALL CHECK(NF90_GET_VAR(ncid, varid(2), mp(2:sjx_points, 2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(3), wp(2:lbx_points, 1)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(4), wp(2:lbx_points, 2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(5), ngrmw(:, 2:sjx_points)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(6), ngrwm_temp(:, 2:lbx_points)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(7), n_ngrwm(2:lbx_points)))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件

      ! from ngrwm_temp to ngrwm
      ngrmw = ngrmw + 1
      ngrwm = ngrwm_temp(1:7, :)
      ngrwm = ngrwm + 1

      where(mp(:, 1) > 180._r8) mp(:, 1) = mp(:, 1) - 360._r8
      where(wp(:, 1) > 180._r8) wp(:, 1) = wp(:, 1) - 360._r8
      where(mp(:, 1) <-180._r8) mp(:, 1) = mp(:, 1) + 360._r8
      where(wp(:, 1) <-180._r8) wp(:, 1) = wp(:, 1) + 360._r8

      ! 将数据保持保存为EarthMesh需要的形式
      CALL CHECK(NF90_CREATE(trim(outputfile), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "sjx_points", sjx_points, spDimID))
      CALL CHECK(NF90_DEF_DIM(ncid, "lbx_points", lbx_points, lpDimID))
      CALL CHECK(NF90_DEF_DIM(ncid, "dimb", 3, thDimID))
      CALL CHECK(NF90_DEF_DIM(ncid, "dimc", 7, seDimID))

      CALL CHECK(NF90_DEF_VAR(ncid, "GLONM", NF90_DOUBLE, (/ spDimID /), varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLATM", NF90_DOUBLE, (/ spDimID /), varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLONW", NF90_DOUBLE, (/ lpDimID /), varid(3)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLATW", NF90_DOUBLE, (/ lpDimID /), varid(4)))
      CALL CHECK(NF90_DEF_VAR(ncid, "itab_m%iw", NF90_INT, (/ thDimID, spDimID /), varid(5)))
      CALL CHECK(NF90_DEF_VAR(ncid, "itab_w%im", NF90_INT, (/ seDimID, lpDimID /), varid(6)))
      CALL CHECK(NF90_DEF_VAR(ncid, "n_ngrwm", NF90_INT, (/ lpDimID /), varid(7)))
      CALL CHECK(NF90_ENDDEF(ncid))

      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), mp(:, 1)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2), mp(:, 2)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3), wp(:, 1)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(4), wp(:, 2)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(5), ngrmw))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(6), ngrwm))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(7), n_ngrwm))
      CALL CHECK(NF90_CLOSE(ncid))
      deallocate(mp, wp, ngrmw, ngrwm, ngrwm_temp, n_ngrwm)

   END SUBROUTINE FVCOM_Mesh_Read

   SUBROUTINE FVCOM_Mesh_Save(ustr_points, ustr_bounds, ustr_vertex, ustr_ngr_center)
      ! 生成用于FVCOMmesh常用的2dm形式，以及对应的dat形式才ok！！！
      IMPLICIT NONE
      integer, intent(in) :: ustr_points, ustr_bounds
      real(r8), allocatable, intent(in) :: ustr_vertex(:, :)
      integer,  allocatable, intent(in) :: ustr_ngr_center(:, :)
      integer :: i, j, k
      integer :: ncid, varid, dimID_bdy, bdy_num
      integer,  allocatable :: obc_order(:), obc_order_use(:)
      character(pathlen) :: filename, lndname
      integer :: unit_number = 10

      ! 读取obc数据
      lndname = trim(file_dir) // 'result/obc.nc4'
      if (mask_patch_on) lndname = trim(file_dir) // 'result/obc_patch.nc4'
      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid))
      CALL CHECK(NF90_INQ_DIMID(ncid, "bdy_num", dimID_bdy))
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_bdy, len = bdy_num))
      allocate(obc_order(bdy_num))
      CALL CHECK(NF90_INQ_VARID(ncid, "obc_order", varid))
      CALL CHECK(NF90_GET_VAR(ncid, varid, obc_order))
      CALL CHECK(NF90_CLOSE(ncid))

      if (obc_order(bdy_num) /= 1) then
         allocate(obc_order_use(bdy_num+1))
         obc_order_use(1:bdy_num) = obc_order
         obc_order_use(1+bdy_num) = 1
         bdy_num = bdy_num + 1
      else
         allocate(obc_order_use(bdy_num))
         obc_order_use = obc_order
      end if

      ! 打开文件
      open(unit=unit_number, file=trim(file_dir)// 'result/fvcom.2dm', status='replace', action='write')

      ! 写入数据
      write(unit_number, '(A)') 'MESH2D'
      write(unit_number, '(A)') 'MESHNAME "FVCOM Mesh"'
      
      ! 写入 E3T 数据
      do i = 2, ustr_points, 1
         write(unit_number, '(A, 5(1X, I0))') 'E3T', i - 1, &
                              ustr_ngr_center(1, i) - 1, &
                              ustr_ngr_center(2, i) - 1, &
                              ustr_ngr_center(3, i) - 1, 1
      end do

      ! 写入 ND 数据 ! 默认水深为0
      do i = 2, ustr_bounds, 1
         write(unit_number, '(A, 1X, I0, 3(1X, F0.6))') 'ND', i - 1, &
                              ustr_vertex(i, 1), ustr_vertex(i, 2), 0.0
      end do        

      ! 写入 NS 数据
      ! 当指定 advance='no' 时，write 语句在输出完成后不会换行
      j = 0 ! 标记obc段的个数
      k = 0 ! 标记在NS中一行的哪一个位置
      do i = 2, bdy_num - 1, 1
         if (obc_order_use(i) == 1) cycle ! jump  because -1 before !!!!!!!
         if (mod(k, 10) == 0) then
               if (i < bdy_num) write(unit_number, '(A, 1X)', advance='no') 'NS'
         end if
         if (obc_order_use(i+1) == 1) then
               j = j + 1
               ! 负数并标记 obc 段号
               write(unit_number, '(I0, 1X, I0)', advance='yes') -obc_order_use(i) + 1, j
               k = 0
         else
               k = k + 1 ! 每一行写诗歌最多
               write(unit_number, '(I0, 1X)', advance='no') obc_order_use(i) - 1
         end if

      end do     

      ! 关闭文件
      close(unit_number)

   END SUBROUTINE FVCOM_Mesh_Save

   SUBROUTINE IAP_Mesh_Read(inputfile, sjx_points, lbx_points, wp, ngrmm, ngrmw)
      IMPLICIT NONE
      character(pathlen), intent(in) :: inputfile
      integer :: i, ncid, dimID_sjx, dimID_lbx
      integer, dimension(4) :: varid
      integer, intent(out)  :: sjx_points, lbx_points
      integer,  dimension(:, :), allocatable, intent(out) :: ngrmm, ngrmw
      real(r8), dimension(:, :), allocatable, intent(out) :: wp

      CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "sjx_points", dimID_sjx))! 三个参数依次是：文件编号，NC文件中的维度名
      CALL CHECK(NF90_INQ_DIMID(ncid, "lbx_points", dimID_lbx))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_sjx, len = sjx_points))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lbx, len = lbx_points))! 三个参数依次是：文件编号，维度ID，维度名
      sjx_points = sjx_points + 1
      lbx_points = lbx_points + 1
      allocate(wp(lbx_points, 2)); wp = 0.            
      allocate(ngrmm(3, sjx_points)); ngrmm = 0
      allocate(ngrmw(3, sjx_points)); ngrmw = 0

      CALL CHECK(NF90_INQ_VARID(ncid, 'GLONW', varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'GLATW', varid(2)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'itab_m%im', varid(3)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'itab_m%iw', varid(4)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), wp(2:lbx_points, 1)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(2), wp(2:lbx_points, 2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(3), ngrmm(:, 2:sjx_points)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(4), ngrmw(:, 2:sjx_points)))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
      
      ngrmm = ngrmm + 1
      ngrmw = ngrmw + 1

      wp = wp * piu180_r8
      where(wp(:, 1) > 180._r8) wp(:, 1) = wp(:, 1) - 360._r8
      where(wp(:, 1) <-180._r8) wp(:, 1) = wp(:, 1) + 360._r8

   END SUBROUTINE IAP_Mesh_Read

   ! 这里应该修改为如果找不到对应的变量就提示！！！
   SUBROUTINE MPAS_Mesh_Read(inputfile, outputfile)
      ! change MAPS mesh to EarthMesh 需要需要先检查是否是MPAS格式的文件
      USE consts_coms, only : io6
      IMPLICIT NONE
      character(pathlen), intent(in) :: inputfile, outputfile
      integer :: i, ncid, dimID_sjx, dimID_lbx, DimID_maxEdges
      integer :: spDimID, lpDimID, thDimID, seDimID
      integer :: varid(7), maxnum
      integer  :: sjx_points, lbx_points, maxEdges
      integer,  dimension(:), allocatable :: n_ngrwm
      integer,  dimension(:, :), allocatable :: ngrmw_temp, ngrwm_temp
      integer,  dimension(:, :), allocatable :: ngrmw, ngrwm
      real(r8), dimension(:, :), allocatable :: mp, wp
      
      CALL CHECK(NF90_OPEN(trim(inputfile), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "nVertices", dimID_sjx))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQ_DIMID(ncid, "nCells", dimID_lbx))! 三个参数依次是：文件编号，NC文件中的维度名，维度ID
      CALL CHECK(NF90_INQ_DIMID(ncid, "maxEdges", DimID_maxEdges))
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_sjx, len = sjx_points))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lbx, len = lbx_points))! 三个参数依次是：文件编号，维度ID，维度名
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_maxEdges, len = maxEdges))
      print*, "sjx_points = ", sjx_points
      print*, "lbx_points = ", lbx_points
      print*, "maxEdges   = ", maxEdges
      sjx_points = sjx_points + 1
      lbx_points = lbx_points + 1
      allocate(mp(sjx_points, 2)); mp(1, :) = 0.
      allocate(wp(lbx_points, 2)); wp(1, :) = 0.            
      allocate(ngrmw_temp(3, sjx_points-1))
      allocate(ngrwm_temp(maxEdges, lbx_points-1))
      allocate(n_ngrwm( lbx_points)); n_ngrwm(1) = 1

      CALL CHECK(NF90_INQ_VARID(ncid, 'lonVertex', varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'latVertex', varid(2)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'lonCell', varid(3)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'latCell', varid(4)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'cellsOnVertex', varid(5)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'verticesOnCell', varid(6)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'nEdgesOnCell', varid(7)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), mp(2:sjx_points, 1)))! 6. NF90_GET_VAR：文件编号，变量ID，程序中的变量名
      CALL CHECK(NF90_GET_VAR(ncid, varid(2), mp(2:sjx_points, 2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(3), wp(2:lbx_points, 1)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(4), wp(2:lbx_points, 2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(5), ngrmw_temp))
      CALL CHECK(NF90_GET_VAR(ncid, varid(6), ngrwm_temp))
      CALL CHECK(NF90_GET_VAR(ncid, varid(7), n_ngrwm(2:lbx_points)))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
      
      ! from ngrwm_temp to ngrwm from ngrmw_temp to ngrmw
      maxnum = int(max(7, maxval(n_ngrwm)))
      allocate(ngrmw(3, sjx_points)); ngrmw = 0
      allocate(ngrwm(maxnum, lbx_points)); ngrwm = 0
      do i = 2, sjx_points, 1
         ngrmw(1:3, i) = ngrmw_temp(1:3, i-1)
      end do
      do i = 2, lbx_points, 1
         ngrwm(1:maxnum, i) = ngrwm_temp(1:maxnum, i-1)
      end do
      ngrmw = ngrmw + 1
      ngrwm = ngrwm + 1

      ! 从弧度转为角度，而且注意调制经纬度范围，确保在-180到180，90到-90之间
      mp = mp * piu180_r8 ! * 57.295779513
      wp = wp * piu180_r8
      where(mp(:, 1) > 180._r8) mp(:, 1) = mp(:, 1) - 360._r8
      where(wp(:, 1) > 180._r8) wp(:, 1) = wp(:, 1) - 360._r8
      where(mp(:, 1) <-180._r8) mp(:, 1) = mp(:, 1) + 360._r8
      where(wp(:, 1) <-180._r8) wp(:, 1) = wp(:, 1) + 360._r8
      if (int((sjx_points-1)/20) /= int(nxp*nxp)) write(io6, *), "nxp read from namelist diff from nxp in the mode_file"

      ! 将数据保持保存为EarthMesh需要的形式
      CALL CHECK(NF90_CREATE(trim(outputfile), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "sjx_points", sjx_points, spDimID))
      CALL CHECK(NF90_DEF_DIM(ncid, "lbx_points", lbx_points, lpDimID))
      CALL CHECK(NF90_DEF_DIM(ncid, "dimb", 3, thDimID))
      CALL CHECK(NF90_DEF_DIM(ncid, "dimc", maxnum, seDimID))

      CALL CHECK(NF90_DEF_VAR(ncid, "GLONM", NF90_DOUBLE, (/ spDimID /), varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLATM", NF90_DOUBLE, (/ spDimID /), varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLONW", NF90_DOUBLE, (/ lpDimID /), varid(3)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLATW", NF90_DOUBLE, (/ lpDimID /), varid(4)))
      CALL CHECK(NF90_DEF_VAR(ncid, "itab_m%iw", NF90_INT, (/ thDimID, spDimID /), varid(5)))
      CALL CHECK(NF90_DEF_VAR(ncid, "itab_w%im", NF90_INT, (/ seDimID, lpDimID /), varid(6)))
      CALL CHECK(NF90_DEF_VAR(ncid, "n_ngrwm", NF90_INT, (/ lpDimID /), varid(7)))
      CALL CHECK(NF90_ENDDEF(ncid))

      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), mp(:, 1)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2), mp(:, 2)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3), wp(:, 1)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(4), wp(:, 2)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(5), ngrmw))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(6), ngrwm))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(7), n_ngrwm))
      CALL CHECK(NF90_CLOSE(ncid))
      deallocate(mp, wp, ngrmw, ngrwm, n_ngrwm, ngrmw_temp, ngrwm_temp)

   END SUBROUTINE MPAS_Mesh_Read

   SUBROUTINE MPAS_Mesh_Save(lndname, num_sjx, num_dbx, num_edge, latCell, lonCell, xCell, yCell, zCell, &
            latVertex, lonVertex, xVertex, yVertex, zVertex, latEdge, lonEdge, xEdge, yEdge, zEdge, &
            nEdgesOnCell, cellsOnCell, verticesOnCell, edgesOnCell, cellsOnVertex, edgesOnVertex, &
            cellsOnEdge, verticesOnEdge, nEdgesOnEdge, edgesOnEdge, areaCell, areaTriangle, kiteAreasOnVertex, dvEdge, dcEdge, &
            angleEdge, weightsOnEdge, meshDensity, nominalMinDc, error_segment)
      ! 输出MPAS所需要的文件格式change EarthMesh to MAPS mesh 
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer, intent(in)  :: num_sjx, num_dbx, num_edge

      ! 定义维度大小
      integer, parameter :: maxEdges = 10
      integer, parameter :: maxEdges2 = 20
      integer, parameter :: TWO = 2
      integer, parameter :: vertexDegree = 3

      ! netcdf变量ID和文件ID
      integer :: ncid, varid(38)
      integer :: DimID_nCells, DimID_nVertices, DimID_nEdges
      integer :: DimID_maxEdges, DimID_maxEdges2, DimID_TWO, DimID_vertexDegree

      ! 定义变量（这里只是声明，实际数据需要填充）
      integer, allocatable :: indexToCellID(:)
      integer, allocatable  :: indexToVertexID(:)
      integer, allocatable  :: indexToEdgeID(:)

      real(r8), intent(in) :: latCell(:), lonCell(:), xCell(:), yCell(:), zCell(:)
      real(r8), intent(in) :: latVertex(:), lonVertex(:), xVertex(:), yVertex(:), zVertex(:)
      real(r8), intent(in) :: latEdge(:), lonEdge(:), xEdge(:), yEdge(:), zEdge(:)

      integer, intent(in) :: nEdgesOnCell(:) ! yes
      integer, intent(in) :: cellsOnCell(:, :) ! yes
      integer, intent(in) :: verticesOnCell(:, :) ! yes
      integer, intent(in) :: edgesOnCell(:, :) ! yes

      integer, intent(in) :: cellsOnVertex(:, :) ! yes
      integer, intent(in) :: edgesOnVertex(:, :) ! yes

      integer, intent(in) :: cellsOnEdge(:, :) ! yes
      integer, intent(in) :: verticesOnEdge(:, :) ! yes
      integer, intent(in) :: nEdgesOnEdge(:) ! yes
      integer, intent(in) :: edgesOnEdge(:, :) ! yes

      real(r8), intent(in) :: areaCell(:), areaTriangle(:) ! yes
      real(r8), intent(in) :: kiteAreasOnVertex(:, :) ! yes 但是偏差还是比较大
      real(r8), intent(in) :: dvEdge(:), dcEdge(:) ! yes
      real(r8), intent(in) :: angleEdge(:) ! 计算出来了，但是还没有验证是否正确

      real(r8), intent(in) :: weightsOnEdge(:, :) ! 这个不知道是如何确定的，而且这个很重要
      real(r8), intent(in) :: meshDensity(:) ! 这个不知道是如何确定的
      real(r8), intent(in) :: nominalMinDc ! yes ! 但是有一定的疑问，可以问一问赵纯老师
      real(r8), intent(in) :: error_segment(:)

      allocate(indexToCellID(num_dbx-1));   indexToCellID = [1:num_dbx-1]
      allocate(indexToVertexID(num_sjx-1)); indexToVertexID = [1:num_sjx-1]
      allocate(indexToEdgeID(num_edge-1));   indexToEdgeID = [1:num_edge-1]
      
      ! 创建NetCDF文件和定义维度
      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "nCells", num_dbx-1, DimID_nCells))
      CALL CHECK(NF90_DEF_DIM(ncid, "nVertices", num_sjx-1, DimID_nVertices))
      CALL CHECK(NF90_DEF_DIM(ncid, "nEdges", num_edge-1, DimID_nEdges))
      CALL CHECK(NF90_DEF_DIM(ncid, "maxEdges", maxEdges, DimID_maxEdges))
      CALL CHECK(NF90_DEF_DIM(ncid, "maxEdges2", maxEdges2, DimID_maxEdges2))
      CALL CHECK(NF90_DEF_DIM(ncid, "TWO", TWO, DimID_TWO))
      CALL CHECK(NF90_DEF_DIM(ncid, "vertexDegree", vertexDegree, DimID_vertexDegree))
      write(6, *) "NF90_DEF_DIM finish"

      ! 定义变量及其属性
      ! 1. indexToCellID
      CALL CHECK(NF90_DEF_VAR(ncid, "indexToCellID", NF90_INT, (/ DimID_nCells /), varid(1)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(1), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(1), 'long_name', 'Mapping from local array index to global cell ID'))
      
      ! 2. latCell
      CALL CHECK(NF90_DEF_VAR(ncid, "latCell", NF90_DOUBLE, (/ DimID_nCells /), varid(2)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(2), 'units', 'rad'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(2), 'long_name', 'Latitude of cells'))
      
      ! 3. lonCell
      CALL CHECK(NF90_DEF_VAR(ncid, "lonCell", NF90_DOUBLE, (/ DimID_nCells /), varid(3)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(3), 'units', 'rad'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(3), 'long_name', 'Longitude of cells'))
      
      ! 4. xCell
      CALL CHECK(NF90_DEF_VAR(ncid, "xCell", NF90_DOUBLE, (/ DimID_nCells /), varid(4)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(4), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(4), 'long_name', 'Cartesian x-coordinate of cells'))

      ! 5. yCell
      CALL CHECK(NF90_DEF_VAR(ncid, "yCell", NF90_DOUBLE, (/ DimID_nCells /), varid(5)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(5), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(5), 'long_name', 'Cartesian y-coordinate of cells'))

      ! 6. zCell
      CALL CHECK(NF90_DEF_VAR(ncid, "zCell", NF90_DOUBLE, (/ DimID_nCells /), varid(6)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(6), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(6), 'long_name', 'Cartesian z-coordinate of cells'))

      ! 7. indexToVertexID
      CALL CHECK(NF90_DEF_VAR(ncid, "indexToVertexID", NF90_INT, (/ DimID_nVertices /), varid(7)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(7), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(7), 'long_name', 'Mapping from local array index to global vertex ID'))

      ! 8. latVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "latVertex", NF90_DOUBLE, (/ DimID_nVertices /), varid(8)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(8), 'units', 'rad'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(8), 'long_name', 'Latitude of vertices'))

      ! 9. lonVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "lonVertex", NF90_DOUBLE, (/ DimID_nVertices /), varid(9)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(9), 'units', 'rad'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(9), 'long_name', 'Longitude of vertices'))

      ! 10. xVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "xVertex", NF90_DOUBLE, (/ DimID_nVertices /), varid(10)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(10), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(10), 'long_name', 'Cartesian x-coordinate of cells'))

      ! 11. yVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "yVertex", NF90_DOUBLE, (/ DimID_nVertices /), varid(11)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(11), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(11), 'long_name', 'Cartesian y-coordinate of cells'))

      ! 12. zVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "zVertex", NF90_DOUBLE, (/ DimID_nVertices /), varid(12)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(12), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(12), 'long_name', 'Cartesian z-coordinate of cells'))

      ! 13. indexToEdgeID
      CALL CHECK(NF90_DEF_VAR(ncid, "indexToEdgeID", NF90_INT, (/ DimID_nEdges /), varid(13)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(13), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(13), 'long_name', 'Mapping from local array index to global edge ID'))

      ! 14. latEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "latEdge", NF90_DOUBLE, (/ DimID_nEdges /), varid(14)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(14), 'units', 'rad'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(14), 'long_name', 'Latitude of vertices'))

      ! 15. lonEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "lonEdge", NF90_DOUBLE, (/ DimID_nEdges /), varid(15)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(15), 'units', 'rad'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(15), 'long_name', 'Longitude of vertices'))

      ! 16. xEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "xEdge", NF90_DOUBLE, (/ DimID_nEdges /), varid(16)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(16), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(16), 'long_name', 'Cartesian x-coordinate of cells'))

      ! 17. yEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "yEdge", NF90_DOUBLE, (/ DimID_nEdges /), varid(17)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(17), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(17), 'long_name', 'Cartesian y-coordinate of cells'))

      ! 18. zEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "zEdge", NF90_DOUBLE, (/ DimID_nEdges /), varid(18)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(18), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(18), 'long_name', 'Cartesian z-coordinate of cells'))

      ! 19. nEdgesOnCell
      CALL CHECK(NF90_DEF_VAR(ncid, "nEdgesOnCell", NF90_INT, (/ DimID_nCells /), varid(19)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(19), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(19), 'long_name', 'Number of edges forming the boundary of a cell'))

      ! 20. cellsOnCell
      CALL CHECK(NF90_DEF_VAR(ncid, "cellsOnCell", NF90_INT, (/ dimid_maxEdges, DimID_nCells /), varid(20)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(20), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(20), 'long_name', 'IDs of cells neighboring a cell'))
      
      ! 21. verticesOnCell
      CALL CHECK(NF90_DEF_VAR(ncid, "verticesOnCell", NF90_INT, (/ dimid_maxEdges, DimID_nCells /), varid(21)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(21), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(21), 'long_name', 'IDs of vertices (corner points) of a cell'))

      ! 22. edgesOnCell
      CALL CHECK(NF90_DEF_VAR(ncid, "edgesOnCell", NF90_INT, (/ dimid_maxEdges, DimID_nCells /), varid(22)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(22), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(22), 'long_name', 'IDs of edges forming the boundary of a cell'))
      
      ! 23. cellsOnVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "cellsOnVertex", NF90_INT, (/ dimid_vertexDegree, dimid_nVertices /), varid(23)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(23), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(23), 'long_name', 'IDs of the cells that meet at a vertex'))

      ! 24. edgesOnVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "edgesOnVertex", NF90_INT, (/ dimid_vertexDegree, dimid_nVertices /), varid(24)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(24), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(24), 'long_name', 'IDs of the edges that meet at a vertex'))

      ! 25. cellsOnEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "cellsOnEdge", NF90_INT, (/ dimid_TWO, dimid_nEdges /), varid(25)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(25), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(25), 'long_name', 'IDs of cells divided by an edge'))
      
      ! 26. verticesOnEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "verticesOnEdge", NF90_INT, (/ dimid_TWO, dimid_nEdges /), varid(26)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(26), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(26), 'long_name', 'IDs of the two vertex endpoints of an edge'))
      
      ! 27. nEdgesOnEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "nEdgesOnEdge", NF90_INT, (/ dimid_nEdges /), varid(27)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(27), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(27), 'long_name', 'Number of edges involved in reconstruction of tangential velocity for an edge'))
      
      ! 28. edgesOnEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "edgesOnEdge", NF90_INT, (/ dimid_maxEdges2, dimid_nEdges /), varid(28)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(28), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(28), 'long_name', 'IDs of edges involved in reconstruction of tangential velocity for an edge'))
      
      ! 29. areaCell
      CALL CHECK(NF90_DEF_VAR(ncid, "areaCell", NF90_DOUBLE, (/ dimid_nCells /), varid(29)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(29), 'units', 'm^2'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(29), 'long_name', 'Spherical area of a Voronoi cell'))

      ! 30. areaTriangle
      CALL CHECK(NF90_DEF_VAR(ncid, "areaTriangle", NF90_DOUBLE, (/ dimid_nVertices /), varid(30)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(30), 'units', 'm^2'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(30), 'long_name', 'Spherical area of a Voronoi cell'))
      
      ! 31. kiteAreasOnVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "kiteAreasOnVertex", NF90_DOUBLE, (/ dimid_vertexDegree, dimid_nVertices /), varid(31)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(31), 'units', 'm^2'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(31), 'long_name', 'Intersection areas between primal (Voronoi) and dual (triangular) mesh cells'))
      
      ! 32. dvEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "dvEdge", NF90_DOUBLE, (/ dimid_nEdges /), varid(32)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(32), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(32), 'long_name', 'Spherical distance between vertex endpoints of an edge'))
      
      ! 33. dcEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "dcEdge", NF90_DOUBLE, (/ dimid_nEdges /), varid(33)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(33), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(33), 'long_name', 'Spherical distance between cells separated by an edge'))
      
      ! 34. angleEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "angleEdge", NF90_DOUBLE, (/ dimid_nEdges /), varid(34)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(34), 'units', 'rad'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(34), 'long_name', 'Angle between local north and the positive tangential direction of an edge'))

      ! 35. weightsOnEdge
      CALL CHECK(NF90_DEF_VAR(ncid, "weightsOnEdge", NF90_DOUBLE, (/ dimid_maxEdges2, dimid_nEdges /), varid(35)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(35), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(35), 'long_name', 'Weights used in reconstruction of tangential velocity for an edge'))

      ! 36. meshDensity
      CALL CHECK(NF90_DEF_VAR(ncid, "meshDensity", NF90_DOUBLE, (/ dimid_nCells /), varid(36)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(36), 'units', 'unitless'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(36), 'long_name', 'Mesh density function (used when generating the mesh) evaluated at a cell'))

      ! 37. nominalMinDc
      CALL CHECK(NF90_DEF_VAR(ncid, "nominalMinDc", NF90_DOUBLE, varid(37)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(37), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(37), 'long_name', 'Nominal minimum dcEdge value where meshDensity == 1.0'))

      ! 38. error_segment
      CALL CHECK(NF90_DEF_VAR(ncid, "error_segment", NF90_DOUBLE, (/ dimid_nEdges /), varid(38)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(38), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(38), 'long_name', 'error in reconstruction of tangential velocity for an edge'))
      write(6, *) "NF90_DEF_VAR and  NF90_PUT_ATT finish"

      ! 添加全局属性
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "mesh_spec", "1.0")) ! 这个是否有影响？？
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "on_a_sphere", "YES"))
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "sphere_radius", 1.0))
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "is_periodic", "NO"))
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "x_period", 0.0))
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "y_period", 0.0))
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "file_id", "bbdd9043")) ! 这个是否有影响？？
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "source", "Generated by EarthMesh"))
      CALL CHECK(NF90_ENDDEF(ncid))
      write(6, *) "NF90_PUT_att finish"

      ! 变量放置
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1),  indexToCellID))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2),  latCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3),  lonCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(4),  xCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(5),  yCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(6),  zCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(7),  indexToVertexID))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(8),  latVertex(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(9),  lonVertex(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(10), xVertex(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(11), yVertex(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(12), zVertex(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(13), indexToEdgeID))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(14), latEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(15), lonEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(16), xEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(17), yEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(18), zEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(19), nEdgesOnCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(20), cellsOnCell(:, 2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(21), verticesOnCell(:, 2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(22), edgesOnCell(:, 2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(23), cellsOnVertex(:, 2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(24), edgesOnVertex(:, 2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(25), cellsOnEdge(:, 2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(26), verticesOnEdge(:, 2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(27), nEdgesOnEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(28), edgesOnEdge(:, 2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(29), areaCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(30), areaTriangle(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(31), kiteAreasOnVertex(:, 2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(32), dvEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(33), dcEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(34), angleEdge(2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(35), weightsOnEdge(:, 2:num_edge)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(36), meshDensity(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(37), nominalMinDc))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(38), error_segment(2:num_edge)))
      CALL CHECK(NF90_CLOSE(ncid))
      write(6, *) "NF90_PUT_VAR finish"
      deallocate(indexToCellID, indexToVertexID, indexToEdgeID)

   END SUBROUTINE MPAS_Mesh_Save

   ! refer to MPAS-Tools/mesh_tools/hex_projection/ph_utils.f90 in https://github.com/MPAS-Dev/MPAS-Tools
   SUBROUTINE MPAS_info_Save(lndname, maxEdges, nCells, nEdges, cellsOnCell, cellsOnEdge, nEdgesOnCell)

      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer, intent(in) :: maxEdges, nCells, nEdges
      integer, dimension(maxEdges,nCells), intent(in) :: cellsOnCell
      integer, dimension(nCells), intent(in) :: nEdgesOnCell
      integer, dimension(2,nEdges), intent(in) :: cellsOnEdge
      integer :: iCell, iEdge, nout, edgeCells, nEdgesInterior
      integer, dimension(maxEdges) :: cellsOut

      write(6,*) " creating graph.info file "
      open(unit=10, file=trim(lndname), form="formatted", status="replace")

      write(6,*) " min and max cellsOnEdge ",minval(cellsOnEdge(:,2:nEdges)),maxval(cellsOnEdge(:,2:nEdges))
      nEdgesInterior = 0
      do iEdge = 2, nEdges, 1
         if ((cellsOnEdge(1,iEdge).ne.0) .and. (cellsOnEdge(2,iEdge).ne.0)) nEdgesInterior = nEdgesInterior + 1
      end do
      write(6,*) " nEdges, nEdgesInterior ",nEdges-1,nEdgesInterior

      write(10,"(2i10)") nCells-1, nEdgesInterior

      edgeCells = 0
      do iCell = 2, nCells, 1
         nout = 0
         do iEdge = 1, nEdgesOnCell(iCell), 1
            if (cellsOnCell(iEdge, iCell) .gt. 0) then
               nout = nout + 1
               cellsOut(nout) = cellsOnCell(iEdge, iCell)
            end if
         end do
         write(10,"(10i10)") cellsOut(1:nout)
         if(nout .lt. nEdgesOnCell(iCell)) edgeCells = edgeCells+1
      end do

      close(unit=10)

      write(6,*) ' finished creating graph.info file, edgeCells = ',edgeCells

   END SUBROUTINE MPAS_info_Save

   SUBROUTINE MPAS_Mesh_Simple_Save(lndname, num_sjx, num_dbx, xCell, yCell, zCell, xVertex, yVertex, zVertex, cellsOnVertex, meshDensity)
      USE consts_coms, only: io6
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer, intent(in)  :: num_sjx, num_dbx

      ! 定义变量（这里只是声明，实际数据需要填充）
      real(r8), intent(in) :: xCell(:), yCell(:), zCell(:)
      real(r8), intent(in) :: xVertex(:), yVertex(:), zVertex(:)
      integer, intent(in) :: cellsOnVertex(:, :) ! yes
      real(r8), intent(in) :: meshDensity(:) ! 这个不知道是如何确定的

      ! netcdf变量ID和文件ID
      integer, parameter :: vertexDegree = 3
      integer :: ncid, varid(8)
      integer :: DimID_nCells, DimID_nVertices
      integer :: DimID_vertexDegree
         
      ! 创建NetCDF文件和定义维度
      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "nCells", num_dbx-1, DimID_nCells))
      CALL CHECK(NF90_DEF_DIM(ncid, "nVertices", num_sjx-1, DimID_nVertices))
      CALL CHECK(NF90_DEF_DIM(ncid, "vertexDegree", vertexDegree, DimID_vertexDegree))
      write(io6, *) "NF90_DEF_DIM finish"

      ! 定义变量及其属性
      ! 1. xCell
      CALL CHECK(NF90_DEF_VAR(ncid, "xCell", NF90_DOUBLE, (/ DimID_nCells /), varid(1)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(1), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(1), 'long_name', 'Cartesian x-coordinate of cells'))

      ! 2. yCell
      CALL CHECK(NF90_DEF_VAR(ncid, "yCell", NF90_DOUBLE, (/ DimID_nCells /), varid(2)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(2), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(2), 'long_name', 'Cartesian y-coordinate of cells'))

      ! 3. zCell
      CALL CHECK(NF90_DEF_VAR(ncid, "zCell", NF90_DOUBLE, (/ DimID_nCells /), varid(3)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(3), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(3), 'long_name', 'Cartesian z-coordinate of cells'))

      ! 4. xVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "xVertex", NF90_DOUBLE, (/ DimID_nVertices /), varid(4)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(4), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(4), 'long_name', 'Cartesian x-coordinate of cells'))

      ! 5. yVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "yVertex", NF90_DOUBLE, (/ DimID_nVertices /), varid(5)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(5), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(5), 'long_name', 'Cartesian y-coordinate of cells'))

      ! 6. zVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "zVertex", NF90_DOUBLE, (/ DimID_nVertices /), varid(6)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(6), 'units', 'm'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(6), 'long_name', 'Cartesian z-coordinate of cells'))
      
      ! 7. cellsOnVertex
      CALL CHECK(NF90_DEF_VAR(ncid, "cellsOnVertex", NF90_INT, (/ dimid_vertexDegree, dimid_nVertices /), varid(7)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(7), 'units', '-'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(7), 'long_name', 'IDs of the cells that meet at a vertex'))

      ! 8. meshDensity
      CALL CHECK(NF90_DEF_VAR(ncid, "meshDensity", NF90_DOUBLE, (/ dimid_nCells /), varid(8)))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(8), 'units', 'unitless'))
      CALL CHECK(NF90_PUT_ATT(ncid, varid(8), 'long_name', 'Mesh density function (used when generating the mesh) evaluated at a cell'))


      ! 添加全局属性
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "on_a_sphere", "YES"))
      CALL CHECK(nf90_put_att(ncid, NF90_GLOBAL, "sphere_radius", 1.0))
      CALL CHECK(NF90_ENDDEF(ncid))
      write(io6, *) "NF90_PUT_att finish"

      ! 变量放置
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1),  xCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2),  yCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3),  zCell(2:num_dbx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(4), xVertex(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(5), yVertex(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(6), zVertex(2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(7), cellsOnVertex(:, 2:num_sjx)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(8), meshDensity(2:num_dbx)))
      CALL CHECK(NF90_CLOSE(ncid))
      write(io6, *) "NF90_PUT_VAR finish"

      CALL CellWidthVsXY_Save()

   END SUBROUTINE MPAS_Mesh_Simple_Save

   ! save nlons_xy, nlats_xy, gridx, gridy, cellWidthVsXY
   SUBROUTINE CellWidthVsXY_Save()
      USE MOD_data_preprocess, only : nlons_xy, nlats_xy, gridx, gridy, cellWidthVsXY
      USE consts_coms, only: io6
      IMPLICIT NONE
      character(pathlen) :: lndname
      integer :: ncid, varid(3)
      integer :: lonDimID, latDimID

      lndname = trim(file_dir) // "result/CellWidthVsXY.nc4"
      write(io6, *), lndname
      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncID, "nlons_xy", nlons_xy, lonDimID))
      CALL CHECK(NF90_DEF_DIM(ncID, "nlats_xy", nlats_xy, latDimID))
      CALL CHECK(NF90_DEF_VAR(ncID, "gridx", NF90_DOUBLE, (/ lonDimID /), Varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncID, "gridy", NF90_DOUBLE, (/ latDimID /), Varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncID, "CellWidthVsXY", NF90_DOUBLE, (/ lonDimID, latDimID /), Varid(3)))
      CALL CHECK(NF90_ENDDEF(ncID))
      CALL CHECK(NF90_PUT_VAR(ncID, varid(1), gridx)) ! 含有非全细化三角形的多边形编号
      CALL CHECK(NF90_PUT_VAR(ncID, varid(2), gridy)) ! 含有非全细化三角形的多边形编号
      CALL CHECK(NF90_PUT_VAR(ncID, varid(3), CellWidthVsXY)) ! 含有非全细化三角形的多边形编号
      CALL CHECK(NF90_CLOSE(ncID))

   END SUBROUTINE CellWidthVsXY_Save

   SUBROUTINE Mode4_Mesh_Read(lndname, bound_points, mode_points, lonlat_bound, ngr_bound, n_ngr)
      IMPLICIT NONE
      integer :: ncid, dimID_bound, dimID_points
      integer, dimension(3) :: varid
      character(pathlen), intent(in) :: lndname
      integer,  intent(out) :: bound_points, mode_points
      real(r8), dimension(:, :), allocatable, intent(out) :: lonlat_bound
      integer,  dimension(:, :), allocatable, intent(out) :: ngr_bound
      integer,  dimension(:),    allocatable, intent(out) :: n_ngr
      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "bound_points", dimID_bound))
      CALL CHECK(NF90_INQ_DIMID(ncid, "mode_points", dimID_points))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_bound,  len = bound_points))
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_points, len = mode_points))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      allocate(lonlat_bound(bound_points, 2))
      allocate(ngr_bound(4, mode_points))
      allocate(n_ngr(mode_points))
      CALL CHECK(NF90_INQ_VARID(ncid, 'lonlat_bound', varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'ngr_bound', varid(2)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'n_ngr', varid(3)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), lonlat_bound))
      CALL CHECK(NF90_GET_VAR(ncid, varid(2), ngr_bound))
      CALL CHECK(NF90_GET_VAR(ncid, varid(3), n_ngr))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
  
   END SUBROUTINE Mode4_Mesh_Read

   SUBROUTINE Mode4_Mesh_Save(lndname, bound_points, mode_points, lonlat_bound, ngr_bound, n_ngr)

      IMPLICIT NONE
      
      character(pathlen), intent(in) :: lndname
      integer :: ncid, dimID_bound, dimID_points, dimID_two, dimID_four, varid(3)
      integer, intent(in) :: bound_points, mode_points
      real(r8), dimension(:, :), allocatable, intent(in) :: lonlat_bound
      integer,  dimension(:, :), allocatable, intent(in) :: ngr_bound
      integer,  dimension(:),    allocatable, intent(in) :: n_ngr
 
      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "bound_points", bound_points, DimID_bound))
      CALL CHECK(NF90_DEF_DIM(ncid, "mode_points", mode_points, DimID_points))
      CALL CHECK(NF90_DEF_DIM(ncid, "two", 2, DimID_two))
      CALL CHECK(NF90_DEF_DIM(ncid, "four", 4, DimID_four))
      CALL CHECK(NF90_DEF_VAR(ncid, "lonlat_bound", NF90_DOUBLE, (/ DimID_bound, DimID_two /), varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncid, "ngr_bound", NF90_INT, (/ DimID_four, DimID_points /), varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncid, "n_ngr", NF90_INT, (/ DimID_points /), varid(3)))
      CALL CHECK(NF90_ENDDEF(ncid))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), lonlat_bound))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2), ngr_bound))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3), n_ngr))
      CALL CHECK(NF90_CLOSE(ncid))
      print*, "mode4 mesh save finish"

   END SUBROUTINE Mode4_Mesh_Save

   SUBROUTINE Unstructured_Mesh_Read(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)
      
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer,  intent(out) :: sjx_points, lbx_points
      integer,  dimension(:), allocatable, intent(out) :: n_ngrwm
      integer,  dimension(:, :), allocatable, intent(out) :: ngrmw, ngrwm
      real(r8), dimension(:, :), allocatable, intent(out) :: mp, wp
      integer :: ncid, dimID_sjx, dimID_lbx, dimID_edge, DimID_c, maxnum
      integer, dimension(7) :: varid

      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "sjx_points", dimID_sjx))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQ_DIMID(ncid, "lbx_points", dimID_lbx))! 三个参数依次是：文件编号，NC文件中的维度名，维度ID
      CALL CHECK(NF90_INQ_DIMID(ncid, "dimc", DimID_c))! 三个参数依次是：文件编号，NC文件中的维度名，维度ID
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_sjx, len = sjx_points))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, dimID_lbx, len = lbx_points))! 三个参数依次是：文件编号，维度ID，维度名
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, DimID_c, len = maxnum))! 三个参数依次是：文件编号，维度ID，维度名

      allocate(mp(sjx_points, 2))
      allocate(wp(lbx_points, 2))            
      allocate(ngrmw(3, sjx_points))
      allocate(ngrwm(maxnum, lbx_points))      
      allocate(n_ngrwm( lbx_points)) ! 4. ALLOCATE分配动态数组

      CALL CHECK(NF90_INQ_VARID(ncid, 'GLONM', varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'GLATM', varid(2)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'GLONW', varid(3)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'GLATW', varid(4)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'itab_m%iw', varid(5)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'itab_w%im', varid(6)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'n_ngrwm', varid(7)))

      CALL CHECK(NF90_GET_VAR(ncid, varid(1), mp(:, 1)))! 6. NF90_GET_VAR：文件编号，变量ID，程序中的变量名
      CALL CHECK(NF90_GET_VAR(ncid, varid(2), mp(:, 2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(3), wp(:, 1)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(4), wp(:, 2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(5), ngrmw))
      CALL CHECK(NF90_GET_VAR(ncid, varid(6), ngrwm))
      CALL CHECK(NF90_GET_VAR(ncid, varid(7), n_ngrwm))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
  
   END SUBROUTINE Unstructured_Mesh_Read

   SUBROUTINE Unstructured_Mesh_Save(lndname, sjx_points, lbx_points, mp, wp, ngrmw, ngrwm, n_ngrwm)

      IMPLICIT NONE
      character(LEN = 256), intent(in) :: lndname
      integer,  intent(in) :: sjx_points, lbx_points
      real(r8), dimension(:, :), allocatable, intent(in) :: mp, wp
      integer,  dimension(:, :), allocatable, intent(in) :: ngrmw, ngrwm
      integer,  dimension(:),    intent(in), optional :: n_ngrwm
      integer :: i, j, ncid, spDimID, lpDimID, thDimID, seDimID, maxnum
      integer, dimension(7) :: varid

      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "sjx_points", sjx_points, spDimID))
      CALL CHECK(NF90_DEF_DIM(ncid, "lbx_points", lbx_points, lpDimID))
      CALL CHECK(NF90_DEF_DIM(ncid, "dimb", 3, thDimID))
      if (present(n_ngrwm)) then
         maxnum = maxval(n_ngrwm)
         if (maxnum < 7) maxnum = 7
         CALL CHECK(NF90_DEF_DIM(ncid, "dimc", maxnum, seDimID))
      else
         CALL CHECK(NF90_DEF_DIM(ncid, "dimc", 7, seDimID))
      end if

      CALL CHECK(NF90_DEF_VAR(ncid, "GLONM", NF90_DOUBLE, (/ spDimID /), varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLATM", NF90_DOUBLE, (/ spDimID /), varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLONW", NF90_DOUBLE, (/ lpDimID /), varid(3)))
      CALL CHECK(NF90_DEF_VAR(ncid, "GLATW", NF90_DOUBLE, (/ lpDimID /), varid(4)))
      CALL CHECK(NF90_DEF_VAR(ncid, "itab_m%iw", NF90_INT, (/ thDimID, spDimID /), varid(5)))
      CALL CHECK(NF90_DEF_VAR(ncid, "itab_w%im", NF90_INT, (/ seDimID, lpDimID /), varid(6)))
      if (present(n_ngrwm)) CALL CHECK(NF90_DEF_VAR(ncid, "n_ngrwm", NF90_INT, (/ lpDimID /), varid(7)))
      CALL CHECK(NF90_ENDDEF(ncid))

      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), mp(1:sjx_points, 1)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2), mp(1:sjx_points, 2)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3), wp(1:lbx_points, 1)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(4), wp(1:lbx_points, 2)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(5), ngrmw(:, 1:sjx_points)))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(6), ngrwm(:, 1:lbx_points)))
      if (present(n_ngrwm)) CALL CHECK(NF90_PUT_VAR(ncid, varid(7), n_ngrwm))
      CALL CHECK(NF90_CLOSE(ncid))

   END SUBROUTINE Unstructured_Mesh_Save

   SUBROUTINE bbox_Mesh_Read(lndname, bbox_num, bbox_points)
      ! 读取bboxmesh数据
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer,  intent(out) :: bbox_num
      real(r8), allocatable, intent(out) :: bbox_points(:,:)
      integer :: ncid, DimID_num, varid(2)

      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "bbox_num", DimID_num))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, DimID_num, len = bbox_num))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      allocate(bbox_points(bbox_num, 4)); bbox_points = 0.
      CALL CHECK(NF90_INQ_VARID(ncid, 'bbox_points',   varid(1)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), bbox_points))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_bbox关闭文件
      print*, "bbox mesh read finish"

   END SUBROUTINE bbox_Mesh_Read

   SUBROUTINE bbox_Mesh_Save(lndname, bbox_num, bbox_points)
      ! 存储bboxmesh数据
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer,  intent(in) :: bbox_num
      real(r8), allocatable, intent(in) :: bbox_points(:,:)
      integer :: ncid, DimID_num, DimID_four, varid(2)

      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "bbox_num", bbox_num, DimID_num))
      CALL CHECK(NF90_DEF_DIM(ncid, "four", 4, DimID_four))
      CALL CHECK(NF90_DEF_VAR(ncid, "bbox_points", NF90_DOUBLE, (/ DimID_num, DimID_four /), varid(1)))
      CALL CHECK(NF90_ENDDEF(ncid))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), bbox_points))
      CALL CHECK(NF90_CLOSE(ncid))
      print*, "bbox mesh save finish"

   END SUBROUTINE bbox_Mesh_Save

   SUBROUTINE circle_Mesh_Read(lndname, circle_num, circle_points, circle_radius)
      ! 读取circlemesh数据
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer,  intent(out) :: circle_num
      real(r8), allocatable, intent(out) :: circle_points(:,:), circle_radius(:)
      integer :: ncid, DimID_num, varid(3)

      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "circle_num", DimID_num))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, DimID_num, len = circle_num))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      
      allocate(circle_points(circle_num, 2)); circle_points = 0.
      allocate(circle_radius(circle_num));    circle_radius = 0.

      CALL CHECK(NF90_INQ_VARID(ncid, 'circle_points',   varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'circle_radius',   varid(2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), circle_points))
      CALL CHECK(NF90_GET_VAR(ncid, varid(2), circle_radius))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
      print*, "circle mesh read finish"

   END SUBROUTINE circle_Mesh_Read

   SUBROUTINE circle_Mesh_Save(lndname, circle_num, circle_points, circle_radius)
      ! 存储circlemesh数据
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer,  intent(in) :: circle_num
      real(r8), allocatable, intent(in) :: circle_points(:,:), circle_radius(:)
      integer :: ncid, DimID_num, DimID_two, varid(3)

      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "circle_num", circle_num, DimID_num))
      CALL CHECK(NF90_DEF_DIM(ncid, "two", 2, DimID_two))
      CALL CHECK(NF90_DEF_VAR(ncid, "circle_points", NF90_DOUBLE, (/ DimID_num, DimID_two /), varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncid, "circle_radius", NF90_DOUBLE, (/ DimID_num/), varid(2)))
      CALL CHECK(NF90_ENDDEF(ncid))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), circle_points))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2), circle_radius))
      CALL CHECK(NF90_CLOSE(ncid))
      print*, "circle mesh save finish"

   END SUBROUTINE circle_Mesh_Save

   SUBROUTINE close_Mesh_Read(lndname, close_num, close_points)
      ! 读取closemesh数据
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer,  intent(out) :: close_num
      real(r8), allocatable, intent(out) :: close_points(:,:)
      integer :: ncid, DimID_num, varid(2)

      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "close_num", DimID_num))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, DimID_num, len = close_num))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      allocate(close_points(close_num, 2)); close_points = 0.

      CALL CHECK(NF90_INQ_VARID(ncid, 'close_points',   varid(1)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), close_points))
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件
      print*, "close mesh read finish"

   END SUBROUTINE close_Mesh_Read

   SUBROUTINE close_Mesh_Save(lndname, close_num, close_points)
      ! 存储closemesh数据
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer,  intent(in) :: close_num
      real(r8), allocatable, intent(in) :: close_points(:,:)
      integer :: ncid, DimID_num, DimID_two, varid(2)

      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "close_num", close_num, DimID_num))
      CALL CHECK(NF90_DEF_DIM(ncid, "two", 2, DimID_two))
      CALL CHECK(NF90_DEF_VAR(ncid, "close_points", NF90_DOUBLE, (/ DimID_num, DimID_two /), varid(1)))
      CALL CHECK(NF90_ENDDEF(ncid))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), close_points))
      CALL CHECK(NF90_CLOSE(ncid))
      print*, "close mesh save finish"

   END SUBROUTINE close_Mesh_Save

   ! 这个是应该只有三角形的情况，还需要检查一些
   SUBROUTINE Contain_Read(lndname, num_ustr, num_ii, ustr_id, ustr_ii, IsInArea_ustr)

      IMPLICIT NONE

      character(pathlen), intent(in) :: lndname
      integer,  intent(out) :: num_ustr, num_ii
      integer,  dimension(:,:), allocatable, intent(out) :: ustr_id, ustr_ii
      integer,  dimension(:),   allocatable, intent(out), optional :: IsInArea_ustr
      integer :: ncid, Dim_ustrID, Dim_iiID, Dim_aID, Dim_bID
      integer :: dim_a_len, dim_b_len
      integer :: varid(3)

      CALL CHECK(NF90_OPEN(trim(lndname), nf90_nowrite, ncid))! 1. NF90_OPEN 打开文件 
      CALL CHECK(NF90_INQ_DIMID(ncid, "num_ustr", Dim_ustrID))! 2. NF90_INQ_DIMID获取维度ID 
      CALL CHECK(NF90_INQ_DIMID(ncid, "num_ii", Dim_iiID))! 三个参数依次是：文件编号，NC文件中的维度名，维度ID
      CALL CHECK(NF90_INQ_DIMID(ncid, "dim_a", Dim_aID))! 三个参数依次是：文件编号，NC文件中的维度名，维度ID
      CALL CHECK(NF90_INQ_DIMID(ncid, "dim_b", Dim_bID))! 三个参数依次是：文件编号，NC文件中的维度名，维度ID
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, Dim_ustrID, len = num_ustr))! 3. NF90_INQUIRE_DIMENSION获取各维度的长度
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, Dim_iiID, len = num_ii))! 三个参数依次是：文件编号，维度ID，维度名
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, Dim_aID, len = dim_a_len))! 三个参数依次是：文件编号，维度ID，维度名
      CALL CHECK(NF90_INQUIRE_DIMENSION(ncid, Dim_bID, len = dim_b_len))! 三个参数依次是：文件编号，维度ID，维度名
      allocate(ustr_id(num_ustr, dim_a_len)) ! 4. ALLOCATE分配动态数组         
      allocate(ustr_ii(num_ii, dim_b_len))     
      CALL CHECK(NF90_INQ_VARID(ncid, 'ustr_id',   varid(1)))
      CALL CHECK(NF90_INQ_VARID(ncid, 'ustr_ii',   varid(2)))
      CALL CHECK(NF90_GET_VAR(ncid, varid(1), ustr_id))
      CALL CHECK(NF90_GET_VAR(ncid, varid(2), ustr_ii))
      if (present(IsInArea_ustr)) then
         allocate(IsInArea_ustr(num_ustr))
         CALL CHECK(NF90_INQ_VARID(ncid, 'IsInArea_ustr',   varid(3)))
         CALL CHECK(NF90_GET_VAR(ncid, varid(3), IsInArea_ustr))
      end if
      CALL CHECK(NF90_CLOSE(ncid))! 7. NF90_CLOSE关闭文件

   END SUBROUTINE Contain_Read

   SUBROUTINE Contain_Save(lndname, num_ustr, num_ii, ustr_id, ustr_ii, IsInArea_ustr)
      IMPLICIT NONE

      character(pathlen), intent(in) :: lndname
      integer, intent(in) :: num_ustr, num_ii
      integer,  dimension(:,:), allocatable, intent(in) :: ustr_id, ustr_ii
      integer,  dimension(:), allocatable, intent(in), optional :: IsInArea_ustr
      integer :: ncid, Dim_ustrID, Dim_iiID, Dim_aID, Dim_bID
      integer :: dim_a_len, dim_b_len
      integer :: varid(3)

      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "num_ustr", num_ustr, Dim_ustrID))
      CALL CHECK(NF90_DEF_DIM(ncid, "num_ii", num_ii, Dim_iiID))
      dim_a_len = size(ustr_id, 2) ! 因为陆地网格有两列，海洋网格有三列, 海陆网格有四列
      dim_b_len = size(ustr_ii, 2) ! 因为陆地和海洋网格有两列，但是海陆网格有四列
      CALL CHECK(NF90_DEF_DIM(ncid, "dim_a", dim_a_len, Dim_aID)) ! *_id
      CALL CHECK(NF90_DEF_DIM(ncid, "dim_b", dim_b_len, Dim_bID))
      CALL CHECK(NF90_DEF_VAR(ncid, 'ustr_id', NF90_INT, (/ Dim_ustrID, Dim_aID /), varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncid, 'ustr_ii', NF90_INT, (/ Dim_iiID, Dim_bID /), varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncid, 'IsInArea_ustr', NF90_INT, (/ Dim_ustrID /), varid(3)))
      CALL CHECK(NF90_ENDDEF(ncid))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), ustr_id))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2), ustr_ii))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3), IsInArea_ustr))
      CALL CHECK(NF90_CLOSE(ncid))

   END SUBROUTINE Contain_Save

   SUBROUTINE LOCmesh_info_save(lndname, num_step, num_ustr, num_step_f, refine_degree_f, seaorland_ustr_f)
      
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer, intent(in) :: num_step, num_ustr
      integer,  dimension(:), allocatable, intent(in) :: num_step_f, refine_degree_f, seaorland_ustr_f
      integer :: ncid, Dim_stepID, Dim_ustrID
      integer :: varid(3)

      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncid))
      CALL CHECK(NF90_DEF_DIM(ncid, "num_step", num_step, Dim_stepID))
      CALL CHECK(NF90_DEF_DIM(ncid, "num_ustr", num_ustr, Dim_ustrID))
      CALL CHECK(NF90_DEF_VAR(ncid, 'num_step_f', NF90_INT, (/ Dim_stepID /), varid(1)))
      CALL CHECK(NF90_DEF_VAR(ncid, 'refine_degree_f', NF90_INT, (/ Dim_ustrID /), varid(2)))
      CALL CHECK(NF90_DEF_VAR(ncid, 'seaorland_ustr_f', NF90_INT, (/ Dim_ustrID /), varid(3)))
      CALL CHECK(NF90_ENDDEF(ncid))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(1), num_step_f))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(2), refine_degree_f))
      CALL CHECK(NF90_PUT_VAR(ncid, varid(3), seaorland_ustr_f))
      CALL CHECK(NF90_CLOSE(ncid))

   END SUBROUTINE LOCmesh_info_save

   SUBROUTINE quality_save_global(lndname, num_sjx, num_wbx, num_lbx, num_qbx, & 
      length_sjx, angle_sjx, Extr_sjx, Eavg_sjx, Savg_sjx, less_sjx, more_sjx, & 
      length_wbx, angle_wbx, Extr_wbx, Eavg_wbx, Savg_wbx, less_wbx, more_wbx, & 
      length_lbx, angle_lbx, Extr_lbx, Eavg_lbx, Savg_lbx, less_lbx, more_lbx, &
      length_qbx, angle_qbx, Extr_qbx, Eavg_qbx, Savg_qbx, less_qbx, more_qbx)
      IMPLICIT NONE
      character(pathlen), intent(in) :: lndname
      integer,  intent(in) :: num_sjx, num_wbx, num_lbx, num_qbx
      integer,  intent(in) :: less_sjx(num_sjx), more_sjx(num_sjx), less_wbx(num_wbx), more_wbx(num_wbx), less_lbx(num_lbx), more_lbx(num_lbx)
      real(r8), intent(in) :: length_sjx(num_sjx, 3), angle_sjx(num_sjx, 3), Extr_sjx(2), Eavg_sjx(2), Savg_sjx
      real(r8), intent(in) :: length_wbx(num_wbx, 5), angle_wbx(num_wbx, 5), Extr_wbx(2), Eavg_wbx(2), Savg_wbx
      real(r8), intent(in) :: length_lbx(num_lbx, 6), angle_lbx(num_lbx, 6), Extr_lbx(2), Eavg_lbx(2), Savg_lbx
      real(r8), intent(in), optional :: length_qbx(num_qbx, 7), angle_qbx(num_qbx, 7), Extr_qbx(2), Eavg_qbx(2), Savg_qbx
      integer,  intent(in), optional :: less_qbx(num_qbx), more_qbx(num_qbx)
      integer :: ncVarID(28), ncid
      integer :: DimID_sjx, DimID_wbx, DimID_lbx, DimID_qbx
      integer :: DimID_two, DimID_thr, DimID_fiv, DimID_six, DimID_sev

      CALL CHECK(NF90_CREATE(trim(lndname), ior(nf90_clobber, nf90_netcdf4), ncID))
      CALL CHECK(NF90_DEF_DIM(ncID, "num_sjx", num_sjx, DimID_sjx))
      CALL CHECK(NF90_DEF_DIM(ncID, "num_wbx", num_wbx, DimID_wbx))
      CALL CHECK(NF90_DEF_DIM(ncID, "num_lbx", num_lbx, DimID_lbx))
      CALL CHECK(NF90_DEF_DIM(ncID, "two", 2, DimID_two))
      CALL CHECK(NF90_DEF_DIM(ncID, "thr", 3, DimID_thr))
      CALL CHECK(NF90_DEF_DIM(ncID, "fiv", 5, DimID_fiv))
      CALL CHECK(NF90_DEF_DIM(ncID, "six", 6, DimID_six))

      CALL CHECK(NF90_DEF_VAR(ncID, "length_sjx", NF90_DOUBLE, (/ DimID_sjx, DimID_thr /), ncVarID(1)))
      CALL CHECK(NF90_DEF_VAR(ncID, "angle_sjx",  NF90_DOUBLE, (/ DimID_sjx, DimID_thr /), ncVarID(2)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Extr_sjx", NF90_DOUBLE, (/ DimID_two /), ncVarID(3)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Eavg_sjx", NF90_DOUBLE, (/ DimID_two /), ncVarID(4)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Savg_sjx", NF90_DOUBLE, ncVarID(5)))
      CALL CHECK(NF90_DEF_VAR(ncID, "less_sjx",   NF90_INT, (/ DimID_sjx /), ncVarID(6)))
      CALL CHECK(NF90_DEF_VAR(ncID, "more_sjx",   NF90_INT, (/ DimID_sjx /), ncVarID(7)))

      CALL CHECK(NF90_DEF_VAR(ncID, "length_wbx", NF90_DOUBLE, (/ DimID_wbx, DimID_fiv /), ncVarID(8)))
      CALL CHECK(NF90_DEF_VAR(ncID, "angle_wbx",  NF90_DOUBLE, (/ DimID_wbx, DimID_fiv /), ncVarID(9)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Extr_wbx", NF90_DOUBLE, (/ DimID_two /), ncVarID(10)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Eavg_wbx", NF90_DOUBLE, (/ DimID_two /), ncVarID(11)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Savg_wbx", NF90_DOUBLE, ncVarID(12)))
      CALL CHECK(NF90_DEF_VAR(ncID, "less_wbx",   NF90_INT, (/ DimID_wbx /), ncVarID(13)))
      CALL CHECK(NF90_DEF_VAR(ncID, "more_wbx",   NF90_INT, (/ DimID_wbx /), ncVarID(14)))

      CALL CHECK(NF90_DEF_VAR(ncID, "length_lbx", NF90_DOUBLE, (/ DimID_lbx, DimID_six /), ncVarID(15)))
      CALL CHECK(NF90_DEF_VAR(ncID, "angle_lbx",  NF90_DOUBLE, (/ DimID_lbx, DimID_six /), ncVarID(16)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Extr_lbx", NF90_DOUBLE, (/ DimID_two /), ncVarID(17)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Eavg_lbx", NF90_DOUBLE, (/ DimID_two /), ncVarID(18)))
      CALL CHECK(NF90_DEF_VAR(ncID, "Savg_lbx", NF90_DOUBLE, ncVarID(19)))
      CALL CHECK(NF90_DEF_VAR(ncID, "less_lbx",   NF90_INT, (/ DimID_lbx /), ncVarID(20)))
      CALL CHECK(NF90_DEF_VAR(ncID, "more_lbx",   NF90_INT, (/ DimID_lbx /), ncVarID(21)))

      if (num_qbx /= 0) then
         CALL CHECK(NF90_DEF_DIM(ncID, "num_qbx", num_qbx, DimID_qbx))
         CALL CHECK(NF90_DEF_DIM(ncID, "sev", 7, DimID_sev))
         CALL CHECK(NF90_DEF_VAR(ncID, "length_qbx", NF90_DOUBLE, (/ DimID_qbx, DimID_sev /), ncVarID(22)))
         CALL CHECK(NF90_DEF_VAR(ncID, "angle_qbx",  NF90_DOUBLE, (/ DimID_qbx, DimID_sev /), ncVarID(23)))
         CALL CHECK(NF90_DEF_VAR(ncID, "Extr_qbx", NF90_DOUBLE, (/ DimID_two /), ncVarID(24)))
         CALL CHECK(NF90_DEF_VAR(ncID, "Eavg_qbx", NF90_DOUBLE, (/ DimID_two /), ncVarID(25)))
         CALL CHECK(NF90_DEF_VAR(ncID, "Savg_qbx", NF90_DOUBLE, ncVarID(26)))
         CALL CHECK(NF90_DEF_VAR(ncID, "less_qbx",   NF90_INT, (/ DimID_qbx /), ncVarID(27)))
         CALL CHECK(NF90_DEF_VAR(ncID, "more_qbx",   NF90_INT, (/ DimID_qbx /), ncVarID(28)))
      end if
      CALL CHECK(NF90_ENDDEF(ncID))

      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(1),  length_sjx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(2),  angle_sjx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(3),  Extr_sjx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(4),  Eavg_sjx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(5),  Savg_sjx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(6),  less_sjx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(7),  more_sjx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(8),  length_wbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(9),  angle_wbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(10), Extr_wbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(11), Eavg_wbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(12), Savg_wbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(13), less_wbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(14), more_wbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(15), length_lbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(16), angle_lbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(17), Extr_lbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(18), Eavg_lbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(19), Savg_lbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(20), less_lbx))
      CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(21), more_lbx))
      if (num_qbx /= 0) then
         CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(22), length_qbx))
         CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(23), angle_qbx))
         CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(24), Extr_qbx))
         CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(25), Eavg_qbx))
         CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(26), Savg_qbx))
         CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(27), less_qbx))
         CALL CHECK(NF90_PUT_VAR(ncID, ncVarID(28), more_qbx))
      end if
      CALL CHECK(NF90_CLOSE(ncID))

   END SUBROUTINE quality_save_global

END Module MOD_file_preprocess
