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

subroutine icosahedron(nxp0)

  use mem_ijtabs,   only: jtm_grid, jtu_grid, jtv_grid, jtw_grid, &
                          jtm_init, jtu_init, jtv_init, jtw_init, &
                          jtm_prog, jtu_prog, jtv_prog, jtw_prog, &
                          jtm_wadj, jtu_wadj, jtv_wadj, jtw_wadj, &
                          jtm_wstn, jtu_wstn, jtv_wstn, jtw_wstn, &
                          jtm_lbcp, jtu_lbcp, jtv_lbcp, jtw_lbcp, &
                          jtm_vadj, jtu_wall, jtv_wall, jtw_vadj

  use mem_delaunay, only: itab_md, itab_ud, itab_wd, alloc_itabsd, &
                          xemd, yemd, zemd, nmd, nud, nwd

  ! use mem_grid,     only: impent
  use consts_coms,  only: pi2, erad, erador5, impent
  implicit none
  integer, intent(in) :: nxp0

  real, parameter :: pwrd = 0.9  ! 0.9 is close to making uniform-sized triangles
   !real, parameter :: pwrd = 1.0  ! 1.0 is original value

  integer :: ibigd,i,j,idiamond,im_left,iu0,iu1,iu2,iu3,iu4,iw1,iw2,im &
     ,idiamond_top,im_top,im_right,im_bot,nn10,idiamond_right,idiamond_bot &
     ,iu,iw
  integer :: id
 
  real :: wts,wtn,wtw,wte,expansion,anglen,anglew,anglee,angles,wtw0,wte0,sumwt

  integer, parameter :: ibigd_ne(10) = [ 6,7,8,9,10,7,8,9,10,6 ]
  integer, parameter :: ibigd_se(10) = [ 2,3,4,5, 1,2,3,4, 5,1 ]

  real, dimension(10) :: xed_s,xed_n,xed_w,xed_e, &
                         yed_s,yed_n,yed_w,yed_e, &
                         zed_s,zed_n,zed_w,zed_e

  ! Define triangles, edges, and vertices for icosahedral faces and subdivisions

  ! For now, use nxp0 to divide each face

  nn10 = nxp0 * nxp0 * 10

  ! ADD 1 to total number of points needed

  nmd =     nn10 + 2 + 1  ! ADDING 1 for reference point (index = 1)
  nud = 3 * nn10     + 1  ! ADDING 1 for reference point (index = 1)
  nwd = 2 * nn10     + 1  ! ADDING 1 for reference point (index = 1)

  ! Allocate memory for itabs and M earth coords
  ! Initialize all neighbor indices to zero

  call alloc_itabsd(nmd,nud,nwd)

  do im = 2,nmd
     itab_md(im)%imp = im
     itab_md(im)%mrlm = 1
     itab_md(im)%mrlm_orig = 1
     itab_md(im)%ngr = 1
     call mdloopf('f',im, jtm_grid, jtm_init, jtm_prog, jtm_wadj, jtm_wstn, 0)
  enddo

  do iu = 2,nud
     itab_ud(iu)%iup = iu
     call udloopf('f',iu, jtu_grid, jtu_init, jtu_prog, jtu_wadj, jtu_wstn, 0)
  enddo

  do iw = 2,nwd
     itab_wd(iw)%iwp = iw
     call wdloopf('f',iw, jtw_grid, jtw_vadj, 0, 0, 0, 0)
  enddo

  ! Fill big diamond corner coordinates

  do id = 1,5

     anglen = .2 * (id-1) * pi2
     anglew = anglen - .1 * pi2
     anglee = anglen + .1 * pi2

     zed_s(id) = -erad
     xed_s(id) = 0.
     yed_s(id) = 0.

     zed_n(id) = erador5
     xed_n(id) = erador5 * 2. * cos(anglen)
     yed_n(id) = erador5 * 2. * sin(anglen)

     zed_w(id) = -erador5
     xed_w(id) = erador5 * 2. * cos(anglew)
     yed_w(id) = erador5 * 2. * sin(anglew)

     zed_e(id) = -erador5
     xed_e(id) = erador5 * 2. * cos(anglee)
     yed_e(id) = erador5 * 2. * sin(anglee)

  enddo

  do id = 6,10

     angles = .2 * (id-6) * pi2 + .1 * pi2
     anglew = angles - .1 * pi2
     anglee = angles + .1 * pi2

     zed_s(id) = -erador5
     xed_s(id) = erador5 * 2. * cos(angles)
     yed_s(id) = erador5 * 2. * sin(angles)

     zed_n(id) = erad
     xed_n(id) = 0.
     yed_n(id) = 0.

     zed_w(id) = erador5
     xed_w(id) = erador5 * 2. * cos(anglew)
     yed_w(id) = erador5 * 2. * sin(anglew)

     zed_e(id) = erador5
     xed_e(id) = erador5 * 2. * cos(anglee)
     yed_e(id) = erador5 * 2. * sin(anglee)

  enddo

  ! Store IM index of south-pole and north-pole pentagonal points

  impent(1) = 2
  impent(12) = nmd

  do ibigd = 1,10

     do j = 1,nxp0
        do i = 1,nxp0

           idiamond = (ibigd - 1) * nxp0 * nxp0 &
                    + (j - 1)     * nxp0        &
                    +  i

  ! Indices that are "attached" to this diamond

           im_left  = idiamond + 2

           iu0 = 3 * idiamond
           iu1 = 3 * idiamond - 1
           iu3 = 3 * idiamond + 1

           iw1 = 2 * idiamond
           iw2 = 2 * idiamond + 1

  ! Store IM index of 10 out of 12 pentagonal points

           if (i == 1 .and. j == nxp0) impent(ibigd+1) = im_left

  ! Indices that are "attached" to another diamond

           if (ibigd < 6) then   ! Southern 5 diamonds

              ! Top diamond indices

              if (i < nxp0) then
                 idiamond_top = idiamond + 1
              else
                 idiamond_top = (ibigd_ne(ibigd) - 1) * nxp0 * nxp0 &
                              + (j - 1)               * nxp0        &
                              +  1
              endif

              im_top   = idiamond_top + 2
              iu4 = 3 * idiamond_top - 1   ! (it's the iu1 for id_top)

              ! Right diamond indices

              if (j > 1 .and. i < nxp0) then
                 idiamond_right = idiamond - nxp0 + 1
              elseif (j == 1) then
                 idiamond_right = (ibigd_se(ibigd)-1) * nxp0 * nxp0 &
                                + (i - 1)             * nxp0        &
                                +  1
                 iu2 = 3 * idiamond_right - 1 ! (it's the iu1 for id_right)
              else            ! case for i = nxp0 and j > 1
                 idiamond_right = (ibigd_ne(ibigd)-1) * nxp0 * nxp0 &
                                + (j - 2)             * nxp0        &
                                +  1
              endif

              im_right = idiamond_right + 2

              ! Bottom diamond indices

              if (j > 1) then
                 idiamond_bot = idiamond - nxp0
                 iu2 = 3 * idiamond_bot + 1 ! (it's the iu3 for id_bot)
              else
                 idiamond_bot = (ibigd_se(ibigd)-1) * nxp0 * nxp0 &
                              + (i - 2)             * nxp0        &
                              +  1
              endif
              im_bot = idiamond_bot + 2

              if (i == 1 .and. j == 1) im_bot = 2

           else                  ! Northern 5 diamonds

             ! Top diamond indices

              if (i < nxp0) then
                 idiamond_top = idiamond + 1
                 iu4 = 3 * idiamond_top - 1   ! (it's the iu1 for id_top)
              else
                 idiamond_top = (ibigd_ne(ibigd)-1) * nxp0 * nxp0 &
                              + (nxp0 - 1)          * nxp0        &
                              +  j + 1
              endif

              im_top = idiamond_top + 2

              ! Right diamond indices

              if (j > 1 .and. i < nxp0) then
                 idiamond_right = idiamond - nxp0 + 1
              elseif (j == 1 .and. i < nxp0) then
                 idiamond_right = (ibigd_se(ibigd)-1) * nxp0 * nxp0 &
                                + (nxp0 - 1)          * nxp0        &
                                + i + 1
              else            ! case for i = nxp0
                 idiamond_right = (ibigd_ne(ibigd)-1) * nxp0 * nxp0 &
                                + (nxp0 - 1)          * nxp0        &
                                +  j
                 iu4 = 3 * idiamond_right + 1 ! (it's the iu3 for id_right)
              endif

              im_right = idiamond_right + 2

              ! Bottom diamond indices

              if (j > 1) then
                 idiamond_bot = idiamond - nxp0
              else
                 idiamond_bot = (ibigd_se(ibigd)-1) * nxp0 * nxp0 &
                              + (nxp0 - 1)          * nxp0        &
                              + i
              endif

              im_bot = idiamond_bot + 2
              iu2 = 3 * idiamond_bot + 1 ! (it's the iu3 for id_bot)

              if (i == nxp0 .and. j == nxp0) &
                 im_top = 10 * nxp0 * nxp0 + 3

           endif

           call fill_diamond(im_left,im_right,im_top,im_bot &
              ,iu0,iu1,iu2,iu3,iu4,iw1,iw2)

           ! M point (xemd,yemd,zemd) coordinates

           if (i + j <= nxp0) then
              wts  = max(0.,min(1.,real(nxp0 + 1 - i - j) / real(nxp0)))
              wtn  = 0.
              wtw0 = max(0.,min(1.,real(j) / real(i + j - 1)))
              wte0 = 1. - wtw0
           else
              wts  = 0.
              wtn  = max(0.,min(1.,real(i + j - nxp0 - 1) / real(nxp0)))
              wte0 = max(0.,min(1.,real(nxp0 - j) &
                   / real(2 * nxp0 + 1 - i - j)))
              wtw0 = 1. - wte0
           endif

           ! Experimental adjustment in spacing
           ! Compute sum of weights raised to pwrd

           wtw = (1. - wts - wtn) * wtw0
           wte = (1. - wts - wtn) * wte0
           sumwt = wts**pwrd + wtn**pwrd + wtw**pwrd + wte**pwrd

           wts = wts**pwrd / sumwt
           wtn = wtn**pwrd / sumwt
           wtw = wtw**pwrd / sumwt
           wte = wte**pwrd / sumwt

           xemd(im_left) = wts * xed_s(ibigd) &
                         + wtn * xed_n(ibigd) &
                         + wtw * xed_w(ibigd) &
                         + wte * xed_e(ibigd)

           yemd(im_left) = wts * yed_s(ibigd) &
                         + wtn * yed_n(ibigd) &
                         + wtw * yed_w(ibigd) &
                         + wte * yed_e(ibigd)

           zemd(im_left) = wts * zed_s(ibigd) &
                         + wtn * zed_n(ibigd) &
                         + wtw * zed_w(ibigd) &
                         + wte * zed_e(ibigd)

           ! Push M point coordinates out to earth radius

           expansion = erad / sqrt( xemd(im_left) ** 2 &
                                  + yemd(im_left) ** 2 &
                                  + zemd(im_left) ** 2 )

           xemd(im_left) = xemd(im_left) * expansion
           yemd(im_left) = yemd(im_left) * expansion
           zemd(im_left) = zemd(im_left) * expansion

        enddo  ! end i loop
     enddo     ! end j loop

  enddo        ! end idbig loop

  xemd(2) = 0.
  yemd(2) = 0.
  zemd(2) = -erad

  xemd(nmd) = 0.
  yemd(nmd) = 0.
  zemd(nmd) = erad

  call tri_neighbors(nmd, nud, nwd, itab_md, itab_ud, itab_wd)

  call spring_dynamics1(1, nxp0, nmd, nud, nwd, xemd, yemd, zemd, itab_md, itab_ud, itab_wd)

end subroutine icosahedron
!===============================================================================

subroutine fill_diamond(im_left,im_right,im_top,im_bot,  &
                        iu0,iu1,iu2,iu3,iu4,iw1,iw2)

  use mem_delaunay, only: itab_ud, itab_wd

  implicit none

  integer, intent(in) :: im_left,im_right,im_top,im_bot
  integer, intent(in) :: iu0,iu1,iu2,iu3,iu4,iw1,iw2

  itab_ud(iu0)%im(1) = im_left
  itab_ud(iu0)%im(2) = im_right
  itab_ud(iu0)%iw(1) = iw1
  itab_ud(iu0)%iw(2) = iw2
  itab_ud(iu0)%mrlu = 1

  itab_ud(iu1)%im(1) = im_left
  itab_ud(iu1)%im(2) = im_bot
  itab_ud(iu1)%iw(2) = iw1
  itab_ud(iu1)%mrlu = 1

  itab_ud(iu2)%iw(1) = iw1

  itab_ud(iu3)%im(1) = im_top
  itab_ud(iu3)%im(2) = im_left
  itab_ud(iu3)%iw(2) = iw2
  itab_ud(iu3)%mrlu = 1

  itab_ud(iu4)%iw(1) = iw2

  itab_wd(iw1)%iu(1) = iu0
  itab_wd(iw1)%iu(2) = iu1
  itab_wd(iw1)%iu(3) = iu2
  itab_wd(iw1)%mrlw = 1
  itab_wd(iw1)%mrlw_orig = 1
  itab_wd(iw1)%ngr = 1

  itab_wd(iw2)%iu(1) = iu0
  itab_wd(iw2)%iu(2) = iu4
  itab_wd(iw2)%iu(3) = iu3
  itab_wd(iw2)%mrlw = 1
  itab_wd(iw2)%mrlw_orig = 1
  itab_wd(iw2)%ngr = 1

end subroutine fill_diamond
!===============================================================================

subroutine spring_dynamics1( ngr, nxp, nmd, nud, nwd, xemd, yemd, zemd, itab_md, itab_ud, itab_wd )

   ! Subroutine spring_dynamics1 is used only for adjusting grid 1 (i.e.,
   ! the quasi-uniform global atm grid) prior to any mesh refinements.

   use mem_delaunay, only: itab_md_vars, itab_ud_vars, itab_wd_vars
   use consts_coms,  only: pi2_r8, erad8, r8, io6, niter, beta, relax, impent
   ! use mem_grid,     only: impent
   implicit none
   integer, intent(in) :: ngr, nxp, nmd, nud, nwd
   real, intent(inout) :: xemd(nmd), yemd(nmd), zemd(nmd)

   type (itab_md_vars), intent(inout) :: itab_md(nmd)
   type (itab_ud_vars), intent(inout) :: itab_ud(nud)
   type (itab_wd_vars), intent(inout) :: itab_wd(nwd)

   integer, parameter :: nprnt = 100

   real(r8) :: dist(nud), dx(nud), dy(nud), dz(nud)
   real(r8) :: distm, ratio, frac_change
   real(r8) :: dirs(7, nmd)

   integer  :: iv, iu, iu1, iu2, iu3, iu4
   integer  :: im, im1, im2
   integer  :: j, iter

   real(r8) :: twocosphi3, twocosphi4
   real(r8) :: dist00, disto12
   real(r8) :: dsm(nmd)

   real(r8) :: dist0(nmd)
   real(r8) :: xemd8(nmd), yemd8(nmd), zemd8(nmd)
   real(r8) :: xemd0(nmd), yemd0(nmd), zemd0(nmd)
   real(r8) :: expansion


   integer :: iumn(nud, 2)
   integer :: iuun(nud, 4)
   integer :: imnp(nmd), imiu(7, nmd)

   dsm(1) = 0.0
   xemd8(:) = real(xemd(:), r8)
   yemd8(:) = real(yemd(:), r8)
   zemd8(:) = real(zemd(:), r8)

   ! Compute mean length of coarse mesh U segments
   dist00 = beta * pi2_r8 * erad8 / (5. * real(nxp))
   disto12 = dist00 / 1.2 ! in olam-6.4.0


   do iu = 2, nud
      iumn(iu,1) = itab_ud(iu)%im(1)
      iumn(iu,2) = itab_ud(iu)%im(2)

      iuun(iu,1) = itab_ud(iu)%iu(1)
      iuun(iu,2) = itab_ud(iu)%iu(2)
      iuun(iu,3) = itab_ud(iu)%iu(3)
      iuun(iu,4) = itab_ud(iu)%iu(4)
   enddo

   do im = 2, nmd
      imnp(im) = itab_md(im)%npoly

      do j = 1, itab_md(im)%npoly
         iu = itab_md(im)%iu(j)
         imiu(j,im) = iu

         if (itab_ud(iu)%im(2) == im) then
            dirs(j,im) =  relax
         else
            dirs(j,im) = -relax
         endif
      enddo
   enddo

   ! Main iteration loop
   write(io6,'(a,4i9)') "In spring dynamics: ngr, nmd, niter = ",ngr, nmd, niter
   !$omp PARALLEL private(iter, im, iu, j, iv, im1, im2, iu1, iu2, iu3, iu4, &
   !$omp&                 twocosphi3, twocosphi4, distm, frac_change, ratio, expansion)
   do iter = 1, niter

      if (iter == 1 .or. mod(iter,nprnt) == 0) then
         !$omp do
         do im = 2, nmd
            xemd0(im) = xemd8(im)
            yemd0(im) = yemd8(im)
            zemd0(im) = zemd8(im)
         enddo
         !$omp end do
      endif

      ! Compute length of each U segment
      !$omp do
      do iu = 2, nud
         im1 = iumn(iu, 1)
         im2 = iumn(iu, 2)
         dx(iu) = real( xemd8(im2) - xemd8(im1) )
         dy(iu) = real( yemd8(im2) - yemd8(im1) )
         dz(iu) = real( zemd8(im2) - zemd8(im1) )
         dist(iu) = sqrt( dx(iu) * dx(iu) + dy(iu) * dy(iu)+ dz(iu) * dz(iu) )
      enddo
      !$omp end do

      ! Adjustment of dist0 based on opposite angles of triangles
      !$omp do
      do iu = 2, nud
         iu1 = iuun(iu, 1)
         iu2 = iuun(iu, 2)
         iu3 = iuun(iu, 3)
         iu4 = iuun(iu, 4)

         ! Compute cosine of angles at IM3 and IM4
         twocosphi3 = (dist(iu1)**2 + dist(iu2)**2 - dist(iu)**2) / (dist(iu1) * dist(iu2))
         twocosphi4 = (dist(iu3)**2 + dist(iu4)**2 - dist(iu)**2) / (dist(iu3) * dist(iu4))

         ! Ratio of smaller cosine to limiting value of cos(72 deg)
         ratio = max(0.15_r8, min(twocosphi3 + twocosphi4, 1.2_r8)) ! in olam-6.4.0
         distm = disto12 * ratio ! in olam-6.4.0

         ! Fractional change to dist that would make it equal dist0
         frac_change = (distm - dist(iu)) / dist(iu)

         ! Compute components of displacement that gives dist0
         dx(iu) = dx(iu) * frac_change
         dy(iu) = dy(iu) * frac_change
         dz(iu) = dz(iu) * frac_change
      enddo
      !$omp end do

      !$omp do private(j, iv)
      do im = 2, nmd
         ! For preventing all pentagonal points from moving:
         ! if (any(im == impent(1:12))) cycle
         ! Apply the displacement components to each M point
         do j = 1, imnp(im)
            iv = imiu(j,im)
            xemd8(im) = xemd8(im) + dirs(j,im) * dx(iv)
            yemd8(im) = yemd8(im) + dirs(j,im) * dy(iv)
            zemd8(im) = zemd8(im) + dirs(j,im) * dz(iv)
         enddo
      enddo
      !$omp end do

      ! Push M point coordinates out to earth radius
      !$omp do private(expansion)
      do im = 2, nmd
         expansion = erad8 / sqrt( xemd8(im) ** 2 + yemd8(im) ** 2 + zemd8(im) ** 2 )
         xemd8(im) = xemd8(im) * expansion
         yemd8(im) = yemd8(im) * expansion
         zemd8(im) = zemd8(im) * expansion
      enddo
      !$omp end do

      ! Print iteration status
      if (iter == 1 .or. mod(iter,nprnt) == 0) then
         !$omp do
         do im = 2, nmd
            dsm(im) = real( (xemd8(im) - xemd0(im))**2 &
               + (yemd8(im) - yemd0(im))**2 &
               + (zemd8(im) - zemd0(im))**2 )
         enddo
         !$omp end do

         !$omp single
         write(*,'(3x,A,I5,A,I5,A,f0.4,A)') &
         "Iteration ", iter, " of ", niter, ",  Max DS = ", &
         sqrt( maxval(dsm) ), " meters."
         !$omp end single

      endif

   enddo ! iter
   !$omp end parallel

   xemd(:) = real(xemd8(:))
   yemd(:) = real(yemd8(:))
   zemd(:) = real(zemd8(:))

end subroutine spring_dynamics1
!===============================================================================

subroutine mdloopf(init,im,j1,j2,j3,j4,j5,j6)

   use mem_delaunay, only: itab_md
 
   implicit none
 
   character(1), intent(in) :: init
   integer,      intent(in) :: im
   integer,      intent(in) :: j1,j2,j3,j4,j5,j6
 
   if (init == 'f') itab_md(im)%loop(:) = .false.
 
   if (j1 < 0) itab_md(im)%loop(abs(j1)) = .false.
   if (j2 < 0) itab_md(im)%loop(abs(j2)) = .false.
   if (j3 < 0) itab_md(im)%loop(abs(j3)) = .false.
   if (j4 < 0) itab_md(im)%loop(abs(j4)) = .false.
   if (j5 < 0) itab_md(im)%loop(abs(j5)) = .false.
   if (j6 < 0) itab_md(im)%loop(abs(j6)) = .false.
 
   if (j1 > 0) itab_md(im)%loop(j1) = .true.
   if (j2 > 0) itab_md(im)%loop(j2) = .true.
   if (j3 > 0) itab_md(im)%loop(j3) = .true.
   if (j4 > 0) itab_md(im)%loop(j4) = .true.
   if (j5 > 0) itab_md(im)%loop(j5) = .true.
   if (j6 > 0) itab_md(im)%loop(j6) = .true.
 
end subroutine mdloopf
!===============================================================================

 subroutine udloopf(init,iu,j1,j2,j3,j4,j5,j6)
 
   use mem_delaunay, only: itab_ud
 
   implicit none
 
   character(1), intent(in) :: init
   integer,      intent(in) :: iu
   integer,      intent(in) :: j1,j2,j3,j4,j5,j6
 
   if (init == 'f') itab_ud(iu)%loop(:) = .false.
 
   if (j1 < 0) itab_ud(iu)%loop(abs(j1)) = .false.
   if (j2 < 0) itab_ud(iu)%loop(abs(j2)) = .false.
   if (j3 < 0) itab_ud(iu)%loop(abs(j3)) = .false.
   if (j4 < 0) itab_ud(iu)%loop(abs(j4)) = .false.
   if (j5 < 0) itab_ud(iu)%loop(abs(j5)) = .false.
   if (j6 < 0) itab_ud(iu)%loop(abs(j6)) = .false.
 
   if (j1 > 0) itab_ud(iu)%loop(j1) = .true.
   if (j2 > 0) itab_ud(iu)%loop(j2) = .true.
   if (j3 > 0) itab_ud(iu)%loop(j3) = .true.
   if (j4 > 0) itab_ud(iu)%loop(j4) = .true.
   if (j5 > 0) itab_ud(iu)%loop(j5) = .true.
   if (j6 > 0) itab_ud(iu)%loop(j6) = .true.
 
 end subroutine udloopf
 !===============================================================================
 
 subroutine wdloopf(init,iw,j1,j2,j3,j4,j5,j6)
 
   use mem_delaunay, only: itab_wd
 
   implicit none
 
   character(1), intent(in) :: init
   integer,      intent(in) :: iw
   integer,      intent(in) :: j1,j2,j3,j4,j5,j6
 
   if (init == 'f') itab_wd(iw)%loop(:) = .false.
 
   if (j1 < 0) itab_wd(iw)%loop(abs(j1)) = .false.
   if (j2 < 0) itab_wd(iw)%loop(abs(j2)) = .false.
   if (j3 < 0) itab_wd(iw)%loop(abs(j3)) = .false.
   if (j4 < 0) itab_wd(iw)%loop(abs(j4)) = .false.
   if (j5 < 0) itab_wd(iw)%loop(abs(j5)) = .false.
   if (j6 < 0) itab_wd(iw)%loop(abs(j6)) = .false.
 
   if (j1 > 0) itab_wd(iw)%loop(j1) = .true.
   if (j2 > 0) itab_wd(iw)%loop(j2) = .true.
   if (j3 > 0) itab_wd(iw)%loop(j3) = .true.
   if (j4 > 0) itab_wd(iw)%loop(j4) = .true.
   if (j5 > 0) itab_wd(iw)%loop(j5) = .true.
   if (j6 > 0) itab_wd(iw)%loop(j6) = .true.
 
 end subroutine wdloopf
!===============================================================================

subroutine tri_neighbors(nmd, nud, nwd, itab_md, itab_ud, itab_wd)

   use mem_delaunay, only: itab_md_vars, itab_ud_vars, itab_wd_vars
 
   implicit none
 
   integer, intent(in) :: nmd, nud, nwd
 
   type (itab_md_vars), intent(inout) :: itab_md(nmd)
   type (itab_ud_vars), intent(inout) :: itab_ud(nud)
   type (itab_wd_vars), intent(inout) :: itab_wd(nwd)
 
   integer :: iu,iw
   integer :: iu1,iu2,iu3,iu4
   integer :: iw1,iw2,iw3,iw4,iw5,iw6
   integer :: iw1_iu1,iw1_iu2,iw1_iu3
   integer :: iw2_iu1,iw2_iu2,iw2_iu3
   integer :: j,iunow,iu0,npoly,im
 
   ! Loop over W points
 
   !$omp parallel
   !$omp do private(iu1,iu2,iu3)
   do iw = 2, nwd
      itab_wd(iw)%npoly = 0
 
      iu1 = itab_wd(iw)%iu(1)
      iu2 = itab_wd(iw)%iu(2)
      iu3 = itab_wd(iw)%iu(3)
 
      if (iu1 > 1) itab_wd(iw)%npoly = itab_wd(iw)%npoly + 1
      if (iu2 > 1) itab_wd(iw)%npoly = itab_wd(iw)%npoly + 1
      if (iu3 > 1) itab_wd(iw)%npoly = itab_wd(iw)%npoly + 1
 
      ! Fill M and inner W neighbors for current W point
 
      if (iu1 > 1) then
         if     (iw == itab_ud(iu1)%iw(1)) then
            itab_wd(iw)%im(3) = itab_ud(iu1)%im(1)
            itab_wd(iw)%im(2) = itab_ud(iu1)%im(2)
            itab_wd(iw)%iw(1) = itab_ud(iu1)%iw(2)
         elseif (iw == itab_ud(iu1)%iw(2)) then
            itab_wd(iw)%im(3) = itab_ud(iu1)%im(2)
            itab_wd(iw)%im(2) = itab_ud(iu1)%im(1)
            itab_wd(iw)%iw(1) = itab_ud(iu1)%iw(1)
         endif
      endif
 
      if (iu2 > 1) then
         if     (iw == itab_ud(iu2)%iw(1)) then
            itab_wd(iw)%im(1) = itab_ud(iu2)%im(1)
            itab_wd(iw)%im(3) = itab_ud(iu2)%im(2)
            itab_wd(iw)%iw(2) = itab_ud(iu2)%iw(2)
         elseif (iw == itab_ud(iu2)%iw(2)) then
            itab_wd(iw)%im(1) = itab_ud(iu2)%im(2)
            itab_wd(iw)%im(3) = itab_ud(iu2)%im(1)
            itab_wd(iw)%iw(2) = itab_ud(iu2)%iw(1)
         endif
      endif
 
      if (iu3 > 1) then
         if     (iw == itab_ud(iu3)%iw(1)) then
            itab_wd(iw)%im(2) = itab_ud(iu3)%im(1)
            itab_wd(iw)%im(1) = itab_ud(iu3)%im(2)
            itab_wd(iw)%iw(3) = itab_ud(iu3)%iw(2)
         elseif (iw == itab_ud(iu3)%iw(2)) then
            itab_wd(iw)%im(2) = itab_ud(iu3)%im(2)
            itab_wd(iw)%im(1) = itab_ud(iu3)%im(1)
            itab_wd(iw)%iw(3) = itab_ud(iu3)%iw(1)
         endif
      endif
 
   enddo
   !$omp end do
 
   ! Fill outer W points for current W point
 
   !$omp do private(iw1,iw2,iw3)
   do iw = 2, nwd
      iw1 = itab_wd(iw)%iw(1)
      iw2 = itab_wd(iw)%iw(2)
      iw3 = itab_wd(iw)%iw(3)
 
      ! This should work ok for iw1 = 1, but may bypass if desired
 
      if (iw == itab_wd(iw1)%iw(1)) then
 
         itab_wd(iw)%iw(4) = itab_wd(iw1)%iw(2)
         itab_wd(iw)%iw(5) = itab_wd(iw1)%iw(3)
 
      elseif (iw == itab_wd(iw1)%iw(2)) then
 
         itab_wd(iw)%iw(4) = itab_wd(iw1)%iw(3)
         itab_wd(iw)%iw(5) = itab_wd(iw1)%iw(1)
 
      else
 
         itab_wd(iw)%iw(4) = itab_wd(iw1)%iw(1)
         itab_wd(iw)%iw(5) = itab_wd(iw1)%iw(2)
 
      endif
 
      ! This should work ok for iw2 = 1, but may bypass if desired
 
      if (iw == itab_wd(iw2)%iw(1)) then
 
         itab_wd(iw)%iw(6) = itab_wd(iw2)%iw(2)
         itab_wd(iw)%iw(7) = itab_wd(iw2)%iw(3)
 
      elseif (iw == itab_wd(iw2)%iw(2)) then
 
         itab_wd(iw)%iw(6) = itab_wd(iw2)%iw(3)
         itab_wd(iw)%iw(7) = itab_wd(iw2)%iw(1)
 
      else
 
         itab_wd(iw)%iw(6) = itab_wd(iw2)%iw(1)
         itab_wd(iw)%iw(7) = itab_wd(iw2)%iw(2)
 
      endif
 
      ! This should work ok for iw3 = 1, but may bypass if desired
 
      if (iw == itab_wd(iw3)%iw(1)) then
 
         itab_wd(iw)%iw(8) = itab_wd(iw3)%iw(2)
         itab_wd(iw)%iw(9) = itab_wd(iw3)%iw(3)
 
      elseif (iw == itab_wd(iw3)%iw(2)) then
 
         itab_wd(iw)%iw(8) = itab_wd(iw3)%iw(3)
         itab_wd(iw)%iw(9) = itab_wd(iw3)%iw(1)
 
      else
 
         itab_wd(iw)%iw(8) = itab_wd(iw3)%iw(1)
         itab_wd(iw)%iw(9) = itab_wd(iw3)%iw(2)
 
      endif
 
   enddo
   !$omp end do
 
   ! Loop over U points
 
   !$omp do private(iw1,iw2,iw1_iu1,iw1_iu2,iw1_iu3,iw2_iu1,iw2_iu2,iw2_iu3, &
   !$omp            iu1,iu2,iu3,iu4,iw3,iw4,iw5,iw6)
   do iu = 2, nud
 
      iw1 = itab_ud(iu)%iw(1)
      iw2 = itab_ud(iu)%iw(2)
 
      itab_ud(iu)%mrlu = max(itab_wd(iw1)%mrlw,itab_wd(iw2)%mrlw)
 
      ! This should work ok for iw1 = 1, but may bypass if desired
 
      iw1_iu1 = itab_wd(iw1)%iu(1)
      iw1_iu2 = itab_wd(iw1)%iu(2)
      iw1_iu3 = itab_wd(iw1)%iu(3)
 
      ! Fill IU1 and IU2 for current U point
 
      if (iw1_iu1 == iu) then
         itab_ud(iu)%iu(1) = iw1_iu2
         itab_ud(iu)%iu(2) = iw1_iu3
      elseif (iw1_iu2 == iu) then
         itab_ud(iu)%iu(1) = iw1_iu3
         itab_ud(iu)%iu(2) = iw1_iu1
      else
         itab_ud(iu)%iu(1) = iw1_iu1
         itab_ud(iu)%iu(2) = iw1_iu2
      endif
 
      ! This should work ok for iw2 = 1, but may bypass if desired
 
      iw2_iu1 = itab_wd(iw2)%iu(1)
      iw2_iu2 = itab_wd(iw2)%iu(2)
      iw2_iu3 = itab_wd(iw2)%iu(3)
 
      ! Fill IU3 and IU4 for current U point
 
      if (iw2_iu1 == iu) then
         itab_ud(iu)%iu(3) = iw2_iu3
         itab_ud(iu)%iu(4) = iw2_iu2
      elseif (iw2_iu2 == iu) then
         itab_ud(iu)%iu(3) = iw2_iu1
         itab_ud(iu)%iu(4) = iw2_iu3
      else
         itab_ud(iu)%iu(3) = iw2_iu2
         itab_ud(iu)%iu(4) = iw2_iu1
      endif
 
      iu1 = itab_ud(iu)%iu(1)
      iu2 = itab_ud(iu)%iu(2)
      iu3 = itab_ud(iu)%iu(3)
      iu4 = itab_ud(iu)%iu(4)
 
      ! Fill IW3 for current U point
      ! This should work ok for iu1 = 1, but may bypass if desired
 
      if (itab_ud(iu1)%iw(1) == iw1) then
         itab_ud(iu)%iw(3) = itab_ud(iu1)%iw(2)
      else
         itab_ud(iu)%iw(3) = itab_ud(iu1)%iw(1)
      endif
 
      ! Fill IW4 for current U point
      ! This should work ok for iu2 = 1, but may bypass if desired
 
      if (itab_ud(iu2)%iw(1) == iw1) then
         itab_ud(iu)%iw(4) = itab_ud(iu2)%iw(2)
      else
         itab_ud(iu)%iw(4) = itab_ud(iu2)%iw(1)
      endif
 
      ! Fill IW5 for current U point
      ! This should work ok for iu3 = 1, but may bypass if desired
 
      if (itab_ud(iu3)%iw(1) == iw2) then
         itab_ud(iu)%iw(5) = itab_ud(iu3)%iw(2)
      else
         itab_ud(iu)%iw(5) = itab_ud(iu3)%iw(1)
      endif
 
      ! Fill IW6 for current U point
      ! This should work ok for iu4 = 1, but may bypass if desired
 
      if (itab_ud(iu4)%iw(1) == iw2) then
         itab_ud(iu)%iw(6) = itab_ud(iu4)%iw(2)
      else
         itab_ud(iu)%iw(6) = itab_ud(iu4)%iw(1)
      endif
 
      iw3 = itab_ud(iu)%iw(3)
      iw4 = itab_ud(iu)%iw(4)
      iw5 = itab_ud(iu)%iw(5)
      iw6 = itab_ud(iu)%iw(6)
 
      ! Fill IU5 and IU6 for current U point
      ! This should work ok for iw3 = 1, but may bypass if desired
 
      if (iu1 == itab_wd(iw3)%iu(1)) then
         itab_ud(iu)%iu(5) = itab_wd(iw3)%iu(2)
         itab_ud(iu)%iu(6) = itab_wd(iw3)%iu(3)
      elseif (iu1 == itab_wd(iw3)%iu(2)) then
         itab_ud(iu)%iu(5) = itab_wd(iw3)%iu(3)
         itab_ud(iu)%iu(6) = itab_wd(iw3)%iu(1)
      else
         itab_ud(iu)%iu(5) = itab_wd(iw3)%iu(1)
         itab_ud(iu)%iu(6) = itab_wd(iw3)%iu(2)
      endif
 
      ! Fill IU7 and IU8 for current U point
      ! This should work ok for iw4 = 1, but may bypass if desired
 
      if (iu2 == itab_wd(iw4)%iu(1)) then
         itab_ud(iu)%iu(7) = itab_wd(iw4)%iu(2)
         itab_ud(iu)%iu(8) = itab_wd(iw4)%iu(3)
      elseif (iu2 == itab_wd(iw4)%iu(2)) then
         itab_ud(iu)%iu(7) = itab_wd(iw4)%iu(3)
         itab_ud(iu)%iu(8) = itab_wd(iw4)%iu(1)
      else
         itab_ud(iu)%iu(7) = itab_wd(iw4)%iu(1)
         itab_ud(iu)%iu(8) = itab_wd(iw4)%iu(2)
      endif
 
      ! Fill IU9 and IU10 for current U point
      ! This should work ok for iw5 = 1, but may bypass if desired
 
      if (iu3 == itab_wd(iw5)%iu(1)) then
         itab_ud(iu)%iu(9)  = itab_wd(iw5)%iu(3)
         itab_ud(iu)%iu(10) = itab_wd(iw5)%iu(2)
      elseif (iu3 == itab_wd(iw5)%iu(2)) then
         itab_ud(iu)%iu(9)  = itab_wd(iw5)%iu(1)
         itab_ud(iu)%iu(10) = itab_wd(iw5)%iu(3)
      else
         itab_ud(iu)%iu(9)  = itab_wd(iw5)%iu(2)
         itab_ud(iu)%iu(10) = itab_wd(iw5)%iu(1)
      endif
 
      ! Fill IU11 and IU12 for current U point
      ! This should work ok for iw6 = 1, but may bypass if desired
 
      if (iu4 == itab_wd(iw6)%iu(1)) then
         itab_ud(iu)%iu(11) = itab_wd(iw6)%iu(3)
         itab_ud(iu)%iu(12) = itab_wd(iw6)%iu(2)
      elseif (iu4 == itab_wd(iw6)%iu(2)) then
         itab_ud(iu)%iu(11) = itab_wd(iw6)%iu(1)
         itab_ud(iu)%iu(12) = itab_wd(iw6)%iu(3)
      else
         itab_ud(iu)%iu(11) = itab_wd(iw6)%iu(2)
         itab_ud(iu)%iu(12) = itab_wd(iw6)%iu(1)
      endif
 
   enddo  ! end loop over U points
   !$omp end do
   !$omp end parallel
 
   ! Fill U and W points for M points (do this as loop over U points)
 
   itab_md(:)%npoly = 0
 
   do iu = 2, nud
      do j = 1,2
         im = itab_ud(iu)%im(j)
         iw = itab_ud(iu)%iw(j)
 
         if ( (itab_md(im)%npoly == 0) .or. &  ! The and next line added for walls:
              (itab_wd(iw)%npoly < 3) ) then   ! npoly check allows WD at cart_hex boundaries
 
            iunow = iu
            iu0 = 0
            npoly = 0
 
            do while (iunow /= iu0 .and. iunow > 1)
 
               iu0 = iu
               npoly = npoly + 1  ! MOVED HERE 8/24/2012
 
               if (npoly > 7) stop 'stop tri_neighbors npoly'
 
               itab_md(im)%iu(npoly) = iunow
 
               if (itab_ud(iunow)%im(1) == im) then
 
                  if (itab_ud(iunow)%iw(2) > 1) then
                     itab_md(im)%iw(npoly) = itab_ud(iunow)%iw(2)
                     iunow = itab_ud(iunow)%iu(3)
                  else
                     iunow = iu0  ! this section added for walls
                  endif
 
               else
 
                  if (itab_ud(iunow)%iw(1) > 1) then
                     itab_md(im)%iw(npoly) = itab_ud(iunow)%iw(1)
                     iunow = itab_ud(iunow)%iu(2)
                  else
                     iunow = iu0  ! this section added for walls
                  endif
 
               endif
 
               itab_md(im)%npoly = npoly
 
            enddo
 
         endif
 
      enddo
   enddo
 
end subroutine tri_neighbors
!============================================================================

subroutine de_ps(dxe,dye,dze,cosplat,sinplat,cosplon,sinplon,x,y)

   use consts_coms, only: erad2
   
   implicit none
   
   real, intent(in) :: dxe
   real, intent(in) :: dye
   real, intent(in) :: dze
   
   real, intent(in) :: cosplat
   real, intent(in) :: sinplat
   real, intent(in) :: cosplon
   real, intent(in) :: sinplon
   
   real, intent(out) :: x
   real, intent(out) :: y
   
   real :: xq
   real :: yq
   real :: zq
   real :: t
   
   ! This subroutine computes coordinates (x,y) of point q projected onto polar
   ! stereographic plane given the sines and cosines of the pole point
   ! located at geographic coordinates (polelat,polelon).
   ! Input vector (dxe,dye,dze) is the distance of point q from the pole point
   ! in "earth cartesian space", where the origin is the center of the earth,
   ! the z axis is the north pole, the x axis is the equator and prime meridian,
   ! and the y axis is the equator and 90 E..
   
   ! Transform q point from (xe,ye,ze) coordinates to 3D coordinates relative to
   ! polar stereographic plane with origin at the pole point, the z axis pointing
   ! radially outward from the center of the earth, and the y axis pointing
   ! northward along the local earth meridian from the pole point.
   
   xq =                           - sinplon * dxe + cosplon * dye
   yq =  cosplat * dze - sinplat * (cosplon * dxe + sinplon * dye)
   zq =  sinplat * dze + cosplat * (cosplon * dxe + sinplon * dye)
   
   ! Parametric equation for line from antipodal point at (0,0,-2 erad) in 3D
   ! coordinates of polar stereographic plane to point q has the following
   ! parameter (t) value on the polar stereographic plane (zq <= 0):
   
   t = erad2 / (erad2 + zq)
   
   ! This gives the following x and y coordinates for the projection of point q
   ! onto the polar stereographic plane:
   
   x = xq * t
   y = yq * t
   
end subroutine de_ps

subroutine de_ps_r8(dxe,dye,dze,cosplat,sinplat,cosplon,sinplon,x,y)

   use consts_coms, only: r8, erad2_r8
   implicit none
   real(r8), intent(in) :: dxe
   real(r8), intent(in) :: dye
   real(r8), intent(in) :: dze
   
   real(r8), intent(in) :: cosplat
   real(r8), intent(in) :: sinplat
   real(r8), intent(in) :: cosplon
   real(r8), intent(in) :: sinplon
   
   real(r8), intent(out) :: x
   real(r8), intent(out) :: y
   
   real(r8) :: xq
   real(r8) :: yq
   real(r8) :: zq
   real(r8) :: t

   ! This subroutine computes coordinates (x,y) of point q projected onto polar
   ! stereographic plane given the sines and cosines of the pole point
   ! located at geographic coordinates (polelat,polelon).
   ! Input vector (dxe,dye,dze) is the distance of point q from the pole point
   ! in "earth cartesian space", where the origin is the center of the earth,
   ! the z axis is the north pole, the x axis is the equator and prime meridian,
   ! and the y axis is the equator and 90 E..
   
   ! Transform q point from (xe,ye,ze) coordinates to 3D coordinates relative to
   ! polar stereographic plane with origin at the pole point, the z axis pointing
   ! radially outward from the center of the earth, and the y axis pointing
   ! northward along the local earth meridian from the pole point.
   
   xq =                           - sinplon * dxe + cosplon * dye
   yq =  cosplat * dze - sinplat * (cosplon * dxe + sinplon * dye)
   zq =  sinplat * dze + cosplat * (cosplon * dxe + sinplon * dye)
   
   ! Parametric equation for line from antipodal point at (0,0,-2 erad) in 3D
   ! coordinates of polar stereographic plane to point q has the following
   ! parameter (t) value on the polar stereographic plane (zq <= 0):
   
   t = erad2_r8 / (erad2_r8 + zq)
   
   ! This gives the following x and y coordinates for the projection of point q
   ! onto the polar stereographic plane:
   
   x = xq * t
   y = yq * t
   
end subroutine de_ps_r8

subroutine ps_de(dxe,dye,dze,cosplat,sinplat,cosplon,sinplon,x,y)

      use consts_coms, only: erad2, erad2sq
    
      implicit none
    
      real, intent(out) :: dxe
      real, intent(out) :: dye
      real, intent(out) :: dze
    
      real, intent(in) :: cosplat
      real, intent(in) :: sinplat
      real, intent(in) :: cosplon
      real, intent(in) :: sinplon
    
      real, intent(in) :: x
      real, intent(in) :: y
    
      real :: xq
      real :: yq
      real :: zq
      real :: t
    ! real :: alpha
    
    ! Given coordinates (x,y) of point q projected onto polar stereographic plane
    ! whose pole point is located at geographic coordinates (polelat,polelon),
    ! this subroutine computes coordinates (xeq,yeq,zeq) of q on earth spherical
    ! surface defined in "earth cartesian space", where the origin is the center
    ! of the earth, the z axis is the north pole, the x axis is the equator and
    ! prime meridian, and the y axis is the equator and 90 E..
    
    ! Parametric equation for line from antipodal point at (0,0,-2 erad) in 3D
    ! coordinates of polar stereographic plane to point (x,y) on the polar stereographic
    ! plane has the following parameter (t) value on the earth's surface (zq <= 0):
    
    ! alpha = 2. * atan2(sqrt(x**2 + y**2),erad2)
    ! t     = .5 * (1. + cos(alpha))
      t     = erad2sq / (x*x + y*y + erad2sq)
    
    ! This gives the following xq, yq, zq coordinates relative to the polar
    ! stereographic plane for the projection of point q from the polar stereographic
    ! plane to the earth surface:
    
      xq = x * t
      yq = y * t
      zq = erad2 * (t - 1.)
    
    ! Transform q point located on the earth's surface from ps coordinates (xq,yq,zq)
    ! to earth coordinates (xe,ye,ze).  The polar stereographic plane has its origin
    ! at the pole point, with the z axis pointing radially outward from the center
    ! of the earth, and the y axis pointing northward along the local earth meridian
    ! from the pole point.
    
      dxe = -sinplon * xq + cosplon * (-sinplat * yq + cosplat * zq)
      dye =  cosplon * xq - sinplon * ( sinplat * yq - cosplat * zq)
      dze =  cosplat * yq + sinplat * zq
    
end subroutine ps_de

subroutine ps_de_r8(dxe,dye,dze,cosplat,sinplat,cosplon,sinplon,x,y)

      use consts_coms, only: r8, erad2_r8, erad2sq_r8
    
      implicit none
      real(r8), intent(out) :: dxe
      real(r8), intent(out) :: dye
      real(r8), intent(out) :: dze
    
      real(r8), intent(in) :: cosplat
      real(r8), intent(in) :: sinplat
      real(r8), intent(in) :: cosplon
      real(r8), intent(in) :: sinplon
    
      real(r8), intent(in) :: x
      real(r8), intent(in) :: y
    
      real(r8) :: xq
      real(r8) :: yq
      real(r8) :: zq
      real(r8) :: t
    ! real :: alpha
    
    ! Given coordinates (x,y) of point q projected onto polar stereographic plane
    ! whose pole point is located at geographic coordinates (polelat,polelon),
    ! this subroutine computes coordinates (xeq,yeq,zeq) of q on earth spherical
    ! surface defined in "earth cartesian space", where the origin is the center
    ! of the earth, the z axis is the north pole, the x axis is the equator and
    ! prime meridian, and the y axis is the equator and 90 E..
    
    ! Parametric equation for line from antipodal point at (0,0,-2 erad) in 3D
    ! coordinates of polar stereographic plane to point (x,y) on the polar stereographic
    ! plane has the following parameter (t) value on the earth's surface (zq <= 0):
    
    ! alpha = 2._r8 * atan2(sqrt(x**2 + y**2), erad2_r8)
    ! t     = .5_r8 * (1._r8 + cos(alpha))
      t     = erad2sq_r8 / (x*x + y*y + erad2sq_r8)
    
    ! This gives the following xq, yq, zq coordinates relative to the polar
    ! stereographic plane for the projection of point q from the polar stereographic
    ! plane to the earth surface:
    
      xq = x * t
      yq = y * t
      zq = erad2_r8 * (t - 1._r8)
    
    ! Transform q point located on the earth's surface from ps coordinates (xq,yq,zq)
    ! to earth coordinates (xe,ye,ze).  The polar stereographic plane has its origin
    ! at the pole point, with the z axis pointing radially outward from the center
    ! of the earth, and the y axis pointing northward along the local earth meridian
    ! from the pole point.
    
      dxe = -sinplon * xq + cosplon * (-sinplat * yq + cosplat * zq)
      dye =  cosplon * xq - sinplon * ( sinplat * yq - cosplat * zq)
      dze =  cosplat * yq + sinplat * zq
    
end subroutine ps_de_r8

!============================================================================
   
