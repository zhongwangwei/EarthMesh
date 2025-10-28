!  =========== DOCUMENTATION ===========
!
! Online html documentation available at
!            http://www.netlib.org/lapack/explore-html/
!
! Download IEEECK + dependencies
! <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/ieeeck.f">
! [TGZ]</a>
! <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/ieeeck.f">
! [ZIP]</a>
! <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/ieeeck.f">
! [TXT]</a>
!
!  Definition:
!  ===========
!
!       INTEGER          FUNCTION IEEECK( ISPEC, ZERO, ONE )
!
!       .. Scalar Arguments ..
!       INTEGER            ISPEC
!       REAL               ONE, ZERO
!       ..
!
!
!> \par Purpose:
!  =============
!>
!> \verbatim
!>
!> IEEECK is called from the ILAENV to verify that Infinity and
!> possibly NaN arithmetic is safe (i.e. will not trap).
!> \endverbatim
!
!  Arguments:
!  ==========
!
!> \param[in] ISPEC
!> \verbatim
!>          ISPEC is INTEGER
!>          Specifies whether to test just for inifinity arithmetic
!>          or whether to test for infinity and NaN arithmetic.
!>          = 0: Verify infinity arithmetic only.
!>          = 1: Verify infinity and NaN arithmetic.
!> \endverbatim
!>
!> \param[in] ZERO
!> \verbatim
!>          ZERO is REAL
!>          Must contain the value 0.0
!>          This is passed to prevent the compiler from optimizing
!>          away this code.
!> \endverbatim
!>
!> \param[in] ONE
!> \verbatim
!>          ONE is REAL
!>          Must contain the value 1.0
!>          This is passed to prevent the compiler from optimizing
!>          away this code.
!>
!>  RETURN VALUE:  INTEGER
!>          = 0:  Arithmetic failed to produce the correct answers
!>          = 1:  Arithmetic produced the correct answers
!> \endverbatim
!
!  Authors:
!  ========
!
!> \author Univ. of Tennessee
!> \author Univ. of California Berkeley
!> \author Univ. of Colorado Denver
!> \author NAG Ltd.
!
!> \date November 2011
!
!> \ingroup auxOTHERauxiliary
!
!  =====================================================================
      INTEGER          FUNCTION IEEECK( ISPEC, ZERO, ONE )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.0) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2011
!
!     .. Scalar Arguments ..
      INTEGER            ISPEC
      REAL               ONE, ZERO
!     ..
!
!  =====================================================================
!
!     .. Local Scalars ..
      REAL               NAN1, NAN2, NAN3, NAN4, NAN5, NAN6, NEGINF, &
                         NEGZRO, NEWZRO, POSINF
!     ..
!     .. Executable Statements ..
      IEEECK = 1
!
      POSINF = ONE / ZERO
      IF( POSINF.LE.ONE ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      NEGINF = -ONE / ZERO
      IF( NEGINF.GE.ZERO ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      NEGZRO = ONE / ( NEGINF+ONE )
      IF( NEGZRO.NE.ZERO ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      NEGINF = ONE / NEGZRO
      IF( NEGINF.GE.ZERO ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      NEWZRO = NEGZRO + ZERO
      IF( NEWZRO.NE.ZERO ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      POSINF = ONE / NEWZRO
      IF( POSINF.LE.ONE ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      NEGINF = NEGINF*POSINF
      IF( NEGINF.GE.ZERO ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      POSINF = POSINF*POSINF
      IF( POSINF.LE.ONE ) THEN
         IEEECK = 0
         RETURN
      END IF
!
!
!
!
!     Return if we were only asked to check infinity arithmetic
!
      IF( ISPEC.EQ.0 ) &
         RETURN
!
      NAN1 = POSINF + NEGINF
!
      NAN2 = POSINF / NEGINF
!
      NAN3 = POSINF / POSINF
!
      NAN4 = POSINF*ZERO
!
      NAN5 = NEGINF*NEGZRO
!
      NAN6 = NAN5*ZERO
!
      IF( NAN1.EQ.NAN1 ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      IF( NAN2.EQ.NAN2 ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      IF( NAN3.EQ.NAN3 ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      IF( NAN4.EQ.NAN4 ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      IF( NAN5.EQ.NAN5 ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      IF( NAN6.EQ.NAN6 ) THEN
         IEEECK = 0
         RETURN
      END IF
!
      RETURN
      END
!> \brief \b ILAENV
!
!  =========== DOCUMENTATION ===========
!
! Online html documentation available at
!            http://www.netlib.org/lapack/explore-html/
!
!> \htmlonly
!> Download ILAENV + dependencies
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/ilaenv.f">
!> [TGZ]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/ilaenv.f">
!> [ZIP]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/ilaenv.f">
!> [TXT]</a>
!> \endhtmlonly
!
!  Definition:
!  ===========
!
!       INTEGER FUNCTION ILAENV( ISPEC, NAME, OPTS, N1, N2, N3, N4 )
!
!       .. Scalar Arguments ..
!       CHARACTER*( * )    NAME, OPTS
!       INTEGER            ISPEC, N1, N2, N3, N4
!       ..
!
!
!> \par Purpose:
!  =============
!>
!> \verbatim
!>
!> ILAENV is called from the LAPACK routines to choose problem-dependent
!> parameters for the local environment.  See ISPEC for a description of
!> the parameters.
!>
!> ILAENV returns an INTEGER
!> if ILAENV >= 0: ILAENV returns the value of the parameter specified by ISPEC
!> if ILAENV < 0:  if ILAENV = -k, the k-th argument had an illegal value.
!>
!> This version provides a set of parameters which should give good,
!> but not optimal, performance on many of the currently available
!> computers.  Users are encouraged to modify this subroutine to set
!> the tuning parameters for their particular machine using the option
!> and problem size information in the arguments.
!>
!> This routine will not function correctly if it is converted to all
!> lower case.  Converting it to all upper case is allowed.
!> \endverbatim
!
!  Arguments:
!  ==========
!
!> \param[in] ISPEC
!> \verbatim
!>          ISPEC is INTEGER
!>          Specifies the parameter to be returned as the value of
!>          ILAENV.
!>          = 1: the optimal blocksize; if this value is 1, an unblocked
!>               algorithm will give the best performance.
!>          = 2: the minimum block size for which the block routine
!>               should be used; if the usable block size is less than
!>               this value, an unblocked routine should be used.
!>          = 3: the crossover point (in a block routine, for N less
!>               than this value, an unblocked routine should be used)
!>          = 4: the number of shifts, used in the nonsymmetric
!>               eigenvalue routines (DEPRECATED)
!>          = 5: the minimum column dimension for blocking to be used;
!>               rectangular blocks must have dimension at least k by m,
!>               where k is given by ILAENV(2,...) and m by ILAENV(5,...)
!>          = 6: the crossover point for the SVD (when reducing an m by n
!>               matrix to bidiagonal form, if max(m,n)/min(m,n) exceeds
!>               this value, a QR factorization is used first to reduce
!>               the matrix to a triangular form.)
!>          = 7: the number of processors
!>          = 8: the crossover point for the multishift QR method
!>               for nonsymmetric eigenvalue problems (DEPRECATED)
!>          = 9: maximum size of the subproblems at the bottom of the
!>               computation tree in the divide-and-conquer algorithm
!>               (used by xGELSD and xGESDD)
!>          =10: ieee NaN arithmetic can be trusted not to trap
!>          =11: infinity arithmetic can be trusted not to trap
!>          12 <= ISPEC <= 16:
!>               xHSEQR or one of its subroutines,
!>               see IPARMQ for detailed explanation
!> \endverbatim
!>
!> \param[in] NAME
!> \verbatim
!>          NAME is CHARACTER*(*)
!>          The name of the calling subroutine, in either upper case or
!>          lower case.
!> \endverbatim
!>
!> \param[in] OPTS
!> \verbatim
!>          OPTS is CHARACTER*(*)
!>          The character options to the subroutine NAME, concatenated
!>          into a single character string.  For example, UPLO = 'U',
!>          TRANS = 'T', and DIAG = 'N' for a triangular routine would
!>          be specified as OPTS = 'UTN'.
!> \endverbatim
!>
!> \param[in] N1
!> \verbatim
!>          N1 is INTEGER
!> \endverbatim
!>
!> \param[in] N2
!> \verbatim
!>          N2 is INTEGER
!> \endverbatim
!>
!> \param[in] N3
!> \verbatim
!>          N3 is INTEGER
!> \endverbatim
!>
!> \param[in] N4
!> \verbatim
!>          N4 is INTEGER
!>          Problem dimensions for the subroutine NAME; these may not all
!>          be required.
!> \endverbatim
!
!  Authors:
!  ========
!
!> \author Univ. of Tennessee
!> \author Univ. of California Berkeley
!> \author Univ. of Colorado Denver
!> \author NAG Ltd.
!
!> \date November 2011
!
!> \ingroup auxOTHERauxiliary
!
!> \par Further Details:
!  =====================
!>
!> \verbatim
!>
!>  The following conventions have been used when calling ILAENV from the
!>  LAPACK routines:
!>  1)  OPTS is a concatenation of all of the character options to
!>      subroutine NAME, in the same order that they appear in the
!>      argument list for NAME, even if they are not used in determining
!>      the value of the parameter specified by ISPEC.
!>  2)  The problem dimensions N1, N2, N3, N4 are specified in the order
!>      that they appear in the argument list for NAME.  N1 is used
!>      first, N2 second, and so on, and unused problem dimensions are
!>      passed a value of -1.
!>  3)  The parameter value returned by ILAENV is checked for validity in
!>      the calling subroutine.  For example, ILAENV is used to retrieve
!>      the optimal blocksize for STRTRI as follows:
!>
!>      NB = ILAENV( 1, 'STRTRI', UPLO // DIAG, N, -1, -1, -1 )
!>      IF( NB.LE.1 ) NB = MAX( 1, N )
!> \endverbatim
!>
!  =====================================================================
      INTEGER FUNCTION ILAENV( ISPEC, NAME, OPTS, N1, N2, N3, N4 )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.0) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2011
!
!     .. Scalar Arguments ..
      CHARACTER( * )     NAME, OPTS
      INTEGER            ISPEC, N1, N2, N3, N4
!     ..
!
!  =====================================================================
!
!     .. Local Scalars ..
      INTEGER            I, IC, IZ, NB, NBMIN, NX
      LOGICAL            CNAME, SNAME
      CHARACTER          C1*1, C2*2, C4*2, C3*3, SUBNAM*6
!     ..
!     .. External Functions ..
      INTEGER            IEEECK, IPARMQ
      EXTERNAL           IEEECK, IPARMQ
!     ..
!     .. Executable Statements ..
!
      GO TO ( 10, 10, 10, 80, 90, 100, 110, 120, &
              130, 140, 150, 160, 160, 160, 160, 160 )ISPEC
!
!     Invalid value for ISPEC
!
      ILAENV = -1
      RETURN
!
   10 CONTINUE
!
!     Convert NAME to upper case if the first character is lower case.
!
      ILAENV = 1
      SUBNAM = NAME
      IC = ICHAR( SUBNAM( 1: 1 ) )
      IZ = ICHAR( 'Z' )
      IF( IZ.EQ.90 .OR. IZ.EQ.122 ) THEN
!
!        ASCII character set
!
         IF( IC.GE.97 .AND. IC.LE.122 ) THEN
            SUBNAM( 1: 1 ) = CHAR( IC-32 )
            DO 20 I = 2, 6
               IC = ICHAR( SUBNAM( I: I ) )
               IF( IC.GE.97 .AND. IC.LE.122 ) &
                  SUBNAM( I: I ) = CHAR( IC-32 )
   20       CONTINUE
         END IF
!
      ELSE IF( IZ.EQ.233 .OR. IZ.EQ.169 ) THEN
!
!        EBCDIC character set
!
         IF( ( IC.GE.129 .AND. IC.LE.137 ) .OR. &
             ( IC.GE.145 .AND. IC.LE.153 ) .OR. &
             ( IC.GE.162 .AND. IC.LE.169 ) ) THEN
            SUBNAM( 1: 1 ) = CHAR( IC+64 )
            DO 30 I = 2, 6
               IC = ICHAR( SUBNAM( I: I ) )
               IF( ( IC.GE.129 .AND. IC.LE.137 ) .OR. &
                   ( IC.GE.145 .AND. IC.LE.153 ) .OR. &
                   ( IC.GE.162 .AND. IC.LE.169 ) )SUBNAM( I: &
                   I ) = CHAR( IC+64 )
   30       CONTINUE
         END IF
!
      ELSE IF( IZ.EQ.218 .OR. IZ.EQ.250 ) THEN
!
!        Prime machines:  ASCII+128
!
         IF( IC.GE.225 .AND. IC.LE.250 ) THEN
            SUBNAM( 1: 1 ) = CHAR( IC-32 )
            DO 40 I = 2, 6
               IC = ICHAR( SUBNAM( I: I ) )
               IF( IC.GE.225 .AND. IC.LE.250 ) &
                  SUBNAM( I: I ) = CHAR( IC-32 )
   40       CONTINUE
         END IF
      END IF
!
      C1 = SUBNAM( 1: 1 )
      SNAME = C1.EQ.'S' .OR. C1.EQ.'D'
      CNAME = C1.EQ.'C' .OR. C1.EQ.'Z'
      IF( .NOT.( CNAME .OR. SNAME ) ) &
         RETURN
      C2 = SUBNAM( 2: 3 )
      C3 = SUBNAM( 4: 6 )
      C4 = C3( 2: 3 )
!
      GO TO ( 50, 60, 70 )ISPEC
!
   50 CONTINUE
!
!     ISPEC = 1:  block size
!
!     In these examples, separate code is provided for setting NB for
!     real and complex.  We assume that NB will take the same value in
!     single or double precision.
!
      NB = 1
!
      IF( C2.EQ.'GE' ) THEN
         IF( C3.EQ.'TRF' ) THEN
            IF( SNAME ) THEN
               NB = 64
            ELSE
               NB = 64
            END IF
         ELSE IF( C3.EQ.'QRF' .OR. C3.EQ.'RQF' .OR. C3.EQ.'LQF' .OR. &
                  C3.EQ.'QLF' ) THEN
            IF( SNAME ) THEN
               NB = 32
            ELSE
               NB = 32
            END IF
         ELSE IF( C3.EQ.'HRD' ) THEN
            IF( SNAME ) THEN
               NB = 32
            ELSE
               NB = 32
            END IF
         ELSE IF( C3.EQ.'BRD' ) THEN
            IF( SNAME ) THEN
               NB = 32
            ELSE
               NB = 32
            END IF
         ELSE IF( C3.EQ.'TRI' ) THEN
            IF( SNAME ) THEN
               NB = 64
            ELSE
               NB = 64
            END IF
         END IF
      ELSE IF( C2.EQ.'PO' ) THEN
         IF( C3.EQ.'TRF' ) THEN
            IF( SNAME ) THEN
               NB = 64
            ELSE
               NB = 64
            END IF
         END IF
      ELSE IF( C2.EQ.'SY' ) THEN
         IF( C3.EQ.'TRF' ) THEN
            IF( SNAME ) THEN
               NB = 64
            ELSE
               NB = 64
            END IF
         ELSE IF( SNAME .AND. C3.EQ.'TRD' ) THEN
            NB = 32
         ELSE IF( SNAME .AND. C3.EQ.'GST' ) THEN
            NB = 64
         END IF
      ELSE IF( CNAME .AND. C2.EQ.'HE' ) THEN
         IF( C3.EQ.'TRF' ) THEN
            NB = 64
         ELSE IF( C3.EQ.'TRD' ) THEN
            NB = 32
         ELSE IF( C3.EQ.'GST' ) THEN
            NB = 64
         END IF
      ELSE IF( SNAME .AND. C2.EQ.'OR' ) THEN
         IF( C3( 1: 1 ).EQ.'G' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NB = 32
            END IF
         ELSE IF( C3( 1: 1 ).EQ.'M' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NB = 32
            END IF
         END IF
      ELSE IF( CNAME .AND. C2.EQ.'UN' ) THEN
         IF( C3( 1: 1 ).EQ.'G' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NB = 32
            END IF
         ELSE IF( C3( 1: 1 ).EQ.'M' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NB = 32
            END IF
         END IF
      ELSE IF( C2.EQ.'GB' ) THEN
         IF( C3.EQ.'TRF' ) THEN
            IF( SNAME ) THEN
               IF( N4.LE.64 ) THEN
                  NB = 1
               ELSE
                  NB = 32
               END IF
            ELSE
               IF( N4.LE.64 ) THEN
                  NB = 1
               ELSE
                  NB = 32
               END IF
            END IF
         END IF
      ELSE IF( C2.EQ.'PB' ) THEN
         IF( C3.EQ.'TRF' ) THEN
            IF( SNAME ) THEN
               IF( N2.LE.64 ) THEN
                  NB = 1
               ELSE
                  NB = 32
               END IF
            ELSE
               IF( N2.LE.64 ) THEN
                  NB = 1
               ELSE
                  NB = 32
               END IF
            END IF
         END IF
      ELSE IF( C2.EQ.'TR' ) THEN
         IF( C3.EQ.'TRI' ) THEN
            IF( SNAME ) THEN
               NB = 64
            ELSE
               NB = 64
            END IF
         END IF
      ELSE IF( C2.EQ.'LA' ) THEN
         IF( C3.EQ.'UUM' ) THEN
            IF( SNAME ) THEN
               NB = 64
            ELSE
               NB = 64
            END IF
         END IF
      ELSE IF( SNAME .AND. C2.EQ.'ST' ) THEN
         IF( C3.EQ.'EBZ' ) THEN
            NB = 1
         END IF
      END IF
      ILAENV = NB
      RETURN
!
   60 CONTINUE
!
!     ISPEC = 2:  minimum block size
!
      NBMIN = 2
      IF( C2.EQ.'GE' ) THEN
         IF( C3.EQ.'QRF' .OR. C3.EQ.'RQF' .OR. C3.EQ.'LQF' .OR. C3.EQ. &
             'QLF' ) THEN
            IF( SNAME ) THEN
               NBMIN = 2
            ELSE
               NBMIN = 2
            END IF
         ELSE IF( C3.EQ.'HRD' ) THEN
            IF( SNAME ) THEN
               NBMIN = 2
            ELSE
               NBMIN = 2
            END IF
         ELSE IF( C3.EQ.'BRD' ) THEN
            IF( SNAME ) THEN
               NBMIN = 2
            ELSE
               NBMIN = 2
            END IF
         ELSE IF( C3.EQ.'TRI' ) THEN
            IF( SNAME ) THEN
               NBMIN = 2
            ELSE
               NBMIN = 2
            END IF
         END IF
      ELSE IF( C2.EQ.'SY' ) THEN
         IF( C3.EQ.'TRF' ) THEN
            IF( SNAME ) THEN
               NBMIN = 8
            ELSE
               NBMIN = 8
            END IF
         ELSE IF( SNAME .AND. C3.EQ.'TRD' ) THEN
            NBMIN = 2
         END IF
      ELSE IF( CNAME .AND. C2.EQ.'HE' ) THEN
         IF( C3.EQ.'TRD' ) THEN
            NBMIN = 2
         END IF
      ELSE IF( SNAME .AND. C2.EQ.'OR' ) THEN
         IF( C3( 1: 1 ).EQ.'G' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NBMIN = 2
            END IF
         ELSE IF( C3( 1: 1 ).EQ.'M' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NBMIN = 2
            END IF
         END IF
      ELSE IF( CNAME .AND. C2.EQ.'UN' ) THEN
         IF( C3( 1: 1 ).EQ.'G' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NBMIN = 2
            END IF
         ELSE IF( C3( 1: 1 ).EQ.'M' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NBMIN = 2
            END IF
         END IF
      END IF
      ILAENV = NBMIN
      RETURN
!
   70 CONTINUE
!
!     ISPEC = 3:  crossover point
!
      NX = 0
      IF( C2.EQ.'GE' ) THEN
         IF( C3.EQ.'QRF' .OR. C3.EQ.'RQF' .OR. C3.EQ.'LQF' .OR. C3.EQ. &
             'QLF' ) THEN
            IF( SNAME ) THEN
               NX = 128
            ELSE
               NX = 128
            END IF
         ELSE IF( C3.EQ.'HRD' ) THEN
            IF( SNAME ) THEN
               NX = 128
            ELSE
               NX = 128
            END IF
         ELSE IF( C3.EQ.'BRD' ) THEN
            IF( SNAME ) THEN
               NX = 128
            ELSE
               NX = 128
            END IF
         END IF
      ELSE IF( C2.EQ.'SY' ) THEN
         IF( SNAME .AND. C3.EQ.'TRD' ) THEN
            NX = 32
         END IF
      ELSE IF( CNAME .AND. C2.EQ.'HE' ) THEN
         IF( C3.EQ.'TRD' ) THEN
            NX = 32
         END IF
      ELSE IF( SNAME .AND. C2.EQ.'OR' ) THEN
         IF( C3( 1: 1 ).EQ.'G' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NX = 128
            END IF
         END IF
      ELSE IF( CNAME .AND. C2.EQ.'UN' ) THEN
         IF( C3( 1: 1 ).EQ.'G' ) THEN
            IF( C4.EQ.'QR' .OR. C4.EQ.'RQ' .OR. C4.EQ.'LQ' .OR. C4.EQ. &
                'QL' .OR. C4.EQ.'HR' .OR. C4.EQ.'TR' .OR. C4.EQ.'BR' ) &
                 THEN
               NX = 128
            END IF
         END IF
      END IF
      ILAENV = NX
      RETURN
!
   80 CONTINUE
!
!     ISPEC = 4:  number of shifts (used by xHSEQR)
!
      ILAENV = 6
      RETURN
!
   90 CONTINUE
!
!     ISPEC = 5:  minimum column dimension (not used)
!
      ILAENV = 2
      RETURN
!
  100 CONTINUE
!
!     ISPEC = 6:  crossover point for SVD (used by xGELSS and xGESVD)
!
      ILAENV = INT( REAL( MIN( N1, N2 ) )*1.6E0 )
      RETURN
!
  110 CONTINUE
!
!     ISPEC = 7:  number of processors (not used)
!
      ILAENV = 1
      RETURN
!
  120 CONTINUE
!
!     ISPEC = 8:  crossover point for multishift (used by xHSEQR)
!
      ILAENV = 50
      RETURN
!
  130 CONTINUE
!
!     ISPEC = 9:  maximum size of the subproblems at the bottom of the
!                 computation tree in the divide-and-conquer algorithm
!                 (used by xGELSD and xGESDD)
!
      ILAENV = 25
      RETURN
!
  140 CONTINUE
!
!     ISPEC = 10: ieee NaN arithmetic can be trusted not to trap
!
!     ILAENV = 0
      ILAENV = 1
      IF( ILAENV.EQ.1 ) THEN
         ILAENV = IEEECK( 1, 0.0, 1.0 )
      END IF
      RETURN
!
  150 CONTINUE
!
!     ISPEC = 11: infinity arithmetic can be trusted not to trap
!
!     ILAENV = 0
      ILAENV = 1
      IF( ILAENV.EQ.1 ) THEN
         ILAENV = IEEECK( 0, 0.0, 1.0 )
      END IF
      RETURN
!
  160 CONTINUE
!
!     12 <= ISPEC <= 16: xHSEQR or one of its subroutines.
!
      ILAENV = IPARMQ( ISPEC, NAME, OPTS, N1, N2, N3, N4 )
      RETURN
!
!     End of ILAENV
!
      END
!> \brief \b ILASLC scans a matrix for its last non-zero column.
!
!  =========== DOCUMENTATION ===========
!
! Online html documentation available at
!            http://www.netlib.org/lapack/explore-html/
!
!> \htmlonly
!> Download ILASLC + dependencies
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/ilaslc.f">
!> [TGZ]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/ilaslc.f">
!> [ZIP]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/ilaslc.f">
!> [TXT]</a>
!> \endhtmlonly
!
!  Definition:
!  ===========
!
!       INTEGER FUNCTION ILASLC( M, N, A, LDA )
!
!       .. Scalar Arguments ..
!       INTEGER            M, N, LDA
!       ..
!       .. Array Arguments ..
!       REAL               A( LDA, * )
!       ..
!
!
!> \par Purpose:
!  =============
!>
!> \verbatim
!>
!> ILASLC scans A for its last non-zero column.
!> \endverbatim
!
!  Arguments:
!  ==========
!
!> \param[in] M
!> \verbatim
!>          M is INTEGER
!>          The number of rows of the matrix A.
!> \endverbatim
!>
!> \param[in] N
!> \verbatim
!>          N is INTEGER
!>          The number of columns of the matrix A.
!> \endverbatim
!>
!> \param[in] A
!> \verbatim
!>          A is REAL array, dimension (LDA,N)
!>          The m by n matrix A.
!> \endverbatim
!>
!> \param[in] LDA
!> \verbatim
!>          LDA is INTEGER
!>          The leading dimension of the array A. LDA >= max(1,M).
!> \endverbatim
!
!  Authors:
!  ========
!
!> \author Univ. of Tennessee
!> \author Univ. of California Berkeley
!> \author Univ. of Colorado Denver
!> \author NAG Ltd.
!
!> \date September 2012
!
!> \ingroup realOTHERauxiliary
!
!  =====================================================================
      INTEGER FUNCTION ILASLC( M, N, A, LDA )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     September 2012
!
!     .. Scalar Arguments ..
      INTEGER            M, N, LDA
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * )
!     ..
!
!  =====================================================================
!
!     .. Parameters ..
      REAL             ZERO
      PARAMETER ( ZERO = 0.0D+0 )
!     ..
!     .. Local Scalars ..
      INTEGER I
!     ..
!     .. Executable Statements ..
!
!     Quick test for the common case where one corner is non-zero.
      IF( N.EQ.0 ) THEN
         ILASLC = N
      ELSE IF( A(1, N).NE.ZERO .OR. A(M, N).NE.ZERO ) THEN
         ILASLC = N
      ELSE
!     Now scan each column from the end, returning with the first non-zero.
         DO ILASLC = N, 1, -1
            DO I = 1, M
               IF( A(I, ILASLC).NE.ZERO ) RETURN
            END DO
         END DO
      END IF
      RETURN
      END
!> \brief \b ILASLR scans a matrix for its last non-zero row.
!
!  =========== DOCUMENTATION ===========
!
! Online html documentation available at
!            http://www.netlib.org/lapack/explore-html/
!
!> \htmlonly
!> Download ILASLR + dependencies
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/ilaslr.f">
!> [TGZ]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/ilaslr.f">
!> [ZIP]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/ilaslr.f">
!> [TXT]</a>
!> \endhtmlonly
!
!  Definition:
!  ===========
!
!       INTEGER FUNCTION ILASLR( M, N, A, LDA )
!
!       IMPLICIT NONE
!
!       .. Scalar Arguments ..
!       INTEGER            M, N, LDA
!       ..
!       .. Array Arguments ..
!       REAL               A( LDA, * )
!       ..
!
!
!> \par Purpose:
!  =============
!>
!> \verbatim
!>
!> ILASLR scans A for its last non-zero row.
!> \endverbatim
!
!  Arguments:
!  ==========
!
!> \param[in] M
!> \verbatim
!>          M is INTEGER
!>          The number of rows of the matrix A.
!> \endverbatim
!>
!> \param[in] N
!> \verbatim
!>          N is INTEGER
!>          The number of columns of the matrix A.
!> \endverbatim
!>
!> \param[in] A
!> \verbatim
!>          A is REAL array, dimension (LDA,N)
!>          The m by n matrix A.
!> \endverbatim
!>
!> \param[in] LDA
!> \verbatim
!>          LDA is INTEGER
!>          The leading dimension of the array A. LDA >= max(1,M).
!> \endverbatim
!
!  Authors:
!  ========
!
!> \author Univ. of Tennessee
!> \author Univ. of California Berkeley
!> \author Univ. of Colorado Denver
!> \author NAG Ltd.
!
!> \date September 2012
!
!> \ingroup realOTHERauxiliary
!
!  =====================================================================
      INTEGER FUNCTION ILASLR( M, N, A, LDA )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     September 2012
!
!     .. Scalar Arguments ..
      INTEGER            M, N, LDA
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * )
!     ..
!
!  =====================================================================
!
!     .. Parameters ..
      REAL             ZERO
      PARAMETER ( ZERO = 0.0E+0 )
!     ..
!     .. Local Scalars ..
      INTEGER I, J
!     ..
!     .. Executable Statements ..
!
!     Quick test for the common case where one corner is non-zero.
      IF( M.EQ.0 ) THEN
         ILASLR = M
      ELSEIF( A(M, 1).NE.ZERO .OR. A(M, N).NE.ZERO ) THEN
         ILASLR = M
      ELSE
!     Scan up each column tracking the last zero row seen.
         ILASLR = 0
         DO J = 1, N
            I=M
            DO WHILE((A(MAX(I,1),J).EQ.ZERO).AND.(I.GE.1))
               I=I-1
            ENDDO
            ILASLR = MAX( ILASLR, I )
         END DO
      END IF
      RETURN
      END
!> \brief \b IPARMQ
!
!  =========== DOCUMENTATION ===========
!
! Online html documentation available at
!            http://www.netlib.org/lapack/explore-html/
!
!> \htmlonly
!> Download IPARMQ + dependencies
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/iparmq.f">
!> [TGZ]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/iparmq.f">
!> [ZIP]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/iparmq.f">
!> [TXT]</a>
!> \endhtmlonly
!
!  Definition:
!  ===========
!
!       INTEGER FUNCTION IPARMQ( ISPEC, NAME, OPTS, N, ILO, IHI, LWORK )
!
!       IMPLICIT NONE
!
!       .. Scalar Arguments ..
!       INTEGER            IHI, ILO, ISPEC, LWORK, N
!       CHARACTER          NAME*( * ), OPTS*( * )
!
!
!> \par Purpose:
!  =============
!>
!> \verbatim
!>
!>      This program sets problem and machine dependent parameters
!>      useful for xHSEQR and its subroutines. It is called whenever
!>      ILAENV is called with 12 <= ISPEC <= 16
!> \endverbatim
!
!  Arguments:
!  ==========
!
!> \param[in] ISPEC
!> \verbatim
!>          ISPEC is integer scalar
!>              ISPEC specifies which tunable parameter IPARMQ should
!>              return.
!>
!>              ISPEC=12: (INMIN)  Matrices of order nmin or less
!>                        are sent directly to xLAHQR, the implicit
!>                        double shift QR algorithm.  NMIN must be
!>                        at least 11.
!>
!>              ISPEC=13: (INWIN)  Size of the deflation window.
!>                        This is best set greater than or equal to
!>                        the number of simultaneous shifts NS.
!>                        Larger matrices benefit from larger deflation
!>                        windows.
!>
!>              ISPEC=14: (INIBL) Determines when to stop nibbling and
!>                        invest in an (expensive) multi-shift QR sweep.
!>                        If the aggressive early deflation subroutine
!>                        finds LD converged eigenvalues from an order
!>                        NW deflation window and LD.GT.(NW*NIBBLE)/100,
!>                        then the next QR sweep is skipped and early
!>                        deflation is applied immediately to the
!>                        remaining active diagonal block.  Setting
!>                        IPARMQ(ISPEC=14) = 0 causes TTQRE to skip a
!>                        multi-shift QR sweep whenever early deflation
!>                        finds a converged eigenvalue.  Setting
!>                        IPARMQ(ISPEC=14) greater than or equal to 100
!>                        prevents TTQRE from skipping a multi-shift
!>                        QR sweep.
!>
!>              ISPEC=15: (NSHFTS) The number of simultaneous shifts in
!>                        a multi-shift QR iteration.
!>
!>              ISPEC=16: (IACC22) IPARMQ is set to 0, 1 or 2 with the
!>                        following meanings.
!>                        0:  During the multi-shift QR sweep,
!>                            xLAQR5 does not accumulate reflections and
!>                            does not use matrix-matrix multiply to
!>                            update the far-from-diagonal matrix
!>                            entries.
!>                        1:  During the multi-shift QR sweep,
!>                            xLAQR5 and/or xLAQRaccumulates reflections and uses
!>                            matrix-matrix multiply to update the
!>                            far-from-diagonal matrix entries.
!>                        2:  During the multi-shift QR sweep.
!>                            xLAQR5 accumulates reflections and takes
!>                            advantage of 2-by-2 block structure during
!>                            matrix-matrix multiplies.
!>                        (If xTRMM is slower than xGEMM, then
!>                        IPARMQ(ISPEC=16)=1 may be more efficient than
!>                        IPARMQ(ISPEC=16)=2 despite the greater level of
!>                        arithmetic work implied by the latter choice.)
!> \endverbatim
!>
!> \param[in] NAME
!> \verbatim
!>          NAME is character string
!>               Name of the calling subroutine
!> \endverbatim
!>
!> \param[in] OPTS
!> \verbatim
!>          OPTS is character string
!>               This is a concatenation of the string arguments to
!>               TTQRE.
!> \endverbatim
!>
!> \param[in] N
!> \verbatim
!>          N is integer scalar
!>               N is the order of the Hessenberg matrix H.
!> \endverbatim
!>
!> \param[in] ILO
!> \verbatim
!>          ILO is INTEGER
!> \endverbatim
!>
!> \param[in] IHI
!> \verbatim
!>          IHI is INTEGER
!>               It is assumed that H is already upper triangular
!>               in rows and columns 1:ILO-1 and IHI+1:N.
!> \endverbatim
!>
!> \param[in] LWORK
!> \verbatim
!>          LWORK is integer scalar
!>               The amount of workspace available.
!> \endverbatim
!
!  Authors:
!  ========
!
!> \author Univ. of Tennessee
!> \author Univ. of California Berkeley
!> \author Univ. of Colorado Denver
!> \author NAG Ltd.
!
!> \date November 2011
!
!> \ingroup auxOTHERauxiliary
!
!> \par Further Details:
!  =====================
!>
!> \verbatim
!>
!>       Little is known about how best to choose these parameters.
!>       It is possible to use different values of the parameters
!>       for each of CHSEQR, DHSEQR, SHSEQR and ZHSEQR.
!>
!>       It is probably best to choose different parameters for
!>       different matrices and different parameters at different
!>       times during the iteration, but this has not been
!>       implemented --- yet.
!>
!>
!>       The best choices of most of the parameters depend
!>       in an ill-understood way on the relative execution
!>       rate of xLAQR3 and xLAQR5 and on the nature of each
!>       particular eigenvalue problem.  Experiment may be the
!>       only practical way to determine which choices are most
!>       effective.
!>
!>       Following is a list of default values supplied by IPARMQ.
!>       These defaults may be adjusted in order to attain better
!>       performance in any particular computational environment.
!>
!>       IPARMQ(ISPEC=12) The xLAHQR vs xLAQR0 crossover point.
!>                        Default: 75. (Must be at least 11.)
!>
!>       IPARMQ(ISPEC=13) Recommended deflation window size.
!>                        This depends on ILO, IHI and NS, the
!>                        number of simultaneous shifts returned
!>                        by IPARMQ(ISPEC=15).  The default for
!>                        (IHI-ILO+1).LE.500 is NS.  The default
!>                        for (IHI-ILO+1).GT.500 is 3*NS/2.
!>
!>       IPARMQ(ISPEC=14) Nibble crossover point.  Default: 14.
!>
!>       IPARMQ(ISPEC=15) Number of simultaneous shifts, NS.
!>                        a multi-shift QR iteration.
!>
!>                        If IHI-ILO+1 is ...
!>
!>                        greater than      ...but less    ... the
!>                        or equal to ...      than        default is
!>
!>                                0               30       NS =   2+
!>                               30               60       NS =   4+
!>                               60              150       NS =  10
!>                              150              590       NS =  **
!>                              590             3000       NS =  64
!>                             3000             6000       NS = 128
!>                             6000             infinity   NS = 256
!>
!>                    (+)  By default matrices of this order are
!>                         passed to the implicit double shift routine
!>                         xLAHQR.  See IPARMQ(ISPEC=12) above.   These
!>                         values of NS are used only in case of a rare
!>                         xLAHQR failure.
!>
!>                    (**) The asterisks (**) indicate an ad-hoc
!>                         function increasing from 10 to 64.
!>
!>       IPARMQ(ISPEC=16) Select structured matrix multiply.
!>                        (See ISPEC=16 above for details.)
!>                        Default: 3.
!> \endverbatim
!>
!  =====================================================================
      INTEGER FUNCTION IPARMQ( ISPEC, NAME, OPTS, N, ILO, IHI, LWORK )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.0) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2011
!
!     .. Scalar Arguments ..
      INTEGER            IHI, ILO, ISPEC, LWORK, N
      CHARACTER          NAME*( * ), OPTS*( * )
!
!  ================================================================
!     .. Parameters ..
      INTEGER            INMIN, INWIN, INIBL, ISHFTS, IACC22
      PARAMETER          ( INMIN = 12, INWIN = 13, INIBL = 14, &
                         ISHFTS = 15, IACC22 = 16 )
      INTEGER            NMIN, K22MIN, KACMIN, NIBBLE, KNWSWP
      PARAMETER          ( NMIN = 75, K22MIN = 14, KACMIN = 14, &
                         NIBBLE = 14, KNWSWP = 500 )
      REAL               TWO
      PARAMETER          ( TWO = 2.0 )
!     ..
!     .. Local Scalars ..
      INTEGER            NH, NS
!     ..
!     .. Executable Statements ..
      IF( ( ISPEC.EQ.ISHFTS ) .OR. ( ISPEC.EQ.INWIN ) .OR. &
          ( ISPEC.EQ.IACC22 ) ) THEN
!
!        ==== Set the number simultaneous shifts ====
!
         NH = IHI - ILO + 1
         NS = 2
         IF( NH.GE.30 ) &
            NS = 4
         IF( NH.GE.60 ) &
            NS = 10
         IF( NH.GE.150 ) &
            NS = MAX( 10, NH / NINT( LOG( REAL( NH ) ) / LOG( TWO ) ) )
         IF( NH.GE.590 ) &
            NS = 64
         IF( NH.GE.3000 ) &
            NS = 128
         IF( NH.GE.6000 ) &
            NS = 256
         NS = MAX( 2, NS-MOD( NS, 2 ) )
      END IF
!
      IF( ISPEC.EQ.INMIN ) THEN
!
!
!        ===== Matrices of order smaller than NMIN get sent
!        .     to xLAHQR, the classic double shift algorithm.
!        .     This must be at least 11. ====
!
         IPARMQ = NMIN
!
      ELSE IF( ISPEC.EQ.INIBL ) THEN
!
!        ==== INIBL: skip a multi-shift qr iteration and
!        .    whenever aggressive early deflation finds
!        .    at least (NIBBLE*(window size)/100) deflations. ====
!
         IPARMQ = NIBBLE
!
      ELSE IF( ISPEC.EQ.ISHFTS ) THEN
!
!        ==== NSHFTS: The number of simultaneous shifts =====
!
         IPARMQ = NS
!
      ELSE IF( ISPEC.EQ.INWIN ) THEN
!
!        ==== NW: deflation window size.  ====
!
         IF( NH.LE.KNWSWP ) THEN
            IPARMQ = NS
         ELSE
            IPARMQ = 3*NS / 2
         END IF
!
      ELSE IF( ISPEC.EQ.IACC22 ) THEN
!
!        ==== IACC22: Whether to accumulate reflections
!        .     before updating the far-from-diagonal elements
!        .     and whether to use 2-by-2 block structure while
!        .     doing it.  A small amount of work could be saved
!        .     by making this choice dependent also upon the
!        .     NH=IHI-ILO+1.
!
         IPARMQ = 0
         IF( NS.GE.KACMIN ) &
            IPARMQ = 1
         IF( NS.GE.K22MIN ) &
            IPARMQ = 2
!
      ELSE
!        ===== invalid value of ispec =====
         IPARMQ = -1
!
      END IF
!
!     ==== End of IPARMQ ====
!
      END
!> \brief \b LSAME
!
!  =========== DOCUMENTATION ===========
!
! Online html documentation available at
!            http://www.netlib.org/lapack/explore-html/
!
!  Definition:
!  ===========
!
!      LOGICAL FUNCTION LSAME( CA, CB )
!
!      IMPLICIT NONE
!
!     .. Scalar Arguments ..
!      CHARACTER          CA, CB
!     ..
!
!
!> \par Purpose:
!  =============
!>
!> \verbatim
!>
!> LSAME returns .TRUE. if CA is the same letter as CB regardless of
!> case.
!> \endverbatim
!
!  Arguments:
!  ==========
!
!> \param[in] CA
!> \verbatim
!> \endverbatim
!>
!> \param[in] CB
!> \verbatim
!>          CA and CB specify the single characters to be compared.
!> \endverbatim
!
!  Authors:
!  ========
!
!> \author Univ. of Tennessee
!> \author Univ. of California Berkeley
!> \author Univ. of Colorado Denver
!> \author NAG Ltd.
!
!> \date November 2011
!
!> \ingroup auxOTHERauxiliary
!
!  =====================================================================
      LOGICAL FUNCTION LSAME( CA, CB )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.0) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2011
!
!     .. Scalar Arguments ..
      CHARACTER          CA, CB
!     ..
!
! =====================================================================
!
!     ..
!     .. Local Scalars ..
      INTEGER            INTA, INTB, ZCODE
!     ..
!     .. Executable Statements ..
!
!     Test if the characters are equal
!
      LSAME = CA.EQ.CB
      IF( LSAME ) &
         RETURN
!
!     Now test for equivalence if both characters are alphabetic.
!
      ZCODE = ICHAR( 'Z' )
!
!     Use 'Z' rather than 'A' so that ASCII can be detected on Prime
!     machines, on which ICHAR returns a value with bit 8 set.
!     ICHAR('A') on Prime machines returns 193 which is the same as
!     ICHAR('A') on an EBCDIC machine.
!
      INTA = ICHAR( CA )
      INTB = ICHAR( CB )
!
      IF( ZCODE.EQ.90 .OR. ZCODE.EQ.122 ) THEN
!
!        ASCII is assumed - ZCODE is the ASCII code of either lower or
!        upper case 'Z'.
!
         IF( INTA.GE.97 .AND. INTA.LE.122 ) INTA = INTA - 32
         IF( INTB.GE.97 .AND. INTB.LE.122 ) INTB = INTB - 32
!
      ELSE IF( ZCODE.EQ.233 .OR. ZCODE.EQ.169 ) THEN
!
!        EBCDIC is assumed - ZCODE is the EBCDIC code of either lower or
!        upper case 'Z'.
!
         IF( INTA.GE.129 .AND. INTA.LE.137 .OR. &
             INTA.GE.145 .AND. INTA.LE.153 .OR. &
             INTA.GE.162 .AND. INTA.LE.169 ) INTA = INTA + 64
         IF( INTB.GE.129 .AND. INTB.LE.137 .OR. &
             INTB.GE.145 .AND. INTB.LE.153 .OR. &
             INTB.GE.162 .AND. INTB.LE.169 ) INTB = INTB + 64
!
      ELSE IF( ZCODE.EQ.218 .OR. ZCODE.EQ.250 ) THEN
!
!        ASCII is assumed, on Prime machines - ZCODE is the ASCII code
!        plus 128 of either lower or upper case 'Z'.
!
         IF( INTA.GE.225 .AND. INTA.LE.250 ) INTA = INTA - 32
         IF( INTB.GE.225 .AND. INTB.LE.250 ) INTB = INTB - 32
      END IF
      LSAME = INTA.EQ.INTB
!
!     RETURN
!
!     End of LSAME
!
      END
!> \brief \b SLAMCH
!
!  =========== DOCUMENTATION ===========
!
! Online html documentation available at
!            http://www.netlib.org/lapack/explore-html/
!
!  Definition:
!  ===========
!
!      REAL             FUNCTION SLAMCH( CMACH )
!
!       IMPLICIT NONE
!
!     .. Scalar Arguments ..
!      CHARACTER          CMACH
!     ..
!
!
!> \par Purpose:
!  =============
!>
!> \verbatim
!>
!> SLAMCH determines single precision machine parameters.
!> \endverbatim
!
!  Arguments:
!  ==========
!
!> \param[in] CMACH
!> \verbatim
!>          Specifies the value to be returned by SLAMCH:
!>          = 'E' or 'e',   SLAMCH := eps
!>          = 'S' or 's ,   SLAMCH := sfmin
!>          = 'B' or 'b',   SLAMCH := base
!>          = 'P' or 'p',   SLAMCH := eps*base
!>          = 'N' or 'n',   SLAMCH := t
!>          = 'R' or 'r',   SLAMCH := rnd
!>          = 'M' or 'm',   SLAMCH := emin
!>          = 'U' or 'u',   SLAMCH := rmin
!>          = 'L' or 'l',   SLAMCH := emax
!>          = 'O' or 'o',   SLAMCH := rmax
!>          where
!>          eps   = relative machine precision
!>          sfmin = safe minimum, such that 1/sfmin does not overflow
!>          base  = base of the machine
!>          prec  = eps*base
!>          t     = number of (base) digits in the mantissa
!>          rnd   = 1.0 when rounding occurs in addition, 0.0 otherwise
!>          emin  = minimum exponent before (gradual) underflow
!>          rmin  = underflow threshold - base**(emin-1)
!>          emax  = largest exponent before overflow
!>          rmax  = overflow threshold  - (base**emax)*(1-eps)
!> \endverbatim
!
!  Authors:
!  ========
!
!> \author Univ. of Tennessee
!> \author Univ. of California Berkeley
!> \author Univ. of Colorado Denver
!> \author NAG Ltd.
!
!> \date November 2011
!
!> \ingroup auxOTHERauxiliary
!
!  =====================================================================
      REAL             FUNCTION SLAMCH( CMACH )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.0) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2011
!
!     .. Scalar Arguments ..
      CHARACTER          CMACH
!     ..
!
! =====================================================================
!
!     .. Parameters ..
      REAL               ONE, ZERO
      PARAMETER          ( ONE = 1.0E+0, ZERO = 0.0E+0 )
!     ..
!     .. Local Scalars ..
      REAL               RND, EPS, SFMIN, SMALL, RMACH
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!     ..
!     ..
!     .. Executable Statements ..
!
!
!     Assume rounding, not chopping. Always.
!
      RND = ONE
!
      IF( ONE.EQ.RND ) THEN
         EPS = EPSILON(ZERO) * 0.5
      ELSE
         EPS = EPSILON(ZERO)
      END IF
!
      IF( LSAME( CMACH, 'E' ) ) THEN
         RMACH = EPS
      ELSE IF( LSAME( CMACH, 'S' ) ) THEN
         SFMIN = TINY(ZERO)
         SMALL = ONE / HUGE(ZERO)
         IF( SMALL.GE.SFMIN ) THEN
!
!           Use SMALL plus a bit, to avoid the possibility of rounding
!           causing overflow when computing  1/sfmin.
!
            SFMIN = SMALL*( ONE+EPS )
         END IF
         RMACH = SFMIN
      ELSE IF( LSAME( CMACH, 'B' ) ) THEN
         RMACH = RADIX(ZERO)
      ELSE IF( LSAME( CMACH, 'P' ) ) THEN
         RMACH = EPS * RADIX(ZERO)
      ELSE IF( LSAME( CMACH, 'N' ) ) THEN
         RMACH = DIGITS(ZERO)
      ELSE IF( LSAME( CMACH, 'R' ) ) THEN
         RMACH = RND
      ELSE IF( LSAME( CMACH, 'M' ) ) THEN
         RMACH = MINEXPONENT(ZERO)
      ELSE IF( LSAME( CMACH, 'U' ) ) THEN
         RMACH = tiny(zero)
      ELSE IF( LSAME( CMACH, 'L' ) ) THEN
         RMACH = MAXEXPONENT(ZERO)
      ELSE IF( LSAME( CMACH, 'O' ) ) THEN
         RMACH = HUGE(ZERO)
      ELSE
         RMACH = ZERO
      END IF
!
      SLAMCH = RMACH
      RETURN
!
!     End of SLAMCH
!
      END
!***********************************************************************
!> \brief \b SLAMC3
!> \details
!> \b Purpose:
!> \verbatim
!> SLAMC3  is intended to force  A  and  B  to be stored prior to doing
!> the addition of  A  and  B ,  for use in situations where optimizers
!> might hold one of these in a register.
!> \endverbatim
!> \author LAPACK is a software package provided by Univ. of Tennessee,
!> Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..
!> \date November 2011
!> \ingroup auxOTHERauxiliary
!>
!> \param[in] A
!> \verbatim
!> \endverbatim
!>
!> \param[in] B
!> \verbatim
!>          The values A and B.
!> \endverbatim
!>
!
      REAL             FUNCTION SLAMC3( A, B )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.0) --
!     Univ. of Tennessee, Univ. of California Berkeley and NAG Ltd..
!     November 2010
!
!     .. Scalar Arguments ..
      REAL               A, B
!     ..
! =====================================================================
!
!     .. Executable Statements ..
!
      SLAMC3 = A + B
!
      RETURN
!
!     End of SLAMC3
!
      END
!
!***********************************************************************
!> \brief \b XERBLA
!
!  =========== DOCUMENTATION ===========
!
! Online html documentation available at
!            http://www.netlib.org/lapack/explore-html/
!
!> \htmlonly
!> Download XERBLA + dependencies
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/xerbla.f">
!> [TGZ]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/xerbla.f">
!> [ZIP]</a>
!> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/xerbla.f">
!> [TXT]</a>
!> \endhtmlonly
!
!  Definition:
!  ===========
!
!       SUBROUTINE XERBLA( SRNAME, INFO )
!
!       .. Scalar Arguments ..
!       CHARACTER*(*)      SRNAME
!       INTEGER            INFO
!       ..
!
!
!> \par Purpose:
!  =============
!>
!> \verbatim
!>
!> XERBLA  is an error handler for the LAPACK routines.
!> It is called by an LAPACK routine if an input parameter has an
!> invalid value.  A message is printed and execution stops.
!>
!> Installers may consider modifying the STOP statement in order to
!> call system-specific exception-handling facilities.
!> \endverbatim
!
!  Arguments:
!  ==========
!
!> \param[in] SRNAME
!> \verbatim
!>          SRNAME is CHARACTER*(*)
!>          The name of the routine which called XERBLA.
!> \endverbatim
!>
!> \param[in] INFO
!> \verbatim
!>          INFO is INTEGER
!>          The position of the invalid parameter in the parameter list
!>          of the calling routine.
!> \endverbatim
!
!  Authors:
!  ========
!
!> \author Univ. of Tennessee
!> \author Univ. of California Berkeley
!> \author Univ. of Colorado Denver
!> \author NAG Ltd.
!
!> \date November 2011
!
!> \ingroup auxOTHERauxiliary
!
!  =====================================================================
      SUBROUTINE XERBLA( SRNAME, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.4.0) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2011
!
!     .. Scalar Arguments ..
      CHARACTER(*)       SRNAME
      INTEGER            INFO
!     ..
!
! =====================================================================
!
!     ..
!     .. Executable Statements ..
!
      WRITE( *, FMT = 9999 )SRNAME( 1:LEN_TRIM( SRNAME ) ), INFO
!
      STOP
!
 9999 FORMAT( ' ** On entry to ', A, ' parameter number ', I2, ' had ', &
            'an illegal value' )
!
!     End of XERBLA
!
      END
      SUBROUTINE SGELQ2( M, N, A, LDA, TAU, WORK, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      INTEGER            INFO, LDA, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), TAU( * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SGELQ2 computes an LQ factorization of a real m by n matrix A:
!  A = L * Q.
!
!  Arguments
!  =========
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.
!
!  A       (input/output) REAL array, dimension (LDA,N)
!          On entry, the m by n matrix A.
!          On exit, the elements on and below the diagonal of the array
!          contain the m by min(m,n) lower trapezoidal matrix L (L is
!          lower triangular if m <= n); the elements above the diagonal,
!          with the array TAU, represent the orthogonal matrix Q as a
!          product of elementary reflectors (see Further Details).
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,M).
!
!  TAU     (output) REAL array, dimension (min(M,N))
!          The scalar factors of the elementary reflectors (see Further
!          Details).
!
!  WORK    (workspace) REAL array, dimension (M)
!
!  INFO    (output) INTEGER
!          = 0: successful exit
!          < 0: if INFO = -i, the i-th argument had an illegal value
!
!  Further Details
!  ===============
!
!  The matrix Q is represented as a product of elementary reflectors
!
!     Q = H(k) . . . H(2) H(1), where k = min(m,n).
!
!  Each H(i) has the form
!
!     H(i) = I - tau * v * v**T
!
!  where tau is a real scalar, and v is a real vector with
!  v(1:i-1) = 0 and v(i) = 1; v(i+1:n) is stored on exit in A(i,i+1:n),
!  and tau in TAU(i).
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ONE
      PARAMETER          ( ONE = 1.0E+0 )
!     ..
!     .. Local Scalars ..
      INTEGER            I, K
      REAL               AII
!     ..
!     .. External Subroutines ..
      EXTERNAL           SLARF, SLARFG, XERBLA
!     ..
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SGELQ2', -INFO )
         RETURN
      END IF
!
      K = MIN( M, N )
!
      DO 10 I = 1, K
!
!        Generate elementary reflector H(i) to annihilate A(i,i+1:n)
!
         CALL SLARFG( N-I+1, A( I, I ), A( I, MIN( I+1, N ) ), LDA, &
                      TAU( I ) )
         IF( I.LT.M ) THEN
!
!           Apply H(i) to A(i+1:m,i:n) from the right
!
            AII = A( I, I )
            A( I, I ) = ONE
            CALL SLARF( 'Right', M-I, N-I+1, A( I, I ), LDA, TAU( I ), &
                        A( I+1, I ), LDA, WORK )
            A( I, I ) = AII
         END IF
   10 CONTINUE
      RETURN
!
!     End of SGELQ2
!
      END
      SUBROUTINE SGELQF( M, N, A, LDA, TAU, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      INTEGER            INFO, LDA, LWORK, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), TAU( * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SGELQF computes an LQ factorization of a real M-by-N matrix A:
!  A = L * Q.
!
!  Arguments
!  =========
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.
!
!  A       (input/output) REAL array, dimension (LDA,N)
!          On entry, the M-by-N matrix A.
!          On exit, the elements on and below the diagonal of the array
!          contain the m-by-min(m,n) lower trapezoidal matrix L (L is
!          lower triangular if m <= n); the elements above the diagonal,
!          with the array TAU, represent the orthogonal matrix Q as a
!          product of elementary reflectors (see Further Details).
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,M).
!
!  TAU     (output) REAL array, dimension (min(M,N))
!          The scalar factors of the elementary reflectors (see Further
!          Details).
!
!  WORK    (workspace/output) REAL array, dimension (MAX(1,LWORK))
!          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!
!  LWORK   (input) INTEGER
!          The dimension of the array WORK.  LWORK >= max(1,M).
!          For optimum performance LWORK >= M*NB, where NB is the
!          optimal blocksize.
!
!          If LWORK = -1, then a workspace query is assumed; the routine
!          only calculates the optimal size of the WORK array, returns
!          this value as the first entry of the WORK array, and no error
!          message related to LWORK is issued by XERBLA.
!
!  INFO    (output) INTEGER
!          = 0:  successful exit
!          < 0:  if INFO = -i, the i-th argument had an illegal value
!
!  Further Details
!  ===============
!
!  The matrix Q is represented as a product of elementary reflectors
!
!     Q = H(k) . . . H(2) H(1), where k = min(m,n).
!
!  Each H(i) has the form
!
!     H(i) = I - tau * v * v**T
!
!  where tau is a real scalar, and v is a real vector with
!  v(1:i-1) = 0 and v(i) = 1; v(i+1:n) is stored on exit in A(i,i+1:n),
!  and tau in TAU(i).
!
!  =====================================================================
!
!     .. Local Scalars ..
      LOGICAL            LQUERY
      INTEGER            I, IB, IINFO, IWS, K, LDWORK, LWKOPT, NB, &
                         NBMIN, NX
!     ..
!     .. External Subroutines ..
      EXTERNAL           SGELQ2, SLARFB, SLARFT, XERBLA
!     ..
!     ..
!     .. External Functions ..
      INTEGER            ILAENV
      EXTERNAL           ILAENV
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
      NB = ILAENV( 1, 'SGELQF', ' ', M, N, -1, -1 )
      LWKOPT = M*NB
      WORK( 1 ) = LWKOPT
      LQUERY = ( LWORK.EQ.-1 )
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      ELSE IF( LWORK.LT.MAX( 1, M ) .AND. .NOT.LQUERY ) THEN
         INFO = -7
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SGELQF', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!
!     Quick return if possible
!
      K = MIN( M, N )
      IF( K.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!
      NBMIN = 2
      NX = 0
      IWS = M
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
!
!        Determine when to cross over from blocked to unblocked code.
!
         NX = MAX( 0, ILAENV( 3, 'SGELQF', ' ', M, N, -1, -1 ) )
         IF( NX.LT.K ) THEN
!
!           Determine if workspace is large enough for blocked code.
!
            LDWORK = M
            IWS = LDWORK*NB
            IF( LWORK.LT.IWS ) THEN
!
!              Not enough workspace to use optimal NB:  reduce NB and
!              determine the minimum value of NB.
!
               NB = LWORK / LDWORK
               NBMIN = MAX( 2, ILAENV( 2, 'SGELQF', ' ', M, N, -1, &
                       -1 ) )
            END IF
         END IF
      END IF
!
      IF( NB.GE.NBMIN .AND. NB.LT.K .AND. NX.LT.K ) THEN
!
!        Use blocked code initially
!
         DO 10 I = 1, K - NX, NB
            IB = MIN( K-I+1, NB )
!
!           Compute the LQ factorization of the current block
!           A(i:i+ib-1,i:n)
!
            CALL SGELQ2( IB, N-I+1, A( I, I ), LDA, TAU( I ), WORK, &
                         IINFO )
            IF( I+IB.LE.M ) THEN
!
!              Form the triangular factor of the block reflector
!              H = H(i) H(i+1) . . . H(i+ib-1)
!
               CALL SLARFT( 'Forward', 'Rowwise', N-I+1, IB, A( I, I ), &
                            LDA, TAU( I ), WORK, LDWORK )
!
!              Apply H to A(i+ib:m,i:n) from the right
!
               CALL SLARFB( 'Right', 'No transpose', 'Forward', &
                            'Rowwise', M-I-IB+1, N-I+1, IB, A( I, I ), &
                            LDA, WORK, LDWORK, A( I+IB, I ), LDA, &
                            WORK( IB+1 ), LDWORK )
            END IF
   10    CONTINUE
      ELSE
         I = 1
      END IF
!
!     Use unblocked code to factor the last or only block.
!
      IF( I.LE.K ) &
         CALL SGELQ2( M-I+1, N-I+1, A( I, I ), LDA, TAU( I ), WORK, &
                      IINFO )
!
      WORK( 1 ) = IWS
      RETURN
!
!     End of SGELQF
!
      END
      SUBROUTINE SGELS( TRANS, M, N, NRHS, A, LDA, B, LDB, WORK, LWORK, &
                        INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK driver routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          TRANS
      INTEGER            INFO, LDA, LDB, LWORK, M, N, NRHS
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), B( LDB, * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SGELS solves overdetermined or underdetermined real linear systems
!  involving an M-by-N matrix A, or its transpose, using a QR or LQ
!  factorization of A.  It is assumed that A has full rank.
!
!  The following options are provided:
!
!  1. If TRANS = 'N' and m >= n:  find the least squares solution of
!     an overdetermined system, i.e., solve the least squares problem
!                  minimize || B - A*X ||.
!
!  2. If TRANS = 'N' and m < n:  find the minimum norm solution of
!     an underdetermined system A * X = B.
!
!  3. If TRANS = 'T' and m >= n:  find the minimum norm solution of
!     an undetermined system A**T * X = B.
!
!  4. If TRANS = 'T' and m < n:  find the least squares solution of
!     an overdetermined system, i.e., solve the least squares problem
!                  minimize || B - A**T * X ||.
!
!  Several right hand side vectors b and solution vectors x can be
!  handled in a single call; they are stored as the columns of the
!  M-by-NRHS right hand side matrix B and the N-by-NRHS solution
!  matrix X.
!
!  Arguments
!  =========
!
!  TRANS   (input) CHARACTER*1
!          = 'N': the linear system involves A;
!          = 'T': the linear system involves A**T.
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.
!
!  NRHS    (input) INTEGER
!          The number of right hand sides, i.e., the number of
!          columns of the matrices B and X. NRHS >=0.
!
!  A       (input/output) REAL array, dimension (LDA,N)
!          On entry, the M-by-N matrix A.
!          On exit,
!            if M >= N, A is overwritten by details of its QR
!                       factorization as returned by SGEQRF;
!            if M <  N, A is overwritten by details of its LQ
!                       factorization as returned by SGELQF.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,M).
!
!  B       (input/output) REAL array, dimension (LDB,NRHS)
!          On entry, the matrix B of right hand side vectors, stored
!          columnwise; B is M-by-NRHS if TRANS = 'N', or N-by-NRHS
!          if TRANS = 'T'.
!          On exit, if INFO = 0, B is overwritten by the solution
!          vectors, stored columnwise:
!          if TRANS = 'N' and m >= n, rows 1 to n of B contain the least
!          squares solution vectors; the residual sum of squares for the
!          solution in each column is given by the sum of squares of
!          elements N+1 to M in that column;
!          if TRANS = 'N' and m < n, rows 1 to N of B contain the
!          minimum norm solution vectors;
!          if TRANS = 'T' and m >= n, rows 1 to M of B contain the
!          minimum norm solution vectors;
!          if TRANS = 'T' and m < n, rows 1 to M of B contain the
!          least squares solution vectors; the residual sum of squares
!          for the solution in each column is given by the sum of
!          squares of elements M+1 to N in that column.
!
!  LDB     (input) INTEGER
!          The leading dimension of the array B. LDB >= MAX(1,M,N).
!
!  WORK    (workspace/output) REAL array, dimension (MAX(1,LWORK))
!          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!
!  LWORK   (input) INTEGER
!          The dimension of the array WORK.
!          LWORK >= max( 1, MN + max( MN, NRHS ) ).
!          For optimal performance,
!          LWORK >= max( 1, MN + max( MN, NRHS )*NB ).
!          where MN = min(M,N) and NB is the optimum block size.
!
!          If LWORK = -1, then a workspace query is assumed; the routine
!          only calculates the optimal size of the WORK array, returns
!          this value as the first entry of the WORK array, and no error
!          message related to LWORK is issued by XERBLA.
!
!  INFO    (output) INTEGER
!          = 0:  successful exit
!          < 0:  if INFO = -i, the i-th argument had an illegal value
!          > 0:  if INFO =  i, the i-th diagonal element of the
!                triangular factor of A is zero, so that A does not have
!                full rank; the least squares solution could not be
!                computed.
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ZERO, ONE
      PARAMETER          ( ZERO = 0.0E0, ONE = 1.0E0 )
!     ..
!     .. Local Scalars ..
      LOGICAL            LQUERY, TPSD
      INTEGER            BROW, I, IASCL, IBSCL, J, MN, NB, SCLLEN, WSIZE
      REAL               ANRM, BIGNUM, BNRM, SMLNUM
!     ..
!     .. Local Arrays ..
      REAL               RWORK( 1 )
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      REAL               SLAMCH, SLANGE
      EXTERNAL           LSAME, ILAENV, SLAMCH, SLANGE
!     ..
!     .. External Subroutines ..
      EXTERNAL           SGELQF, SGEQRF, SLABAD, SLASCL, SLASET, SORMLQ, &
                         SORMQR, STRTRS, XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments.
!
      INFO = 0
      MN = MIN( M, N )
      LQUERY = ( LWORK.EQ.-1 )
      IF( .NOT.( LSAME( TRANS, 'N' ) .OR. LSAME( TRANS, 'T' ) ) ) THEN
         INFO = -1
      ELSE IF( M.LT.0 ) THEN
         INFO = -2
      ELSE IF( N.LT.0 ) THEN
         INFO = -3
      ELSE IF( NRHS.LT.0 ) THEN
         INFO = -4
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -6
      ELSE IF( LDB.LT.MAX( 1, M, N ) ) THEN
         INFO = -8
      ELSE IF( LWORK.LT.MAX( 1, MN + MAX( MN, NRHS ) ) .AND. &
         .NOT.LQUERY ) THEN
         INFO = -10
      END IF
!
!     Figure out optimal block size
!
      IF( INFO.EQ.0 .OR. INFO.EQ.-10 ) THEN
!
         TPSD = .TRUE.
         IF( LSAME( TRANS, 'N' ) ) &
            TPSD = .FALSE.
!
         IF( M.GE.N ) THEN
            NB = ILAENV( 1, 'SGEQRF', ' ', M, N, -1, -1 )
            IF( TPSD ) THEN
               NB = MAX( NB, ILAENV( 1, 'SORMQR', 'LN', M, NRHS, N, &
                    -1 ) )
            ELSE
               NB = MAX( NB, ILAENV( 1, 'SORMQR', 'LT', M, NRHS, N, &
                    -1 ) )
            END IF
         ELSE
            NB = ILAENV( 1, 'SGELQF', ' ', M, N, -1, -1 )
            IF( TPSD ) THEN
               NB = MAX( NB, ILAENV( 1, 'SORMLQ', 'LT', N, NRHS, M, &
                    -1 ) )
            ELSE
               NB = MAX( NB, ILAENV( 1, 'SORMLQ', 'LN', N, NRHS, M, &
                    -1 ) )
            END IF
         END IF
!
         WSIZE = MAX( 1, MN + MAX( MN, NRHS )*NB )
         WORK( 1 ) = REAL( WSIZE )
!
      END IF
!
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SGELS ', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( MIN( M, N, NRHS ).EQ.0 ) THEN
         CALL SLASET( 'Full', MAX( M, N ), NRHS, ZERO, ZERO, B, LDB )
         RETURN
      END IF
!
!     Get machine parameters
!
      SMLNUM = SLAMCH( 'S' ) / SLAMCH( 'P' )
      BIGNUM = ONE / SMLNUM
      CALL SLABAD( SMLNUM, BIGNUM )
!
!     Scale A, B if max element outside range [SMLNUM,BIGNUM]
!
      ANRM = SLANGE( 'M', M, N, A, LDA, RWORK )
      IASCL = 0
      IF( ANRM.GT.ZERO .AND. ANRM.LT.SMLNUM ) THEN
!
!        Scale matrix norm up to SMLNUM
!
         CALL SLASCL( 'G', 0, 0, ANRM, SMLNUM, M, N, A, LDA, INFO )
         IASCL = 1
      ELSE IF( ANRM.GT.BIGNUM ) THEN
!
!        Scale matrix norm down to BIGNUM
!
         CALL SLASCL( 'G', 0, 0, ANRM, BIGNUM, M, N, A, LDA, INFO )
         IASCL = 2
      ELSE IF( ANRM.EQ.ZERO ) THEN
!
!        Matrix all zero. Return zero solution.
!
         CALL SLASET( 'F', MAX( M, N ), NRHS, ZERO, ZERO, B, LDB )
         GO TO 50
      END IF
!
      BROW = M
      IF( TPSD ) &
         BROW = N
      BNRM = SLANGE( 'M', BROW, NRHS, B, LDB, RWORK )
      IBSCL = 0
      IF( BNRM.GT.ZERO .AND. BNRM.LT.SMLNUM ) THEN
!
!        Scale matrix norm up to SMLNUM
!
         CALL SLASCL( 'G', 0, 0, BNRM, SMLNUM, BROW, NRHS, B, LDB, &
                      INFO )
         IBSCL = 1
      ELSE IF( BNRM.GT.BIGNUM ) THEN
!
!        Scale matrix norm down to BIGNUM
!
         CALL SLASCL( 'G', 0, 0, BNRM, BIGNUM, BROW, NRHS, B, LDB, &
                      INFO )
         IBSCL = 2
      END IF
!
      IF( M.GE.N ) THEN
!
!        compute QR factorization of A
!
         CALL SGEQRF( M, N, A, LDA, WORK( 1 ), WORK( MN+1 ), LWORK-MN, &
                      INFO )
!
!        workspace at least N, optimally N*NB
!
         IF( .NOT.TPSD ) THEN
!
!           Least-Squares Problem min || A * X - B ||
!
!           B(1:M,1:NRHS) := Q**T * B(1:M,1:NRHS)
!
            CALL SORMQR( 'Left', 'Transpose', M, NRHS, N, A, LDA, &
                         WORK( 1 ), B, LDB, WORK( MN+1 ), LWORK-MN, &
                         INFO )
!
!           workspace at least NRHS, optimally NRHS*NB
!
!           B(1:N,1:NRHS) := inv(R) * B(1:N,1:NRHS)
!
            CALL STRTRS( 'Upper', 'No transpose', 'Non-unit', N, NRHS, &
                         A, LDA, B, LDB, INFO )
!
            IF( INFO.GT.0 ) THEN
               RETURN
            END IF
!
            SCLLEN = N
!
         ELSE
!
!           Overdetermined system of equations A**T * X = B
!
!           B(1:N,1:NRHS) := inv(R**T) * B(1:N,1:NRHS)
!
            CALL STRTRS( 'Upper', 'Transpose', 'Non-unit', N, NRHS, &
                         A, LDA, B, LDB, INFO )
!
            IF( INFO.GT.0 ) THEN
               RETURN
            END IF
!
!           B(N+1:M,1:NRHS) = ZERO
!
            DO 20 J = 1, NRHS
               DO 10 I = N + 1, M
                  B( I, J ) = ZERO
   10          CONTINUE
   20       CONTINUE
!
!           B(1:M,1:NRHS) := Q(1:N,:) * B(1:N,1:NRHS)
!
            CALL SORMQR( 'Left', 'No transpose', M, NRHS, N, A, LDA, &
                         WORK( 1 ), B, LDB, WORK( MN+1 ), LWORK-MN, &
                         INFO )
!
!           workspace at least NRHS, optimally NRHS*NB
!
            SCLLEN = M
!
         END IF
!
      ELSE
!
!        Compute LQ factorization of A
!
         CALL SGELQF( M, N, A, LDA, WORK( 1 ), WORK( MN+1 ), LWORK-MN, &
                      INFO )
!
!        workspace at least M, optimally M*NB.
!
         IF( .NOT.TPSD ) THEN
!
!           underdetermined system of equations A * X = B
!
!           B(1:M,1:NRHS) := inv(L) * B(1:M,1:NRHS)
!
            CALL STRTRS( 'Lower', 'No transpose', 'Non-unit', M, NRHS, &
                         A, LDA, B, LDB, INFO )
!
            IF( INFO.GT.0 ) THEN
               RETURN
            END IF
!
!           B(M+1:N,1:NRHS) = 0
!
            DO 40 J = 1, NRHS
               DO 30 I = M + 1, N
                  B( I, J ) = ZERO
   30          CONTINUE
   40       CONTINUE
!
!           B(1:N,1:NRHS) := Q(1:N,:)**T * B(1:M,1:NRHS)
!
            CALL SORMLQ( 'Left', 'Transpose', N, NRHS, M, A, LDA, &
                         WORK( 1 ), B, LDB, WORK( MN+1 ), LWORK-MN, &
                         INFO )
!
!           workspace at least NRHS, optimally NRHS*NB
!
            SCLLEN = N
!
         ELSE
!
!           overdetermined system min || A**T * X - B ||
!
!           B(1:N,1:NRHS) := Q * B(1:N,1:NRHS)
!
            CALL SORMLQ( 'Left', 'No transpose', N, NRHS, M, A, LDA, &
                         WORK( 1 ), B, LDB, WORK( MN+1 ), LWORK-MN, &
                         INFO )
!
!           workspace at least NRHS, optimally NRHS*NB
!
!           B(1:M,1:NRHS) := inv(L**T) * B(1:M,1:NRHS)
!
            CALL STRTRS( 'Lower', 'Transpose', 'Non-unit', M, NRHS, &
                         A, LDA, B, LDB, INFO )
!
            IF( INFO.GT.0 ) THEN
               RETURN
            END IF
!
            SCLLEN = M
!
         END IF
!
      END IF
!
!     Undo scaling
!
      IF( IASCL.EQ.1 ) THEN
         CALL SLASCL( 'G', 0, 0, ANRM, SMLNUM, SCLLEN, NRHS, B, LDB, &
                      INFO )
      ELSE IF( IASCL.EQ.2 ) THEN
         CALL SLASCL( 'G', 0, 0, ANRM, BIGNUM, SCLLEN, NRHS, B, LDB, &
                      INFO )
      END IF
      IF( IBSCL.EQ.1 ) THEN
         CALL SLASCL( 'G', 0, 0, SMLNUM, BNRM, SCLLEN, NRHS, B, LDB, &
                      INFO )
      ELSE IF( IBSCL.EQ.2 ) THEN
         CALL SLASCL( 'G', 0, 0, BIGNUM, BNRM, SCLLEN, NRHS, B, LDB, &
                      INFO )
      END IF
!
   50 CONTINUE
      WORK( 1 ) = REAL( WSIZE )
!
      RETURN
!
!     End of SGELS
!
      END
      SUBROUTINE SGEQR2( M, N, A, LDA, TAU, WORK, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      INTEGER            INFO, LDA, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), TAU( * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SGEQR2 computes a QR factorization of a real m by n matrix A:
!  A = Q * R.
!
!  Arguments
!  =========
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.
!
!  A       (input/output) REAL array, dimension (LDA,N)
!          On entry, the m by n matrix A.
!          On exit, the elements on and above the diagonal of the array
!          contain the min(m,n) by n upper trapezoidal matrix R (R is
!          upper triangular if m >= n); the elements below the diagonal,
!          with the array TAU, represent the orthogonal matrix Q as a
!          product of elementary reflectors (see Further Details).
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,M).
!
!  TAU     (output) REAL array, dimension (min(M,N))
!          The scalar factors of the elementary reflectors (see Further
!          Details).
!
!  WORK    (workspace) REAL array, dimension (N)
!
!  INFO    (output) INTEGER
!          = 0: successful exit
!          < 0: if INFO = -i, the i-th argument had an illegal value
!
!  Further Details
!  ===============
!
!  The matrix Q is represented as a product of elementary reflectors
!
!     Q = H(1) H(2) . . . H(k), where k = min(m,n).
!
!  Each H(i) has the form
!
!     H(i) = I - tau * v * v**T
!
!  where tau is a real scalar, and v is a real vector with
!  v(1:i-1) = 0 and v(i) = 1; v(i+1:m) is stored on exit in A(i+1:m,i),
!  and tau in TAU(i).
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ONE
      PARAMETER          ( ONE = 1.0E+0 )
!     ..
!     .. Local Scalars ..
      INTEGER            I, K
      REAL               AII
!     ..
!     .. External Subroutines ..
      EXTERNAL           SLARF, SLARFG, XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SGEQR2', -INFO )
         RETURN
      END IF
!
      K = MIN( M, N )
!
      DO 10 I = 1, K
!
!        Generate elementary reflector H(i) to annihilate A(i+1:m,i)
!
         CALL SLARFG( M-I+1, A( I, I ), A( MIN( I+1, M ), I ), 1, &
                      TAU( I ) )
         IF( I.LT.N ) THEN
!
!           Apply H(i) to A(i:m,i+1:n) from the left
!
            AII = A( I, I )
            A( I, I ) = ONE
            CALL SLARF( 'Left', M-I+1, N-I, A( I, I ), 1, TAU( I ), &
                        A( I, I+1 ), LDA, WORK )
            A( I, I ) = AII
         END IF
   10 CONTINUE
      RETURN
!
!     End of SGEQR2
!
      END
      SUBROUTINE SGEQRF( M, N, A, LDA, TAU, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      INTEGER            INFO, LDA, LWORK, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), TAU( * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SGEQRF computes a QR factorization of a real M-by-N matrix A:
!  A = Q * R.
!
!  Arguments
!  =========
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.
!
!  A       (input/output) REAL array, dimension (LDA,N)
!          On entry, the M-by-N matrix A.
!          On exit, the elements on and above the diagonal of the array
!          contain the min(M,N)-by-N upper trapezoidal matrix R (R is
!          upper triangular if m >= n); the elements below the diagonal,
!          with the array TAU, represent the orthogonal matrix Q as a
!          product of min(m,n) elementary reflectors (see Further
!          Details).
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,M).
!
!  TAU     (output) REAL array, dimension (min(M,N))
!          The scalar factors of the elementary reflectors (see Further
!          Details).
!
!  WORK    (workspace/output) REAL array, dimension (MAX(1,LWORK))
!          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!
!  LWORK   (input) INTEGER
!          The dimension of the array WORK.  LWORK >= max(1,N).
!          For optimum performance LWORK >= N*NB, where NB is
!          the optimal blocksize.
!
!          If LWORK = -1, then a workspace query is assumed; the routine
!          only calculates the optimal size of the WORK array, returns
!          this value as the first entry of the WORK array, and no error
!          message related to LWORK is issued by XERBLA.
!
!  INFO    (output) INTEGER
!          = 0:  successful exit
!          < 0:  if INFO = -i, the i-th argument had an illegal value
!
!  Further Details
!  ===============
!
!  The matrix Q is represented as a product of elementary reflectors
!
!     Q = H(1) H(2) . . . H(k), where k = min(m,n).
!
!  Each H(i) has the form
!
!     H(i) = I - tau * v * v**T
!
!  where tau is a real scalar, and v is a real vector with
!  v(1:i-1) = 0 and v(i) = 1; v(i+1:m) is stored on exit in A(i+1:m,i),
!  and tau in TAU(i).
!
!  =====================================================================
!
!     .. Local Scalars ..
      LOGICAL            LQUERY
      INTEGER            I, IB, IINFO, IWS, K, LDWORK, LWKOPT, NB, &
                         NBMIN, NX
!     ..
!     .. External Subroutines ..
      EXTERNAL           SGEQR2, SLARFB, SLARFT, XERBLA
!     ..
!     .. External Functions ..
      INTEGER            ILAENV
      EXTERNAL           ILAENV
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
      NB = ILAENV( 1, 'SGEQRF', ' ', M, N, -1, -1 )
      LWKOPT = N*NB
      WORK( 1 ) = LWKOPT
      LQUERY = ( LWORK.EQ.-1 )
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      ELSE IF( LWORK.LT.MAX( 1, N ) .AND. .NOT.LQUERY ) THEN
         INFO = -7
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SGEQRF', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!
!     Quick return if possible
!
      K = MIN( M, N )
      IF( K.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!
      NBMIN = 2
      NX = 0
      IWS = N
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
!
!        Determine when to cross over from blocked to unblocked code.
!
         NX = MAX( 0, ILAENV( 3, 'SGEQRF', ' ', M, N, -1, -1 ) )
         IF( NX.LT.K ) THEN
!
!           Determine if workspace is large enough for blocked code.
!
            LDWORK = N
            IWS = LDWORK*NB
            IF( LWORK.LT.IWS ) THEN
!
!              Not enough workspace to use optimal NB:  reduce NB and
!              determine the minimum value of NB.
!
               NB = LWORK / LDWORK
               NBMIN = MAX( 2, ILAENV( 2, 'SGEQRF', ' ', M, N, -1, &
                       -1 ) )
            END IF
         END IF
      END IF
!
      IF( NB.GE.NBMIN .AND. NB.LT.K .AND. NX.LT.K ) THEN
!
!        Use blocked code initially
!
         DO 10 I = 1, K - NX, NB
            IB = MIN( K-I+1, NB )
!
!           Compute the QR factorization of the current block
!           A(i:m,i:i+ib-1)
!
            CALL SGEQR2( M-I+1, IB, A( I, I ), LDA, TAU( I ), WORK, &
                         IINFO )
            IF( I+IB.LE.N ) THEN
!
!              Form the triangular factor of the block reflector
!              H = H(i) H(i+1) . . . H(i+ib-1)
!
               CALL SLARFT( 'Forward', 'Columnwise', M-I+1, IB, &
                            A( I, I ), LDA, TAU( I ), WORK, LDWORK )
!
!              Apply H**T to A(i:m,i+ib:n) from the left
!
               CALL SLARFB( 'Left', 'Transpose', 'Forward', &
                            'Columnwise', M-I+1, N-I-IB+1, IB, &
                            A( I, I ), LDA, WORK, LDWORK, A( I, I+IB ), &
                            LDA, WORK( IB+1 ), LDWORK )
            END IF
   10    CONTINUE
      ELSE
         I = 1
      END IF
!
!     Use unblocked code to factor the last or only block.
!
      IF( I.LE.K ) &
         CALL SGEQR2( M-I+1, N-I+1, A( I, I ), LDA, TAU( I ), WORK, &
                      IINFO )
!
      WORK( 1 ) = IWS
      RETURN
!
!     End of SGEQRF
!
      END
      LOGICAL FUNCTION SISNAN( SSIN )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.2.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     June 2010
!
!     .. Scalar Arguments ..
      REAL               SSIN
!     ..
!
!  Purpose
!  =======
!
!  SISNAN returns .TRUE. if its argument is NaN, and .FALSE.
!  otherwise.  To be replaced by the Fortran 2003 intrinsic in the
!  future.
!
!  Arguments
!  =========
!
!  SIN     (input) REAL
!          Input to test for NaN.
!
!  =====================================================================
!
!  .. External Functions ..
      LOGICAL SLAISNAN
      EXTERNAL SLAISNAN
!  ..
!  .. Executable Statements ..
      SISNAN = SLAISNAN(SSIN,SSIN)
      RETURN
      END
      SUBROUTINE SLABAD( SMALL, LARGE )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2006
!
!     .. Scalar Arguments ..
      REAL               LARGE, SMALL
!     ..
!
!  Purpose
!  =======
!
!  SLABAD takes as input the values computed by SLAMCH for underflow and
!  overflow, and returns the square root of each of these values if the
!  log of LARGE is sufficiently large.  This subroutine is intended to
!  identify machines with a large exponent range, such as the Crays, and
!  redefine the underflow and overflow limits to be the square roots of
!  the values computed by SLAMCH.  This subroutine is needed because
!  SLAMCH does not compensate for poor arithmetic in the upper half of
!  the exponent range, as is found on a Cray.
!
!  Arguments
!  =========
!
!  SMALL   (input/output) REAL
!          On entry, the underflow threshold as computed by SLAMCH.
!          On exit, if LOG10(LARGE) is sufficiently large, the square
!          root of SMALL, otherwise unchanged.
!
!  LARGE   (input/output) REAL
!          On entry, the overflow threshold as computed by SLAMCH.
!          On exit, if LOG10(LARGE) is sufficiently large, the square
!          root of LARGE, otherwise unchanged.
!
!  =====================================================================
!
!     ..
!     .. Executable Statements ..
!
!     If it looks like we're on a Cray, take the square root of
!     SMALL and LARGE to avoid overflow and underflow problems.
!
      IF( LOG10( LARGE ).GT.2000. ) THEN
         SMALL = SQRT( SMALL )
         LARGE = SQRT( LARGE )
      END IF
!
      RETURN
!
!     End of SLABAD
!
      END
      LOGICAL FUNCTION SLAISNAN( SIN1, SIN2 )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.2.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     June 2010
!
!     .. Scalar Arguments ..
      REAL               SIN1, SIN2
!     ..
!
!  Purpose
!  =======
!
!  This routine is not for general use.  It exists solely to avoid
!  over-optimization in SISNAN.
!
!  SLAISNAN checks for NaNs by comparing its two arguments for
!  inequality.  NaN is the only floating-point value where NaN != NaN
!  returns .TRUE.  To check for NaNs, pass the same variable as both
!  arguments.
!
!  A compiler must assume that the two arguments are
!  not the same variable, and the test will not be optimized away.
!  Interprocedural or whole-program optimization may delete this
!  test.  The ISNAN functions will be replaced by the correct
!  Fortran 03 intrinsic once the intrinsic is widely available.
!
!  Arguments
!  =========
!
!  SIN1     (input) REAL
!
!  SIN2     (input) REAL
!          Two numbers to compare for inequality.
!
!  =====================================================================
!
!  .. Executable Statements ..
      SLAISNAN = (SIN1.NE.SIN2)
      RETURN
      END
      REAL             FUNCTION SLANGE( NORM, M, N, A, LDA, WORK )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2006
!
!     .. Scalar Arguments ..
      CHARACTER          NORM
      INTEGER            LDA, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SLANGE  returns the value of the one norm,  or the Frobenius norm, or
!  the  infinity norm,  or the  element of  largest absolute value  of a
!  real matrix A.
!
!  Description
!  ===========
!
!  SLANGE returns the value
!
!     SLANGE = ( max(abs(A(i,j))), NORM = 'M' or 'm'
!              (
!              ( norm1(A),         NORM = '1', 'O' or 'o'
!              (
!              ( normI(A),         NORM = 'I' or 'i'
!              (
!              ( normF(A),         NORM = 'F', 'f', 'E' or 'e'
!
!  where  norm1  denotes the  one norm of a matrix (maximum column sum),
!  normI  denotes the  infinity norm  of a matrix  (maximum row sum) and
!  normF  denotes the  Frobenius norm of a matrix (square root of sum of
!  squares).  Note that  max(abs(A(i,j)))  is not a consistent matrix norm.
!
!  Arguments
!  =========
!
!  NORM    (input) CHARACTER*1
!          Specifies the value to be returned in SLANGE as described
!          above.
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.  When M = 0,
!          SLANGE is set to zero.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.  When N = 0,
!          SLANGE is set to zero.
!
!  A       (input) REAL array, dimension (LDA,N)
!          The m by n matrix A.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(M,1).
!
!  WORK    (workspace) REAL array, dimension (MAX(1,LWORK)),
!          where LWORK >= M when NORM = 'I'; otherwise, WORK is not
!          referenced.
!
! =====================================================================
!
!     .. Parameters ..
      REAL               ONE, ZERO
      PARAMETER          ( ONE = 1.0E+0, ZERO = 0.0E+0 )
!     ..
!     .. Local Scalars ..
      INTEGER            I, J
      REAL               SCALE, SUM, VALUE
!     ..
!     .. External Subroutines ..
      EXTERNAL           SLASSQ
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!     ..
!     .. Executable Statements ..
!
      IF( MIN( M, N ).EQ.0 ) THEN
         VALUE = ZERO
      ELSE IF( LSAME( NORM, 'M' ) ) THEN
!
!        Find max(abs(A(i,j))).
!
         VALUE = ZERO
         DO 20 J = 1, N
            DO 10 I = 1, M
               VALUE = MAX( VALUE, ABS( A( I, J ) ) )
   10       CONTINUE
   20    CONTINUE
      ELSE IF( ( LSAME( NORM, 'O' ) ) .OR. ( NORM.EQ.'1' ) ) THEN
!
!        Find norm1(A).
!
         VALUE = ZERO
         DO 40 J = 1, N
            SUM = ZERO
            DO 30 I = 1, M
               SUM = SUM + ABS( A( I, J ) )
   30       CONTINUE
            VALUE = MAX( VALUE, SUM )
   40    CONTINUE
      ELSE IF( LSAME( NORM, 'I' ) ) THEN
!
!        Find normI(A).
!
         DO 50 I = 1, M
            WORK( I ) = ZERO
   50    CONTINUE
         DO 70 J = 1, N
            DO 60 I = 1, M
               WORK( I ) = WORK( I ) + ABS( A( I, J ) )
   60       CONTINUE
   70    CONTINUE
         VALUE = ZERO
         DO 80 I = 1, M
            VALUE = MAX( VALUE, WORK( I ) )
   80    CONTINUE
      ELSE IF( ( LSAME( NORM, 'F' ) ) .OR. ( LSAME( NORM, 'E' ) ) ) THEN
!
!        Find normF(A).
!
         SCALE = ZERO
         SUM = ONE
         DO 90 J = 1, N
            CALL SLASSQ( M, A( 1, J ), 1, SCALE, SUM )
   90    CONTINUE
         VALUE = SCALE*SQRT( SUM )
      END IF
!
      SLANGE = VALUE
      RETURN
!
!     End of SLANGE
!
      END
      REAL             FUNCTION SLAPY2( X, Y )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2006
!
!     .. Scalar Arguments ..
      REAL               X, Y
!     ..
!
!  Purpose
!  =======
!
!  SLAPY2 returns sqrt(x**2+y**2), taking care not to cause unnecessary
!  overflow.
!
!  Arguments
!  =========
!
!  X       (input) REAL
!  Y       (input) REAL
!          X and Y specify the values x and y.
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ZERO
      PARAMETER          ( ZERO = 0.0E0 )
      REAL               ONE
      PARAMETER          ( ONE = 1.0E0 )
!     ..
!     .. Local Scalars ..
      REAL               W, XABS, YABS, Z
!     ..
!     .. Executable Statements ..
!
      XABS = ABS( X )
      YABS = ABS( Y )
      W = MAX( XABS, YABS )
      Z = MIN( XABS, YABS )
      IF( Z.EQ.ZERO ) THEN
         SLAPY2 = W
      ELSE
         SLAPY2 = W*SQRT( ONE+( Z / W )**2 )
      END IF
      RETURN
!
!     End of SLAPY2
!
      END
      SUBROUTINE SLARF( SIDE, M, N, V, INCV, TAU, C, LDC, WORK )
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          SIDE
      INTEGER            INCV, LDC, M, N
      REAL               TAU
!     ..
!     .. Array Arguments ..
      REAL               C( LDC, * ), V( * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SLARF applies a real elementary reflector H to a real m by n matrix
!  C, from either the left or the right. H is represented in the form
!
!        H = I - tau * v * v**T
!
!  where tau is a real scalar and v is a real vector.
!
!  If tau = 0, then H is taken to be the unit matrix.
!
!  Arguments
!  =========
!
!  SIDE    (input) CHARACTER*1
!          = 'L': form  H * C
!          = 'R': form  C * H
!
!  M       (input) INTEGER
!          The number of rows of the matrix C.
!
!  N       (input) INTEGER
!          The number of columns of the matrix C.
!
!  V       (input) REAL array, dimension
!                     (1 + (M-1)*abs(INCV)) if SIDE = 'L'
!                  or (1 + (N-1)*abs(INCV)) if SIDE = 'R'
!          The vector v in the representation of H. V is not used if
!          TAU = 0.
!
!  INCV    (input) INTEGER
!          The increment between elements of v. INCV <> 0.
!
!  TAU     (input) REAL
!          The value tau in the representation of H.
!
!  C       (input/output) REAL array, dimension (LDC,N)
!          On entry, the m by n matrix C.
!          On exit, C is overwritten by the matrix H * C if SIDE = 'L',
!          or C * H if SIDE = 'R'.
!
!  LDC     (input) INTEGER
!          The leading dimension of the array C. LDC >= max(1,M).
!
!  WORK    (workspace) REAL array, dimension
!                         (N) if SIDE = 'L'
!                      or (M) if SIDE = 'R'
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ONE, ZERO
      PARAMETER          ( ONE = 1.0E+0, ZERO = 0.0E+0 )
!     ..
!     .. Local Scalars ..
      LOGICAL            APPLYLEFT
      INTEGER            I, LASTV, LASTC
!     ..
!     .. External Subroutines ..
      EXTERNAL           SGEMV, SGER
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILASLR, ILASLC
      EXTERNAL           LSAME, ILASLR, ILASLC
!     ..
!     .. Executable Statements ..
!
      APPLYLEFT = LSAME( SIDE, 'L' )
      LASTV = 0
      LASTC = 0
      IF( TAU.NE.ZERO ) THEN
!     Set up variables for scanning V.  LASTV begins pointing to the end
!     of V.
         IF( APPLYLEFT ) THEN
            LASTV = M
         ELSE
            LASTV = N
         END IF
         IF( INCV.GT.0 ) THEN
            I = 1 + (LASTV-1) * INCV
         ELSE
            I = 1
         END IF
!     Look for the last non-zero row in V.
         DO WHILE( LASTV.GT.0 .AND. V( I ).EQ.ZERO )
            LASTV = LASTV - 1
            I = I - INCV
         END DO
         IF( APPLYLEFT ) THEN
!     Scan for the last non-zero column in C(1:lastv,:).
            LASTC = ILASLC(LASTV, N, C, LDC)
         ELSE
!     Scan for the last non-zero row in C(:,1:lastv).
            LASTC = ILASLR(M, LASTV, C, LDC)
         END IF
      END IF
!     Note that lastc.eq.0 renders the BLAS operations null; no special
!     case is needed at this level.
      IF( APPLYLEFT ) THEN
!
!        Form  H * C
!
         IF( LASTV.GT.0 ) THEN
!
!           w(1:lastc,1) := C(1:lastv,1:lastc)**T * v(1:lastv,1)
!
            CALL SGEMV( 'Transpose', LASTV, LASTC, ONE, C, LDC, V, INCV, &
                 ZERO, WORK, 1 )
!
!           C(1:lastv,1:lastc) := C(...) - v(1:lastv,1) * w(1:lastc,1)**T
!
            CALL SGER( LASTV, LASTC, -TAU, V, INCV, WORK, 1, C, LDC )
         END IF
      ELSE
!
!        Form  C * H
!
         IF( LASTV.GT.0 ) THEN
!
!           w(1:lastc,1) := C(1:lastc,1:lastv) * v(1:lastv,1)
!
            CALL SGEMV( 'No transpose', LASTC, LASTV, ONE, C, LDC, &
                 V, INCV, ZERO, WORK, 1 )
!
!           C(1:lastc,1:lastv) := C(...) - w(1:lastc,1) * v(1:lastv,1)**T
!
            CALL SGER( LASTC, LASTV, -TAU, WORK, 1, V, INCV, C, LDC )
         END IF
      END IF
      RETURN
!
!     End of SLARF
!
      END
      SUBROUTINE SLARFB( SIDE, TRANS, DIRECT, STOREV, M, N, K, V, LDV, &
                         T, LDT, C, LDC, WORK, LDWORK )
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          DIRECT, SIDE, STOREV, TRANS
      INTEGER            K, LDC, LDT, LDV, LDWORK, M, N
!     ..
!     .. Array Arguments ..
      REAL               C( LDC, * ), T( LDT, * ), V( LDV, * ), &
                         WORK( LDWORK, * )
!     ..
!
!  Purpose
!  =======
!
!  SLARFB applies a real block reflector H or its transpose H**T to a
!  real m by n matrix C, from either the left or the right.
!
!  Arguments
!  =========
!
!  SIDE    (input) CHARACTER*1
!          = 'L': apply H or H**T from the Left
!          = 'R': apply H or H**T from the Right
!
!  TRANS   (input) CHARACTER*1
!          = 'N': apply H (No transpose)
!          = 'T': apply H**T (Transpose)
!
!  DIRECT  (input) CHARACTER*1
!          Indicates how H is formed from a product of elementary
!          reflectors
!          = 'F': H = H(1) H(2) . . . H(k) (Forward)
!          = 'B': H = H(k) . . . H(2) H(1) (Backward)
!
!  STOREV  (input) CHARACTER*1
!          Indicates how the vectors which define the elementary
!          reflectors are stored:
!          = 'C': Columnwise
!          = 'R': Rowwise
!
!  M       (input) INTEGER
!          The number of rows of the matrix C.
!
!  N       (input) INTEGER
!          The number of columns of the matrix C.
!
!  K       (input) INTEGER
!          The order of the matrix T (= the number of elementary
!          reflectors whose product defines the block reflector).
!
!  V       (input) REAL array, dimension
!                                (LDV,K) if STOREV = 'C'
!                                (LDV,M) if STOREV = 'R' and SIDE = 'L'
!                                (LDV,N) if STOREV = 'R' and SIDE = 'R'
!          The matrix V. See Further Details.
!
!  LDV     (input) INTEGER
!          The leading dimension of the array V.
!          If STOREV = 'C' and SIDE = 'L', LDV >= max(1,M);
!          if STOREV = 'C' and SIDE = 'R', LDV >= max(1,N);
!          if STOREV = 'R', LDV >= K.
!
!  T       (input) REAL array, dimension (LDT,K)
!          The triangular k by k matrix T in the representation of the
!          block reflector.
!
!  LDT     (input) INTEGER
!          The leading dimension of the array T. LDT >= K.
!
!  C       (input/output) REAL array, dimension (LDC,N)
!          On entry, the m by n matrix C.
!          On exit, C is overwritten by H*C or H**T*C or C*H or C*H**T.
!
!  LDC     (input) INTEGER
!          The leading dimension of the array C. LDC >= max(1,M).
!
!  WORK    (workspace) REAL array, dimension (LDWORK,K)
!
!  LDWORK  (input) INTEGER
!          The leading dimension of the array WORK.
!          If SIDE = 'L', LDWORK >= max(1,N);
!          if SIDE = 'R', LDWORK >= max(1,M).
!
!  Further Details
!  ===============
!
!  The shape of the matrix V and the storage of the vectors which define
!  the H(i) is best illustrated by the following example with n = 5 and
!  k = 3. The elements equal to 1 are not stored; the corresponding
!  array elements are modified but restored on exit. The rest of the
!  array is not used.
!
!  DIRECT = 'F' and STOREV = 'C':         DIRECT = 'F' and STOREV = 'R':
!
!               V = (  1       )                 V = (  1 v1 v1 v1 v1 )
!                   ( v1  1    )                     (     1 v2 v2 v2 )
!                   ( v1 v2  1 )                     (        1 v3 v3 )
!                   ( v1 v2 v3 )
!                   ( v1 v2 v3 )
!
!  DIRECT = 'B' and STOREV = 'C':         DIRECT = 'B' and STOREV = 'R':
!
!               V = ( v1 v2 v3 )                 V = ( v1 v1  1       )
!                   ( v1 v2 v3 )                     ( v2 v2 v2  1    )
!                   (  1 v2 v3 )                     ( v3 v3 v3 v3  1 )
!                   (     1 v3 )
!                   (        1 )
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ONE
      PARAMETER          ( ONE = 1.0E+0 )
!     ..
!     .. Local Scalars ..
      CHARACTER          TRANST
      INTEGER            I, J, LASTV, LASTC
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILASLR, ILASLC
      EXTERNAL           LSAME, ILASLR, ILASLC
!     ..
!     .. External Subroutines ..
      EXTERNAL           SCOPY, SGEMM, STRMM
!     ..
!     .. Executable Statements ..
!
!     Quick return if possible
!
      IF( M.LE.0 .OR. N.LE.0 ) &
         RETURN
!
      IF( LSAME( TRANS, 'N' ) ) THEN
         TRANST = 'T'
      ELSE
         TRANST = 'N'
      END IF
!
      IF( LSAME( STOREV, 'C' ) ) THEN
!
         IF( LSAME( DIRECT, 'F' ) ) THEN
!
!           Let  V =  ( V1 )    (first K rows)
!                     ( V2 )
!           where  V1  is unit lower triangular.
!
            IF( LSAME( SIDE, 'L' ) ) THEN
!
!              Form  H * C  or  H**T * C  where  C = ( C1 )
!                                                    ( C2 )
!
               LASTV = MAX( K, ILASLR( M, K, V, LDV ) )
               LASTC = ILASLC( LASTV, N, C, LDC )
!
!              W := C**T * V  =  (C1**T * V1 + C2**T * V2)  (stored in WORK)
!
!              W := C1**T
!
               DO 10 J = 1, K
                  CALL SCOPY( LASTC, C( J, 1 ), LDC, WORK( 1, J ), 1 )
   10          CONTINUE
!
!              W := W * V1
!
               CALL STRMM( 'Right', 'Lower', 'No transpose', 'Unit', &
                    LASTC, K, ONE, V, LDV, WORK, LDWORK )
               IF( LASTV.GT.K ) THEN
!
!                 W := W + C2**T *V2
!
                  CALL SGEMM( 'Transpose', 'No transpose', &
                       LASTC, K, LASTV-K, &
                       ONE, C( K+1, 1 ), LDC, V( K+1, 1 ), LDV, &
                       ONE, WORK, LDWORK )
               END IF
!
!              W := W * T**T  or  W * T
!
               CALL STRMM( 'Right', 'Upper', TRANST, 'Non-unit', &
                    LASTC, K, ONE, T, LDT, WORK, LDWORK )
!
!              C := C - V * W**T
!
               IF( LASTV.GT.K ) THEN
!
!                 C2 := C2 - V2 * W**T
!
                  CALL SGEMM( 'No transpose', 'Transpose', &
                       LASTV-K, LASTC, K, &
                       -ONE, V( K+1, 1 ), LDV, WORK, LDWORK, ONE, &
                       C( K+1, 1 ), LDC )
               END IF
!
!              W := W * V1**T
!
               CALL STRMM( 'Right', 'Lower', 'Transpose', 'Unit', &
                    LASTC, K, ONE, V, LDV, WORK, LDWORK )
!
!              C1 := C1 - W**T
!
               DO 30 J = 1, K
                  DO 20 I = 1, LASTC
                     C( J, I ) = C( J, I ) - WORK( I, J )
   20             CONTINUE
   30          CONTINUE
!
            ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!
!              Form  C * H  or  C * H**T  where  C = ( C1  C2 )
!
               LASTV = MAX( K, ILASLR( N, K, V, LDV ) )
               LASTC = ILASLR( M, LASTV, C, LDC )
!
!              W := C * V  =  (C1*V1 + C2*V2)  (stored in WORK)
!
!              W := C1
!
               DO 40 J = 1, K
                  CALL SCOPY( LASTC, C( 1, J ), 1, WORK( 1, J ), 1 )
   40          CONTINUE
!
!              W := W * V1
!
               CALL STRMM( 'Right', 'Lower', 'No transpose', 'Unit', &
                    LASTC, K, ONE, V, LDV, WORK, LDWORK )
               IF( LASTV.GT.K ) THEN
!
!                 W := W + C2 * V2
!
                  CALL SGEMM( 'No transpose', 'No transpose', &
                       LASTC, K, LASTV-K, &
                       ONE, C( 1, K+1 ), LDC, V( K+1, 1 ), LDV, &
                       ONE, WORK, LDWORK )
               END IF
!
!              W := W * T  or  W * T**T
!
               CALL STRMM( 'Right', 'Upper', TRANS, 'Non-unit', &
                    LASTC, K, ONE, T, LDT, WORK, LDWORK )
!
!              C := C - W * V**T
!
               IF( LASTV.GT.K ) THEN
!
!                 C2 := C2 - W * V2**T
!
                  CALL SGEMM( 'No transpose', 'Transpose', &
                       LASTC, LASTV-K, K, &
                       -ONE, WORK, LDWORK, V( K+1, 1 ), LDV, ONE, &
                       C( 1, K+1 ), LDC )
               END IF
!
!              W := W * V1**T
!
               CALL STRMM( 'Right', 'Lower', 'Transpose', 'Unit', &
                    LASTC, K, ONE, V, LDV, WORK, LDWORK )
!
!              C1 := C1 - W
!
               DO 60 J = 1, K
                  DO 50 I = 1, LASTC
                     C( I, J ) = C( I, J ) - WORK( I, J )
   50             CONTINUE
   60          CONTINUE
            END IF
!
         ELSE
!
!           Let  V =  ( V1 )
!                     ( V2 )    (last K rows)
!           where  V2  is unit upper triangular.
!
            IF( LSAME( SIDE, 'L' ) ) THEN
!
!              Form  H * C  or  H**T * C  where  C = ( C1 )
!                                                    ( C2 )
!
               LASTV = MAX( K, ILASLR( M, K, V, LDV ) )
               LASTC = ILASLC( LASTV, N, C, LDC )
!
!              W := C**T * V  =  (C1**T * V1 + C2**T * V2)  (stored in WORK)
!
!              W := C2**T
!
               DO 70 J = 1, K
                  CALL SCOPY( LASTC, C( LASTV-K+J, 1 ), LDC, &
                       WORK( 1, J ), 1 )
   70          CONTINUE
!
!              W := W * V2
!
               CALL STRMM( 'Right', 'Upper', 'No transpose', 'Unit', &
                    LASTC, K, ONE, V( LASTV-K+1, 1 ), LDV, &
                    WORK, LDWORK )
               IF( LASTV.GT.K ) THEN
!
!                 W := W + C1**T*V1
!
                  CALL SGEMM( 'Transpose', 'No transpose', &
                       LASTC, K, LASTV-K, ONE, C, LDC, V, LDV, &
                       ONE, WORK, LDWORK )
               END IF
!
!              W := W * T**T  or  W * T
!
               CALL STRMM( 'Right', 'Lower', TRANST, 'Non-unit', &
                    LASTC, K, ONE, T, LDT, WORK, LDWORK )
!
!              C := C - V * W**T
!
               IF( LASTV.GT.K ) THEN
!
!                 C1 := C1 - V1 * W**T
!
                  CALL SGEMM( 'No transpose', 'Transpose', &
                       LASTV-K, LASTC, K, -ONE, V, LDV, WORK, LDWORK, &
                       ONE, C, LDC )
               END IF
!
!              W := W * V2**T
!
               CALL STRMM( 'Right', 'Upper', 'Transpose', 'Unit', &
                    LASTC, K, ONE, V( LASTV-K+1, 1 ), LDV, &
                    WORK, LDWORK )
!
!              C2 := C2 - W**T
!
               DO 90 J = 1, K
                  DO 80 I = 1, LASTC
                     C( LASTV-K+J, I ) = C( LASTV-K+J, I ) - WORK(I, J)
   80             CONTINUE
   90          CONTINUE
!
            ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!
!              Form  C * H  or  C * H**T  where  C = ( C1  C2 )
!
               LASTV = MAX( K, ILASLR( N, K, V, LDV ) )
               LASTC = ILASLR( M, LASTV, C, LDC )
!
!              W := C * V  =  (C1*V1 + C2*V2)  (stored in WORK)
!
!              W := C2
!
               DO 100 J = 1, K
                  CALL SCOPY( LASTC, C( 1, N-K+J ), 1, WORK( 1, J ), 1 )
  100          CONTINUE
!
!              W := W * V2
!
               CALL STRMM( 'Right', 'Upper', 'No transpose', 'Unit', &
                    LASTC, K, ONE, V( LASTV-K+1, 1 ), LDV, &
                    WORK, LDWORK )
               IF( LASTV.GT.K ) THEN
!
!                 W := W + C1 * V1
!
                  CALL SGEMM( 'No transpose', 'No transpose', &
                       LASTC, K, LASTV-K, ONE, C, LDC, V, LDV, &
                       ONE, WORK, LDWORK )
               END IF
!
!              W := W * T  or  W * T**T
!
               CALL STRMM( 'Right', 'Lower', TRANS, 'Non-unit', &
                    LASTC, K, ONE, T, LDT, WORK, LDWORK )
!
!              C := C - W * V**T
!
               IF( LASTV.GT.K ) THEN
!
!                 C1 := C1 - W * V1**T
!
                  CALL SGEMM( 'No transpose', 'Transpose', &
                       LASTC, LASTV-K, K, -ONE, WORK, LDWORK, V, LDV, &
                       ONE, C, LDC )
               END IF
!
!              W := W * V2**T
!
               CALL STRMM( 'Right', 'Upper', 'Transpose', 'Unit', &
                    LASTC, K, ONE, V( LASTV-K+1, 1 ), LDV, &
                    WORK, LDWORK )
!
!              C2 := C2 - W
!
               DO 120 J = 1, K
                  DO 110 I = 1, LASTC
                     C( I, LASTV-K+J ) = C( I, LASTV-K+J ) - WORK(I, J)
  110             CONTINUE
  120          CONTINUE
            END IF
         END IF
!
      ELSE IF( LSAME( STOREV, 'R' ) ) THEN
!
         IF( LSAME( DIRECT, 'F' ) ) THEN
!
!           Let  V =  ( V1  V2 )    (V1: first K columns)
!           where  V1  is unit upper triangular.
!
            IF( LSAME( SIDE, 'L' ) ) THEN
!
!              Form  H * C  or  H**T * C  where  C = ( C1 )
!                                                    ( C2 )
!
               LASTV = MAX( K, ILASLC( K, M, V, LDV ) )
               LASTC = ILASLC( LASTV, N, C, LDC )
!
!              W := C**T * V**T  =  (C1**T * V1**T + C2**T * V2**T) (stored in WORK)
!
!              W := C1**T
!
               DO 130 J = 1, K
                  CALL SCOPY( LASTC, C( J, 1 ), LDC, WORK( 1, J ), 1 )
  130          CONTINUE
!
!              W := W * V1**T
!
               CALL STRMM( 'Right', 'Upper', 'Transpose', 'Unit', &
                    LASTC, K, ONE, V, LDV, WORK, LDWORK )
               IF( LASTV.GT.K ) THEN
!
!                 W := W + C2**T*V2**T
!
                  CALL SGEMM( 'Transpose', 'Transpose', &
                       LASTC, K, LASTV-K, &
                       ONE, C( K+1, 1 ), LDC, V( 1, K+1 ), LDV, &
                       ONE, WORK, LDWORK )
               END IF
!
!              W := W * T**T  or  W * T
!
               CALL STRMM( 'Right', 'Upper', TRANST, 'Non-unit', &
                    LASTC, K, ONE, T, LDT, WORK, LDWORK )
!
!              C := C - V**T * W**T
!
               IF( LASTV.GT.K ) THEN
!
!                 C2 := C2 - V2**T * W**T
!
                  CALL SGEMM( 'Transpose', 'Transpose', &
                       LASTV-K, LASTC, K, &
                       -ONE, V( 1, K+1 ), LDV, WORK, LDWORK, &
                       ONE, C( K+1, 1 ), LDC )
               END IF
!
!              W := W * V1
!
               CALL STRMM( 'Right', 'Upper', 'No transpose', 'Unit', &
                    LASTC, K, ONE, V, LDV, WORK, LDWORK )
!
!              C1 := C1 - W**T
!
               DO 150 J = 1, K
                  DO 140 I = 1, LASTC
                     C( J, I ) = C( J, I ) - WORK( I, J )
  140             CONTINUE
  150          CONTINUE
!
            ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!
!              Form  C * H  or  C * H**T  where  C = ( C1  C2 )
!
               LASTV = MAX( K, ILASLC( K, N, V, LDV ) )
               LASTC = ILASLR( M, LASTV, C, LDC )
!
!              W := C * V**T  =  (C1*V1**T + C2*V2**T)  (stored in WORK)
!
!              W := C1
!
               DO 160 J = 1, K
                  CALL SCOPY( LASTC, C( 1, J ), 1, WORK( 1, J ), 1 )
  160          CONTINUE
!
!              W := W * V1**T
!
               CALL STRMM( 'Right', 'Upper', 'Transpose', 'Unit', &
                    LASTC, K, ONE, V, LDV, WORK, LDWORK )
               IF( LASTV.GT.K ) THEN
!
!                 W := W + C2 * V2**T
!
                  CALL SGEMM( 'No transpose', 'Transpose', &
                       LASTC, K, LASTV-K, &
                       ONE, C( 1, K+1 ), LDC, V( 1, K+1 ), LDV, &
                       ONE, WORK, LDWORK )
               END IF
!
!              W := W * T  or  W * T**T
!
               CALL STRMM( 'Right', 'Upper', TRANS, 'Non-unit', &
                    LASTC, K, ONE, T, LDT, WORK, LDWORK )
!
!              C := C - W * V
!
               IF( LASTV.GT.K ) THEN
!
!                 C2 := C2 - W * V2
!
                  CALL SGEMM( 'No transpose', 'No transpose', &
                       LASTC, LASTV-K, K, &
                       -ONE, WORK, LDWORK, V( 1, K+1 ), LDV, &
                       ONE, C( 1, K+1 ), LDC )
               END IF
!
!              W := W * V1
!
               CALL STRMM( 'Right', 'Upper', 'No transpose', 'Unit', &
                    LASTC, K, ONE, V, LDV, WORK, LDWORK )
!
!              C1 := C1 - W
!
               DO 180 J = 1, K
                  DO 170 I = 1, LASTC
                     C( I, J ) = C( I, J ) - WORK( I, J )
  170             CONTINUE
  180          CONTINUE
!
            END IF
!
         ELSE
!
!           Let  V =  ( V1  V2 )    (V2: last K columns)
!           where  V2  is unit lower triangular.
!
            IF( LSAME( SIDE, 'L' ) ) THEN
!
!              Form  H * C  or  H**T * C  where  C = ( C1 )
!                                                    ( C2 )
!
               LASTV = MAX( K, ILASLC( K, M, V, LDV ) )
               LASTC = ILASLC( LASTV, N, C, LDC )
!
!              W := C**T * V**T  =  (C1**T * V1**T + C2**T * V2**T) (stored in WORK)
!
!              W := C2**T
!
               DO 190 J = 1, K
                  CALL SCOPY( LASTC, C( LASTV-K+J, 1 ), LDC, &
                       WORK( 1, J ), 1 )
  190          CONTINUE
!
!              W := W * V2**T
!
               CALL STRMM( 'Right', 'Lower', 'Transpose', 'Unit', &
                    LASTC, K, ONE, V( 1, LASTV-K+1 ), LDV, &
                    WORK, LDWORK )
               IF( LASTV.GT.K ) THEN
!
!                 W := W + C1**T * V1**T
!
                  CALL SGEMM( 'Transpose', 'Transpose', &
                       LASTC, K, LASTV-K, ONE, C, LDC, V, LDV, &
                       ONE, WORK, LDWORK )
               END IF
!
!              W := W * T**T  or  W * T
!
               CALL STRMM( 'Right', 'Lower', TRANST, 'Non-unit', &
                    LASTC, K, ONE, T, LDT, WORK, LDWORK )
!
!              C := C - V**T * W**T
!
               IF( LASTV.GT.K ) THEN
!
!                 C1 := C1 - V1**T * W**T
!
                  CALL SGEMM( 'Transpose', 'Transpose', &
                       LASTV-K, LASTC, K, -ONE, V, LDV, WORK, LDWORK, &
                       ONE, C, LDC )
               END IF
!
!              W := W * V2
!
               CALL STRMM( 'Right', 'Lower', 'No transpose', 'Unit', &
                    LASTC, K, ONE, V( 1, LASTV-K+1 ), LDV, &
                    WORK, LDWORK )
!
!              C2 := C2 - W**T
!
               DO 210 J = 1, K
                  DO 200 I = 1, LASTC
                     C( LASTV-K+J, I ) = C( LASTV-K+J, I ) - WORK(I, J)
  200             CONTINUE
  210          CONTINUE
!
            ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!
!              Form  C * H  or  C * H**T  where  C = ( C1  C2 )
!
               LASTV = MAX( K, ILASLC( K, N, V, LDV ) )
               LASTC = ILASLR( M, LASTV, C, LDC )
!
!              W := C * V**T  =  (C1*V1**T + C2*V2**T)  (stored in WORK)
!
!              W := C2
!
               DO 220 J = 1, K
                  CALL SCOPY( LASTC, C( 1, LASTV-K+J ), 1, &
                       WORK( 1, J ), 1 )
  220          CONTINUE
!
!              W := W * V2**T
!
               CALL STRMM( 'Right', 'Lower', 'Transpose', 'Unit', &
                    LASTC, K, ONE, V( 1, LASTV-K+1 ), LDV, &
                    WORK, LDWORK )
               IF( LASTV.GT.K ) THEN
!
!                 W := W + C1 * V1**T
!
                  CALL SGEMM( 'No transpose', 'Transpose', &
                       LASTC, K, LASTV-K, ONE, C, LDC, V, LDV, &
                       ONE, WORK, LDWORK )
               END IF
!
!              W := W * T  or  W * T**T
!
               CALL STRMM( 'Right', 'Lower', TRANS, 'Non-unit', &
                    LASTC, K, ONE, T, LDT, WORK, LDWORK )
!
!              C := C - W * V
!
               IF( LASTV.GT.K ) THEN
!
!                 C1 := C1 - W * V1
!
                  CALL SGEMM( 'No transpose', 'No transpose', &
                       LASTC, LASTV-K, K, -ONE, WORK, LDWORK, V, LDV, &
                       ONE, C, LDC )
               END IF
!
!              W := W * V2
!
               CALL STRMM( 'Right', 'Lower', 'No transpose', 'Unit', &
                    LASTC, K, ONE, V( 1, LASTV-K+1 ), LDV, &
                    WORK, LDWORK )
!
!              C1 := C1 - W
!
               DO 240 J = 1, K
                  DO 230 I = 1, LASTC
                     C( I, LASTV-K+J ) = C( I, LASTV-K+J ) &
                          - WORK( I, J )
  230             CONTINUE
  240          CONTINUE
!
            END IF
!
         END IF
      END IF
!
      RETURN
!
!     End of SLARFB
!
      END
      SUBROUTINE SLARFG( N, ALPHA, X, INCX, TAU )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      INTEGER            INCX, N
      REAL               ALPHA, TAU
!     ..
!     .. Array Arguments ..
      REAL               X( * )
!     ..
!
!  Purpose
!  =======
!
!  SLARFG generates a real elementary reflector H of order n, such
!  that
!
!        H * ( alpha ) = ( beta ),   H**T * H = I.
!            (   x   )   (   0  )
!
!  where alpha and beta are scalars, and x is an (n-1)-element real
!  vector. H is represented in the form
!
!        H = I - tau * ( 1 ) * ( 1 v**T ) ,
!                      ( v )
!
!  where tau is a real scalar and v is a real (n-1)-element
!  vector.
!
!  If the elements of x are all zero, then tau = 0 and H is taken to be
!  the unit matrix.
!
!  Otherwise  1 <= tau <= 2.
!
!  Arguments
!  =========
!
!  N       (input) INTEGER
!          The order of the elementary reflector.
!
!  ALPHA   (input/output) REAL
!          On entry, the value alpha.
!          On exit, it is overwritten with the value beta.
!
!  X       (input/output) REAL array, dimension
!                         (1+(N-2)*abs(INCX))
!          On entry, the vector x.
!          On exit, it is overwritten with the vector v.
!
!  INCX    (input) INTEGER
!          The increment between elements of X. INCX > 0.
!
!  TAU     (output) REAL
!          The value tau.
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ONE, ZERO
      PARAMETER          ( ONE = 1.0E+0, ZERO = 0.0E+0 )
!     ..
!     .. Local Scalars ..
      INTEGER            J, KNT
      REAL               BETA, RSAFMN, SAFMIN, XNORM
!     ..
!     .. External Functions ..
      REAL               SLAMCH, SLAPY2, SNRM2
      EXTERNAL           SLAMCH, SLAPY2, SNRM2
!     ..
!     .. External Subroutines ..
      EXTERNAL           SSCAL
!     ..
!     .. Executable Statements ..
!
      IF( N.LE.1 ) THEN
         TAU = ZERO
         RETURN
      END IF
!
      XNORM = SNRM2( N-1, X, INCX )
!
      IF( XNORM.EQ.ZERO ) THEN
!
!        H  =  I
!
         TAU = ZERO
      ELSE
!
!        general case
!
         BETA = -SIGN( SLAPY2( ALPHA, XNORM ), ALPHA )
         SAFMIN = SLAMCH( 'S' ) / SLAMCH( 'E' )
         KNT = 0
         IF( ABS( BETA ).LT.SAFMIN ) THEN
!
!           XNORM, BETA may be inaccurate; scale X and recompute them
!
            RSAFMN = ONE / SAFMIN
   10       CONTINUE
            KNT = KNT + 1
            CALL SSCAL( N-1, RSAFMN, X, INCX )
            BETA = BETA*RSAFMN
            ALPHA = ALPHA*RSAFMN
            IF( ABS( BETA ).LT.SAFMIN ) &
               GO TO 10
!
!           New BETA is at most 1, at least SAFMIN
!
            XNORM = SNRM2( N-1, X, INCX )
            BETA = -SIGN( SLAPY2( ALPHA, XNORM ), ALPHA )
         END IF
         TAU = ( BETA-ALPHA ) / BETA
         CALL SSCAL( N-1, ONE / ( ALPHA-BETA ), X, INCX )
!
!        If ALPHA is subnormal, it may lose relative accuracy
!
         DO 20 J = 1, KNT
            BETA = BETA*SAFMIN
 20      CONTINUE
         ALPHA = BETA
      END IF
!
      RETURN
!
!     End of SLARFG
!
      END
      SUBROUTINE SLARFT( DIRECT, STOREV, N, K, V, LDV, TAU, T, LDT )
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          DIRECT, STOREV
      INTEGER            K, LDT, LDV, N
!     ..
!     .. Array Arguments ..
      REAL               T( LDT, * ), TAU( * ), V( LDV, * )
!     ..
!
!  Purpose
!  =======
!
!  SLARFT forms the triangular factor T of a real block reflector H
!  of order n, which is defined as a product of k elementary reflectors.
!
!  If DIRECT = 'F', H = H(1) H(2) . . . H(k) and T is upper triangular;
!
!  If DIRECT = 'B', H = H(k) . . . H(2) H(1) and T is lower triangular.
!
!  If STOREV = 'C', the vector which defines the elementary reflector
!  H(i) is stored in the i-th column of the array V, and
!
!     H  =  I - V * T * V**T
!
!  If STOREV = 'R', the vector which defines the elementary reflector
!  H(i) is stored in the i-th row of the array V, and
!
!     H  =  I - V**T * T * V
!
!  Arguments
!  =========
!
!  DIRECT  (input) CHARACTER*1
!          Specifies the order in which the elementary reflectors are
!          multiplied to form the block reflector:
!          = 'F': H = H(1) H(2) . . . H(k) (Forward)
!          = 'B': H = H(k) . . . H(2) H(1) (Backward)
!
!  STOREV  (input) CHARACTER*1
!          Specifies how the vectors which define the elementary
!          reflectors are stored (see also Further Details):
!          = 'C': columnwise
!          = 'R': rowwise
!
!  N       (input) INTEGER
!          The order of the block reflector H. N >= 0.
!
!  K       (input) INTEGER
!          The order of the triangular factor T (= the number of
!          elementary reflectors). K >= 1.
!
!  V       (input/output) REAL array, dimension
!                               (LDV,K) if STOREV = 'C'
!                               (LDV,N) if STOREV = 'R'
!          The matrix V. See further details.
!
!  LDV     (input) INTEGER
!          The leading dimension of the array V.
!          If STOREV = 'C', LDV >= max(1,N); if STOREV = 'R', LDV >= K.
!
!  TAU     (input) REAL array, dimension (K)
!          TAU(i) must contain the scalar factor of the elementary
!          reflector H(i).
!
!  T       (output) REAL array, dimension (LDT,K)
!          The k by k triangular factor T of the block reflector.
!          If DIRECT = 'F', T is upper triangular; if DIRECT = 'B', T is
!          lower triangular. The rest of the array is not used.
!
!  LDT     (input) INTEGER
!          The leading dimension of the array T. LDT >= K.
!
!  Further Details
!  ===============
!
!  The shape of the matrix V and the storage of the vectors which define
!  the H(i) is best illustrated by the following example with n = 5 and
!  k = 3. The elements equal to 1 are not stored; the corresponding
!  array elements are modified but restored on exit. The rest of the
!  array is not used.
!
!  DIRECT = 'F' and STOREV = 'C':         DIRECT = 'F' and STOREV = 'R':
!
!               V = (  1       )                 V = (  1 v1 v1 v1 v1 )
!                   ( v1  1    )                     (     1 v2 v2 v2 )
!                   ( v1 v2  1 )                     (        1 v3 v3 )
!                   ( v1 v2 v3 )
!                   ( v1 v2 v3 )
!
!  DIRECT = 'B' and STOREV = 'C':         DIRECT = 'B' and STOREV = 'R':
!
!               V = ( v1 v2 v3 )                 V = ( v1 v1  1       )
!                   ( v1 v2 v3 )                     ( v2 v2 v2  1    )
!                   (  1 v2 v3 )                     ( v3 v3 v3 v3  1 )
!                   (     1 v3 )
!                   (        1 )
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ONE, ZERO
      PARAMETER          ( ONE = 1.0E+0, ZERO = 0.0E+0 )
!     ..
!     .. Local Scalars ..
      INTEGER            I, J, PREVLASTV, LASTV
      REAL               VII
!     ..
!     .. External Subroutines ..
      EXTERNAL           SGEMV, STRMV
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!     ..
!     .. Executable Statements ..
!
!     Quick return if possible
!
      IF( N.EQ.0 ) &
         RETURN
!
      IF( LSAME( DIRECT, 'F' ) ) THEN
         PREVLASTV = N
         DO 20 I = 1, K
            PREVLASTV = MAX( I, PREVLASTV )
            IF( TAU( I ).EQ.ZERO ) THEN
!
!              H(i)  =  I
!
               DO 10 J = 1, I
                  T( J, I ) = ZERO
   10          CONTINUE
            ELSE
!
!              general case
!
               VII = V( I, I )
               V( I, I ) = ONE
               IF( LSAME( STOREV, 'C' ) ) THEN
!                 Skip any trailing zeros.
                  DO LASTV = N, I+1, -1
                     IF( V( LASTV, I ).NE.ZERO ) EXIT
                  END DO
                  J = MIN( LASTV, PREVLASTV )
!
!                 T(1:i-1,i) := - tau(i) * V(i:j,1:i-1)**T * V(i:j,i)
!
                  CALL SGEMV( 'Transpose', J-I+1, I-1, -TAU( I ), &
                              V( I, 1 ), LDV, V( I, I ), 1, ZERO, &
                              T( 1, I ), 1 )
               ELSE
!                 Skip any trailing zeros.
                  DO LASTV = N, I+1, -1
                     IF( V( I, LASTV ).NE.ZERO ) EXIT
                  END DO
                  J = MIN( LASTV, PREVLASTV )
!
!                 T(1:i-1,i) := - tau(i) * V(1:i-1,i:j) * V(i,i:j)**T
!
                  CALL SGEMV( 'No transpose', I-1, J-I+1, -TAU( I ), &
                              V( 1, I ), LDV, V( I, I ), LDV, ZERO, &
                              T( 1, I ), 1 )
               END IF
               V( I, I ) = VII
!
!              T(1:i-1,i) := T(1:i-1,1:i-1) * T(1:i-1,i)
!
               CALL STRMV( 'Upper', 'No transpose', 'Non-unit', I-1, T, &
                           LDT, T( 1, I ), 1 )
               T( I, I ) = TAU( I )
               IF( I.GT.1 ) THEN
                  PREVLASTV = MAX( PREVLASTV, LASTV )
               ELSE
                  PREVLASTV = LASTV
               END IF
            END IF
   20    CONTINUE
      ELSE
         PREVLASTV = 1
         DO 40 I = K, 1, -1
            IF( TAU( I ).EQ.ZERO ) THEN
!
!              H(i)  =  I
!
               DO 30 J = I, K
                  T( J, I ) = ZERO
   30          CONTINUE
            ELSE
!
!              general case
!
               IF( I.LT.K ) THEN
                  IF( LSAME( STOREV, 'C' ) ) THEN
                     VII = V( N-K+I, I )
                     V( N-K+I, I ) = ONE
!                    Skip any leading zeros.
                     DO LASTV = 1, I-1
                        IF( V( LASTV, I ).NE.ZERO ) EXIT
                     END DO
                     J = MAX( LASTV, PREVLASTV )
!
!                    T(i+1:k,i) :=
!                            - tau(i) * V(j:n-k+i,i+1:k)**T * V(j:n-k+i,i)
!
                     CALL SGEMV( 'Transpose', N-K+I-J+1, K-I, -TAU( I ), &
                                 V( J, I+1 ), LDV, V( J, I ), 1, ZERO, &
                                 T( I+1, I ), 1 )
                     V( N-K+I, I ) = VII
                  ELSE
                     VII = V( I, N-K+I )
                     V( I, N-K+I ) = ONE
!                    Skip any leading zeros.
                     DO LASTV = 1, I-1
                        IF( V( I, LASTV ).NE.ZERO ) EXIT
                     END DO
                     J = MAX( LASTV, PREVLASTV )
!
!                    T(i+1:k,i) :=
!                            - tau(i) * V(i+1:k,j:n-k+i) * V(i,j:n-k+i)**T
!
                     CALL SGEMV( 'No transpose', K-I, N-K+I-J+1, &
                          -TAU( I ), V( I+1, J ), LDV, V( I, J ), LDV, &
                          ZERO, T( I+1, I ), 1 )
                     V( I, N-K+I ) = VII
                  END IF
!
!                 T(i+1:k,i) := T(i+1:k,i+1:k) * T(i+1:k,i)
!
                  CALL STRMV( 'Lower', 'No transpose', 'Non-unit', K-I, &
                              T( I+1, I+1 ), LDT, T( I+1, I ), 1 )
                  IF( I.GT.1 ) THEN
                     PREVLASTV = MIN( PREVLASTV, LASTV )
                  ELSE
                     PREVLASTV = LASTV
                  END IF
               END IF
               T( I, I ) = TAU( I )
            END IF
   40    CONTINUE
      END IF
      RETURN
!
!     End of SLARFT
!
      END
      SUBROUTINE SLASCL( TYPE, KL, KU, CFROM, CTO, M, N, A, LDA, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.3.0) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2010
!
!     .. Scalar Arguments ..
      CHARACTER          TYPE
      INTEGER            INFO, KL, KU, LDA, M, N
      REAL               CFROM, CTO
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * )
!     ..
!
!  Purpose
!  =======
!
!  SLASCL multiplies the M by N real matrix A by the real scalar
!  CTO/CFROM.  This is done without over/underflow as long as the final
!  result CTO*A(I,J)/CFROM does not over/underflow. TYPE specifies that
!  A may be full, upper triangular, lower triangular, upper Hessenberg,
!  or banded.
!
!  Arguments
!  =========
!
!  TYPE    (input) CHARACTER*1
!          TYPE indices the storage type of the input matrix.
!          = 'G':  A is a full matrix.
!          = 'L':  A is a lower triangular matrix.
!          = 'U':  A is an upper triangular matrix.
!          = 'H':  A is an upper Hessenberg matrix.
!          = 'B':  A is a symmetric band matrix with lower bandwidth KL
!                  and upper bandwidth KU and with the only the lower
!                  half stored.
!          = 'Q':  A is a symmetric band matrix with lower bandwidth KL
!                  and upper bandwidth KU and with the only the upper
!                  half stored.
!          = 'Z':  A is a band matrix with lower bandwidth KL and upper
!                  bandwidth KU. See SGBTRF for storage details.
!
!  KL      (input) INTEGER
!          The lower bandwidth of A.  Referenced only if TYPE = 'B',
!          'Q' or 'Z'.
!
!  KU      (input) INTEGER
!          The upper bandwidth of A.  Referenced only if TYPE = 'B',
!          'Q' or 'Z'.
!
!  CFROM   (input) REAL
!  CTO     (input) REAL
!          The matrix A is multiplied by CTO/CFROM. A(I,J) is computed
!          without over/underflow if the final result CTO*A(I,J)/CFROM
!          can be represented without over/underflow.  CFROM must be
!          nonzero.
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.
!
!  A       (input/output) REAL array, dimension (LDA,N)
!          The matrix to be multiplied by CTO/CFROM.  See TYPE for the
!          storage type.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,M).
!
!  INFO    (output) INTEGER
!          0  - successful exit
!          <0 - if INFO = -i, the i-th argument had an illegal value.
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ZERO, ONE
      PARAMETER          ( ZERO = 0.0E0, ONE = 1.0E0 )
!     ..
!     .. Local Scalars ..
      LOGICAL            DONE
      INTEGER            I, ITYPE, J, K1, K2, K3, K4
      REAL               BIGNUM, CFROM1, CFROMC, CTO1, CTOC, MUL, SMLNUM
!     ..
!     .. External Functions ..
      LOGICAL            LSAME, SISNAN
      REAL               SLAMCH
      EXTERNAL           LSAME, SLAMCH, SISNAN
!     ..
!     .. External Subroutines ..
      EXTERNAL           XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
!
      IF( LSAME( TYPE, 'G' ) ) THEN
         ITYPE = 0
      ELSE IF( LSAME( TYPE, 'L' ) ) THEN
         ITYPE = 1
      ELSE IF( LSAME( TYPE, 'U' ) ) THEN
         ITYPE = 2
      ELSE IF( LSAME( TYPE, 'H' ) ) THEN
         ITYPE = 3
      ELSE IF( LSAME( TYPE, 'B' ) ) THEN
         ITYPE = 4
      ELSE IF( LSAME( TYPE, 'Q' ) ) THEN
         ITYPE = 5
      ELSE IF( LSAME( TYPE, 'Z' ) ) THEN
         ITYPE = 6
      ELSE
         ITYPE = -1
      END IF
!
      IF( ITYPE.EQ.-1 ) THEN
         INFO = -1
      ELSE IF( CFROM.EQ.ZERO .OR. SISNAN(CFROM) ) THEN
         INFO = -4
      ELSE IF( SISNAN(CTO) ) THEN
         INFO = -5
      ELSE IF( M.LT.0 ) THEN
         INFO = -6
      ELSE IF( N.LT.0 .OR. ( ITYPE.EQ.4 .AND. N.NE.M ) .OR. &
               ( ITYPE.EQ.5 .AND. N.NE.M ) ) THEN
         INFO = -7
      ELSE IF( ITYPE.LE.3 .AND. LDA.LT.MAX( 1, M ) ) THEN
         INFO = -9
      ELSE IF( ITYPE.GE.4 ) THEN
         IF( KL.LT.0 .OR. KL.GT.MAX( M-1, 0 ) ) THEN
            INFO = -2
         ELSE IF( KU.LT.0 .OR. KU.GT.MAX( N-1, 0 ) .OR. &
                  ( ( ITYPE.EQ.4 .OR. ITYPE.EQ.5 ) .AND. KL.NE.KU ) ) &
                   THEN
            INFO = -3
         ELSE IF( ( ITYPE.EQ.4 .AND. LDA.LT.KL+1 ) .OR. &
                  ( ITYPE.EQ.5 .AND. LDA.LT.KU+1 ) .OR. &
                  ( ITYPE.EQ.6 .AND. LDA.LT.2*KL+KU+1 ) ) THEN
            INFO = -9
         END IF
      END IF
!
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SLASCL', -INFO )
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( N.EQ.0 .OR. M.EQ.0 ) &
         RETURN
!
!     Get machine parameters
!
      SMLNUM = SLAMCH( 'S' )
      BIGNUM = ONE / SMLNUM
!
      CFROMC = CFROM
      CTOC = CTO
!
   10 CONTINUE
      CFROM1 = CFROMC*SMLNUM
      IF( CFROM1.EQ.CFROMC ) THEN
!        CFROMC is an inf.  Multiply by a correctly signed zero for
!        finite CTOC, or a NaN if CTOC is infinite.
         MUL = CTOC / CFROMC
         DONE = .TRUE.
         CTO1 = CTOC
      ELSE
         CTO1 = CTOC / BIGNUM
         IF( CTO1.EQ.CTOC ) THEN
!           CTOC is either 0 or an inf.  In both cases, CTOC itself
!           serves as the correct multiplication factor.
            MUL = CTOC
            DONE = .TRUE.
            CFROMC = ONE
         ELSE IF( ABS( CFROM1 ).GT.ABS( CTOC ) .AND. CTOC.NE.ZERO ) THEN
            MUL = SMLNUM
            DONE = .FALSE.
            CFROMC = CFROM1
         ELSE IF( ABS( CTO1 ).GT.ABS( CFROMC ) ) THEN
            MUL = BIGNUM
            DONE = .FALSE.
            CTOC = CTO1
         ELSE
            MUL = CTOC / CFROMC
            DONE = .TRUE.
         END IF
      END IF
!
      IF( ITYPE.EQ.0 ) THEN
!
!        Full matrix
!
         DO 30 J = 1, N
            DO 20 I = 1, M
               A( I, J ) = A( I, J )*MUL
   20       CONTINUE
   30    CONTINUE
!
      ELSE IF( ITYPE.EQ.1 ) THEN
!
!        Lower triangular matrix
!
         DO 50 J = 1, N
            DO 40 I = J, M
               A( I, J ) = A( I, J )*MUL
   40       CONTINUE
   50    CONTINUE
!
      ELSE IF( ITYPE.EQ.2 ) THEN
!
!        Upper triangular matrix
!
         DO 70 J = 1, N
            DO 60 I = 1, MIN( J, M )
               A( I, J ) = A( I, J )*MUL
   60       CONTINUE
   70    CONTINUE
!
      ELSE IF( ITYPE.EQ.3 ) THEN
!
!        Upper Hessenberg matrix
!
         DO 90 J = 1, N
            DO 80 I = 1, MIN( J+1, M )
               A( I, J ) = A( I, J )*MUL
   80       CONTINUE
   90    CONTINUE
!
      ELSE IF( ITYPE.EQ.4 ) THEN
!
!        Lower half of a symmetric band matrix
!
         K3 = KL + 1
         K4 = N + 1
         DO 110 J = 1, N
            DO 100 I = 1, MIN( K3, K4-J )
               A( I, J ) = A( I, J )*MUL
  100       CONTINUE
  110    CONTINUE
!
      ELSE IF( ITYPE.EQ.5 ) THEN
!
!        Upper half of a symmetric band matrix
!
         K1 = KU + 2
         K3 = KU + 1
         DO 130 J = 1, N
            DO 120 I = MAX( K1-J, 1 ), K3
               A( I, J ) = A( I, J )*MUL
  120       CONTINUE
  130    CONTINUE
!
      ELSE IF( ITYPE.EQ.6 ) THEN
!
!        Band matrix
!
         K1 = KL + KU + 2
         K2 = KL + 1
         K3 = 2*KL + KU + 1
         K4 = KL + KU + 1 + M
         DO 150 J = 1, N
            DO 140 I = MAX( K1-J, K2 ), MIN( K3, K4-J )
               A( I, J ) = A( I, J )*MUL
  140       CONTINUE
  150    CONTINUE
!
      END IF
!
      IF( .NOT.DONE ) &
         GO TO 10
!
      RETURN
!
!     End of SLASCL
!
      END
      SUBROUTINE SLASET( UPLO, M, N, ALPHA, BETA, A, LDA )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2006
!
!     .. Scalar Arguments ..
      CHARACTER          UPLO
      INTEGER            LDA, M, N
      REAL               ALPHA, BETA
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * )
!     ..
!
!  Purpose
!  =======
!
!  SLASET initializes an m-by-n matrix A to BETA on the diagonal and
!  ALPHA on the offdiagonals.
!
!  Arguments
!  =========
!
!  UPLO    (input) CHARACTER*1
!          Specifies the part of the matrix A to be set.
!          = 'U':      Upper triangular part is set; the strictly lower
!                      triangular part of A is not changed.
!          = 'L':      Lower triangular part is set; the strictly upper
!                      triangular part of A is not changed.
!          Otherwise:  All of the matrix A is set.
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.
!
!  ALPHA   (input) REAL
!          The constant to which the offdiagonal elements are to be set.
!
!  BETA    (input) REAL
!          The constant to which the diagonal elements are to be set.
!
!  A       (input/output) REAL array, dimension (LDA,N)
!          On exit, the leading m-by-n submatrix of A is set as follows:
!
!          if UPLO = 'U', A(i,j) = ALPHA, 1<=i<=j-1, 1<=j<=n,
!          if UPLO = 'L', A(i,j) = ALPHA, j+1<=i<=m, 1<=j<=n,
!          otherwise,     A(i,j) = ALPHA, 1<=i<=m, 1<=j<=n, i.ne.j,
!
!          and, for all UPLO, A(i,i) = BETA, 1<=i<=min(m,n).
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,M).
!
! =====================================================================
!
!     .. Local Scalars ..
      INTEGER            I, J
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!     ..
!     .. Executable Statements ..
!
      IF( LSAME( UPLO, 'U' ) ) THEN
!
!        Set the strictly upper triangular or trapezoidal part of the
!        array to ALPHA.
!
         DO 20 J = 2, N
            DO 10 I = 1, MIN( J-1, M )
               A( I, J ) = ALPHA
   10       CONTINUE
   20    CONTINUE
!
      ELSE IF( LSAME( UPLO, 'L' ) ) THEN
!
!        Set the strictly lower triangular or trapezoidal part of the
!        array to ALPHA.
!
         DO 40 J = 1, MIN( M, N )
            DO 30 I = J + 1, M
               A( I, J ) = ALPHA
   30       CONTINUE
   40    CONTINUE
!
      ELSE
!
!        Set the leading m-by-n submatrix to ALPHA.
!
         DO 60 J = 1, N
            DO 50 I = 1, M
               A( I, J ) = ALPHA
   50       CONTINUE
   60    CONTINUE
      END IF
!
!     Set the first min(M,N) diagonal elements to BETA.
!
      DO 70 I = 1, MIN( M, N )
         A( I, I ) = BETA
   70 CONTINUE
!
      RETURN
!
!     End of SLASET
!
      END
      SUBROUTINE SLASSQ( N, X, INCX, SCALE, SUMSQ )
!
      IMPLICIT NONE
!
!  -- LAPACK auxiliary routine (version 3.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2006
!
!     .. Scalar Arguments ..
      INTEGER            INCX, N
      REAL               SCALE, SUMSQ
!     ..
!     .. Array Arguments ..
      REAL               X( * )
!     ..
!
!  Purpose
!  =======
!
!  SLASSQ  returns the values  scl  and  smsq  such that
!
!     ( scl**2 )*smsq = x( 1 )**2 +...+ x( n )**2 + ( scale**2 )*sumsq,
!
!  where  x( i ) = X( 1 + ( i - 1 )*INCX ). The value of  sumsq  is
!  assumed to be non-negative and  scl  returns the value
!
!     scl = max( scale, abs( x( i ) ) ).
!
!  scale and sumsq must be supplied in SCALE and SUMSQ and
!  scl and smsq are overwritten on SCALE and SUMSQ respectively.
!
!  The routine makes only one pass through the vector x.
!
!  Arguments
!  =========
!
!  N       (input) INTEGER
!          The number of elements to be used from the vector X.
!
!  X       (input) REAL array, dimension (N)
!          The vector for which a scaled sum of squares is computed.
!             x( i )  = X( 1 + ( i - 1 )*INCX ), 1 <= i <= n.
!
!  INCX    (input) INTEGER
!          The increment between successive values of the vector X.
!          INCX > 0.
!
!  SCALE   (input/output) REAL
!          On entry, the value  scale  in the equation above.
!          On exit, SCALE is overwritten with  scl , the scaling factor
!          for the sum of squares.
!
!  SUMSQ   (input/output) REAL
!          On entry, the value  sumsq  in the equation above.
!          On exit, SUMSQ is overwritten with  smsq , the basic sum of
!          squares from which  scl  has been factored out.
!
! =====================================================================
!
!     .. Parameters ..
      REAL               ZERO
      PARAMETER          ( ZERO = 0.0E+0 )
!     ..
!     .. Local Scalars ..
      INTEGER            IX
      REAL               ABSXI
!     ..
!     .. Executable Statements ..
!
      IF( N.GT.0 ) THEN
         DO 10 IX = 1, 1 + ( N-1 )*INCX, INCX
            IF( X( IX ).NE.ZERO ) THEN
               ABSXI = ABS( X( IX ) )
               IF( SCALE.LT.ABSXI ) THEN
                  SUMSQ = 1 + SUMSQ*( SCALE / ABSXI )**2
                  SCALE = ABSXI
               ELSE
                  SUMSQ = SUMSQ + ( ABSXI / SCALE )**2
               END IF
            END IF
   10    CONTINUE
      END IF
      RETURN
!
!     End of SLASSQ
!
      END
      SUBROUTINE SORM2R( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS
      INTEGER            INFO, K, LDA, LDC, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SORM2R overwrites the general real m by n matrix C with
!
!        Q * C  if SIDE = 'L' and TRANS = 'N', or
!
!        Q**T* C  if SIDE = 'L' and TRANS = 'T', or
!
!        C * Q  if SIDE = 'R' and TRANS = 'N', or
!
!        C * Q**T if SIDE = 'R' and TRANS = 'T',
!
!  where Q is a real orthogonal matrix defined as the product of k
!  elementary reflectors
!
!        Q = H(1) H(2) . . . H(k)
!
!  as returned by SGEQRF. Q is of order m if SIDE = 'L' and of order n
!  if SIDE = 'R'.
!
!  Arguments
!  =========
!
!  SIDE    (input) CHARACTER*1
!          = 'L': apply Q or Q**T from the Left
!          = 'R': apply Q or Q**T from the Right
!
!  TRANS   (input) CHARACTER*1
!          = 'N': apply Q  (No transpose)
!          = 'T': apply Q**T (Transpose)
!
!  M       (input) INTEGER
!          The number of rows of the matrix C. M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix C. N >= 0.
!
!  K       (input) INTEGER
!          The number of elementary reflectors whose product defines
!          the matrix Q.
!          If SIDE = 'L', M >= K >= 0;
!          if SIDE = 'R', N >= K >= 0.
!
!  A       (input) REAL array, dimension (LDA,K)
!          The i-th column must contain the vector which defines the
!          elementary reflector H(i), for i = 1,2,...,k, as returned by
!          SGEQRF in the first k columns of its array argument A.
!          A is modified by the routine but restored on exit.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.
!          If SIDE = 'L', LDA >= max(1,M);
!          if SIDE = 'R', LDA >= max(1,N).
!
!  TAU     (input) REAL array, dimension (K)
!          TAU(i) must contain the scalar factor of the elementary
!          reflector H(i), as returned by SGEQRF.
!
!  C       (input/output) REAL array, dimension (LDC,N)
!          On entry, the m by n matrix C.
!          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q.
!
!  LDC     (input) INTEGER
!          The leading dimension of the array C. LDC >= max(1,M).
!
!  WORK    (workspace) REAL array, dimension
!                                   (N) if SIDE = 'L',
!                                   (M) if SIDE = 'R'
!
!  INFO    (output) INTEGER
!          = 0: successful exit
!          < 0: if INFO = -i, the i-th argument had an illegal value
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ONE
      PARAMETER          ( ONE = 1.0E+0 )
!     ..
!     .. Local Scalars ..
      LOGICAL            LEFT, NOTRAN
      INTEGER            I, I1, I2, I3, IC, JC, MI, NI, NQ
      REAL               AII
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!     ..
!     .. External Subroutines ..
      EXTERNAL           SLARF, XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
!
!     NQ is the order of Q
!
      IF( LEFT ) THEN
         NQ = M
      ELSE
         NQ = N
      END IF
      IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( K.LT.0 .OR. K.GT.NQ ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, NQ ) ) THEN
         INFO = -7
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -10
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SORM2R', -INFO )
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( M.EQ.0 .OR. N.EQ.0 .OR. K.EQ.0 ) &
         RETURN
!
      IF( ( LEFT .AND. .NOT.NOTRAN ) .OR. ( .NOT.LEFT .AND. NOTRAN ) ) &
           THEN
         I1 = 1
         I2 = K
         I3 = 1
      ELSE
         I1 = K
         I2 = 1
         I3 = -1
      END IF
!
      IF( LEFT ) THEN
         NI = N
         JC = 1
      ELSE
         MI = M
         IC = 1
      END IF
!
      DO 10 I = I1, I2, I3
         IF( LEFT ) THEN
!
!           H(i) is applied to C(i:m,1:n)
!
            MI = M - I + 1
            IC = I
         ELSE
!
!           H(i) is applied to C(1:m,i:n)
!
            NI = N - I + 1
            JC = I
         END IF
!
!        Apply H(i)
!
         AII = A( I, I )
         A( I, I ) = ONE
         CALL SLARF( SIDE, MI, NI, A( I, I ), 1, TAU( I ), C( IC, JC ), &
                     LDC, WORK )
         A( I, I ) = AII
   10 CONTINUE
      RETURN
!
!     End of SORM2R
!
      END
      SUBROUTINE SORML2( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS
      INTEGER            INFO, K, LDA, LDC, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SORML2 overwrites the general real m by n matrix C with
!
!        Q * C  if SIDE = 'L' and TRANS = 'N', or
!
!        Q**T* C  if SIDE = 'L' and TRANS = 'T', or
!
!        C * Q  if SIDE = 'R' and TRANS = 'N', or
!
!        C * Q**T if SIDE = 'R' and TRANS = 'T',
!
!  where Q is a real orthogonal matrix defined as the product of k
!  elementary reflectors
!
!        Q = H(k) . . . H(2) H(1)
!
!  as returned by SGELQF. Q is of order m if SIDE = 'L' and of order n
!  if SIDE = 'R'.
!
!  Arguments
!  =========
!
!  SIDE    (input) CHARACTER*1
!          = 'L': apply Q or Q**T from the Left
!          = 'R': apply Q or Q**T from the Right
!
!  TRANS   (input) CHARACTER*1
!          = 'N': apply Q  (No transpose)
!          = 'T': apply Q**T (Transpose)
!
!  M       (input) INTEGER
!          The number of rows of the matrix C. M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix C. N >= 0.
!
!  K       (input) INTEGER
!          The number of elementary reflectors whose product defines
!          the matrix Q.
!          If SIDE = 'L', M >= K >= 0;
!          if SIDE = 'R', N >= K >= 0.
!
!  A       (input) REAL array, dimension
!                               (LDA,M) if SIDE = 'L',
!                               (LDA,N) if SIDE = 'R'
!          The i-th row must contain the vector which defines the
!          elementary reflector H(i), for i = 1,2,...,k, as returned by
!          SGELQF in the first k rows of its array argument A.
!          A is modified by the routine but restored on exit.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A. LDA >= max(1,K).
!
!  TAU     (input) REAL array, dimension (K)
!          TAU(i) must contain the scalar factor of the elementary
!          reflector H(i), as returned by SGELQF.
!
!  C       (input/output) REAL array, dimension (LDC,N)
!          On entry, the m by n matrix C.
!          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q.
!
!  LDC     (input) INTEGER
!          The leading dimension of the array C. LDC >= max(1,M).
!
!  WORK    (workspace) REAL array, dimension
!                                   (N) if SIDE = 'L',
!                                   (M) if SIDE = 'R'
!
!  INFO    (output) INTEGER
!          = 0: successful exit
!          < 0: if INFO = -i, the i-th argument had an illegal value
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ONE
      PARAMETER          ( ONE = 1.0E+0 )
!     ..
!     .. Local Scalars ..
      LOGICAL            LEFT, NOTRAN
      INTEGER            I, I1, I2, I3, IC, JC, MI, NI, NQ
      REAL               AII
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!     ..
!     .. External Subroutines ..
      EXTERNAL           SLARF, XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
!
!     NQ is the order of Q
!
      IF( LEFT ) THEN
         NQ = M
      ELSE
         NQ = N
      END IF
      IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( K.LT.0 .OR. K.GT.NQ ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, K ) ) THEN
         INFO = -7
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -10
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SORML2', -INFO )
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( M.EQ.0 .OR. N.EQ.0 .OR. K.EQ.0 ) &
         RETURN
!
      IF( ( LEFT .AND. NOTRAN ) .OR. ( .NOT.LEFT .AND. .NOT.NOTRAN ) ) &
           THEN
         I1 = 1
         I2 = K
         I3 = 1
      ELSE
         I1 = K
         I2 = 1
         I3 = -1
      END IF
!
      IF( LEFT ) THEN
         NI = N
         JC = 1
      ELSE
         MI = M
         IC = 1
      END IF
!
      DO 10 I = I1, I2, I3
         IF( LEFT ) THEN
!
!           H(i) is applied to C(i:m,1:n)
!
            MI = M - I + 1
            IC = I
         ELSE
!
!           H(i) is applied to C(1:m,i:n)
!
            NI = N - I + 1
            JC = I
         END IF
!
!        Apply H(i)
!
         AII = A( I, I )
         A( I, I ) = ONE
         CALL SLARF( SIDE, MI, NI, A( I, I ), LDA, TAU( I ), &
                     C( IC, JC ), LDC, WORK )
         A( I, I ) = AII
   10 CONTINUE
      RETURN
!
!     End of SORML2
!
      END
      SUBROUTINE SORMLQ( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, LWORK, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS
      INTEGER            INFO, K, LDA, LDC, LWORK, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), C( LDC, * ), TAU( * ), &
                         WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SORMLQ overwrites the general real M-by-N matrix C with
!
!                  SIDE = 'L'     SIDE = 'R'
!  TRANS = 'N':      Q * C          C * Q
!  TRANS = 'T':      Q**T * C       C * Q**T
!
!  where Q is a real orthogonal matrix defined as the product of k
!  elementary reflectors
!
!        Q = H(k) . . . H(2) H(1)
!
!  as returned by SGELQF. Q is of order M if SIDE = 'L' and of order N
!  if SIDE = 'R'.
!
!  Arguments
!  =========
!
!  SIDE    (input) CHARACTER*1
!          = 'L': apply Q or Q**T from the Left;
!          = 'R': apply Q or Q**T from the Right.
!
!  TRANS   (input) CHARACTER*1
!          = 'N':  No transpose, apply Q;
!          = 'T':  Transpose, apply Q**T.
!
!  M       (input) INTEGER
!          The number of rows of the matrix C. M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix C. N >= 0.
!
!  K       (input) INTEGER
!          The number of elementary reflectors whose product defines
!          the matrix Q.
!          If SIDE = 'L', M >= K >= 0;
!          if SIDE = 'R', N >= K >= 0.
!
!  A       (input) REAL array, dimension
!                               (LDA,M) if SIDE = 'L',
!                               (LDA,N) if SIDE = 'R'
!          The i-th row must contain the vector which defines the
!          elementary reflector H(i), for i = 1,2,...,k, as returned by
!          SGELQF in the first k rows of its array argument A.
!          A is modified by the routine but restored on exit.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A. LDA >= max(1,K).
!
!  TAU     (input) REAL array, dimension (K)
!          TAU(i) must contain the scalar factor of the elementary
!          reflector H(i), as returned by SGELQF.
!
!  C       (input/output) REAL array, dimension (LDC,N)
!          On entry, the M-by-N matrix C.
!          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q.
!
!  LDC     (input) INTEGER
!          The leading dimension of the array C. LDC >= max(1,M).
!
!  WORK    (workspace/output) REAL array, dimension (MAX(1,LWORK))
!          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!
!  LWORK   (input) INTEGER
!          The dimension of the array WORK.
!          If SIDE = 'L', LWORK >= max(1,N);
!          if SIDE = 'R', LWORK >= max(1,M).
!          For optimum performance LWORK >= N*NB if SIDE = 'L', and
!          LWORK >= M*NB if SIDE = 'R', where NB is the optimal
!          blocksize.
!
!          If LWORK = -1, then a workspace query is assumed; the routine
!          only calculates the optimal size of the WORK array, returns
!          this value as the first entry of the WORK array, and no error
!          message related to LWORK is issued by XERBLA.
!
!  INFO    (output) INTEGER
!          = 0:  successful exit
!          < 0:  if INFO = -i, the i-th argument had an illegal value
!
!  =====================================================================
!
!     .. Parameters ..
      INTEGER            NBMAX, LDT
      PARAMETER          ( NBMAX = 64, LDT = NBMAX+1 )
!     ..
!     .. Local Scalars ..
      LOGICAL            LEFT, LQUERY, NOTRAN
      CHARACTER          TRANST
      INTEGER            I, I1, I2, I3, IB, IC, IINFO, IWS, JC, LDWORK, &
                         LWKOPT, MI, NB, NBMIN, NI, NQ, NW
!     ..
!     .. Local Arrays ..
      REAL               T( LDT, NBMAX )
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      EXTERNAL           LSAME, ILAENV
!     ..
!     .. External Subroutines ..
      EXTERNAL           SLARFB, SLARFT, SORML2, XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
      LQUERY = ( LWORK.EQ.-1 )
!
!     NQ is the order of Q and NW is the minimum dimension of WORK
!
      IF( LEFT ) THEN
         NQ = M
         NW = N
      ELSE
         NQ = N
         NW = M
      END IF
      IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( K.LT.0 .OR. K.GT.NQ ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, K ) ) THEN
         INFO = -7
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -10
      ELSE IF( LWORK.LT.MAX( 1, NW ) .AND. .NOT.LQUERY ) THEN
         INFO = -12
      END IF
!
      IF( INFO.EQ.0 ) THEN
!
!        Determine the block size.  NB may be at most NBMAX, where NBMAX
!        is used to define the local array T.
!
         NB = MIN( NBMAX, ILAENV( 1, 'SORMLQ', SIDE // TRANS, M, N, K, &
                   -1 ) )
         LWKOPT = MAX( 1, NW )*NB
         WORK( 1 ) = LWKOPT
      END IF
!
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SORMLQ', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( M.EQ.0 .OR. N.EQ.0 .OR. K.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!
      NBMIN = 2
      LDWORK = NW
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
         IWS = NW*NB
         IF( LWORK.LT.IWS ) THEN
            NB = LWORK / LDWORK
            NBMIN = MAX( 2, ILAENV( 2, 'SORMLQ', SIDE // TRANS, M, N, K, &
                    -1 ) )
         END IF
      ELSE
         IWS = NW
      END IF
!
      IF( NB.LT.NBMIN .OR. NB.GE.K ) THEN
!
!        Use unblocked code
!
         CALL SORML2( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, &
                      IINFO )
      ELSE
!
!        Use blocked code
!
         IF( ( LEFT .AND. NOTRAN ) .OR. &
             ( .NOT.LEFT .AND. .NOT.NOTRAN ) ) THEN
            I1 = 1
            I2 = K
            I3 = NB
         ELSE
            I1 = ( ( K-1 ) / NB )*NB + 1
            I2 = 1
            I3 = -NB
         END IF
!
         IF( LEFT ) THEN
            NI = N
            JC = 1
         ELSE
            MI = M
            IC = 1
         END IF
!
         IF( NOTRAN ) THEN
            TRANST = 'T'
         ELSE
            TRANST = 'N'
         END IF
!
         DO 10 I = I1, I2, I3
            IB = MIN( NB, K-I+1 )
!
!           Form the triangular factor of the block reflector
!           H = H(i) H(i+1) . . . H(i+ib-1)
!
            CALL SLARFT( 'Forward', 'Rowwise', NQ-I+1, IB, A( I, I ), &
                         LDA, TAU( I ), T, LDT )
            IF( LEFT ) THEN
!
!              H or H**T is applied to C(i:m,1:n)
!
               MI = M - I + 1
               IC = I
            ELSE
!
!              H or H**T is applied to C(1:m,i:n)
!
               NI = N - I + 1
               JC = I
            END IF
!
!           Apply H or H**T
!
            CALL SLARFB( SIDE, TRANST, 'Forward', 'Rowwise', MI, NI, IB, &
                         A( I, I ), LDA, T, LDT, C( IC, JC ), LDC, WORK, &
                         LDWORK )
   10    CONTINUE
      END IF
      WORK( 1 ) = LWKOPT
      RETURN
!
!     End of SORMLQ
!
      END
      SUBROUTINE SORMQR( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, LWORK, INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS
      INTEGER            INFO, K, LDA, LDC, LWORK, M, N
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), C( LDC, * ), TAU( * ), &
                         WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  SORMQR overwrites the general real M-by-N matrix C with
!
!                  SIDE = 'L'     SIDE = 'R'
!  TRANS = 'N':      Q * C          C * Q
!  TRANS = 'T':      Q**T * C       C * Q**T
!
!  where Q is a real orthogonal matrix defined as the product of k
!  elementary reflectors
!
!        Q = H(1) H(2) . . . H(k)
!
!  as returned by SGEQRF. Q is of order M if SIDE = 'L' and of order N
!  if SIDE = 'R'.
!
!  Arguments
!  =========
!
!  SIDE    (input) CHARACTER*1
!          = 'L': apply Q or Q**T from the Left;
!          = 'R': apply Q or Q**T from the Right.
!
!  TRANS   (input) CHARACTER*1
!          = 'N':  No transpose, apply Q;
!          = 'T':  Transpose, apply Q**T.
!
!  M       (input) INTEGER
!          The number of rows of the matrix C. M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix C. N >= 0.
!
!  K       (input) INTEGER
!          The number of elementary reflectors whose product defines
!          the matrix Q.
!          If SIDE = 'L', M >= K >= 0;
!          if SIDE = 'R', N >= K >= 0.
!
!  A       (input) REAL array, dimension (LDA,K)
!          The i-th column must contain the vector which defines the
!          elementary reflector H(i), for i = 1,2,...,k, as returned by
!          SGEQRF in the first k columns of its array argument A.
!          A is modified by the routine but restored on exit.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.
!          If SIDE = 'L', LDA >= max(1,M);
!          if SIDE = 'R', LDA >= max(1,N).
!
!  TAU     (input) REAL array, dimension (K)
!          TAU(i) must contain the scalar factor of the elementary
!          reflector H(i), as returned by SGEQRF.
!
!  C       (input/output) REAL array, dimension (LDC,N)
!          On entry, the M-by-N matrix C.
!          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q.
!
!  LDC     (input) INTEGER
!          The leading dimension of the array C. LDC >= max(1,M).
!
!  WORK    (workspace/output) REAL array, dimension (MAX(1,LWORK))
!          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!
!  LWORK   (input) INTEGER
!          The dimension of the array WORK.
!          If SIDE = 'L', LWORK >= max(1,N);
!          if SIDE = 'R', LWORK >= max(1,M).
!          For optimum performance LWORK >= N*NB if SIDE = 'L', and
!          LWORK >= M*NB if SIDE = 'R', where NB is the optimal
!          blocksize.
!
!          If LWORK = -1, then a workspace query is assumed; the routine
!          only calculates the optimal size of the WORK array, returns
!          this value as the first entry of the WORK array, and no error
!          message related to LWORK is issued by XERBLA.
!
!  INFO    (output) INTEGER
!          = 0:  successful exit
!          < 0:  if INFO = -i, the i-th argument had an illegal value
!
!  =====================================================================
!
!     .. Parameters ..
      INTEGER            NBMAX, LDT
      PARAMETER          ( NBMAX = 64, LDT = NBMAX+1 )
!     ..
!     .. Local Scalars ..
      LOGICAL            LEFT, LQUERY, NOTRAN
      INTEGER            I, I1, I2, I3, IB, IC, IINFO, IWS, JC, LDWORK, &
                         LWKOPT, MI, NB, NBMIN, NI, NQ, NW
!     ..
!     .. Local Arrays ..
      REAL               T( LDT, NBMAX )
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      EXTERNAL           LSAME, ILAENV
!     ..
!     .. External Subroutines ..
      EXTERNAL           SLARFB, SLARFT, SORM2R, XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments
!
      INFO = 0
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
      LQUERY = ( LWORK.EQ.-1 )
!
!     NQ is the order of Q and NW is the minimum dimension of WORK
!
      IF( LEFT ) THEN
         NQ = M
         NW = N
      ELSE
         NQ = N
         NW = M
      END IF
      IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( K.LT.0 .OR. K.GT.NQ ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, NQ ) ) THEN
         INFO = -7
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -10
      ELSE IF( LWORK.LT.MAX( 1, NW ) .AND. .NOT.LQUERY ) THEN
         INFO = -12
      END IF
!
      IF( INFO.EQ.0 ) THEN
!
!        Determine the block size.  NB may be at most NBMAX, where NBMAX
!        is used to define the local array T.
!
         NB = MIN( NBMAX, ILAENV( 1, 'SORMQR', SIDE // TRANS, M, N, K, &
              -1 ) )
         LWKOPT = MAX( 1, NW )*NB
         WORK( 1 ) = LWKOPT
      END IF
!
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'SORMQR', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( M.EQ.0 .OR. N.EQ.0 .OR. K.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!
      NBMIN = 2
      LDWORK = NW
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
         IWS = NW*NB
         IF( LWORK.LT.IWS ) THEN
            NB = LWORK / LDWORK
            NBMIN = MAX( 2, ILAENV( 2, 'SORMQR', SIDE // TRANS, M, N, K, &
                    -1 ) )
         END IF
      ELSE
         IWS = NW
      END IF
!
      IF( NB.LT.NBMIN .OR. NB.GE.K ) THEN
!
!        Use unblocked code
!
         CALL SORM2R( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, &
                      IINFO )
      ELSE
!
!        Use blocked code
!
         IF( ( LEFT .AND. .NOT.NOTRAN ) .OR. &
             ( .NOT.LEFT .AND. NOTRAN ) ) THEN
            I1 = 1
            I2 = K
            I3 = NB
         ELSE
            I1 = ( ( K-1 ) / NB )*NB + 1
            I2 = 1
            I3 = -NB
         END IF
!
         IF( LEFT ) THEN
            NI = N
            JC = 1
         ELSE
            MI = M
            IC = 1
         END IF
!
         DO 10 I = I1, I2, I3
            IB = MIN( NB, K-I+1 )
!
!           Form the triangular factor of the block reflector
!           H = H(i) H(i+1) . . . H(i+ib-1)
!
            CALL SLARFT( 'Forward', 'Columnwise', NQ-I+1, IB, A( I, I ), &
                         LDA, TAU( I ), T, LDT )
            IF( LEFT ) THEN
!
!              H or H**T is applied to C(i:m,1:n)
!
               MI = M - I + 1
               IC = I
            ELSE
!
!              H or H**T is applied to C(1:m,i:n)
!
               NI = N - I + 1
               JC = I
            END IF
!
!           Apply H or H**T
!
            CALL SLARFB( SIDE, TRANS, 'Forward', 'Columnwise', MI, NI, &
                         IB, A( I, I ), LDA, T, LDT, C( IC, JC ), LDC, &
                         WORK, LDWORK )
   10    CONTINUE
      END IF
      WORK( 1 ) = LWKOPT
      RETURN
!
!     End of SORMQR
!
      END
      SUBROUTINE STRTRS( UPLO, TRANS, DIAG, N, NRHS, A, LDA, B, LDB, &
                         INFO )
!
      IMPLICIT NONE
!
!  -- LAPACK routine (version 3.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2006
!
!     .. Scalar Arguments ..
      CHARACTER          DIAG, TRANS, UPLO
      INTEGER            INFO, LDA, LDB, N, NRHS
!     ..
!     .. Array Arguments ..
      REAL               A( LDA, * ), B( LDB, * )
!     ..
!
!  Purpose
!  =======
!
!  STRTRS solves a triangular system of the form
!
!     A * X = B  or  A**T * X = B,
!
!  where A is a triangular matrix of order N, and B is an N-by-NRHS
!  matrix.  A check is made to verify that A is nonsingular.
!
!  Arguments
!  =========
!
!  UPLO    (input) CHARACTER*1
!          = 'U':  A is upper triangular;
!          = 'L':  A is lower triangular.
!
!  TRANS   (input) CHARACTER*1
!          Specifies the form of the system of equations:
!          = 'N':  A * X = B  (No transpose)
!          = 'T':  A**T * X = B  (Transpose)
!          = 'C':  A**H * X = B  (Conjugate transpose = Transpose)
!
!  DIAG    (input) CHARACTER*1
!          = 'N':  A is non-unit triangular;
!          = 'U':  A is unit triangular.
!
!  N       (input) INTEGER
!          The order of the matrix A.  N >= 0.
!
!  NRHS    (input) INTEGER
!          The number of right hand sides, i.e., the number of columns
!          of the matrix B.  NRHS >= 0.
!
!  A       (input) REAL array, dimension (LDA,N)
!          The triangular matrix A.  If UPLO = 'U', the leading N-by-N
!          upper triangular part of the array A contains the upper
!          triangular matrix, and the strictly lower triangular part of
!          A is not referenced.  If UPLO = 'L', the leading N-by-N lower
!          triangular part of the array A contains the lower triangular
!          matrix, and the strictly upper triangular part of A is not
!          referenced.  If DIAG = 'U', the diagonal elements of A are
!          also not referenced and are assumed to be 1.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,N).
!
!  B       (input/output) REAL array, dimension (LDB,NRHS)
!          On entry, the right hand side matrix B.
!          On exit, if INFO = 0, the solution matrix X.
!
!  LDB     (input) INTEGER
!          The leading dimension of the array B.  LDB >= max(1,N).
!
!  INFO    (output) INTEGER
!          = 0:  successful exit
!          < 0: if INFO = -i, the i-th argument had an illegal value
!          > 0: if INFO = i, the i-th diagonal element of A is zero,
!               indicating that the matrix is singular and the solutions
!               X have not been computed.
!
!  =====================================================================
!
!     .. Parameters ..
      REAL               ZERO, ONE
      PARAMETER          ( ZERO = 0.0E+0, ONE = 1.0E+0 )
!     ..
!     .. Local Scalars ..
      LOGICAL            NOUNIT
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!     ..
!     .. External Subroutines ..
      EXTERNAL           STRSM, XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input parameters.
!
      INFO = 0
      NOUNIT = LSAME( DIAG, 'N' )
      IF( .NOT.LSAME( UPLO, 'U' ) .AND. .NOT.LSAME( UPLO, 'L' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.LSAME( TRANS, 'N' ) .AND. .NOT. &
               LSAME( TRANS, 'T' ) .AND. .NOT.LSAME( TRANS, 'C' ) ) THEN
         INFO = -2
      ELSE IF( .NOT.NOUNIT .AND. .NOT.LSAME( DIAG, 'U' ) ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( NRHS.LT.0 ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, N ) ) THEN
         INFO = -7
      ELSE IF( LDB.LT.MAX( 1, N ) ) THEN
         INFO = -9
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'STRTRS', -INFO )
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( N.EQ.0 ) &
         RETURN
!
!     Check for singularity.
!
      IF( NOUNIT ) THEN
         DO 10 INFO = 1, N
            IF( A( INFO, INFO ).EQ.ZERO ) &
               RETURN
   10    CONTINUE
      END IF
      INFO = 0
!
!     Solve A * x = b  or  A**T * x = b.
!
      CALL STRSM( 'Left', UPLO, TRANS, DIAG, N, NRHS, ONE, A, LDA, B, &
                  LDB )
!
      RETURN
!
!     End of STRTRS
!
      END

!===================================================================================

!*> \brief <b> DGESVD computes the singular value decomposition (SVD) for GE matrices</b>
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DGESVD + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dgesvd.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dgesvd.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dgesvd.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGESVD( JOBU, JOBVT, M, N, A, LDA, S, U, LDU, VT, LDVT,
!*                          WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          JOBU, JOBVT
!*       INTEGER            INFO, LDA, LDU, LDVT, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), S( * ), U( LDU, * ), &
!*                          VT( LDVT, * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGESVD computes the singular value decomposition (SVD) of a real
!*> M-by-N matrix A, optionally computing the left and/or right singular
!*> vectors. The SVD is written
!*>
!*>      A = U * SIGMA * transpose(V)
!*>
!*> where SIGMA is an M-by-N matrix which is zero except for its
!*> min(m,n) diagonal elements, U is an M-by-M orthogonal matrix, and
!*> V is an N-by-N orthogonal matrix.  The diagonal elements of SIGMA
!*> are the singular values of A; they are real and non-negative, and
!*> are returned in descending order.  The first min(m,n) columns of
!*> U and V are the left and right singular vectors of A.
!*>
!*> Note that the routine returns V**T, not V.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] JOBU
!*> \verbatim
!*>          JOBU is CHARACTER*1
!*>          Specifies options for computing all or part of the matrix U:
!*>          = 'A':  all M columns of U are returned in array U:
!*>          = 'S':  the first min(m,n) columns of U (the left singular
!*>                  vectors) are returned in the array U;
!*>          = 'O':  the first min(m,n) columns of U (the left singular
!*>                  vectors) are overwritten on the array A;
!*>          = 'N':  no columns of U (no left singular vectors) are
!*>                  computed.
!*> \endverbatim
!*>
!*> \param[in] JOBVT
!*> \verbatim
!*>          JOBVT is CHARACTER*1
!*>          Specifies options for computing all or part of the matrix
!*>          V**T:
!*>          = 'A':  all N rows of V**T are returned in the array VT;
!*>          = 'S':  the first min(m,n) rows of V**T (the right singular
!*>                  vectors) are returned in the array VT;
!*>          = 'O':  the first min(m,n) rows of V**T (the right singular
!*>                  vectors) are overwritten on the array A;
!*>          = 'N':  no rows of V**T (no right singular vectors) are
!*>                  computed.
!*>
!*>          JOBVT and JOBU cannot both be 'O'.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the input matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the input matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the M-by-N matrix A.
!*>          On exit,
!*>          if JOBU = 'O',  A is overwritten with the first min(m,n)
!*>                          columns of U (the left singular vectors,
!*>                          stored columnwise);
!*>          if JOBVT = 'O', A is overwritten with the first min(m,n)
!*>                          rows of V**T (the right singular vectors,
!*>                          stored rowwise);
!*>          if JOBU .ne. 'O' and JOBVT .ne. 'O', the contents of A
!*>                          are destroyed.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] S
!*> \verbatim
!*>          S is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The singular values of A, sorted so that S(i) >= S(i+1).
!*> \endverbatim
!*>
!*> \param[out] U
!*> \verbatim
!*>          U is DOUBLE PRECISION array, dimension (LDU,UCOL)
!*>          (LDU,M) if JOBU = 'A' or (LDU,min(M,N)) if JOBU = 'S'.
!*>          If JOBU = 'A', U contains the M-by-M orthogonal matrix U;
!*>          if JOBU = 'S', U contains the first min(m,n) columns of U
!*>          (the left singular vectors, stored columnwise);
!*>          if JOBU = 'N' or 'O', U is not referenced.
!*> \endverbatim
!*>
!*> \param[in] LDU
!*> \verbatim
!*>          LDU is INTEGER
!*>          The leading dimension of the array U.  LDU >= 1; if
!*>          JOBU = 'S' or 'A', LDU >= M.
!*> \endverbatim
!*>
!*> \param[out] VT
!*> \verbatim
!*>          VT is DOUBLE PRECISION array, dimension (LDVT,N)
!*>          If JOBVT = 'A', VT contains the N-by-N orthogonal matrix
!*>          V**T;
!*>          if JOBVT = 'S', VT contains the first min(m,n) rows of
!*>          V**T (the right singular vectors, stored rowwise);
!*>          if JOBVT = 'N' or 'O', VT is not referenced.
!*> \endverbatim
!*>
!*> \param[in] LDVT
!*> \verbatim
!*>          LDVT is INTEGER
!*>          The leading dimension of the array VT.  LDVT >= 1; if
!*>          JOBVT = 'A', LDVT >= N; if JOBVT = 'S', LDVT >= min(M,N).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK;
!*>          if INFO > 0, WORK(2:MIN(M,N)) contains the unconverged
!*>          superdiagonal elements of an upper bidiagonal matrix B
!*>          whose diagonal is in S (not necessarily sorted). B
!*>          satisfies A = U * B * VT, so it has the same singular values
!*>          as A, and singular vectors related by U and VT.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK.
!*>          LWORK >= MAX(1,5*MIN(M,N)) for the paths (see comments inside code):
!*>             - PATH 1  (M much larger than N, JOBU='N')
!*>             - PATH 1t (N much larger than M, JOBVT='N')
!*>          LWORK >= MAX(1,3*MIN(M,N)+MAX(M,N),5*MIN(M,N)) for the other paths
!*>          For good performance, LWORK should generally be larger.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit.
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value.
!*>          > 0:  if DBDSQR did not converge, INFO specifies how many
!*>                superdiagonals of an intermediate bidiagonal form B
!*>                did not converge to zero. See the description of WORK
!*>                above for details.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date April 2012
!*
!*> \ingroup doubleGEsing
!*
!*  =====================================================================
      SUBROUTINE DGESVD( JOBU, JOBVT, M, N, A, LDA, S, U, LDU, &
                         VT, LDVT, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK driver routine (version 3.4.1) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     April 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          JOBU, JOBVT
      INTEGER            INFO, LDA, LDU, LDVT, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), S( * ), U( LDU, * ), &
                         VT( LDVT, * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO, ONE
      PARAMETER          ( ZERO = 0.0D0, ONE = 1.0D0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LQUERY, WNTUA, WNTUAS, WNTUN, WNTUO, WNTUS, &
                         WNTVA, WNTVAS, WNTVN, WNTVO, WNTVS
      INTEGER            BDSPAC, BLK, CHUNK, I, IE, IERR, IR, ISCL, &
                         ITAU, ITAUP, ITAUQ, IU, IWORK, LDWRKR, LDWRKU, &
                         MAXWRK, MINMN, MINWRK, MNTHR, NCU, NCVT, NRU, &
                         NRVT, WRKBL
      INTEGER            LWORK_DGEQRF, LWORK_DORGQR_N, LWORK_DORGQR_M, &
                         LWORK_DGEBRD, LWORK_DORGBR_P, LWORK_DORGBR_Q, &
                         LWORK_DGELQF, LWORK_DORGLQ_N, LWORK_DORGLQ_M
      DOUBLE PRECISION   ANRM, BIGNUM, EPS, SMLNUM
!*     ..
!*     .. Local Arrays ..
      DOUBLE PRECISION   DUM( 1 )
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DBDSQR, DGEBRD, DGELQF, DGEMM, DGEQRF, DLACPY, &
                         DLASCL, DLASET, DORGBR, DORGLQ, DORGQR, DORMBR, &
                         XERBLA
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      DOUBLE PRECISION   DLAMCH, DLANGE
      EXTERNAL           LSAME, ILAENV, DLAMCH, DLANGE
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      MINMN = MIN( M, N )
      WNTUA = LSAME( JOBU, 'A' )
      WNTUS = LSAME( JOBU, 'S' )
      WNTUAS = WNTUA .OR. WNTUS
      WNTUO = LSAME( JOBU, 'O' )
      WNTUN = LSAME( JOBU, 'N' )
      WNTVA = LSAME( JOBVT, 'A' )
      WNTVS = LSAME( JOBVT, 'S' )
      WNTVAS = WNTVA .OR. WNTVS
      WNTVO = LSAME( JOBVT, 'O' )
      WNTVN = LSAME( JOBVT, 'N' )
      LQUERY = ( LWORK.EQ.-1 )
!*
      IF( .NOT.( WNTUA .OR. WNTUS .OR. WNTUO .OR. WNTUN ) ) THEN
         INFO = -1
      ELSE IF( .NOT.( WNTVA .OR. WNTVS .OR. WNTVO .OR. WNTVN ) .OR. &
               ( WNTVO .AND. WNTUO ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -6
      ELSE IF( LDU.LT.1 .OR. ( WNTUAS .AND. LDU.LT.M ) ) THEN
         INFO = -9
      ELSE IF( LDVT.LT.1 .OR. ( WNTVA .AND. LDVT.LT.N ) .OR. &
               ( WNTVS .AND. LDVT.LT.MINMN ) ) THEN
         INFO = -11
      END IF
!*
!*     Compute workspace
!*      (Note: Comments in the code beginning "Workspace:" describe the
!*       minimal amount of workspace needed at that point in the code,
!*       as well as the preferred amount for good performance.
!*       NB refers to the optimal block size for the immediately
!*       following subroutine, as returned by ILAENV.)
!*
      IF( INFO.EQ.0 ) THEN
         MINWRK = 1
         MAXWRK = 1
         IF( M.GE.N .AND. MINMN.GT.0 ) THEN
!*
!*           Compute space needed for DBDSQR
!*
            MNTHR = ILAENV( 6, 'DGESVD', JOBU // JOBVT, M, N, 0, 0 )
            BDSPAC = 5*N
!*           Compute space needed for DGEQRF
            CALL DGEQRF( M, N, A, LDA, DUM(1), DUM(1), -1, IERR )
            LWORK_DGEQRF=DUM(1)
!*           Compute space needed for DORGQR
            CALL DORGQR( M, N, N, A, LDA, DUM(1), DUM(1), -1, IERR )
            LWORK_DORGQR_N=DUM(1)
            CALL DORGQR( M, M, N, A, LDA, DUM(1), DUM(1), -1, IERR )
            LWORK_DORGQR_M=DUM(1)
!*           Compute space needed for DGEBRD
            CALL DGEBRD( N, N, A, LDA, S, DUM(1), DUM(1), &
                         DUM(1), DUM(1), -1, IERR )
            LWORK_DGEBRD=DUM(1)
!*           Compute space needed for DORGBR P &
            CALL DORGBR( 'P', N, N, N, A, LDA, DUM(1), &
                         DUM(1), -1, IERR )
            LWORK_DORGBR_P=DUM(1)
!*           Compute space needed for DORGBR Q
            CALL DORGBR( 'Q', N, N, N, A, LDA, DUM(1), &
                         DUM(1), -1, IERR )
            LWORK_DORGBR_Q=DUM(1)
!*
            IF( M.GE.MNTHR ) THEN
               IF( WNTUN ) THEN
!*
!*                 Path 1 (M much larger than N, JOBU='N')
!*
                  MAXWRK = N + LWORK_DGEQRF
                  MAXWRK = MAX( MAXWRK, 3*N+LWORK_DGEBRD )
                  IF( WNTVO .OR. WNTVAS ) &
                     MAXWRK = MAX( MAXWRK, 3*N+LWORK_DORGBR_P )
                  MAXWRK = MAX( MAXWRK, BDSPAC )
                  MINWRK = MAX( 4*N, BDSPAC )
               ELSE IF( WNTUO .AND. WNTVN ) THEN
!*
!*                 Path 2 (M much larger than N, JOBU='O', JOBVT='N')
!*
                  WRKBL = N + LWORK_DGEQRF
                  WRKBL = MAX( WRKBL, N+LWORK_DORGQR_N )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = MAX( N*N+WRKBL, N*N+M*N+N )
                  MINWRK = MAX( 3*N+M, BDSPAC )
               ELSE IF( WNTUO .AND. WNTVAS ) THEN
!*
!*                 Path 3 (M much larger than N, JOBU='O', JOBVT='S' or
!*                 'A')
!*
                  WRKBL = N + LWORK_DGEQRF
                  WRKBL = MAX( WRKBL, N+LWORK_DORGQR_N )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = MAX( N*N+WRKBL, N*N+M*N+N )
                  MINWRK = MAX( 3*N+M, BDSPAC )
               ELSE IF( WNTUS .AND. WNTVN ) THEN
!*
!*                 Path 4 (M much larger than N, JOBU='S', JOBVT='N')
!*
                  WRKBL = N + LWORK_DGEQRF
                  WRKBL = MAX( WRKBL, N+LWORK_DORGQR_N )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = N*N + WRKBL
                  MINWRK = MAX( 3*N+M, BDSPAC )
               ELSE IF( WNTUS .AND. WNTVO ) THEN
!*
!*                 Path 5 (M much larger than N, JOBU='S', JOBVT='O')
!*
                  WRKBL = N + LWORK_DGEQRF
                  WRKBL = MAX( WRKBL, N+LWORK_DORGQR_N )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = 2*N*N + WRKBL
                  MINWRK = MAX( 3*N+M, BDSPAC )
               ELSE IF( WNTUS .AND. WNTVAS ) THEN
!*
!*                 Path 6 (M much larger than N, JOBU='S', JOBVT='S' or
!*                 'A')
!*
                  WRKBL = N + LWORK_DGEQRF
                  WRKBL = MAX( WRKBL, N+LWORK_DORGQR_N )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = N*N + WRKBL
                  MINWRK = MAX( 3*N+M, BDSPAC )
               ELSE IF( WNTUA .AND. WNTVN ) THEN
!*
!*                 Path 7 (M much larger than N, JOBU='A', JOBVT='N')
!*
                  WRKBL = N + LWORK_DGEQRF
                  WRKBL = MAX( WRKBL, N+LWORK_DORGQR_M )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = N*N + WRKBL
                  MINWRK = MAX( 3*N+M, BDSPAC )
               ELSE IF( WNTUA .AND. WNTVO ) THEN
!*
!*                 Path 8 (M much larger than N, JOBU='A', JOBVT='O')
!*
                  WRKBL = N + LWORK_DGEQRF
                  WRKBL = MAX( WRKBL, N+LWORK_DORGQR_M )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = 2*N*N + WRKBL
                  MINWRK = MAX( 3*N+M, BDSPAC )
               ELSE IF( WNTUA .AND. WNTVAS ) THEN
!*
!*                 Path 9 (M much larger than N, JOBU='A', JOBVT='S' or
!*                 'A')
!*
                  WRKBL = N + LWORK_DGEQRF
                  WRKBL = MAX( WRKBL, N+LWORK_DORGQR_M )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, 3*N+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = N*N + WRKBL
                  MINWRK = MAX( 3*N+M, BDSPAC )
               END IF
            ELSE
!*
!*              Path 10 (M at least N, but not much larger)
!*
               CALL DGEBRD( M, N, A, LDA, S, DUM(1), DUM(1), &
                         DUM(1), DUM(1), -1, IERR )
               LWORK_DGEBRD=DUM(1)
               MAXWRK = 3*N + LWORK_DGEBRD
               IF( WNTUS .OR. WNTUO ) THEN
                  CALL DORGBR( 'Q', M, N, N, A, LDA, DUM(1), &
                         DUM(1), -1, IERR )
                  LWORK_DORGBR_Q=DUM(1)
                  MAXWRK = MAX( MAXWRK, 3*N+LWORK_DORGBR_Q )
               END IF
               IF( WNTUA ) THEN
                  CALL DORGBR( 'Q', M, M, N, A, LDA, DUM(1), &
                         DUM(1), -1, IERR )
                  LWORK_DORGBR_Q=DUM(1)
                  MAXWRK = MAX( MAXWRK, 3*N+LWORK_DORGBR_Q )
               END IF
               IF( .NOT.WNTVN ) THEN
                 MAXWRK = MAX( MAXWRK, 3*N+LWORK_DORGBR_P )
               END IF
               MAXWRK = MAX( MAXWRK, BDSPAC )
               MINWRK = MAX( 3*N+M, BDSPAC )
            END IF
         ELSE IF( MINMN.GT.0 ) THEN
!*
!*           Compute space needed for DBDSQR
!*
            MNTHR = ILAENV( 6, 'DGESVD', JOBU // JOBVT, M, N, 0, 0 )
            BDSPAC = 5*M
!*           Compute space needed for DGELQF
            CALL DGELQF( M, N, A, LDA, DUM(1), DUM(1), -1, IERR )
            LWORK_DGELQF=DUM(1)
!*           Compute space needed for DORGLQ
            CALL DORGLQ( N, N, M, DUM(1), N, DUM(1), DUM(1), -1, IERR )
            LWORK_DORGLQ_N=DUM(1)
            CALL DORGLQ( M, N, M, A, LDA, DUM(1), DUM(1), -1, IERR )
            LWORK_DORGLQ_M=DUM(1)
!*           Compute space needed for DGEBRD
            CALL DGEBRD( M, M, A, LDA, S, DUM(1), DUM(1), &
                         DUM(1), DUM(1), -1, IERR )
            LWORK_DGEBRD=DUM(1)
!*            Compute space needed for DORGBR P
            CALL DORGBR( 'P', M, M, M, A, N, DUM(1), &
                         DUM(1), -1, IERR )
            LWORK_DORGBR_P=DUM(1)
!*           Compute space needed for DORGBR Q
            CALL DORGBR( 'Q', M, M, M, A, N, DUM(1), &
                         DUM(1), -1, IERR )
            LWORK_DORGBR_Q=DUM(1)
            IF( N.GE.MNTHR ) THEN
               IF( WNTVN ) THEN
!*
!*                 Path 1t(N much larger than M, JOBVT='N')
!*
                  MAXWRK = M + LWORK_DGELQF
                  MAXWRK = MAX( MAXWRK, 3*M+LWORK_DGEBRD )
                  IF( WNTUO .OR. WNTUAS ) &
                     MAXWRK = MAX( MAXWRK, 3*M+LWORK_DORGBR_Q )
                  MAXWRK = MAX( MAXWRK, BDSPAC )
                  MINWRK = MAX( 4*M, BDSPAC )
               ELSE IF( WNTVO .AND. WNTUN ) THEN
!*
!*                 Path 2t(N much larger than M, JOBU='N', JOBVT='O')
!*
                  WRKBL = M + LWORK_DGELQF
                  WRKBL = MAX( WRKBL, M+LWORK_DORGLQ_M )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = MAX( M*M+WRKBL, M*M+M*N+M )
                  MINWRK = MAX( 3*M+N, BDSPAC )
               ELSE IF( WNTVO .AND. WNTUAS ) THEN
!*
!*                 Path 3t(N much larger than M, JOBU='S' or 'A',
!*                 JOBVT='O')
!*
                  WRKBL = M + LWORK_DGELQF
                  WRKBL = MAX( WRKBL, M+LWORK_DORGLQ_M )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = MAX( M*M+WRKBL, M*M+M*N+M )
                  MINWRK = MAX( 3*M+N, BDSPAC )
               ELSE IF( WNTVS .AND. WNTUN ) THEN
!*
!*                 Path 4t(N much larger than M, JOBU='N', JOBVT='S')
!*
                  WRKBL = M + LWORK_DGELQF
                  WRKBL = MAX( WRKBL, M+LWORK_DORGLQ_M )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = M*M + WRKBL
                  MINWRK = MAX( 3*M+N, BDSPAC )
               ELSE IF( WNTVS .AND. WNTUO ) THEN
!*
!*                 Path 5t(N much larger than M, JOBU='O', JOBVT='S')
!*
                  WRKBL = M + LWORK_DGELQF
                  WRKBL = MAX( WRKBL, M+LWORK_DORGLQ_M )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = 2*M*M + WRKBL
                  MINWRK = MAX( 3*M+N, BDSPAC )
               ELSE IF( WNTVS .AND. WNTUAS ) THEN
!*
!*                 Path 6t(N much larger than M, JOBU='S' or 'A',
!*                 JOBVT='S')
!*
                  WRKBL = M + LWORK_DGELQF
                  WRKBL = MAX( WRKBL, M+LWORK_DORGLQ_M )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = M*M + WRKBL
                  MINWRK = MAX( 3*M+N, BDSPAC )
               ELSE IF( WNTVA .AND. WNTUN ) THEN
!*
!*                 Path 7t(N much larger than M, JOBU='N', JOBVT='A')
!*
                  WRKBL = M + LWORK_DGELQF
                  WRKBL = MAX( WRKBL, M+LWORK_DORGLQ_N )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = M*M + WRKBL
                  MINWRK = MAX( 3*M+N, BDSPAC )
               ELSE IF( WNTVA .AND. WNTUO ) THEN
!*
!*                 Path 8t(N much larger than M, JOBU='O', JOBVT='A')
!*
                  WRKBL = M + LWORK_DGELQF
                  WRKBL = MAX( WRKBL, M+LWORK_DORGLQ_N )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = 2*M*M + WRKBL
                  MINWRK = MAX( 3*M+N, BDSPAC )
               ELSE IF( WNTVA .AND. WNTUAS ) THEN
!*
!*                 Path 9t(N much larger than M, JOBU='S' or 'A',
!*                 JOBVT='A')
!*
                  WRKBL = M + LWORK_DGELQF
                  WRKBL = MAX( WRKBL, M+LWORK_DORGLQ_N )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DGEBRD )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_P )
                  WRKBL = MAX( WRKBL, 3*M+LWORK_DORGBR_Q )
                  WRKBL = MAX( WRKBL, BDSPAC )
                  MAXWRK = M*M + WRKBL
                  MINWRK = MAX( 3*M+N, BDSPAC )
               END IF
            ELSE
!*
!*              Path 10t(N greater than M, but not much larger)
!*
               CALL DGEBRD( M, N, A, LDA, S, DUM(1), DUM(1), &
                         DUM(1), DUM(1), -1, IERR )
               LWORK_DGEBRD=DUM(1)
               MAXWRK = 3*M + LWORK_DGEBRD
               IF( WNTVS .OR. WNTVO ) THEN
!*                Compute space needed for DORGBR P
                 CALL DORGBR( 'P', M, N, M, A, N, DUM(1), &
                         DUM(1), -1, IERR )
                 LWORK_DORGBR_P=DUM(1)
                 MAXWRK = MAX( MAXWRK, 3*M+LWORK_DORGBR_P )
               END IF
               IF( WNTVA ) THEN
                 CALL DORGBR( 'P', N, N, M, A, N, DUM(1), &
                         DUM(1), -1, IERR )
                 LWORK_DORGBR_P=DUM(1)
                 MAXWRK = MAX( MAXWRK, 3*M+LWORK_DORGBR_P )
               END IF
               IF( .NOT.WNTUN ) THEN
                  MAXWRK = MAX( MAXWRK, 3*M+LWORK_DORGBR_Q )
               END IF
               MAXWRK = MAX( MAXWRK, BDSPAC )
               MINWRK = MAX( 3*M+N, BDSPAC )
            END IF
         END IF
         MAXWRK = MAX( MAXWRK, MINWRK )
         WORK( 1 ) = MAXWRK
!*
         IF( LWORK.LT.MINWRK .AND. .NOT.LQUERY ) THEN
            INFO = -13
         END IF
      END IF
!*
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DGESVD', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( M.EQ.0 .OR. N.EQ.0 ) THEN
         RETURN
      END IF
!*
!*     Get machine constants
!*
      EPS = DLAMCH( 'P' )
      SMLNUM = SQRT( DLAMCH( 'S' ) ) / EPS
      BIGNUM = ONE / SMLNUM
!*
!*     Scale A if max element outside range [SMLNUM,BIGNUM]
!*
      ANRM = DLANGE( 'M', M, N, A, LDA, DUM )
      ISCL = 0
      IF( ANRM.GT.ZERO .AND. ANRM.LT.SMLNUM ) THEN
         ISCL = 1
         CALL DLASCL( 'G', 0, 0, ANRM, SMLNUM, M, N, A, LDA, IERR )
      ELSE IF( ANRM.GT.BIGNUM ) THEN
         ISCL = 1
         CALL DLASCL( 'G', 0, 0, ANRM, BIGNUM, M, N, A, LDA, IERR )
      END IF
!*
      IF( M.GE.N ) THEN
!*
!*        A has at least as many rows as columns. If A has sufficiently
!*        more rows than columns, first reduce using the QR
!*        decomposition (if sufficient workspace available)
!*
         IF( M.GE.MNTHR ) THEN
!*
            IF( WNTUN ) THEN
!*
!*              Path 1 (M much larger than N, JOBU='N')
!*              No left singular vectors to be computed
!*
               ITAU = 1
               IWORK = ITAU + N
!*
!*              Compute A=Q*R
!*              (Workspace: need 2*N, prefer N+N*NB)
!*
               CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), WORK( IWORK ), &
                            LWORK-IWORK+1, IERR )
!*
!*              Zero out below R
!*
               CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, A( 2, 1 ), LDA )
               IE = 1
               ITAUQ = IE + N
               ITAUP = ITAUQ + N
               IWORK = ITAUP + N
!*
!*              Bidiagonalize R in A
!*              (Workspace: need 4*N, prefer 3*N+2*N*NB)
!*
               CALL DGEBRD( N, N, A, LDA, S, WORK( IE ), WORK( ITAUQ ), &
                            WORK( ITAUP ), WORK( IWORK ), LWORK-IWORK+1, &
                            IERR )
               NCVT = 0
               IF( WNTVO .OR. WNTVAS ) THEN
!*
!*                 If right singular vectors desired, generate P'.
!*                 (Workspace: need 4*N-1, prefer 3*N+(N-1)*NB)
!*
                  CALL DORGBR( 'P', N, N, N, A, LDA, WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  NCVT = N
               END IF
               IWORK = IE + N
!*
!*              Perform bidiagonal QR iteration, computing right
!*              singular vectors of A in A if desired
!*              (Workspace: need BDSPAC)
!*
               CALL DBDSQR( 'U', N, NCVT, 0, 0, S, WORK( IE ), A, LDA, &
                            DUM, 1, DUM, 1, WORK( IWORK ), INFO )
!*
!*              If right singular vectors desired in VT, copy them there
!*
               IF( WNTVAS ) &
                  CALL DLACPY( 'F', N, N, A, LDA, VT, LDVT )
!*
            ELSE IF( WNTUO .AND. WNTVN ) THEN
!*
!*              Path 2 (M much larger than N, JOBU='O', JOBVT='N')
!*              N left singular vectors to be overwritten on A and
!*              no right singular vectors to be computed
!*
               IF( LWORK.GE.N*N+MAX( 4*N, BDSPAC ) ) THEN
!*
!*                 Sufficient workspace for a fast algorithm
!*
                  IR = 1
                  IF( LWORK.GE.MAX( WRKBL, LDA*N+N )+LDA*N ) THEN
!*
!*                    WORK(IU) is LDA by N, WORK(IR) is LDA by N
!*
                     LDWRKU = LDA
                     LDWRKR = LDA
                  ELSE IF( LWORK.GE.MAX( WRKBL, LDA*N+N )+N*N ) THEN
!*
!*                    WORK(IU) is LDA by N, WORK(IR) is N by N
!*
                     LDWRKU = LDA
                     LDWRKR = N
                  ELSE
!*
!*                    WORK(IU) is LDWRKU by N, WORK(IR) is N by N
!*
                     LDWRKU = ( LWORK-N*N-N ) / N
                     LDWRKR = N
                  END IF
                  ITAU = IR + LDWRKR*N
                  IWORK = ITAU + N
!*
!*                 Compute A=Q*R
!*                 (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                  CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Copy R to WORK(IR) and zero out below it
!*
                  CALL DLACPY( 'U', N, N, A, LDA, WORK( IR ), LDWRKR )
                  CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, WORK( IR+1 ), &
                               LDWRKR )
!*
!*                 Generate Q in A
!*                 (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                  CALL DORGQR( M, N, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IE = ITAU
                  ITAUQ = IE + N
                  ITAUP = ITAUQ + N
                  IWORK = ITAUP + N
!*
!*                 Bidiagonalize R in WORK(IR)
!*                 (Workspace: need N*N+4*N, prefer N*N+3*N+2*N*NB)
!*
                  CALL DGEBRD( N, N, WORK( IR ), LDWRKR, S, WORK( IE ), &
                               WORK( ITAUQ ), WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Generate left vectors bidiagonalizing R
!*                 (Workspace: need N*N+4*N, prefer N*N+3*N+N*NB)
!*
                  CALL DORGBR( 'Q', N, N, N, WORK( IR ), LDWRKR, &
                               WORK( ITAUQ ), WORK( IWORK ), &
                               LWORK-IWORK+1, IERR )
                  IWORK = IE + N
!*
!*                 Perform bidiagonal QR iteration, computing left
!*                 singular vectors of R in WORK(IR)
!*                 (Workspace: need N*N+BDSPAC)
!*
                  CALL DBDSQR( 'U', N, 0, N, 0, S, WORK( IE ), DUM, 1, &
                               WORK( IR ), LDWRKR, DUM, 1, &
                               WORK( IWORK ), INFO )
                  IU = IE + N
!*
!*                 Multiply Q in A by left singular vectors of R in
!*                 WORK(IR), storing result in WORK(IU) and copying to A
!*                 (Workspace: need N*N+2*N, prefer N*N+M*N+N)
!*
                  DO 10 I = 1, M, LDWRKU
                     CHUNK = MIN( M-I+1, LDWRKU )
                     CALL DGEMM( 'N', 'N', CHUNK, N, N, ONE, A( I, 1 ), &
                                 LDA, WORK( IR ), LDWRKR, ZERO, &
                                 WORK( IU ), LDWRKU )
                     CALL DLACPY( 'F', CHUNK, N, WORK( IU ), LDWRKU, &
                                  A( I, 1 ), LDA )
   10             CONTINUE
!*
               ELSE
!*
!*                 Insufficient workspace for a fast algorithm
!*
                  IE = 1
                  ITAUQ = IE + N
                  ITAUP = ITAUQ + N
                  IWORK = ITAUP + N
!*
!*                 Bidiagonalize A
!*                 (Workspace: need 3*N+M, prefer 3*N+(M+N)*NB)
!*
                  CALL DGEBRD( M, N, A, LDA, S, WORK( IE ), &
                               WORK( ITAUQ ), WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Generate left vectors bidiagonalizing A
!*                 (Workspace: need 4*N, prefer 3*N+N*NB)
!*
                  CALL DORGBR( 'Q', M, N, N, A, LDA, WORK( ITAUQ ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IWORK = IE + N
!*
!*                 Perform bidiagonal QR iteration, computing left
!*                 singular vectors of A in A
!*                 (Workspace: need BDSPAC)
!*
                  CALL DBDSQR( 'U', N, 0, M, 0, S, WORK( IE ), DUM, 1, &
                               A, LDA, DUM, 1, WORK( IWORK ), INFO )
!*
               END IF
!*
            ELSE IF( WNTUO .AND. WNTVAS ) THEN
!*
!*              Path 3 (M much larger than N, JOBU='O', JOBVT='S' or 'A')
!*              N left singular vectors to be overwritten on A and
!*              N right singular vectors to be computed in VT
!*
               IF( LWORK.GE.N*N+MAX( 4*N, BDSPAC ) ) THEN
!*
!*                 Sufficient workspace for a fast algorithm
!*
                  IR = 1
                  IF( LWORK.GE.MAX( WRKBL, LDA*N+N )+LDA*N ) THEN
!*
!*                    WORK(IU) is LDA by N and WORK(IR) is LDA by N
!*
                     LDWRKU = LDA
                     LDWRKR = LDA
                  ELSE IF( LWORK.GE.MAX( WRKBL, LDA*N+N )+N*N ) THEN
!*
!*                    WORK(IU) is LDA by N and WORK(IR) is N by N
!*
                     LDWRKU = LDA
                     LDWRKR = N
                  ELSE
!*
!*                    WORK(IU) is LDWRKU by N and WORK(IR) is N by N
!*
                     LDWRKU = ( LWORK-N*N-N ) / N
                     LDWRKR = N
                  END IF
                  ITAU = IR + LDWRKR*N
                  IWORK = ITAU + N
!*
!*                 Compute A=Q*R
!*                 (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                  CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Copy R to VT, zeroing out below it
!*
                  CALL DLACPY( 'U', N, N, A, LDA, VT, LDVT )
                  IF( N.GT.1 ) &
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                  VT( 2, 1 ), LDVT )
!*
!*                 Generate Q in A
!*                 (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                  CALL DORGQR( M, N, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IE = ITAU
                  ITAUQ = IE + N
                  ITAUP = ITAUQ + N
                  IWORK = ITAUP + N
!*
!*                 Bidiagonalize R in VT, copying result to WORK(IR)
!*                 (Workspace: need N*N+4*N, prefer N*N+3*N+2*N*NB)
!*
                  CALL DGEBRD( N, N, VT, LDVT, S, WORK( IE ), &
                               WORK( ITAUQ ), WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  CALL DLACPY( 'L', N, N, VT, LDVT, WORK( IR ), LDWRKR )
!*
!*                 Generate left vectors bidiagonalizing R in WORK(IR)
!*                 (Workspace: need N*N+4*N, prefer N*N+3*N+N*NB)
!*
                  CALL DORGBR( 'Q', N, N, N, WORK( IR ), LDWRKR, &
                               WORK( ITAUQ ), WORK( IWORK ), &
                               LWORK-IWORK+1, IERR )
!*
!*                 Generate right vectors bidiagonalizing R in VT
!*                 (Workspace: need N*N+4*N-1, prefer N*N+3*N+(N-1)*NB)
!*
                  CALL DORGBR( 'P', N, N, N, VT, LDVT, WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IWORK = IE + N
!*
!*                 Perform bidiagonal QR iteration, computing left
!*                 singular vectors of R in WORK(IR) and computing right
!*                 singular vectors of R in VT
!*                 (Workspace: need N*N+BDSPAC)
!*
                  CALL DBDSQR( 'U', N, N, N, 0, S, WORK( IE ), VT, LDVT, &
                               WORK( IR ), LDWRKR, DUM, 1, &
                               WORK( IWORK ), INFO )
                  IU = IE + N
!*
!*                 Multiply Q in A by left singular vectors of R in
!*                 WORK(IR), storing result in WORK(IU) and copying to A
!*                 (Workspace: need N*N+2*N, prefer N*N+M*N+N)
!*
                  DO 20 I = 1, M, LDWRKU
                     CHUNK = MIN( M-I+1, LDWRKU )
                     CALL DGEMM( 'N', 'N', CHUNK, N, N, ONE, A( I, 1 ), &
                                 LDA, WORK( IR ), LDWRKR, ZERO, &
                                 WORK( IU ), LDWRKU )
                     CALL DLACPY( 'F', CHUNK, N, WORK( IU ), LDWRKU, &
                                  A( I, 1 ), LDA )
   20             CONTINUE
!*
               ELSE
!*
!*                 Insufficient workspace for a fast algorithm
!*
                  ITAU = 1
                  IWORK = ITAU + N
!*
!*                 Compute A=Q*R
!*                 (Workspace: need 2*N, prefer N+N*NB)
!*
                  CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Copy R to VT, zeroing out below it
!*
                  CALL DLACPY( 'U', N, N, A, LDA, VT, LDVT )
                  IF( N.GT.1 ) &
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                  VT( 2, 1 ), LDVT )
!*
!*                 Generate Q in A
!*                 (Workspace: need 2*N, prefer N+N*NB)
!*
                  CALL DORGQR( M, N, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IE = ITAU
                  ITAUQ = IE + N
                  ITAUP = ITAUQ + N
                  IWORK = ITAUP + N
!*
!*                 Bidiagonalize R in VT
!*                 (Workspace: need 4*N, prefer 3*N+2*N*NB)
!*
                  CALL DGEBRD( N, N, VT, LDVT, S, WORK( IE ), &
                               WORK( ITAUQ ), WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Multiply Q in A by left vectors bidiagonalizing R
!*                 (Workspace: need 3*N+M, prefer 3*N+M*NB)
!*
                  CALL DORMBR( 'Q', 'R', 'N', M, N, N, VT, LDVT, &
                               WORK( ITAUQ ), A, LDA, WORK( IWORK ), &
                               LWORK-IWORK+1, IERR )
!*
!*                 Generate right vectors bidiagonalizing R in VT
!*                 (Workspace: need 4*N-1, prefer 3*N+(N-1)*NB)
!*
                  CALL DORGBR( 'P', N, N, N, VT, LDVT, WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IWORK = IE + N
!*
!*                 Perform bidiagonal QR iteration, computing left
!*                 singular vectors of A in A and computing right
!*                 singular vectors of A in VT
!*                 (Workspace: need BDSPAC)
!*
                  CALL DBDSQR( 'U', N, N, M, 0, S, WORK( IE ), VT, LDVT, &
                               A, LDA, DUM, 1, WORK( IWORK ), INFO )
!*
               END IF
!*
            ELSE IF( WNTUS ) THEN
!*
               IF( WNTVN ) THEN
!*
!*                 Path 4 (M much larger than N, JOBU='S', JOBVT='N')
!*                 N left singular vectors to be computed in U and
!*                 no right singular vectors to be computed
!*
                  IF( LWORK.GE.N*N+MAX( 4*N, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IR = 1
                     IF( LWORK.GE.WRKBL+LDA*N ) THEN
!*
!*                       WORK(IR) is LDA by N
!*
                        LDWRKR = LDA
                     ELSE
!*
!*                       WORK(IR) is N by N
!*
                        LDWRKR = N
                     END IF
                     ITAU = IR + LDWRKR*N
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R
!*                    (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy R to WORK(IR), zeroing out below it
!*
                     CALL DLACPY( 'U', N, N, A, LDA, WORK( IR ), &
                                  LDWRKR )
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                  WORK( IR+1 ), LDWRKR )
!*
!*                    Generate Q in A
!*                    (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                     CALL DORGQR( M, N, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Bidiagonalize R in WORK(IR)
!*                    (Workspace: need N*N+4*N, prefer N*N+3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, WORK( IR ), LDWRKR, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate left vectors bidiagonalizing R in WORK(IR)
!*                    (Workspace: need N*N+4*N, prefer N*N+3*N+N*NB)
!*
                     CALL DORGBR( 'Q', N, N, N, WORK( IR ), LDWRKR, &
                                  WORK( ITAUQ ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of R in WORK(IR)
!*                    (Workspace: need N*N+BDSPAC)
!*
                     CALL DBDSQR( 'U', N, 0, N, 0, S, WORK( IE ), DUM, &
                                  1, WORK( IR ), LDWRKR, DUM, 1, &
                                  WORK( IWORK ), INFO )
!*
!*                    Multiply Q in A by left singular vectors of R in
!*                    WORK(IR), storing result in U
!*                    (Workspace: need N*N)
!*
                     CALL DGEMM( 'N', 'N', M, N, N, ONE, A, LDA, &
                                 WORK( IR ), LDWRKR, ZERO, U, LDU )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Generate Q in U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DORGQR( M, N, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Zero out below R in A
!*
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, A( 2, 1 ), &
                                  LDA )
!*
!*                    Bidiagonalize R in A
!*                    (Workspace: need 4*N, prefer 3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, A, LDA, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply Q in U by left vectors bidiagonalizing R
!*                    (Workspace: need 3*N+M, prefer 3*N+M*NB)
!*
                     CALL DORMBR( 'Q', 'R', 'N', M, N, N, A, LDA, &
                                  WORK( ITAUQ ), U, LDU, WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in U
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', N, 0, M, 0, S, WORK( IE ), DUM, &
                                  1, U, LDU, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               ELSE IF( WNTVO ) THEN
!*
!*                 Path 5 (M much larger than N, JOBU='S', JOBVT='O')
!*                 N left singular vectors to be computed in U and
!*                 N right singular vectors to be overwritten on A
!*
                  IF( LWORK.GE.2*N*N+MAX( 4*N, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IU = 1
                     IF( LWORK.GE.WRKBL+2*LDA*N ) THEN
!*
!*                       WORK(IU) is LDA by N and WORK(IR) is LDA by N
!*
                        LDWRKU = LDA
                        IR = IU + LDWRKU*N
                        LDWRKR = LDA
                     ELSE IF( LWORK.GE.WRKBL+( LDA+N )*N ) THEN
!*
!*                       WORK(IU) is LDA by N and WORK(IR) is N by N
!*
                        LDWRKU = LDA
                        IR = IU + LDWRKU*N
                        LDWRKR = N
                     ELSE
!*
!*                       WORK(IU) is N by N and WORK(IR) is N by N
!*
                        LDWRKU = N
                        IR = IU + LDWRKU*N
                        LDWRKR = N
                     END IF
                     ITAU = IR + LDWRKR*N
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R
!*                    (Workspace: need 2*N*N+2*N, prefer 2*N*N+N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy R to WORK(IU), zeroing out below it
!*
                     CALL DLACPY( 'U', N, N, A, LDA, WORK( IU ), &
                                  LDWRKU )
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                  WORK( IU+1 ), LDWRKU )
!*
!*                    Generate Q in A
!*                    (Workspace: need 2*N*N+2*N, prefer 2*N*N+N+N*NB)
!*
                     CALL DORGQR( M, N, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Bidiagonalize R in WORK(IU), copying result to
!*                    WORK(IR)
!*                    (Workspace: need 2*N*N+4*N,
!*                                prefer 2*N*N+3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, WORK( IU ), LDWRKU, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', N, N, WORK( IU ), LDWRKU, &
                                  WORK( IR ), LDWRKR )
!*
!*                    Generate left bidiagonalizing vectors in WORK(IU)
!*                    (Workspace: need 2*N*N+4*N, prefer 2*N*N+3*N+N*NB)
!*
                     CALL DORGBR( 'Q', N, N, N, WORK( IU ), LDWRKU, &
                                  WORK( ITAUQ ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right bidiagonalizing vectors in WORK(IR)
!*                    (Workspace: need 2*N*N+4*N-1,
!*                                prefer 2*N*N+3*N+(N-1)*NB)
!*
                     CALL DORGBR( 'P', N, N, N, WORK( IR ), LDWRKR, &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of R in WORK(IU) and computing
!*                    right singular vectors of R in WORK(IR)
!*                    (Workspace: need 2*N*N+BDSPAC)
!*
                     CALL DBDSQR( 'U', N, N, N, 0, S, WORK( IE ), &
                                  WORK( IR ), LDWRKR, WORK( IU ), &
                                  LDWRKU, DUM, 1, WORK( IWORK ), INFO )
!*
!*                    Multiply Q in A by left singular vectors of R in
!*                    WORK(IU), storing result in U
!*                    (Workspace: need N*N)
!*
                     CALL DGEMM( 'N', 'N', M, N, N, ONE, A, LDA, &
                                 WORK( IU ), LDWRKU, ZERO, U, LDU )
!*
!*                    Copy right singular vectors of R to A
!*                    (Workspace: need N*N)
!*
                     CALL DLACPY( 'F', N, N, WORK( IR ), LDWRKR, A, &
                                  LDA )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Generate Q in U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DORGQR( M, N, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Zero out below R in A
!*
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, A( 2, 1 ), &
                                  LDA )
!*
!*                    Bidiagonalize R in A
!*                    (Workspace: need 4*N, prefer 3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, A, LDA, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply Q in U by left vectors bidiagonalizing R
!*                    (Workspace: need 3*N+M, prefer 3*N+M*NB)
!*
                     CALL DORMBR( 'Q', 'R', 'N', M, N, N, A, LDA, &
                                  WORK( ITAUQ ), U, LDU, WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right vectors bidiagonalizing R in A
!*                    (Workspace: need 4*N-1, prefer 3*N+(N-1)*NB)
!*
                     CALL DORGBR( 'P', N, N, N, A, LDA, WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in U and computing right
!*                    singular vectors of A in A
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', N, N, M, 0, S, WORK( IE ), A, &
                                  LDA, U, LDU, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               ELSE IF( WNTVAS ) THEN
!*
!*                 Path 6 (M much larger than N, JOBU='S', JOBVT='S'
!*                         or 'A')
!*                 N left singular vectors to be computed in U and
!*                 N right singular vectors to be computed in VT
!*
                  IF( LWORK.GE.N*N+MAX( 4*N, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IU = 1
                     IF( LWORK.GE.WRKBL+LDA*N ) THEN
!*
!*                       WORK(IU) is LDA by N
!*
                        LDWRKU = LDA
                     ELSE
!*
!*                       WORK(IU) is N by N
!*
                        LDWRKU = N
                     END IF
                     ITAU = IU + LDWRKU*N
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R
!*                    (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy R to WORK(IU), zeroing out below it
!*
                     CALL DLACPY( 'U', N, N, A, LDA, WORK( IU ), &
                                  LDWRKU )
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                  WORK( IU+1 ), LDWRKU )
!*
!*                    Generate Q in A
!*                    (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                     CALL DORGQR( M, N, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Bidiagonalize R in WORK(IU), copying result to VT
!*                    (Workspace: need N*N+4*N, prefer N*N+3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, WORK( IU ), LDWRKU, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', N, N, WORK( IU ), LDWRKU, VT, &
                                  LDVT )
!*
!*                    Generate left bidiagonalizing vectors in WORK(IU)
!*                    (Workspace: need N*N+4*N, prefer N*N+3*N+N*NB)
!*
                     CALL DORGBR( 'Q', N, N, N, WORK( IU ), LDWRKU, &
                                  WORK( ITAUQ ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right bidiagonalizing vectors in VT
!*                    (Workspace: need N*N+4*N-1,
!*                                prefer N*N+3*N+(N-1)*NB)
!*
                     CALL DORGBR( 'P', N, N, N, VT, LDVT, WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of R in WORK(IU) and computing
!*                    right singular vectors of R in VT
!*                    (Workspace: need N*N+BDSPAC)
!*
                     CALL DBDSQR( 'U', N, N, N, 0, S, WORK( IE ), VT, &
                                  LDVT, WORK( IU ), LDWRKU, DUM, 1, &
                                  WORK( IWORK ), INFO )
!*
!*                    Multiply Q in A by left singular vectors of R in
!*                    WORK(IU), storing result in U
!*                    (Workspace: need N*N)
!*
                     CALL DGEMM( 'N', 'N', M, N, N, ONE, A, LDA, &
                                 WORK( IU ), LDWRKU, ZERO, U, LDU )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Generate Q in U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DORGQR( M, N, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy R to VT, zeroing out below it
!*
                     CALL DLACPY( 'U', N, N, A, LDA, VT, LDVT )
                     IF( N.GT.1 ) &
                        CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                     VT( 2, 1 ), LDVT )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Bidiagonalize R in VT
!*                    (Workspace: need 4*N, prefer 3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, VT, LDVT, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply Q in U by left bidiagonalizing vectors
!*                    in VT
!*                    (Workspace: need 3*N+M, prefer 3*N+M*NB)
!*
                     CALL DORMBR( 'Q', 'R', 'N', M, N, N, VT, LDVT, &
                                  WORK( ITAUQ ), U, LDU, WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right bidiagonalizing vectors in VT
!*                    (Workspace: need 4*N-1, prefer 3*N+(N-1)*NB)
!*
                     CALL DORGBR( 'P', N, N, N, VT, LDVT, WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in U and computing right
!*                    singular vectors of A in VT
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', N, N, M, 0, S, WORK( IE ), VT, &
                                  LDVT, U, LDU, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               END IF
!*
            ELSE IF( WNTUA ) THEN
!*
               IF( WNTVN ) THEN
!*
!*                 Path 7 (M much larger than N, JOBU='A', JOBVT='N')
!*                 M left singular vectors to be computed in U and
!*                 no right singular vectors to be computed
!*
                  IF( LWORK.GE.N*N+MAX( N+M, 4*N, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IR = 1
                     IF( LWORK.GE.WRKBL+LDA*N ) THEN
!*
!*                       WORK(IR) is LDA by N
!*
                        LDWRKR = LDA
                     ELSE
!*
!*                       WORK(IR) is N by N
!*
                        LDWRKR = N
                     END IF
                     ITAU = IR + LDWRKR*N
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Copy R to WORK(IR), zeroing out below it
!*
                     CALL DLACPY( 'U', N, N, A, LDA, WORK( IR ), &
                                  LDWRKR )
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                  WORK( IR+1 ), LDWRKR )
!*
!*                    Generate Q in U
!*                    (Workspace: need N*N+N+M, prefer N*N+N+M*NB)
!*
                     CALL DORGQR( M, M, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Bidiagonalize R in WORK(IR)
!*                    (Workspace: need N*N+4*N, prefer N*N+3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, WORK( IR ), LDWRKR, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate left bidiagonalizing vectors in WORK(IR)
!*                    (Workspace: need N*N+4*N, prefer N*N+3*N+N*NB)
!*
                     CALL DORGBR( 'Q', N, N, N, WORK( IR ), LDWRKR, &
                                  WORK( ITAUQ ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of R in WORK(IR)
!*                    (Workspace: need N*N+BDSPAC)
!*
                     CALL DBDSQR( 'U', N, 0, N, 0, S, WORK( IE ), DUM, &
                                  1, WORK( IR ), LDWRKR, DUM, 1, &
                                  WORK( IWORK ), INFO )
!*
!*                    Multiply Q in U by left singular vectors of R in
!*                    WORK(IR), storing result in A
!*                    (Workspace: need N*N)
!*
                     CALL DGEMM( 'N', 'N', M, N, N, ONE, U, LDU, &
                                 WORK( IR ), LDWRKR, ZERO, A, LDA )
!*
!*                    Copy left singular vectors of A from A to U
!*
                     CALL DLACPY( 'F', M, N, A, LDA, U, LDU )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Generate Q in U
!*                    (Workspace: need N+M, prefer N+M*NB)
!*
                     CALL DORGQR( M, M, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Zero out below R in A
!*
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, A( 2, 1 ), &
                                  LDA )
!*
!*                    Bidiagonalize R in A
!*                    (Workspace: need 4*N, prefer 3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, A, LDA, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply Q in U by left bidiagonalizing vectors
!*                    in A
!*                    (Workspace: need 3*N+M, prefer 3*N+M*NB)
!*
                     CALL DORMBR( 'Q', 'R', 'N', M, N, N, A, LDA, &
                                  WORK( ITAUQ ), U, LDU, WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in U
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', N, 0, M, 0, S, WORK( IE ), DUM, &
                                  1, U, LDU, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               ELSE IF( WNTVO ) THEN
!*
!*                 Path 8 (M much larger than N, JOBU='A', JOBVT='O')
!*                 M left singular vectors to be computed in U and
!*                 N right singular vectors to be overwritten on A
!*
                  IF( LWORK.GE.2*N*N+MAX( N+M, 4*N, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IU = 1
                     IF( LWORK.GE.WRKBL+2*LDA*N ) THEN
!*
!*                       WORK(IU) is LDA by N and WORK(IR) is LDA by N
!*
                        LDWRKU = LDA
                        IR = IU + LDWRKU*N
                        LDWRKR = LDA
                     ELSE IF( LWORK.GE.WRKBL+( LDA+N )*N ) THEN
!*
!*                       WORK(IU) is LDA by N and WORK(IR) is N by N
!*
                        LDWRKU = LDA
                        IR = IU + LDWRKU*N
                        LDWRKR = N
                     ELSE
!*
!*                       WORK(IU) is N by N and WORK(IR) is N by N
!*
                        LDWRKU = N
                        IR = IU + LDWRKU*N
                        LDWRKR = N
                     END IF
                     ITAU = IR + LDWRKR*N
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need 2*N*N+2*N, prefer 2*N*N+N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Generate Q in U
!*                    (Workspace: need 2*N*N+N+M, prefer 2*N*N+N+M*NB)
!*
                     CALL DORGQR( M, M, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy R to WORK(IU), zeroing out below it
!*
                     CALL DLACPY( 'U', N, N, A, LDA, WORK( IU ), &
                                  LDWRKU )
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                  WORK( IU+1 ), LDWRKU )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Bidiagonalize R in WORK(IU), copying result to
!*                    WORK(IR)
!*                    (Workspace: need 2*N*N+4*N,
!*                                prefer 2*N*N+3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, WORK( IU ), LDWRKU, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', N, N, WORK( IU ), LDWRKU, &
                                  WORK( IR ), LDWRKR )
!*
!*                    Generate left bidiagonalizing vectors in WORK(IU)
!*                    (Workspace: need 2*N*N+4*N, prefer 2*N*N+3*N+N*NB)
!*
                     CALL DORGBR( 'Q', N, N, N, WORK( IU ), LDWRKU, &
                                  WORK( ITAUQ ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right bidiagonalizing vectors in WORK(IR)
!*                    (Workspace: need 2*N*N+4*N-1,
!*                                prefer 2*N*N+3*N+(N-1)*NB)
!*
                     CALL DORGBR( 'P', N, N, N, WORK( IR ), LDWRKR, &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of R in WORK(IU) and computing
!*                    right singular vectors of R in WORK(IR)
!*                    (Workspace: need 2*N*N+BDSPAC)
!*
                     CALL DBDSQR( 'U', N, N, N, 0, S, WORK( IE ), &
                                  WORK( IR ), LDWRKR, WORK( IU ), &
                                  LDWRKU, DUM, 1, WORK( IWORK ), INFO )
!*
!*                    Multiply Q in U by left singular vectors of R in
!*                    WORK(IU), storing result in A
!*                    (Workspace: need N*N)
!*
                     CALL DGEMM( 'N', 'N', M, N, N, ONE, U, LDU, &
                                 WORK( IU ), LDWRKU, ZERO, A, LDA )
!*
!*                    Copy left singular vectors of A from A to U
!*
                     CALL DLACPY( 'F', M, N, A, LDA, U, LDU )
!*
!*                    Copy right singular vectors of R from WORK(IR) to A
!*
                     CALL DLACPY( 'F', N, N, WORK( IR ), LDWRKR, A, &
                                  LDA )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Generate Q in U
!*                    (Workspace: need N+M, prefer N+M*NB)
!*
                     CALL DORGQR( M, M, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Zero out below R in A
!*
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, A( 2, 1 ), &
                                  LDA )
!*
!*                    Bidiagonalize R in A
!*                    (Workspace: need 4*N, prefer 3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, A, LDA, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply Q in U by left bidiagonalizing vectors
!*                    in A
!*                    (Workspace: need 3*N+M, prefer 3*N+M*NB)
!*
                     CALL DORMBR( 'Q', 'R', 'N', M, N, N, A, LDA, &
                                  WORK( ITAUQ ), U, LDU, WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right bidiagonalizing vectors in A
!*                    (Workspace: need 4*N-1, prefer 3*N+(N-1)*NB)
!*
                     CALL DORGBR( 'P', N, N, N, A, LDA, WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in U and computing right
!*                    singular vectors of A in A
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', N, N, M, 0, S, WORK( IE ), A, &
                                  LDA, U, LDU, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               ELSE IF( WNTVAS ) THEN
!*
!*                 Path 9 (M much larger than N, JOBU='A', JOBVT='S'
!*                         or 'A')
!*                 M left singular vectors to be computed in U and
!*                 N right singular vectors to be computed in VT
!*
                  IF( LWORK.GE.N*N+MAX( N+M, 4*N, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IU = 1
                     IF( LWORK.GE.WRKBL+LDA*N ) THEN
!*
!*                       WORK(IU) is LDA by N
!*
                        LDWRKU = LDA
                     ELSE
!*
!*                       WORK(IU) is N by N
!*
                        LDWRKU = N
                     END IF
                     ITAU = IU + LDWRKU*N
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need N*N+2*N, prefer N*N+N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Generate Q in U
!*                    (Workspace: need N*N+N+M, prefer N*N+N+M*NB)
!*
                     CALL DORGQR( M, M, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy R to WORK(IU), zeroing out below it
!*
                     CALL DLACPY( 'U', N, N, A, LDA, WORK( IU ), &
                                  LDWRKU )
                     CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                  WORK( IU+1 ), LDWRKU )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Bidiagonalize R in WORK(IU), copying result to VT
!*                    (Workspace: need N*N+4*N, prefer N*N+3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, WORK( IU ), LDWRKU, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', N, N, WORK( IU ), LDWRKU, VT, &
                                  LDVT )
!*
!*                    Generate left bidiagonalizing vectors in WORK(IU)
!*                    (Workspace: need N*N+4*N, prefer N*N+3*N+N*NB)
!*
                     CALL DORGBR( 'Q', N, N, N, WORK( IU ), LDWRKU, &
                                  WORK( ITAUQ ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right bidiagonalizing vectors in VT
!*                    (Workspace: need N*N+4*N-1,
!*                                prefer N*N+3*N+(N-1)*NB)
!*
                     CALL DORGBR( 'P', N, N, N, VT, LDVT, WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of R in WORK(IU) and computing
!*                    right singular vectors of R in VT
!*                    (Workspace: need N*N+BDSPAC)
!*
                     CALL DBDSQR( 'U', N, N, N, 0, S, WORK( IE ), VT, &
                                  LDVT, WORK( IU ), LDWRKU, DUM, 1, &
                                  WORK( IWORK ), INFO )
!*
!*                    Multiply Q in U by left singular vectors of R in
!*                    WORK(IU), storing result in A
!*                    (Workspace: need N*N)
!*
                     CALL DGEMM( 'N', 'N', M, N, N, ONE, U, LDU, &
                                 WORK( IU ), LDWRKU, ZERO, A, LDA )
!*
!*                    Copy left singular vectors of A from A to U
!*
                     CALL DLACPY( 'F', M, N, A, LDA, U, LDU )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + N
!*
!*                    Compute A=Q*R, copying result to U
!*                    (Workspace: need 2*N, prefer N+N*NB)
!*
                     CALL DGEQRF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
!*
!*                    Generate Q in U
!*                    (Workspace: need N+M, prefer N+M*NB)
!*
                     CALL DORGQR( M, M, N, U, LDU, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy R from A to VT, zeroing out below it
!*
                     CALL DLACPY( 'U', N, N, A, LDA, VT, LDVT )
                     IF( N.GT.1 ) &
                        CALL DLASET( 'L', N-1, N-1, ZERO, ZERO, &
                                     VT( 2, 1 ), LDVT )
                     IE = ITAU
                     ITAUQ = IE + N
                     ITAUP = ITAUQ + N
                     IWORK = ITAUP + N
!*
!*                    Bidiagonalize R in VT
!*                    (Workspace: need 4*N, prefer 3*N+2*N*NB)
!*
                     CALL DGEBRD( N, N, VT, LDVT, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply Q in U by left bidiagonalizing vectors
!*                    in VT
!*                    (Workspace: need 3*N+M, prefer 3*N+M*NB)
!*
                     CALL DORMBR( 'Q', 'R', 'N', M, N, N, VT, LDVT, &
                                  WORK( ITAUQ ), U, LDU, WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right bidiagonalizing vectors in VT
!*                    (Workspace: need 4*N-1, prefer 3*N+(N-1)*NB)
!*
                     CALL DORGBR( 'P', N, N, N, VT, LDVT, WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + N
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in U and computing right
!*                    singular vectors of A in VT
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', N, N, M, 0, S, WORK( IE ), VT, &
                                  LDVT, U, LDU, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               END IF
!*
            END IF
!*
         ELSE
!*
!*           M .LT. MNTHR
!*
!*           Path 10 (M at least N, but not much larger)
!*           Reduce to bidiagonal form without QR decomposition
!*
            IE = 1
            ITAUQ = IE + N
            ITAUP = ITAUQ + N
            IWORK = ITAUP + N
!*
!*           Bidiagonalize A
!*           (Workspace: need 3*N+M, prefer 3*N+(M+N)*NB)
!*
            CALL DGEBRD( M, N, A, LDA, S, WORK( IE ), WORK( ITAUQ ), &
                         WORK( ITAUP ), WORK( IWORK ), LWORK-IWORK+1, &
                         IERR )
            IF( WNTUAS ) THEN
!*
!*              If left singular vectors desired in U, copy result to U
!*              and generate left bidiagonalizing vectors in U
!*              (Workspace: need 3*N+NCU, prefer 3*N+NCU*NB)
!*
               CALL DLACPY( 'L', M, N, A, LDA, U, LDU )
               IF( WNTUS ) &
                  NCU = N
               IF( WNTUA ) &
                  NCU = M
               CALL DORGBR( 'Q', M, NCU, N, U, LDU, WORK( ITAUQ ), &
                            WORK( IWORK ), LWORK-IWORK+1, IERR )
            END IF
            IF( WNTVAS ) THEN
!*
!*              If right singular vectors desired in VT, copy result to
!*              VT and generate right bidiagonalizing vectors in VT
!*              (Workspace: need 4*N-1, prefer 3*N+(N-1)*NB)
!*
               CALL DLACPY( 'U', N, N, A, LDA, VT, LDVT )
               CALL DORGBR( 'P', N, N, N, VT, LDVT, WORK( ITAUP ), &
                            WORK( IWORK ), LWORK-IWORK+1, IERR )
            END IF
            IF( WNTUO ) THEN
!*
!*              If left singular vectors desired in A, generate left
!*              bidiagonalizing vectors in A
!*              (Workspace: need 4*N, prefer 3*N+N*NB)
!*
               CALL DORGBR( 'Q', M, N, N, A, LDA, WORK( ITAUQ ), &
                            WORK( IWORK ), LWORK-IWORK+1, IERR )
            END IF
            IF( WNTVO ) THEN
!*
!*              If right singular vectors desired in A, generate right
!*              bidiagonalizing vectors in A
!*              (Workspace: need 4*N-1, prefer 3*N+(N-1)*NB)
!*
               CALL DORGBR( 'P', N, N, N, A, LDA, WORK( ITAUP ), &
                            WORK( IWORK ), LWORK-IWORK+1, IERR )
            END IF
            IWORK = IE + N
            IF( WNTUAS .OR. WNTUO ) &
               NRU = M
            IF( WNTUN ) &
               NRU = 0
            IF( WNTVAS .OR. WNTVO ) &
               NCVT = N
            IF( WNTVN ) &
               NCVT = 0
            IF( ( .NOT.WNTUO ) .AND. ( .NOT.WNTVO ) ) THEN
!*
!*              Perform bidiagonal QR iteration, if desired, computing
!*              left singular vectors in U and computing right singular
!*              vectors in VT
!*              (Workspace: need BDSPAC)
!*
               CALL DBDSQR( 'U', N, NCVT, NRU, 0, S, WORK( IE ), VT, &
                            LDVT, U, LDU, DUM, 1, WORK( IWORK ), INFO )
            ELSE IF( ( .NOT.WNTUO ) .AND. WNTVO ) THEN
!*
!*              Perform bidiagonal QR iteration, if desired, computing
!*              left singular vectors in U and computing right singular
!*              vectors in A
!*              (Workspace: need BDSPAC)
!*
               CALL DBDSQR( 'U', N, NCVT, NRU, 0, S, WORK( IE ), A, LDA, &
                            U, LDU, DUM, 1, WORK( IWORK ), INFO )
            ELSE
!*
!*              Perform bidiagonal QR iteration, if desired, computing
!*              left singular vectors in A and computing right singular
!*              vectors in VT
!*              (Workspace: need BDSPAC)
!*
               CALL DBDSQR( 'U', N, NCVT, NRU, 0, S, WORK( IE ), VT, &
                            LDVT, A, LDA, DUM, 1, WORK( IWORK ), INFO )
            END IF
!*
         END IF
!*
      ELSE
!*
!*        A has more columns than rows. If A has sufficiently more
!*        columns than rows, first reduce using the LQ decomposition (if
!*        sufficient workspace available)
!*
         IF( N.GE.MNTHR ) THEN
!*
            IF( WNTVN ) THEN
!*
!*              Path 1t(N much larger than M, JOBVT='N')
!*              No right singular vectors to be computed
!*
               ITAU = 1
               IWORK = ITAU + M
!*
!*              Compute A=L*Q
!*              (Workspace: need 2*M, prefer M+M*NB)
!*
               CALL DGELQF( M, N, A, LDA, WORK( ITAU ), WORK( IWORK ), &
                            LWORK-IWORK+1, IERR )
!*
!*              Zero out above L
!*
               CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, A( 1, 2 ), LDA )
               IE = 1
               ITAUQ = IE + M
               ITAUP = ITAUQ + M
               IWORK = ITAUP + M
!*
!*              Bidiagonalize L in A
!*              (Workspace: need 4*M, prefer 3*M+2*M*NB)
!*
               CALL DGEBRD( M, M, A, LDA, S, WORK( IE ), WORK( ITAUQ ), &
                            WORK( ITAUP ), WORK( IWORK ), LWORK-IWORK+1, &
                            IERR )
               IF( WNTUO .OR. WNTUAS ) THEN
!*
!*                 If left singular vectors desired, generate Q
!*                 (Workspace: need 4*M, prefer 3*M+M*NB)
!*
                  CALL DORGBR( 'Q', M, M, M, A, LDA, WORK( ITAUQ ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
               END IF
               IWORK = IE + M
               NRU = 0
               IF( WNTUO .OR. WNTUAS ) &
                  NRU = M
!*
!*              Perform bidiagonal QR iteration, computing left singular
!*              vectors of A in A if desired
!*              (Workspace: need BDSPAC)
!*
               CALL DBDSQR( 'U', M, 0, NRU, 0, S, WORK( IE ), DUM, 1, A, &
                            LDA, DUM, 1, WORK( IWORK ), INFO )
!*
!*              If left singular vectors desired in U, copy them there
!*
               IF( WNTUAS ) &
                  CALL DLACPY( 'F', M, M, A, LDA, U, LDU )
!*
            ELSE IF( WNTVO .AND. WNTUN ) THEN
!*
!*              Path 2t(N much larger than M, JOBU='N', JOBVT='O')
!*              M right singular vectors to be overwritten on A and
!*              no left singular vectors to be computed
!*
               IF( LWORK.GE.M*M+MAX( 4*M, BDSPAC ) ) THEN
!*
!*                 Sufficient workspace for a fast algorithm
!*
                  IR = 1
                  IF( LWORK.GE.MAX( WRKBL, LDA*N+M )+LDA*M ) THEN
!*
!*                    WORK(IU) is LDA by N and WORK(IR) is LDA by M
!*
                     LDWRKU = LDA
                     CHUNK = N
                     LDWRKR = LDA
                  ELSE IF( LWORK.GE.MAX( WRKBL, LDA*N+M )+M*M ) THEN
!*
!*                    WORK(IU) is LDA by N and WORK(IR) is M by M
!*
                     LDWRKU = LDA
                     CHUNK = N
                     LDWRKR = M
                  ELSE
!*
!*                    WORK(IU) is M by CHUNK and WORK(IR) is M by M
!*
                     LDWRKU = M
                     CHUNK = ( LWORK-M*M-M ) / M
                     LDWRKR = M
                  END IF
                  ITAU = IR + LDWRKR*M
                  IWORK = ITAU + M
!*
!*                 Compute A=L*Q
!*                 (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                  CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Copy L to WORK(IR) and zero out above it
!*
                  CALL DLACPY( 'L', M, M, A, LDA, WORK( IR ), LDWRKR )
                  CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, &
                               WORK( IR+LDWRKR ), LDWRKR )
!*
!*                 Generate Q in A
!*                 (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                  CALL DORGLQ( M, N, M, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IE = ITAU
                  ITAUQ = IE + M
                  ITAUP = ITAUQ + M
                  IWORK = ITAUP + M
!*
!*                 Bidiagonalize L in WORK(IR)
!*                 (Workspace: need M*M+4*M, prefer M*M+3*M+2*M*NB)
!*
                  CALL DGEBRD( M, M, WORK( IR ), LDWRKR, S, WORK( IE ), &
                               WORK( ITAUQ ), WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Generate right vectors bidiagonalizing L
!*                 (Workspace: need M*M+4*M-1, prefer M*M+3*M+(M-1)*NB)
!*
                  CALL DORGBR( 'P', M, M, M, WORK( IR ), LDWRKR, &
                               WORK( ITAUP ), WORK( IWORK ), &
                               LWORK-IWORK+1, IERR )
                  IWORK = IE + M
!*
!*                 Perform bidiagonal QR iteration, computing right
!*                 singular vectors of L in WORK(IR)
!*                 (Workspace: need M*M+BDSPAC)
!*
                  CALL DBDSQR( 'U', M, M, 0, 0, S, WORK( IE ), &
                               WORK( IR ), LDWRKR, DUM, 1, DUM, 1, &
                               WORK( IWORK ), INFO )
                  IU = IE + M
!*
!*                 Multiply right singular vectors of L in WORK(IR) by Q
!*                 in A, storing result in WORK(IU) and copying to A
!*                 (Workspace: need M*M+2*M, prefer M*M+M*N+M)
!*
                  DO 30 I = 1, N, CHUNK
                     BLK = MIN( N-I+1, CHUNK )
                     CALL DGEMM( 'N', 'N', M, BLK, M, ONE, WORK( IR ), &
                                 LDWRKR, A( 1, I ), LDA, ZERO, &
                                 WORK( IU ), LDWRKU )
                     CALL DLACPY( 'F', M, BLK, WORK( IU ), LDWRKU, &
                                  A( 1, I ), LDA )
   30             CONTINUE
!*
               ELSE
!*
!*                 Insufficient workspace for a fast algorithm
!*
                  IE = 1
                  ITAUQ = IE + M
                  ITAUP = ITAUQ + M
                  IWORK = ITAUP + M
!*
!*                 Bidiagonalize A
!*                 (Workspace: need 3*M+N, prefer 3*M+(M+N)*NB)
!*
                  CALL DGEBRD( M, N, A, LDA, S, WORK( IE ), &
                               WORK( ITAUQ ), WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Generate right vectors bidiagonalizing A
!*                 (Workspace: need 4*M, prefer 3*M+M*NB)
!*
                  CALL DORGBR( 'P', M, N, M, A, LDA, WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IWORK = IE + M
!*
!*                 Perform bidiagonal QR iteration, computing right
!*                 singular vectors of A in A
!*                 (Workspace: need BDSPAC)
!*
                  CALL DBDSQR( 'L', M, N, 0, 0, S, WORK( IE ), A, LDA, &
                               DUM, 1, DUM, 1, WORK( IWORK ), INFO )
!*
               END IF
!*
            ELSE IF( WNTVO .AND. WNTUAS ) THEN
!*
!*              Path 3t(N much larger than M, JOBU='S' or 'A', JOBVT='O')
!*              M right singular vectors to be overwritten on A and
!*              M left singular vectors to be computed in U
!*
               IF( LWORK.GE.M*M+MAX( 4*M, BDSPAC ) ) THEN
!*
!*                 Sufficient workspace for a fast algorithm
!*
                  IR = 1
                  IF( LWORK.GE.MAX( WRKBL, LDA*N+M )+LDA*M ) THEN
!*
!*                    WORK(IU) is LDA by N and WORK(IR) is LDA by M
!*
                     LDWRKU = LDA
                     CHUNK = N
                     LDWRKR = LDA
                  ELSE IF( LWORK.GE.MAX( WRKBL, LDA*N+M )+M*M ) THEN
!*
!*                    WORK(IU) is LDA by N and WORK(IR) is M by M
!*
                     LDWRKU = LDA
                     CHUNK = N
                     LDWRKR = M
                  ELSE
!*
!*                    WORK(IU) is M by CHUNK and WORK(IR) is M by M
!*
                     LDWRKU = M
                     CHUNK = ( LWORK-M*M-M ) / M
                     LDWRKR = M
                  END IF
                  ITAU = IR + LDWRKR*M
                  IWORK = ITAU + M
!*
!*                 Compute A=L*Q
!*                 (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                  CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Copy L to U, zeroing about above it
!*
                  CALL DLACPY( 'L', M, M, A, LDA, U, LDU )
                  CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, U( 1, 2 ), &
                               LDU )
!*
!*                 Generate Q in A
!*                 (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                  CALL DORGLQ( M, N, M, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IE = ITAU
                  ITAUQ = IE + M
                  ITAUP = ITAUQ + M
                  IWORK = ITAUP + M
!*
!*                 Bidiagonalize L in U, copying result to WORK(IR)
!*                 (Workspace: need M*M+4*M, prefer M*M+3*M+2*M*NB)
!*
                  CALL DGEBRD( M, M, U, LDU, S, WORK( IE ), &
                               WORK( ITAUQ ), WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  CALL DLACPY( 'U', M, M, U, LDU, WORK( IR ), LDWRKR )
!*
!*                 Generate right vectors bidiagonalizing L in WORK(IR)
!*                 (Workspace: need M*M+4*M-1, prefer M*M+3*M+(M-1)*NB)
!*
                  CALL DORGBR( 'P', M, M, M, WORK( IR ), LDWRKR, &
                               WORK( ITAUP ), WORK( IWORK ), &
                               LWORK-IWORK+1, IERR )
!*
!*                 Generate left vectors bidiagonalizing L in U
!*                 (Workspace: need M*M+4*M, prefer M*M+3*M+M*NB)
!*
                  CALL DORGBR( 'Q', M, M, M, U, LDU, WORK( ITAUQ ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IWORK = IE + M
!*
!*                 Perform bidiagonal QR iteration, computing left
!*                 singular vectors of L in U, and computing right
!*                 singular vectors of L in WORK(IR)
!*                 (Workspace: need M*M+BDSPAC)
!*
                  CALL DBDSQR( 'U', M, M, M, 0, S, WORK( IE ), &
                               WORK( IR ), LDWRKR, U, LDU, DUM, 1, &
                               WORK( IWORK ), INFO )
                  IU = IE + M
!*
!*                 Multiply right singular vectors of L in WORK(IR) by Q
!*                 in A, storing result in WORK(IU) and copying to A
!*                 (Workspace: need M*M+2*M, prefer M*M+M*N+M))
!*
                  DO 40 I = 1, N, CHUNK
                     BLK = MIN( N-I+1, CHUNK )
                     CALL DGEMM( 'N', 'N', M, BLK, M, ONE, WORK( IR ), &
                                 LDWRKR, A( 1, I ), LDA, ZERO, &
                                 WORK( IU ), LDWRKU )
                     CALL DLACPY( 'F', M, BLK, WORK( IU ), LDWRKU, &
                                  A( 1, I ), LDA )
   40             CONTINUE
!*
               ELSE
!*
!*                 Insufficient workspace for a fast algorithm
!*
                  ITAU = 1
                  IWORK = ITAU + M
!*
!*                 Compute A=L*Q
!*                 (Workspace: need 2*M, prefer M+M*NB)
!*
                  CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Copy L to U, zeroing out above it
!*
                  CALL DLACPY( 'L', M, M, A, LDA, U, LDU )
                  CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, U( 1, 2 ), &
                               LDU )
!*
!*                 Generate Q in A
!*                 (Workspace: need 2*M, prefer M+M*NB)
!*
                  CALL DORGLQ( M, N, M, A, LDA, WORK( ITAU ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IE = ITAU
                  ITAUQ = IE + M
                  ITAUP = ITAUQ + M
                  IWORK = ITAUP + M
!*
!*                 Bidiagonalize L in U
!*                 (Workspace: need 4*M, prefer 3*M+2*M*NB)
!*
                  CALL DGEBRD( M, M, U, LDU, S, WORK( IE ), &
                               WORK( ITAUQ ), WORK( ITAUP ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                 Multiply right vectors bidiagonalizing L by Q in A
!*                 (Workspace: need 3*M+N, prefer 3*M+N*NB)
!*
                  CALL DORMBR( 'P', 'L', 'T', M, N, M, U, LDU, &
                               WORK( ITAUP ), A, LDA, WORK( IWORK ), &
                               LWORK-IWORK+1, IERR )
!*
!*                 Generate left vectors bidiagonalizing L in U
!*                 (Workspace: need 4*M, prefer 3*M+M*NB)
!*
                  CALL DORGBR( 'Q', M, M, M, U, LDU, WORK( ITAUQ ), &
                               WORK( IWORK ), LWORK-IWORK+1, IERR )
                  IWORK = IE + M
!*
!*                 Perform bidiagonal QR iteration, computing left
!*                 singular vectors of A in U and computing right
!*                 singular vectors of A in A
!*                 (Workspace: need BDSPAC)
!*
                  CALL DBDSQR( 'U', M, N, M, 0, S, WORK( IE ), A, LDA, &
                               U, LDU, DUM, 1, WORK( IWORK ), INFO )
!*
               END IF
!*
            ELSE IF( WNTVS ) THEN
!*
               IF( WNTUN ) THEN
!*
!*                 Path 4t(N much larger than M, JOBU='N', JOBVT='S')
!*                 M right singular vectors to be computed in VT and
!                 no left singular vectors to be computed
!*
                  IF( LWORK.GE.M*M+MAX( 4*M, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IR = 1
                     IF( LWORK.GE.WRKBL+LDA*M ) THEN
!*
!*                       WORK(IR) is LDA by M
!*
                        LDWRKR = LDA
                     ELSE
!*
!*                       WORK(IR) is M by M
!*
                        LDWRKR = M
                     END IF
                     ITAU = IR + LDWRKR*M
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q
!*                    (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy L to WORK(IR), zeroing out above it
!*
                     CALL DLACPY( 'L', M, M, A, LDA, WORK( IR ), &
                                  LDWRKR )
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, &
                                  WORK( IR+LDWRKR ), LDWRKR )
!*
!*                    Generate Q in A
!*                    (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                     CALL DORGLQ( M, N, M, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Bidiagonalize L in WORK(IR)
!*                    (Workspace: need M*M+4*M, prefer M*M+3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, WORK( IR ), LDWRKR, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right vectors bidiagonalizing L in
!*                    WORK(IR)
!*                    (Workspace: need M*M+4*M, prefer M*M+3*M+(M-1)*NB)
!*
                     CALL DORGBR( 'P', M, M, M, WORK( IR ), LDWRKR, &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing right
!*                    singular vectors of L in WORK(IR)
!*                    (Workspace: need M*M+BDSPAC)
!*
                     CALL DBDSQR( 'U', M, M, 0, 0, S, WORK( IE ), &
                                  WORK( IR ), LDWRKR, DUM, 1, DUM, 1, &
                                  WORK( IWORK ), INFO )
!*
!*                    Multiply right singular vectors of L in WORK(IR) by
!*                    Q in A, storing result in VT
!*                    (Workspace: need M*M)
!*
                     CALL DGEMM( 'N', 'N', M, N, M, ONE, WORK( IR ), &
                                 LDWRKR, A, LDA, ZERO, VT, LDVT )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy result to VT
!*
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Generate Q in VT
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DORGLQ( M, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Zero out above L in A
!*
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, A( 1, 2 ), &
                                  LDA )
!*
!*                    Bidiagonalize L in A
!*                    (Workspace: need 4*M, prefer 3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, A, LDA, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply right vectors bidiagonalizing L by Q in VT
!*                    (Workspace: need 3*M+N, prefer 3*M+N*NB)
!*
                     CALL DORMBR( 'P', 'L', 'T', M, N, M, A, LDA, &
                                  WORK( ITAUP ), VT, LDVT, &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing right
!*                    singular vectors of A in VT
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', M, N, 0, 0, S, WORK( IE ), VT, &
                                  LDVT, DUM, 1, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               ELSE IF( WNTUO ) THEN
!*
!*                 Path 5t(N much larger than M, JOBU='O', JOBVT='S')
!*                 M right singular vectors to be computed in VT and
!*                 M left singular vectors to be overwritten on A
!*
                  IF( LWORK.GE.2*M*M+MAX( 4*M, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IU = 1
                     IF( LWORK.GE.WRKBL+2*LDA*M ) THEN
!*
!*                       WORK(IU) is LDA by M and WORK(IR) is LDA by M
!*
                        LDWRKU = LDA
                        IR = IU + LDWRKU*M
                        LDWRKR = LDA
                     ELSE IF( LWORK.GE.WRKBL+( LDA+M )*M ) THEN
!*
!*                       WORK(IU) is LDA by M and WORK(IR) is M by M
!*
                        LDWRKU = LDA
                        IR = IU + LDWRKU*M
                        LDWRKR = M
                     ELSE
!*
!*                       WORK(IU) is M by M and WORK(IR) is M by M
!*
                        LDWRKU = M
                        IR = IU + LDWRKU*M
                        LDWRKR = M
                     END IF
                     ITAU = IR + LDWRKR*M
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q
!*                    (Workspace: need 2*M*M+2*M, prefer 2*M*M+M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy L to WORK(IU), zeroing out below it
!*
                     CALL DLACPY( 'L', M, M, A, LDA, WORK( IU ), &
                                  LDWRKU )
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, &
                                  WORK( IU+LDWRKU ), LDWRKU )
!*
!*                    Generate Q in A
!*                    (Workspace: need 2*M*M+2*M, prefer 2*M*M+M+M*NB)
!*
                     CALL DORGLQ( M, N, M, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Bidiagonalize L in WORK(IU), copying result to
!*                    WORK(IR)
!*                    (Workspace: need 2*M*M+4*M,
!*                                prefer 2*M*M+3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, WORK( IU ), LDWRKU, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, M, WORK( IU ), LDWRKU, &
                                  WORK( IR ), LDWRKR )
!*
!*                    Generate right bidiagonalizing vectors in WORK(IU)
!*                    (Workspace: need 2*M*M+4*M-1,
!*                                prefer 2*M*M+3*M+(M-1)*NB)
!*
                     CALL DORGBR( 'P', M, M, M, WORK( IU ), LDWRKU, &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate left bidiagonalizing vectors in WORK(IR)
!*                    (Workspace: need 2*M*M+4*M, prefer 2*M*M+3*M+M*NB)
!*
                     CALL DORGBR( 'Q', M, M, M, WORK( IR ), LDWRKR, &
                                  WORK( ITAUQ ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of L in WORK(IR) and computing
!*                    right singular vectors of L in WORK(IU)
!*                    (Workspace: need 2*M*M+BDSPAC)
!*
                     CALL DBDSQR( 'U', M, M, M, 0, S, WORK( IE ), &
                                  WORK( IU ), LDWRKU, WORK( IR ), &
                                  LDWRKR, DUM, 1, WORK( IWORK ), INFO )
!*
!*                    Multiply right singular vectors of L in WORK(IU) by
!*                    Q in A, storing result in VT
!*                    (Workspace: need M*M)
!*
                     CALL DGEMM( 'N', 'N', M, N, M, ONE, WORK( IU ), &
                                 LDWRKU, A, LDA, ZERO, VT, LDVT )
!*
!*                    Copy left singular vectors of L to A
!*                    (Workspace: need M*M)
!*
                     CALL DLACPY( 'F', M, M, WORK( IR ), LDWRKR, A, &
                                  LDA )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q, copying result to VT
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Generate Q in VT
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DORGLQ( M, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Zero out above L in A
!*
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, A( 1, 2 ), &
                                  LDA )
!*
!*                    Bidiagonalize L in A
!*                    (Workspace: need 4*M, prefer 3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, A, LDA, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply right vectors bidiagonalizing L by Q in VT
!*                    (Workspace: need 3*M+N, prefer 3*M+N*NB)
!*
                     CALL DORMBR( 'P', 'L', 'T', M, N, M, A, LDA, &
                                  WORK( ITAUP ), VT, LDVT, &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Generate left bidiagonalizing vectors of L in A
!*                    (Workspace: need 4*M, prefer 3*M+M*NB)
!*
                     CALL DORGBR( 'Q', M, M, M, A, LDA, WORK( ITAUQ ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, compute left
!*                    singular vectors of A in A and compute right
!*                    singular vectors of A in VT
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', M, N, M, 0, S, WORK( IE ), VT, &
                                  LDVT, A, LDA, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               ELSE IF( WNTUAS ) THEN
!*
!*                 Path 6t(N much larger than M, JOBU='S' or 'A',
!*                         JOBVT='S')
!*                 M right singular vectors to be computed in VT and
!*                 M left singular vectors to be computed in U
!*
                  IF( LWORK.GE.M*M+MAX( 4*M, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IU = 1
                     IF( LWORK.GE.WRKBL+LDA*M ) THEN
!*
!*                       WORK(IU) is LDA by N
!*
                        LDWRKU = LDA
                     ELSE
!*
!*                       WORK(IU) is LDA by M
!*
                        LDWRKU = M
                     END IF
                     ITAU = IU + LDWRKU*M
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q
!*                    (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy L to WORK(IU), zeroing out above it
!*
                     CALL DLACPY( 'L', M, M, A, LDA, WORK( IU ), &
                                  LDWRKU )
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, &
                                  WORK( IU+LDWRKU ), LDWRKU )
!*
!*                    Generate Q in A
!*                    (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                     CALL DORGLQ( M, N, M, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Bidiagonalize L in WORK(IU), copying result to U
!*                    (Workspace: need M*M+4*M, prefer M*M+3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, WORK( IU ), LDWRKU, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, M, WORK( IU ), LDWRKU, U, &
                                  LDU )
!*
!*                    Generate right bidiagonalizing vectors in WORK(IU)
!*                    (Workspace: need M*M+4*M-1,
!*                                prefer M*M+3*M+(M-1)*NB)
!*
                     CALL DORGBR( 'P', M, M, M, WORK( IU ), LDWRKU, &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!                    Generate left bidiagonalizing vectors in U
!*                    (Workspace: need M*M+4*M, prefer M*M+3*M+M*NB)
!*
                     CALL DORGBR( 'Q', M, M, M, U, LDU, WORK( ITAUQ ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of L in U and computing right
!*                    singular vectors of L in WORK(IU)
!*                    (Workspace: need M*M+BDSPAC)
!*
                     CALL DBDSQR( 'U', M, M, M, 0, S, WORK( IE ), &
                                  WORK( IU ), LDWRKU, U, LDU, DUM, 1, &
                                  WORK( IWORK ), INFO )
!*
!*                    Multiply right singular vectors of L in WORK(IU) by
!*                    Q in A, storing result in VT
!*                    (Workspace: need M*M)
!*
                     CALL DGEMM( 'N', 'N', M, N, M, ONE, WORK( IU ), &
                                 LDWRKU, A, LDA, ZERO, VT, LDVT )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q, copying result to VT
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Generate Q in VT
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DORGLQ( M, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy L to U, zeroing out above it
!*
                     CALL DLACPY( 'L', M, M, A, LDA, U, LDU )
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, U( 1, 2 ), &
                                  LDU )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Bidiagonalize L in U
!*                    (Workspace: need 4*M, prefer 3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, U, LDU, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply right bidiagonalizing vectors in U by Q
!*                    in VT
!*                    (Workspace: need 3*M+N, prefer 3*M+N*NB)
!*
                     CALL DORMBR( 'P', 'L', 'T', M, N, M, U, LDU, &
                                  WORK( ITAUP ), VT, LDVT, &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Generate left bidiagonalizing vectors in U
!*                    (Workspace: need 4*M, prefer 3*M+M*NB)
!*
                     CALL DORGBR( 'Q', M, M, M, U, LDU, WORK( ITAUQ ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in U and computing right
!*                    singular vectors of A in VT
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', M, N, M, 0, S, WORK( IE ), VT, &
                                  LDVT, U, LDU, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               END IF
!*
            ELSE IF( WNTVA ) THEN
!*
               IF( WNTUN ) THEN
!*
!*                 Path 7t(N much larger than M, JOBU='N', JOBVT='A')
!*                 N right singular vectors to be computed in VT and
!*                 no left singular vectors to be computed
!*
                  IF( LWORK.GE.M*M+MAX( N+M, 4*M, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IR = 1
                     IF( LWORK.GE.WRKBL+LDA*M ) THEN
!*
!*                       WORK(IR) is LDA by M
!*
                        LDWRKR = LDA
                     ELSE
!*
!*                       WORK(IR) is M by M
!*
                        LDWRKR = M
                     END IF
                     ITAU = IR + LDWRKR*M
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q, copying result to VT
!*                    (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Copy L to WORK(IR), zeroing out above it
!*
                     CALL DLACPY( 'L', M, M, A, LDA, WORK( IR ), &
                                  LDWRKR )
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, &
                                  WORK( IR+LDWRKR ), LDWRKR )
!*
!*                    Generate Q in VT
!*                    (Workspace: need M*M+M+N, prefer M*M+M+N*NB)
!*
                     CALL DORGLQ( N, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Bidiagonalize L in WORK(IR)
!*                    (Workspace: need M*M+4*M, prefer M*M+3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, WORK( IR ), LDWRKR, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate right bidiagonalizing vectors in WORK(IR)
!*                    (Workspace: need M*M+4*M-1,
!*                                prefer M*M+3*M+(M-1)*NB)
!*
                     CALL DORGBR( 'P', M, M, M, WORK( IR ), LDWRKR, &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing right
!*                    singular vectors of L in WORK(IR)
!*                    (Workspace: need M*M+BDSPAC)
!*
                     CALL DBDSQR( 'U', M, M, 0, 0, S, WORK( IE ), &
                                  WORK( IR ), LDWRKR, DUM, 1, DUM, 1, &
                                  WORK( IWORK ), INFO )
!*
!*                    Multiply right singular vectors of L in WORK(IR) by
!*                    Q in VT, storing result in A
!*                    (Workspace: need M*M)
!*
                     CALL DGEMM( 'N', 'N', M, N, M, ONE, WORK( IR ), &
                                 LDWRKR, VT, LDVT, ZERO, A, LDA )
!*
!*                    Copy right singular vectors of A from A to VT
!*
                     CALL DLACPY( 'F', M, N, A, LDA, VT, LDVT )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q, copying result to VT
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Generate Q in VT
!*                    (Workspace: need M+N, prefer M+N*NB)
!*
                     CALL DORGLQ( N, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Zero out above L in A
!*
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, A( 1, 2 ), &
                                  LDA )
!*
!*                    Bidiagonalize L in A
!*                    (Workspace: need 4*M, prefer 3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, A, LDA, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply right bidiagonalizing vectors in A by Q
!*                    in VT
!*                    (Workspace: need 3*M+N, prefer 3*M+N*NB)
!*
                     CALL DORMBR( 'P', 'L', 'T', M, N, M, A, LDA, &
                                  WORK( ITAUP ), VT, LDVT, &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing right
!*                    singular vectors of A in VT
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', M, N, 0, 0, S, WORK( IE ), VT, &
                                  LDVT, DUM, 1, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               ELSE IF( WNTUO ) THEN
!*
!*                 Path 8t(N much larger than M, JOBU='O', JOBVT='A')
!*                 N right singular vectors to be computed in VT and
!*                 M left singular vectors to be overwritten on A
!*
                  IF( LWORK.GE.2*M*M+MAX( N+M, 4*M, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IU = 1
                     IF( LWORK.GE.WRKBL+2*LDA*M ) THEN
!*
!*                       WORK(IU) is LDA by M and WORK(IR) is LDA by M
!*
                        LDWRKU = LDA
                        IR = IU + LDWRKU*M
                        LDWRKR = LDA
                     ELSE IF( LWORK.GE.WRKBL+( LDA+M )*M ) THEN
!*
!*                       WORK(IU) is LDA by M and WORK(IR) is M by M
!*
                        LDWRKU = LDA
                        IR = IU + LDWRKU*M
                        LDWRKR = M
                     ELSE
!*
!*                       WORK(IU) is M by M and WORK(IR) is M by M
!*
                        LDWRKU = M
                        IR = IU + LDWRKU*M
                        LDWRKR = M
                     END IF
                     ITAU = IR + LDWRKR*M
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q, copying result to VT
!*                    (Workspace: need 2*M*M+2*M, prefer 2*M*M+M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Generate Q in VT
!*                    (Workspace: need 2*M*M+M+N, prefer 2*M*M+M+N*NB)
!*
                     CALL DORGLQ( N, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy L to WORK(IU), zeroing out above it
!*
                     CALL DLACPY( 'L', M, M, A, LDA, WORK( IU ), &
                                  LDWRKU )
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, &
                                  WORK( IU+LDWRKU ), LDWRKU )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Bidiagonalize L in WORK(IU), copying result to
!*                    WORK(IR)
!*                    (Workspace: need 2*M*M+4*M,
!*                                prefer 2*M*M+3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, WORK( IU ), LDWRKU, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, M, WORK( IU ), LDWRKU, &
                                  WORK( IR ), LDWRKR )
!*
!*                    Generate right bidiagonalizing vectors in WORK(IU)
!*                    (Workspace: need 2*M*M+4*M-1,
!                                prefer 2*M*M+3*M+(M-1)*NB)
!*
                     CALL DORGBR( 'P', M, M, M, WORK( IU ), LDWRKU, &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate left bidiagonalizing vectors in WORK(IR)
!*                    (Workspace: need 2*M*M+4*M, prefer 2*M*M+3*M+M*NB)
!*
                     CALL DORGBR( 'Q', M, M, M, WORK( IR ), LDWRKR, &
                                  WORK( ITAUQ ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of L in WORK(IR) and computing
!*                    right singular vectors of L in WORK(IU)
!*                    (Workspace: need 2*M*M+BDSPAC)
!*
                     CALL DBDSQR( 'U', M, M, M, 0, S, WORK( IE ), &
                                  WORK( IU ), LDWRKU, WORK( IR ), &
                                  LDWRKR, DUM, 1, WORK( IWORK ), INFO )
!*
!*                    Multiply right singular vectors of L in WORK(IU) by
!*                    Q in VT, storing result in A
!*                    (Workspace: need M*M)
!*
                     CALL DGEMM( 'N', 'N', M, N, M, ONE, WORK( IU ), &
                                 LDWRKU, VT, LDVT, ZERO, A, LDA )
!*
!*                    Copy right singular vectors of A from A to VT
!*
                     CALL DLACPY( 'F', M, N, A, LDA, VT, LDVT )
!*
!*                    Copy left singular vectors of A from WORK(IR) to A
!*
                     CALL DLACPY( 'F', M, M, WORK( IR ), LDWRKR, A, &
                                  LDA )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q, copying result to VT
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Generate Q in VT
!*                    (Workspace: need M+N, prefer M+N*NB)
!*
                     CALL DORGLQ( N, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Zero out above L in A
!*
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, A( 1, 2 ), &
                                  LDA )
!*
!*                    Bidiagonalize L in A
!*                    (Workspace: need 4*M, prefer 3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, A, LDA, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply right bidiagonalizing vectors in A by Q
!*                    in VT
!*                    (Workspace: need 3*M+N, prefer 3*M+N*NB)
!*
                     CALL DORMBR( 'P', 'L', 'T', M, N, M, A, LDA, &
                                  WORK( ITAUP ), VT, LDVT, &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Generate left bidiagonalizing vectors in A
!*                    (Workspace: need 4*M, prefer 3*M+M*NB)
!*
                     CALL DORGBR( 'Q', M, M, M, A, LDA, WORK( ITAUQ ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in A and computing right
!*                    singular vectors of A in VT
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', M, N, M, 0, S, WORK( IE ), VT, &
                                  LDVT, A, LDA, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               ELSE IF( WNTUAS ) THEN
!*
!*                 Path 9t(N much larger than M, JOBU='S' or 'A',
!*                         JOBVT='A')
!*                 N right singular vectors to be computed in VT and
!*                 M left singular vectors to be computed in U
!*
                  IF( LWORK.GE.M*M+MAX( N+M, 4*M, BDSPAC ) ) THEN
!*
!*                    Sufficient workspace for a fast algorithm
!*
                     IU = 1
                     IF( LWORK.GE.WRKBL+LDA*M ) THEN
!*
!*                       WORK(IU) is LDA by M
!*
                        LDWRKU = LDA
                     ELSE
!*
!*                       WORK(IU) is M by M
!*
                        LDWRKU = M
                     END IF
                     ITAU = IU + LDWRKU*M
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q, copying result to VT
!*                    (Workspace: need M*M+2*M, prefer M*M+M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Generate Q in VT
!*                    (Workspace: need M*M+M+N, prefer M*M+M+N*NB)
!*
                     CALL DORGLQ( N, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy L to WORK(IU), zeroing out above it
!*
                     CALL DLACPY( 'L', M, M, A, LDA, WORK( IU ), &
                                  LDWRKU )
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, &
                                  WORK( IU+LDWRKU ), LDWRKU )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Bidiagonalize L in WORK(IU), copying result to U
!*                    (Workspace: need M*M+4*M, prefer M*M+3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, WORK( IU ), LDWRKU, S, &
                                  WORK( IE ), WORK( ITAUQ ), &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'L', M, M, WORK( IU ), LDWRKU, U, &
                                  LDU )
!*
!*                    Generate right bidiagonalizing vectors in WORK(IU)
!*                    (Workspace: need M*M+4*M, prefer M*M+3*M+(M-1)*NB)
!*
                     CALL DORGBR( 'P', M, M, M, WORK( IU ), LDWRKU, &
                                  WORK( ITAUP ), WORK( IWORK ), &
                                  LWORK-IWORK+1, IERR )
!*
!*                    Generate left bidiagonalizing vectors in U
!*                    (Workspace: need M*M+4*M, prefer M*M+3*M+M*NB)
!*
                     CALL DORGBR( 'Q', M, M, M, U, LDU, WORK( ITAUQ ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of L in U and computing right
!*                    singular vectors of L in WORK(IU)
!*                    (Workspace: need M*M+BDSPAC)
!*
                     CALL DBDSQR( 'U', M, M, M, 0, S, WORK( IE ), &
                                  WORK( IU ), LDWRKU, U, LDU, DUM, 1, &
                                  WORK( IWORK ), INFO )
!*
!*                    Multiply right singular vectors of L in WORK(IU) by
!*                    Q in VT, storing result in A
!*                    (Workspace: need M*M)
!*
                     CALL DGEMM( 'N', 'N', M, N, M, ONE, WORK( IU ), &
                                 LDWRKU, VT, LDVT, ZERO, A, LDA )
!*
!*                    Copy right singular vectors of A from A to VT
!*
                     CALL DLACPY( 'F', M, N, A, LDA, VT, LDVT )
!*
                  ELSE
!*
!*                    Insufficient workspace for a fast algorithm
!*
                     ITAU = 1
                     IWORK = ITAU + M
!*
!*                    Compute A=L*Q, copying result to VT
!*                    (Workspace: need 2*M, prefer M+M*NB)
!*
                     CALL DGELQF( M, N, A, LDA, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
!*
!*                    Generate Q in VT
!*                    (Workspace: need M+N, prefer M+N*NB)
!*
                     CALL DORGLQ( N, N, M, VT, LDVT, WORK( ITAU ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Copy L to U, zeroing out above it
!*
                     CALL DLACPY( 'L', M, M, A, LDA, U, LDU )
                     CALL DLASET( 'U', M-1, M-1, ZERO, ZERO, U( 1, 2 ), &
                                  LDU )
                     IE = ITAU
                     ITAUQ = IE + M
                     ITAUP = ITAUQ + M
                     IWORK = ITAUP + M
!*
!*                    Bidiagonalize L in U
!*                    (Workspace: need 4*M, prefer 3*M+2*M*NB)
!*
                     CALL DGEBRD( M, M, U, LDU, S, WORK( IE ), &
                                  WORK( ITAUQ ), WORK( ITAUP ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Multiply right bidiagonalizing vectors in U by Q
!*                    in VT
!*                    (Workspace: need 3*M+N, prefer 3*M+N*NB)
!*
                     CALL DORMBR( 'P', 'L', 'T', M, N, M, U, LDU, &
                                  WORK( ITAUP ), VT, LDVT, &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
!*
!*                    Generate left bidiagonalizing vectors in U
!*                    (Workspace: need 4*M, prefer 3*M+M*NB)
!*
                     CALL DORGBR( 'Q', M, M, M, U, LDU, WORK( ITAUQ ), &
                                  WORK( IWORK ), LWORK-IWORK+1, IERR )
                     IWORK = IE + M
!*
!*                    Perform bidiagonal QR iteration, computing left
!*                    singular vectors of A in U and computing right
!*                    singular vectors of A in VT
!*                    (Workspace: need BDSPAC)
!*
                     CALL DBDSQR( 'U', M, N, M, 0, S, WORK( IE ), VT, &
                                  LDVT, U, LDU, DUM, 1, WORK( IWORK ), &
                                  INFO )
!*
                  END IF
!*
               END IF
!*
            END IF
!*
         ELSE
!*
!*           N .LT. MNTHR
!*
!*           Path 10t(N greater than M, but not much larger)
!*           Reduce to bidiagonal form without LQ decomposition
!*
            IE = 1
            ITAUQ = IE + M
            ITAUP = ITAUQ + M
            IWORK = ITAUP + M
!*
!*           Bidiagonalize A
!*           (Workspace: need 3*M+N, prefer 3*M+(M+N)*NB)
!*
            CALL DGEBRD( M, N, A, LDA, S, WORK( IE ), WORK( ITAUQ ), &
                         WORK( ITAUP ), WORK( IWORK ), LWORK-IWORK+1, &
                         IERR )
            IF( WNTUAS ) THEN
!*
!*              If left singular vectors desired in U, copy result to U
!*              and generate left bidiagonalizing vectors in U
!*              (Workspace: need 4*M-1, prefer 3*M+(M-1)*NB)
!*
               CALL DLACPY( 'L', M, M, A, LDA, U, LDU )
               CALL DORGBR( 'Q', M, M, N, U, LDU, WORK( ITAUQ ), &
                            WORK( IWORK ), LWORK-IWORK+1, IERR )
            END IF
            IF( WNTVAS ) THEN
!*
!*              If right singular vectors desired in VT, copy result to
!*              VT and generate right bidiagonalizing vectors in VT
!*              (Workspace: need 3*M+NRVT, prefer 3*M+NRVT*NB)
!*
               CALL DLACPY( 'U', M, N, A, LDA, VT, LDVT )
               IF( WNTVA ) &
                  NRVT = N
               IF( WNTVS ) &
                  NRVT = M
               CALL DORGBR( 'P', NRVT, N, M, VT, LDVT, WORK( ITAUP ), &
                            WORK( IWORK ), LWORK-IWORK+1, IERR )
            END IF
            IF( WNTUO ) THEN
!*
!*              If left singular vectors desired in A, generate left
!*              bidiagonalizing vectors in A
!*              (Workspace: need 4*M-1, prefer 3*M+(M-1)*NB)
!*
               CALL DORGBR( 'Q', M, M, N, A, LDA, WORK( ITAUQ ), &
                            WORK( IWORK ), LWORK-IWORK+1, IERR )
            END IF
            IF( WNTVO ) THEN
!*
!*              If right singular vectors desired in A, generate right
!*              bidiagonalizing vectors in A
!*              (Workspace: need 4*M, prefer 3*M+M*NB)
!*
               CALL DORGBR( 'P', M, N, M, A, LDA, WORK( ITAUP ), &
                            WORK( IWORK ), LWORK-IWORK+1, IERR )
            END IF
            IWORK = IE + M
            IF( WNTUAS .OR. WNTUO ) &
               NRU = M
            IF( WNTUN ) &
               NRU = 0
            IF( WNTVAS .OR. WNTVO ) &
               NCVT = N
            IF( WNTVN ) &
               NCVT = 0
            IF( ( .NOT.WNTUO ) .AND. ( .NOT.WNTVO ) ) THEN
!*
!*              Perform bidiagonal QR iteration, if desired, computing
!*              left singular vectors in U and computing right singular
!*              vectors in VT
!*              (Workspace: need BDSPAC)
!*
               CALL DBDSQR( 'L', M, NCVT, NRU, 0, S, WORK( IE ), VT, &
                            LDVT, U, LDU, DUM, 1, WORK( IWORK ), INFO )
            ELSE IF( ( .NOT.WNTUO ) .AND. WNTVO ) THEN
!*
!*              Perform bidiagonal QR iteration, if desired, computing
!*              left singular vectors in U and computing right singular
!*              vectors in A
!*              (Workspace: need BDSPAC)
!*
               CALL DBDSQR( 'L', M, NCVT, NRU, 0, S, WORK( IE ), A, LDA, &
                            U, LDU, DUM, 1, WORK( IWORK ), INFO )
            ELSE
!*
!*              Perform bidiagonal QR iteration, if desired, computing
!*              left singular vectors in A and computing right singular
!*              vectors in VT
!*              (Workspace: need BDSPAC)
!*
               CALL DBDSQR( 'L', M, NCVT, NRU, 0, S, WORK( IE ), VT, &
                            LDVT, A, LDA, DUM, 1, WORK( IWORK ), INFO )
            END IF
!*
         END IF
!*
      END IF
!*
!*     If DBDSQR failed to converge, copy unconverged superdiagonals
!*     to WORK( 2:MINMN )
!*
      IF( INFO.NE.0 ) THEN
         IF( IE.GT.2 ) THEN
            DO 50 I = 1, MINMN - 1
               WORK( I+1 ) = WORK( I+IE-1 )
   50       CONTINUE
         END IF
         IF( IE.LT.2 ) THEN
            DO 60 I = MINMN - 1, 1, -1
               WORK( I+1 ) = WORK( I+IE-1 )
   60       CONTINUE
         END IF
      END IF
!*
!*     Undo scaling if necessary
!*
      IF( ISCL.EQ.1 ) THEN
         IF( ANRM.GT.BIGNUM ) &
            CALL DLASCL( 'G', 0, 0, BIGNUM, ANRM, MINMN, 1, S, MINMN, &
                         IERR )
         IF( INFO.NE.0 .AND. ANRM.GT.BIGNUM ) &
            CALL DLASCL( 'G', 0, 0, BIGNUM, ANRM, MINMN-1, 1, WORK( 2 ), &
                         MINMN, IERR )
         IF( ANRM.LT.SMLNUM ) &
            CALL DLASCL( 'G', 0, 0, SMLNUM, ANRM, MINMN, 1, S, MINMN, &
                         IERR )
         IF( INFO.NE.0 .AND. ANRM.LT.SMLNUM ) &
            CALL DLASCL( 'G', 0, 0, SMLNUM, ANRM, MINMN-1, 1, WORK( 2 ), &
                         MINMN, IERR )
      END IF
!*
!*     Return optimal workspace in WORK(1)
!*
      WORK( 1 ) = MAXWRK
!*
      RETURN
!*
!*     End of DGESVD
!*
      END

!==========================================================================

!*> \brief \b DBDSQR
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DBDSQR + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dbdsqr.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dbdsqr.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dbdsqr.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DBDSQR( UPLO, N, NCVT, NRU, NCC, D, E, VT, LDVT, U,
!*                          LDU, C, LDC, WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          UPLO
!*       INTEGER            INFO, LDC, LDU, LDVT, N, NCC, NCVT, NRU
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   C( LDC, * ), D( * ), E( * ), U( LDU, * ),
!*                          VT( LDVT, * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DBDSQR computes the singular values and, optionally, the right and/or
!*> left singular vectors from the singular value decomposition (SVD) of
!*> a real N-by-N (upper or lower) bidiagonal matrix B using the implicit
!*> zero-shift QR algorithm.  The SVD of B has the form
!*>
!*>    B = Q * S * P**T
!*>
!*> where S is the diagonal matrix of singular values, Q is an orthogonal
!*> matrix of left singular vectors, and P is an orthogonal matrix of
!*> right singular vectors.  If left singular vectors are requested, this
!*> subroutine actually returns U*Q instead of Q, and, if right singular
!*> vectors are requested, this subroutine returns P**T*VT instead of
!*> P**T, for given real input matrices U and VT.  When U and VT are the
!*> orthogonal matrices that reduce a general matrix A to bidiagonal
!*> form:  A = U*B*VT, as computed by DGEBRD, then
!*>
!*>    A = (U*Q) * S * (P**T*VT)
!*>
!*> is the SVD of A.  Optionally, the subroutine may also compute Q**T*C
!*> for a given real input matrix C.
!*>
!*> See "Computing  Small Singular Values of Bidiagonal Matrices With
!*> Guaranteed High Relative Accuracy," by J. Demmel and W. Kahan,
!*> LAPACK Working Note #3 (or SIAM J. Sci. Statist. Comput. vol. 11,
!*> no. 5, pp. 873-912, Sept 1990) and
!*> "Accurate singular values and differential qd algorithms," by
!*> B. Parlett and V. Fernando, Technical Report CPAM-554, Mathematics
!*> Department, University of California at Berkeley, July 1992
!*> for a detailed description of the algorithm.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] UPLO
!*> \verbatim
!*>          UPLO is CHARACTER*1
!*>          = 'U':  B is upper bidiagonal;
!*>          = 'L':  B is lower bidiagonal.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The order of the matrix B.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in] NCVT
!*> \verbatim
!*>          NCVT is INTEGER
!*>          The number of columns of the matrix VT. NCVT >= 0.
!*> \endverbatim
!*>
!*> \param[in] NRU
!*> \verbatim
!*>          NRU is INTEGER
!*>          The number of rows of the matrix U. NRU >= 0.
!*> \endverbatim
!*>
!*> \param[in] NCC
!*> \verbatim
!*>          NCC is INTEGER
!*>          The number of columns of the matrix C. NCC >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] D
!*> \verbatim
!*>          D is DOUBLE PRECISION array, dimension (N)
!*>          On entry, the n diagonal elements of the bidiagonal matrix B.
!*>          On exit, if INFO=0, the singular values of B in decreasing
!*>          order.
!*> \endverbatim
!*>
!*> \param[in,out] E
!*> \verbatim
!*>          E is DOUBLE PRECISION array, dimension (N-1)
!*>          On entry, the N-1 offdiagonal elements of the bidiagonal
!*>          matrix B.
!*>          On exit, if INFO = 0, E is destroyed; if INFO > 0, D and E
!*>          will contain the diagonal and superdiagonal elements of a
!*>          bidiagonal matrix orthogonally equivalent to the one given
!*>          as input.
!*> \endverbatim
!*>
!*> \param[in,out] VT
!*> \verbatim
!*>          VT is DOUBLE PRECISION array, dimension (LDVT, NCVT)
!*>          On entry, an N-by-NCVT matrix VT.
!*>          On exit, VT is overwritten by P**T * VT.
!*>          Not referenced if NCVT = 0.
!*> \endverbatim
!*>
!*> \param[in] LDVT
!*> \verbatim
!*>          LDVT is INTEGER
!*>          The leading dimension of the array VT.
!*>          LDVT >= max(1,N) if NCVT > 0; LDVT >= 1 if NCVT = 0.
!*> \endverbatim
!*>
!*> \param[in,out] U
!*> \verbatim
!*>          U is DOUBLE PRECISION array, dimension (LDU, N)
!*>          On entry, an NRU-by-N matrix U.
!*>          On exit, U is overwritten by U * Q.
!*>          Not referenced if NRU = 0.
!*> \endverbatim
!*>
!*> \param[in] LDU
!*> \verbatim
!*>          LDU is INTEGER
!*>          The leading dimension of the array U.  LDU >= max(1,NRU).
!*> \endverbatim
!*>
!*> \param[in,out] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension (LDC, NCC)
!*>          On entry, an N-by-NCC matrix C.
!*>          On exit, C is overwritten by Q**T * C.
!*>          Not referenced if NCC = 0.
!*> \endverbatim
!*>
!*> \param[in] LDC
!*> \verbatim
!*>          LDC is INTEGER
!*>          The leading dimension of the array C.
!*>          LDC >= max(1,N) if NCC > 0; LDC >=1 if NCC = 0.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (4*N)
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  If INFO = -i, the i-th argument had an illegal value
!*>          > 0:
!*>             if NCVT = NRU = NCC = 0,
!*>                = 1, a split was marked by a positive value in E
!*>                = 2, current block of Z not diagonalized after 30*N
!*>                     iterations (in inner while loop)
!*>                = 3, termination criterion of outer while loop not met
!*>                     (program created more than N unreduced blocks)
!*>             else NCVT = NRU = NCC = 0,
!*>                   the algorithm did not converge; D and E contain the
!*>                   elements of a bidiagonal matrix which is orthogonally
!*>                   similar to the input matrix B;  if INFO = i, i
!*>                   elements of E have not converged to zero.
!*> \endverbatim
!*
!*> \par Internal Parameters:
!*  =========================
!*>
!*> \verbatim
!*>  TOLMUL  DOUBLE PRECISION, default = max(10,min(100,EPS**(-1/8)))
!*>          TOLMUL controls the convergence criterion of the QR loop.
!*>          If it is positive, TOLMUL*EPS is the desired relative
!*>             precision in the computed singular values.
!*>          If it is negative, abs(TOLMUL*EPS*sigma_max) is the
!*>             desired absolute accuracy in the computed singular
!*>             values (corresponds to relative accuracy
!*>             abs(TOLMUL*EPS) in the largest singular value.
!*>          abs(TOLMUL) should be between 1 and 1/EPS, and preferably
!*>             between 10 (for fast convergence) and .1/EPS
!*>             (for there to be some accuracy in the results).
!*>          Default is to lose at either one eighth or 2 of the
!*>             available decimal digits in each computed singular value
!*>             (whichever is smaller).
!*>
!*>  MAXITR  INTEGER, default = 6
!*>          MAXITR controls the maximum number of passes of the
!*>          algorithm through its inner loop. The algorithms stops
!*>          (and so fails to converge) if the number of passes
!*>          through the inner loop exceeds MAXITR*N**2.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup auxOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DBDSQR( UPLO, N, NCVT, NRU, NCC, D, E, VT, LDVT, U, &
                         LDU, C, LDC, WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      CHARACTER          UPLO
      INTEGER            INFO, LDC, LDU, LDVT, N, NCC, NCVT, NRU
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   C( LDC, * ), D( * ), E( * ), U( LDU, * ), &
                         VT( LDVT, * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D0 )
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D0 )
      DOUBLE PRECISION   NEGONE
      PARAMETER          ( NEGONE = -1.0D0 )
      DOUBLE PRECISION   HNDRTH
      PARAMETER          ( HNDRTH = 0.01D0 )
      DOUBLE PRECISION   TEN
      PARAMETER          ( TEN = 10.0D0 )
      DOUBLE PRECISION   HNDRD
      PARAMETER          ( HNDRD = 100.0D0 )
      DOUBLE PRECISION   MEIGTH
      PARAMETER          ( MEIGTH = -0.125D0 )
      INTEGER            MAXITR
      PARAMETER          ( MAXITR = 6 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LOWER, ROTATE
      INTEGER            I, IDIR, ISUB, ITER, J, LL, LLL, M, MAXIT, NM1, &
                         NM12, NM13, OLDLL, OLDM
      DOUBLE PRECISION   ABSE, ABSS, COSL, COSR, CS, EPS, F, G, H, MU, &
                         OLDCS, OLDSN, R, SHIFT, SIGMN, SIGMX, SINL, &
                         SINR, SLL, SMAX, SMIN, SMINL, SMINOA, &
                         SN, THRESH, TOL, TOLMUL, UNFL
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      DOUBLE PRECISION   DLAMCH
      EXTERNAL           LSAME, DLAMCH
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARTG, DLAS2, DLASQ1, DLASR, DLASV2, DROT, &
                         DSCAL, DSWAP, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input parameters.
!*
      INFO = 0
      LOWER = LSAME( UPLO, 'L' )
      IF( .NOT.LSAME( UPLO, 'U' ) .AND. .NOT.LOWER ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( NCVT.LT.0 ) THEN
         INFO = -3
      ELSE IF( NRU.LT.0 ) THEN
         INFO = -4
      ELSE IF( NCC.LT.0 ) THEN
         INFO = -5
      ELSE IF( ( NCVT.EQ.0 .AND. LDVT.LT.1 ) .OR. &
               ( NCVT.GT.0 .AND. LDVT.LT.MAX( 1, N ) ) ) THEN
         INFO = -9
      ELSE IF( LDU.LT.MAX( 1, NRU ) ) THEN
         INFO = -11
      ELSE IF( ( NCC.EQ.0 .AND. LDC.LT.1 ) .OR. &
               ( NCC.GT.0 .AND. LDC.LT.MAX( 1, N ) ) ) THEN
         INFO = -13
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DBDSQR', -INFO )
         RETURN
      END IF
      IF( N.EQ.0 ) &
         RETURN
      IF( N.EQ.1 ) &
         GO TO 160
!*
!*     ROTATE is true if any singular vectors desired, false otherwise
!*
      ROTATE = ( NCVT.GT.0 ) .OR. ( NRU.GT.0 ) .OR. ( NCC.GT.0 )
!*
!*     If no singular vectors desired, use qd algorithm
!*
      IF( .NOT.ROTATE ) THEN
         CALL DLASQ1( N, D, E, WORK, INFO )
!*
!*     If INFO equals 2, dqds didn't finish, try to finish
!*
         IF( INFO .NE. 2 ) RETURN
         INFO = 0
      END IF
!*
      NM1 = N - 1
      NM12 = NM1 + NM1
      NM13 = NM12 + NM1
      IDIR = 0
!*
!*     Get machine constants
!*
      EPS = DLAMCH( 'Epsilon' )
      UNFL = DLAMCH( 'Safe minimum' )
!*
!*     If matrix lower bidiagonal, rotate to be upper bidiagonal
!*     by applying Givens rotations on the left
!*
      IF( LOWER ) THEN
         DO 10 I = 1, N - 1
            CALL DLARTG( D( I ), E( I ), CS, SN, R )
            D( I ) = R
            E( I ) = SN*D( I+1 )
            D( I+1 ) = CS*D( I+1 )
            WORK( I ) = CS
            WORK( NM1+I ) = SN
   10    CONTINUE
!*
!*        Update singular vectors if desired
!*
         IF( NRU.GT.0 ) &
            CALL DLASR( 'R', 'V', 'F', NRU, N, WORK( 1 ), WORK( N ), U, &
                        LDU )
         IF( NCC.GT.0 ) &
            CALL DLASR( 'L', 'V', 'F', N, NCC, WORK( 1 ), WORK( N ), C, &
                        LDC )
      END IF
!*
!*     Compute singular values to relative accuracy TOL
!*     (By setting TOL to be negative, algorithm will compute
!*     singular values to absolute accuracy ABS(TOL)*norm(input matrix))
!*
      TOLMUL = MAX( TEN, MIN( HNDRD, EPS**MEIGTH ) )
      TOL = TOLMUL*EPS
!*
!*     Compute approximate maximum, minimum singular values
!*
      SMAX = ZERO
      DO 20 I = 1, N
         SMAX = MAX( SMAX, ABS( D( I ) ) )
   20 CONTINUE
      DO 30 I = 1, N - 1
         SMAX = MAX( SMAX, ABS( E( I ) ) )
   30 CONTINUE
      SMINL = ZERO
      IF( TOL.GE.ZERO ) THEN
!*
!*        Relative accuracy desired
!*
         SMINOA = ABS( D( 1 ) )
         IF( SMINOA.EQ.ZERO ) &
            GO TO 50
         MU = SMINOA
         DO 40 I = 2, N
            MU = ABS( D( I ) )*( MU / ( MU+ABS( E( I-1 ) ) ) )
            SMINOA = MIN( SMINOA, MU )
            IF( SMINOA.EQ.ZERO ) &
               GO TO 50
   40    CONTINUE
   50    CONTINUE
         SMINOA = SMINOA / SQRT( DBLE( N ) )
         THRESH = MAX( TOL*SMINOA, MAXITR*N*N*UNFL )
      ELSE
!*
!*        Absolute accuracy desired
!*
         THRESH = MAX( ABS( TOL )*SMAX, MAXITR*N*N*UNFL )
      END IF
!*
!*     Prepare for main iteration loop for the singular values
!*     (MAXIT is the maximum number of passes through the inner
!*     loop permitted before nonconvergence signalled.)
!*
      MAXIT = MAXITR*N*N
      ITER = 0
      OLDLL = -1
      OLDM = -1
!*
!*     M points to last element of unconverged part of matrix
!*
      M = N
!*
!*     Begin main iteration loop
!*
   60 CONTINUE
!*
!*     Check for convergence or exceeding iteration count
!*
      IF( M.LE.1 ) &
         GO TO 160
      IF( ITER.GT.MAXIT ) &
         GO TO 200
!*
!*     Find diagonal block of matrix to work on
!*
      IF( TOL.LT.ZERO .AND. ABS( D( M ) ).LE.THRESH ) &
         D( M ) = ZERO
      SMAX = ABS( D( M ) )
      SMIN = SMAX
      DO 70 LLL = 1, M - 1
         LL = M - LLL
         ABSS = ABS( D( LL ) )
         ABSE = ABS( E( LL ) )
         IF( TOL.LT.ZERO .AND. ABSS.LE.THRESH ) &
            D( LL ) = ZERO
         IF( ABSE.LE.THRESH ) &
            GO TO 80
         SMIN = MIN( SMIN, ABSS )
         SMAX = MAX( SMAX, ABSS, ABSE )
   70 CONTINUE
      LL = 0
      GO TO 90
   80 CONTINUE
      E( LL ) = ZERO
!*
!*     Matrix splits since E(LL) = 0
!*
      IF( LL.EQ.M-1 ) THEN
!*
!*        Convergence of bottom singular value, return to top of loop
!*
         M = M - 1
         GO TO 60
      END IF
   90 CONTINUE
      LL = LL + 1
!*
!*     E(LL) through E(M-1) are nonzero, E(LL-1) is zero
!*
      IF( LL.EQ.M-1 ) THEN
!*
!*        2 by 2 block, handle separately
!*
         CALL DLASV2( D( M-1 ), E( M-1 ), D( M ), SIGMN, SIGMX, SINR, &
                      COSR, SINL, COSL )
         D( M-1 ) = SIGMX
         E( M-1 ) = ZERO
         D( M ) = SIGMN
!*
!*        Compute singular vectors, if desired
!*
         IF( NCVT.GT.0 ) &
            CALL DROT( NCVT, VT( M-1, 1 ), LDVT, VT( M, 1 ), LDVT, COSR, &
                       SINR )
         IF( NRU.GT.0 ) &
            CALL DROT( NRU, U( 1, M-1 ), 1, U( 1, M ), 1, COSL, SINL )
         IF( NCC.GT.0 ) &
            CALL DROT( NCC, C( M-1, 1 ), LDC, C( M, 1 ), LDC, COSL, &
                       SINL )
         M = M - 2
         GO TO 60
      END IF
!*
!*     If working on new submatrix, choose shift direction
!*     (from larger end diagonal element towards smaller)
!*
      IF( LL.GT.OLDM .OR. M.LT.OLDLL ) THEN
         IF( ABS( D( LL ) ).GE.ABS( D( M ) ) ) THEN
!*
!*           Chase bulge from top (big end) to bottom (small end)
!*
            IDIR = 1
         ELSE
!*
!*           Chase bulge from bottom (big end) to top (small end)
!*
            IDIR = 2
         END IF
      END IF
!*
!*     Apply convergence tests
!*
      IF( IDIR.EQ.1 ) THEN
!*
!*        Run convergence test in forward direction
!*        First apply standard test to bottom of matrix
!*
         IF( ABS( E( M-1 ) ).LE.ABS( TOL )*ABS( D( M ) ) .OR. &
             ( TOL.LT.ZERO .AND. ABS( E( M-1 ) ).LE.THRESH ) ) THEN
            E( M-1 ) = ZERO
            GO TO 60
         END IF
!*
         IF( TOL.GE.ZERO ) THEN
!*
!*           If relative accuracy desired,
!*           apply convergence criterion forward
!*
            MU = ABS( D( LL ) )
            SMINL = MU
            DO 100 LLL = LL, M - 1
               IF( ABS( E( LLL ) ).LE.TOL*MU ) THEN
                  E( LLL ) = ZERO
                  GO TO 60
               END IF
               MU = ABS( D( LLL+1 ) )*( MU / ( MU+ABS( E( LLL ) ) ) )
               SMINL = MIN( SMINL, MU )
  100       CONTINUE
         END IF
!*
      ELSE
!*
!*        Run convergence test in backward direction
!*        First apply standard test to top of matrix
!*
         IF( ABS( E( LL ) ).LE.ABS( TOL )*ABS( D( LL ) ) .OR. &
             ( TOL.LT.ZERO .AND. ABS( E( LL ) ).LE.THRESH ) ) THEN
            E( LL ) = ZERO
            GO TO 60
         END IF
!*
         IF( TOL.GE.ZERO ) THEN
!*
!*           If relative accuracy desired,
!*           apply convergence criterion backward
!*
            MU = ABS( D( M ) )
            SMINL = MU
            DO 110 LLL = M - 1, LL, -1
               IF( ABS( E( LLL ) ).LE.TOL*MU ) THEN
                  E( LLL ) = ZERO
                  GO TO 60
               END IF
               MU = ABS( D( LLL ) )*( MU / ( MU+ABS( E( LLL ) ) ) )
               SMINL = MIN( SMINL, MU )
  110       CONTINUE
         END IF
      END IF
      OLDLL = LL
      OLDM = M
!*
!*     Compute shift.  First, test if shifting would ruin relative
!*     accuracy, and if so set the shift to zero.
!*
      IF( TOL.GE.ZERO .AND. N*TOL*( SMINL / SMAX ).LE. &
          MAX( EPS, HNDRTH*TOL ) ) THEN
!*
!*        Use a zero shift to avoid loss of relative accuracy
!*
         SHIFT = ZERO
      ELSE
!*
!*        Compute the shift from 2-by-2 block at end of matrix
!*
         IF( IDIR.EQ.1 ) THEN
            SLL = ABS( D( LL ) )
            CALL DLAS2( D( M-1 ), E( M-1 ), D( M ), SHIFT, R )
         ELSE
            SLL = ABS( D( M ) )
            CALL DLAS2( D( LL ), E( LL ), D( LL+1 ), SHIFT, R )
         END IF
!*
!*        Test if shift negligible, and if so set to zero
!*
         IF( SLL.GT.ZERO ) THEN
            IF( ( SHIFT / SLL )**2.LT.EPS ) &
               SHIFT = ZERO
         END IF
      END IF
!*
!*     Increment iteration count
!*
      ITER = ITER + M - LL
!*
!*     If SHIFT = 0, do simplified QR iteration
!*
      IF( SHIFT.EQ.ZERO ) THEN
         IF( IDIR.EQ.1 ) THEN
!*
!*           Chase bulge from top to bottom
!*           Save cosines and sines for later singular vector updates
!*
            CS = ONE
            OLDCS = ONE
            DO 120 I = LL, M - 1
               CALL DLARTG( D( I )*CS, E( I ), CS, SN, R )
               IF( I.GT.LL ) &
                  E( I-1 ) = OLDSN*R
               CALL DLARTG( OLDCS*R, D( I+1 )*SN, OLDCS, OLDSN, D( I ) )
               WORK( I-LL+1 ) = CS
               WORK( I-LL+1+NM1 ) = SN
               WORK( I-LL+1+NM12 ) = OLDCS
               WORK( I-LL+1+NM13 ) = OLDSN
  120       CONTINUE
            H = D( M )*CS
            D( M ) = H*OLDCS
            E( M-1 ) = H*OLDSN
!*
!*           Update singular vectors
!*
            IF( NCVT.GT.0 ) &
               CALL DLASR( 'L', 'V', 'F', M-LL+1, NCVT, WORK( 1 ), &
                           WORK( N ), VT( LL, 1 ), LDVT )
            IF( NRU.GT.0 ) &
               CALL DLASR( 'R', 'V', 'F', NRU, M-LL+1, WORK( NM12+1 ), &
                           WORK( NM13+1 ), U( 1, LL ), LDU )
            IF( NCC.GT.0 ) &
               CALL DLASR( 'L', 'V', 'F', M-LL+1, NCC, WORK( NM12+1 ), &
                           WORK( NM13+1 ), C( LL, 1 ), LDC )
!*
!*           Test convergence
!*
            IF( ABS( E( M-1 ) ).LE.THRESH ) &
               E( M-1 ) = ZERO
!*
         ELSE
!*
!*           Chase bulge from bottom to top
!*           Save cosines and sines for later singular vector updates
!*
            CS = ONE
            OLDCS = ONE
            DO 130 I = M, LL + 1, -1
               CALL DLARTG( D( I )*CS, E( I-1 ), CS, SN, R )
               IF( I.LT.M ) &
                  E( I ) = OLDSN*R
               CALL DLARTG( OLDCS*R, D( I-1 )*SN, OLDCS, OLDSN, D( I ) )
               WORK( I-LL ) = CS
               WORK( I-LL+NM1 ) = -SN
               WORK( I-LL+NM12 ) = OLDCS
               WORK( I-LL+NM13 ) = -OLDSN
  130       CONTINUE
            H = D( LL )*CS
            D( LL ) = H*OLDCS
            E( LL ) = H*OLDSN
!*
!*           Update singular vectors
!*
            IF( NCVT.GT.0 ) &
               CALL DLASR( 'L', 'V', 'B', M-LL+1, NCVT, WORK( NM12+1 ), &
                           WORK( NM13+1 ), VT( LL, 1 ), LDVT )
            IF( NRU.GT.0 ) &
               CALL DLASR( 'R', 'V', 'B', NRU, M-LL+1, WORK( 1 ), &
                           WORK( N ), U( 1, LL ), LDU )
            IF( NCC.GT.0 ) &
               CALL DLASR( 'L', 'V', 'B', M-LL+1, NCC, WORK( 1 ), &
                           WORK( N ), C( LL, 1 ), LDC )
!*
!*           Test convergence
!*
            IF( ABS( E( LL ) ).LE.THRESH ) &
               E( LL ) = ZERO
         END IF
      ELSE
!*
!*        Use nonzero shift
!*
         IF( IDIR.EQ.1 ) THEN
!*
!*           Chase bulge from top to bottom
!*           Save cosines and sines for later singular vector updates
!*
            F = ( ABS( D( LL ) )-SHIFT )* &
                ( SIGN( ONE, D( LL ) )+SHIFT / D( LL ) )
            G = E( LL )
            DO 140 I = LL, M - 1
               CALL DLARTG( F, G, COSR, SINR, R )
               IF( I.GT.LL ) &
                  E( I-1 ) = R
               F = COSR*D( I ) + SINR*E( I )
               E( I ) = COSR*E( I ) - SINR*D( I )
               G = SINR*D( I+1 )
               D( I+1 ) = COSR*D( I+1 )
               CALL DLARTG( F, G, COSL, SINL, R )
               D( I ) = R
               F = COSL*E( I ) + SINL*D( I+1 )
               D( I+1 ) = COSL*D( I+1 ) - SINL*E( I )
               IF( I.LT.M-1 ) THEN
                  G = SINL*E( I+1 )
                  E( I+1 ) = COSL*E( I+1 )
               END IF
               WORK( I-LL+1 ) = COSR
               WORK( I-LL+1+NM1 ) = SINR
               WORK( I-LL+1+NM12 ) = COSL
               WORK( I-LL+1+NM13 ) = SINL
  140       CONTINUE
            E( M-1 ) = F
!*
!*           Update singular vectors
!*
            IF( NCVT.GT.0 ) &
               CALL DLASR( 'L', 'V', 'F', M-LL+1, NCVT, WORK( 1 ), &
                           WORK( N ), VT( LL, 1 ), LDVT )
            IF( NRU.GT.0 ) &
               CALL DLASR( 'R', 'V', 'F', NRU, M-LL+1, WORK( NM12+1 ), &
                           WORK( NM13+1 ), U( 1, LL ), LDU )
            IF( NCC.GT.0 ) &
               CALL DLASR( 'L', 'V', 'F', M-LL+1, NCC, WORK( NM12+1 ), &
                           WORK( NM13+1 ), C( LL, 1 ), LDC )
!*
!*           Test convergence
!*
            IF( ABS( E( M-1 ) ).LE.THRESH ) &
               E( M-1 ) = ZERO
!*
         ELSE
!*
!*           Chase bulge from bottom to top
!*           Save cosines and sines for later singular vector updates
!*
            F = ( ABS( D( M ) )-SHIFT )*( SIGN( ONE, D( M ) )+SHIFT / &
                D( M ) )
            G = E( M-1 )
            DO 150 I = M, LL + 1, -1
               CALL DLARTG( F, G, COSR, SINR, R )
               IF( I.LT.M ) &
                  E( I ) = R
               F = COSR*D( I ) + SINR*E( I-1 )
               E( I-1 ) = COSR*E( I-1 ) - SINR*D( I )
               G = SINR*D( I-1 )
               D( I-1 ) = COSR*D( I-1 )
               CALL DLARTG( F, G, COSL, SINL, R )
               D( I ) = R
               F = COSL*E( I-1 ) + SINL*D( I-1 )
               D( I-1 ) = COSL*D( I-1 ) - SINL*E( I-1 )
               IF( I.GT.LL+1 ) THEN
                  G = SINL*E( I-2 )
                  E( I-2 ) = COSL*E( I-2 )
               END IF
               WORK( I-LL ) = COSR
               WORK( I-LL+NM1 ) = -SINR
               WORK( I-LL+NM12 ) = COSL
               WORK( I-LL+NM13 ) = -SINL
  150       CONTINUE
            E( LL ) = F
!*
!*           Test convergence
!*
            IF( ABS( E( LL ) ).LE.THRESH ) &
               E( LL ) = ZERO
!*
!*           Update singular vectors if desired
!*
            IF( NCVT.GT.0 ) &
               CALL DLASR( 'L', 'V', 'B', M-LL+1, NCVT, WORK( NM12+1 ), &
                           WORK( NM13+1 ), VT( LL, 1 ), LDVT )
            IF( NRU.GT.0 ) &
               CALL DLASR( 'R', 'V', 'B', NRU, M-LL+1, WORK( 1 ), &
                           WORK( N ), U( 1, LL ), LDU )
            IF( NCC.GT.0 ) &
               CALL DLASR( 'L', 'V', 'B', M-LL+1, NCC, WORK( 1 ), &
                           WORK( N ), C( LL, 1 ), LDC )
         END IF
      END IF
!*
!*     QR iteration finished, go back and check convergence
!*
      GO TO 60
!*
!*     All singular values converged, so make them positive
!*
  160 CONTINUE
      DO 170 I = 1, N
         IF( D( I ).LT.ZERO ) THEN
            D( I ) = -D( I )
!*
!*           Change sign of singular vectors, if desired
!*
            IF( NCVT.GT.0 ) &
               CALL DSCAL( NCVT, NEGONE, VT( I, 1 ), LDVT )
         END IF
  170 CONTINUE
!*
!*     Sort the singular values into decreasing order (insertion sort on
!*     singular values, but only one transposition per singular vector)
!*
      DO 190 I = 1, N - 1
!*
!*        Scan for smallest D(I)
!*
         ISUB = 1
         SMIN = D( 1 )
         DO 180 J = 2, N + 1 - I
            IF( D( J ).LE.SMIN ) THEN
               ISUB = J
               SMIN = D( J )
            END IF
  180    CONTINUE
         IF( ISUB.NE.N+1-I ) THEN
!*
!*           Swap singular values and vectors
!*
            D( ISUB ) = D( N+1-I )
            D( N+1-I ) = SMIN
            IF( NCVT.GT.0 ) &
               CALL DSWAP( NCVT, VT( ISUB, 1 ), LDVT, VT( N+1-I, 1 ), &
                           LDVT )
            IF( NRU.GT.0 ) &
               CALL DSWAP( NRU, U( 1, ISUB ), 1, U( 1, N+1-I ), 1 )
            IF( NCC.GT.0 ) &
               CALL DSWAP( NCC, C( ISUB, 1 ), LDC, C( N+1-I, 1 ), LDC )
         END IF
  190 CONTINUE
      GO TO 220
!*
!*     Maximum number of iterations exceeded, failure to converge
!*
  200 CONTINUE
      INFO = 0
      DO 210 I = 1, N - 1
         IF( E( I ).NE.ZERO ) &
            INFO = INFO + 1
  210 CONTINUE
  220 CONTINUE
      RETURN
!*
!*     End of DBDSQR
!*
      END

!==========================================================================

!*> \brief \b DGEBRD
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DGEBRD + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dgebrd.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dgebrd.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dgebrd.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGEBRD( M, N, A, LDA, D, E, TAUQ, TAUP, WORK, LWORK,
!*                          INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, LDA, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), D( * ), E( * ), TAUP( * ), &
!*                          TAUQ( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGEBRD reduces a general real M-by-N matrix A to upper or lower
!*> bidiagonal form B by an orthogonal transformation: Q**T * A * P = B.
!*>
!*> If m >= n, B is upper bidiagonal; if m < n, B is lower bidiagonal.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows in the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns in the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the M-by-N general matrix to be reduced.
!*>          On exit,
!*>          if m >= n, the diagonal and the first superdiagonal are
!*>            overwritten with the upper bidiagonal matrix B; the
!*>            elements below the diagonal, with the array TAUQ, represent
!*>            the orthogonal matrix Q as a product of elementary
!*>            reflectors, and the elements above the first superdiagonal,
!*>            with the array TAUP, represent the orthogonal matrix P as
!*>            a product of elementary reflectors;
!*>          if m < n, the diagonal and the first subdiagonal are
!*>            overwritten with the lower bidiagonal matrix B; the
!*>            elements below the first subdiagonal, with the array TAUQ,
!*>            represent the orthogonal matrix Q as a product of
!*>            elementary reflectors, and the elements above the diagonal,
!*>            with the array TAUP, represent the orthogonal matrix P as
!*>            a product of elementary reflectors.
!*>          See Further Details.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] D
!*> \verbatim
!*>          D is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The diagonal elements of the bidiagonal matrix B:
!*>          D(i) = A(i,i).
!*> \endverbatim
!*>
!*> \param[out] E
!*> \verbatim
!*>          E is DOUBLE PRECISION array, dimension (min(M,N)-1)
!*>          The off-diagonal elements of the bidiagonal matrix B:
!*>          if m >= n, E(i) = A(i,i+1) for i = 1,2,...,n-1;
!*>          if m < n, E(i) = A(i+1,i) for i = 1,2,...,m-1.
!*> \endverbatim
!*>
!*> \param[out] TAUQ
!*> \verbatim
!*>          TAUQ is DOUBLE PRECISION array dimension (min(M,N))
!*>          The scalar factors of the elementary reflectors which
!*>          represent the orthogonal matrix Q. See Further Details.
!*> \endverbatim
!*>
!*> \param[out] TAUP
!*> \verbatim
!*>          TAUP is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The scalar factors of the elementary reflectors which
!*>          represent the orthogonal matrix P. See Further Details.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The length of the array WORK.  LWORK >= max(1,M,N).
!*>          For optimum performance LWORK >= (M+N)*NB, where NB
!*>          is the optimal blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleGEcomputational
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The matrices Q and P are represented as products of elementary
!*>  reflectors:
!*>
!*>  If m >= n,
!*>
!*>     Q = H(1) H(2) . . . H(n)  and  P = G(1) G(2) . . . G(n-1)
!*>
!*>  Each H(i) and G(i) has the form:
!*>
!*>     H(i) = I - tauq * v * v**T  and G(i) = I - taup * u * u**T
!*>
!*>  where tauq and taup are real scalars, and v and u are real vectors;
!*>  v(1:i-1) = 0, v(i) = 1, and v(i+1:m) is stored on exit in A(i+1:m,i);
!*>  u(1:i) = 0, u(i+1) = 1, and u(i+2:n) is stored on exit in A(i,i+2:n);
!*>  tauq is stored in TAUQ(i) and taup in TAUP(i).
!*>
!*>  If m < n,
!*>
!*>     Q = H(1) H(2) . . . H(m-1)  and  P = G(1) G(2) . . . G(m)
!*>
!*>  Each H(i) and G(i) has the form:
!*>
!*>     H(i) = I - tauq * v * v**T  and G(i) = I - taup * u * u**T
!*>
!*>  where tauq and taup are real scalars, and v and u are real vectors;
!*>  v(1:i) = 0, v(i+1) = 1, and v(i+2:m) is stored on exit in A(i+2:m,i);
!*>  u(1:i-1) = 0, u(i) = 1, and u(i+1:n) is stored on exit in A(i,i+1:n);
!*>  tauq is stored in TAUQ(i) and taup in TAUP(i).
!*>
!*>  The contents of A on exit are illustrated by the following examples:
!*>
!*>  m = 6 and n = 5 (m > n):          m = 5 and n = 6 (m < n):
!*>
!*>    (  d   e   u1  u1  u1 )           (  d   u1  u1  u1  u1  u1 )
!*>    (  v1  d   e   u2  u2 )           (  e   d   u2  u2  u2  u2 )
!*>    (  v1  v2  d   e   u3 )           (  v1  e   d   u3  u3  u3 )
!*>    (  v1  v2  v3  d   e  )           (  v1  v2  e   d   u4  u4 )
!*>    (  v1  v2  v3  v4  d  )           (  v1  v2  v3  e   d   u5 )
!*>    (  v1  v2  v3  v4  v5 )
!*>
!*>  where d and e denote diagonal and off-diagonal elements of B, vi
!*>  denotes an element of the vector defining H(i), and ui an element of
!*>  the vector defining G(i).
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DGEBRD( M, N, A, LDA, D, E, TAUQ, TAUP, WORK, LWORK, &
                         INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, LDA, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), D( * ), E( * ), TAUP( * ), &
                         TAUQ( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D+0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LQUERY
      INTEGER            I, IINFO, J, LDWRKX, LDWRKY, LWKOPT, MINMN, NB, &
                         NBMIN, NX
      DOUBLE PRECISION   WS
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DGEBD2, DGEMM, DLABRD, XERBLA
!*     ..
!*     .. External Functions ..
      INTEGER            ILAENV
      EXTERNAL           ILAENV
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input parameters
!*
      INFO = 0
      NB = MAX( 1, ILAENV( 1, 'DGEBRD', ' ', M, N, -1, -1 ) )
      LWKOPT = ( M+N )*NB
      WORK( 1 ) = DBLE( LWKOPT )
      LQUERY = ( LWORK.EQ.-1 )
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      ELSE IF( LWORK.LT.MAX( 1, M, N ) .AND. .NOT.LQUERY ) THEN
         INFO = -10
      END IF
      IF( INFO.LT.0 ) THEN
         CALL XERBLA( 'DGEBRD', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      MINMN = MIN( M, N )
      IF( MINMN.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!*
      WS = MAX( M, N )
      LDWRKX = M
      LDWRKY = N
!*
      IF( NB.GT.1 .AND. NB.LT.MINMN ) THEN
!*
!*        Set the crossover point NX.
!*
         NX = MAX( NB, ILAENV( 3, 'DGEBRD', ' ', M, N, -1, -1 ) )
!*
!*        Determine when to switch from blocked to unblocked code.
!*
         IF( NX.LT.MINMN ) THEN
            WS = ( M+N )*NB
            IF( LWORK.LT.WS ) THEN
!*
!*              Not enough work space for the optimal NB, consider using
!*              a smaller block size.
!*
               NBMIN = ILAENV( 2, 'DGEBRD', ' ', M, N, -1, -1 )
               IF( LWORK.GE.( M+N )*NBMIN ) THEN
                  NB = LWORK / ( M+N )
               ELSE
                  NB = 1
                  NX = MINMN
               END IF
            END IF
         END IF
      ELSE
         NX = MINMN
      END IF
!*
      DO 30 I = 1, MINMN - NX, NB
!*
!*        Reduce rows and columns i:i+nb-1 to bidiagonal form and return
!*        the matrices X and Y which are needed to update the unreduced
!*        part of the matrix
!*
         CALL DLABRD( M-I+1, N-I+1, NB, A( I, I ), LDA, D( I ), E( I ), &
                      TAUQ( I ), TAUP( I ), WORK, LDWRKX, &
                      WORK( LDWRKX*NB+1 ), LDWRKY )
!*
!*        Update the trailing submatrix A(i+nb:m,i+nb:n), using an update
!*        of the form  A := A - V*Y**T - X*U**T
!*
         CALL DGEMM( 'No transpose', 'Transpose', M-I-NB+1, N-I-NB+1, &
                     NB, -ONE, A( I+NB, I ), LDA, &
                     WORK( LDWRKX*NB+NB+1 ), LDWRKY, ONE, &
                     A( I+NB, I+NB ), LDA )
         CALL DGEMM( 'No transpose', 'No transpose', M-I-NB+1, N-I-NB+1, &
                     NB, -ONE, WORK( NB+1 ), LDWRKX, A( I, I+NB ), LDA, &
                     ONE, A( I+NB, I+NB ), LDA )
!*
!*        Copy diagonal and off-diagonal elements of B back into A
!*
         IF( M.GE.N ) THEN
            DO 10 J = I, I + NB - 1
               A( J, J ) = D( J )
               A( J, J+1 ) = E( J )
   10       CONTINUE
         ELSE
            DO 20 J = I, I + NB - 1
               A( J, J ) = D( J )
               A( J+1, J ) = E( J )
   20       CONTINUE
         END IF
   30 CONTINUE
!*
!*     Use unblocked code to reduce the remainder of the matrix
!*
      CALL DGEBD2( M-I+1, N-I+1, A( I, I ), LDA, D( I ), E( I ), &
                   TAUQ( I ), TAUP( I ), WORK, IINFO )
      WORK( 1 ) = WS
      RETURN
!*
!*     End of DGEBRD
!*
      END

!==========================================================================

!*> \brief \b DGELQF
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DGELQF + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dgelqf.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dgelqf.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dgelqf.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGELQF( M, N, A, LDA, TAU, WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, LDA, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGELQF computes an LQ factorization of a real M-by-N matrix A:
!*> A = L * Q.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the M-by-N matrix A.
!*>          On exit, the elements on and below the diagonal of the array
!*>          contain the m-by-min(m,n) lower trapezoidal matrix L (L is
!*>          lower triangular if m <= n); the elements above the diagonal,
!*>          with the array TAU, represent the orthogonal matrix Q as a
!*>          product of elementary reflectors (see Further Details).
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The scalar factors of the elementary reflectors (see Further
!*>          Details).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK.  LWORK >= max(1,M).
!*>          For optimum performance LWORK >= M*NB, where NB is the
!*>          optimal blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleGEcomputational
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The matrix Q is represented as a product of elementary reflectors
!*>
!*>     Q = H(k) . . . H(2) H(1), where k = min(m,n).
!*>
!*>  Each H(i) has the form
!*>
!*>     H(i) = I - tau * v * v**T
!*>
!*>  where tau is a real scalar, and v is a real vector with
!*>  v(1:i-1) = 0 and v(i) = 1; v(i+1:n) is stored on exit in A(i,i+1:n),
!*>  and tau in TAU(i).
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DGELQF( M, N, A, LDA, TAU, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, LDA, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Local Scalars ..
      LOGICAL            LQUERY
      INTEGER            I, IB, IINFO, IWS, K, LDWORK, LWKOPT, NB, &
                         NBMIN, NX
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DGELQ2, DLARFB, DLARFT, XERBLA
!*     ..
!*     .. External Functions ..
      INTEGER            ILAENV
      EXTERNAL           ILAENV
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      NB = ILAENV( 1, 'DGELQF', ' ', M, N, -1, -1 )
      LWKOPT = M*NB
      WORK( 1 ) = LWKOPT
      LQUERY = ( LWORK.EQ.-1 )
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      ELSE IF( LWORK.LT.MAX( 1, M ) .AND. .NOT.LQUERY ) THEN
         INFO = -7
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DGELQF', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      K = MIN( M, N )
      IF( K.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!*
      NBMIN = 2
      NX = 0
      IWS = M
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
!*
!*        Determine when to cross over from blocked to unblocked code.
!*
         NX = MAX( 0, ILAENV( 3, 'DGELQF', ' ', M, N, -1, -1 ) )
         IF( NX.LT.K ) THEN
!*
!*           Determine if workspace is large enough for blocked code.
!*
            LDWORK = M
            IWS = LDWORK*NB
            IF( LWORK.LT.IWS ) THEN
!*
!*              Not enough workspace to use optimal NB:  reduce NB and
!*              determine the minimum value of NB.
!*
               NB = LWORK / LDWORK
               NBMIN = MAX( 2, ILAENV( 2, 'DGELQF', ' ', M, N, -1, &
                       -1 ) )
            END IF
         END IF
      END IF
!*
      IF( NB.GE.NBMIN .AND. NB.LT.K .AND. NX.LT.K ) THEN
!*
!*        Use blocked code initially
!*
         DO 10 I = 1, K - NX, NB
            IB = MIN( K-I+1, NB )
!*
!*           Compute the LQ factorization of the current block
!*           A(i:i+ib-1,i:n)
!*
            CALL DGELQ2( IB, N-I+1, A( I, I ), LDA, TAU( I ), WORK, &
                         IINFO )
            IF( I+IB.LE.M ) THEN
!*
!*              Form the triangular factor of the block reflector
!*              H = H(i) H(i+1) . . . H(i+ib-1)
!*
               CALL DLARFT( 'Forward', 'Rowwise', N-I+1, IB, A( I, I ), &
                            LDA, TAU( I ), WORK, LDWORK )
!*
!*              Apply H to A(i+ib:m,i:n) from the right
!*
               CALL DLARFB( 'Right', 'No transpose', 'Forward', &
                            'Rowwise', M-I-IB+1, N-I+1, IB, A( I, I ), &
                            LDA, WORK, LDWORK, A( I+IB, I ), LDA, &
                            WORK( IB+1 ), LDWORK )
            END IF
   10    CONTINUE
      ELSE
         I = 1
      END IF
!*
!*     Use unblocked code to factor the last or only block.
!*
      IF( I.LE.K ) &
         CALL DGELQ2( M-I+1, N-I+1, A( I, I ), LDA, TAU( I ), WORK, &
                      IINFO )
!*
      WORK( 1 ) = IWS
      RETURN
!*
!*     End of DGELQF
!*
      END

!==========================================================================

!*> \brief \b DGEQRF
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DGEQRF + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dgeqrf.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dgeqrf.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dgeqrf.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGEQRF( M, N, A, LDA, TAU, WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, LDA, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGEQRF computes a QR factorization of a real M-by-N matrix A:
!*> A = Q * R.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the M-by-N matrix A.
!*>          On exit, the elements on and above the diagonal of the array
!*>          contain the min(M,N)-by-N upper trapezoidal matrix R (R is
!*>          upper triangular if m >= n); the elements below the diagonal,
!*>          with the array TAU, represent the orthogonal matrix Q as a
!*>          product of min(m,n) elementary reflectors (see Further
!*>          Details).
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The scalar factors of the elementary reflectors (see Further
!*>          Details).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK.  LWORK >= max(1,N).
!*>          For optimum performance LWORK >= N*NB, where NB is
!*>          the optimal blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleGEcomputational
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The matrix Q is represented as a product of elementary reflectors
!*>
!*>     Q = H(1) H(2) . . . H(k), where k = min(m,n).
!*>
!*>  Each H(i) has the form
!*>
!*>     H(i) = I - tau * v * v**T
!*>
!*>  where tau is a real scalar, and v is a real vector with
!*>  v(1:i-1) = 0 and v(i) = 1; v(i+1:m) is stored on exit in A(i+1:m,i),
!*>  and tau in TAU(i).
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DGEQRF( M, N, A, LDA, TAU, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, LDA, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Local Scalars ..
      LOGICAL            LQUERY
      INTEGER            I, IB, IINFO, IWS, K, LDWORK, LWKOPT, NB, &
                         NBMIN, NX
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DGEQR2, DLARFB, DLARFT, XERBLA
!*     ..
!*     .. External Functions ..
      INTEGER            ILAENV
      EXTERNAL           ILAENV
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      NB = ILAENV( 1, 'DGEQRF', ' ', M, N, -1, -1 )
      LWKOPT = N*NB
      WORK( 1 ) = LWKOPT
      LQUERY = ( LWORK.EQ.-1 )
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      ELSE IF( LWORK.LT.MAX( 1, N ) .AND. .NOT.LQUERY ) THEN
         INFO = -7
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DGEQRF', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      K = MIN( M, N )
      IF( K.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!*
      NBMIN = 2
      NX = 0
      IWS = N
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
!*
!*        Determine when to cross over from blocked to unblocked code.
!*
         NX = MAX( 0, ILAENV( 3, 'DGEQRF', ' ', M, N, -1, -1 ) )
         IF( NX.LT.K ) THEN
!*
!*           Determine if workspace is large enough for blocked code.
!*
            LDWORK = N
            IWS = LDWORK*NB
            IF( LWORK.LT.IWS ) THEN
!*
!*              Not enough workspace to use optimal NB:  reduce NB and
!*              determine the minimum value of NB.
!*
               NB = LWORK / LDWORK
               NBMIN = MAX( 2, ILAENV( 2, 'DGEQRF', ' ', M, N, -1, &
                       -1 ) )
            END IF
         END IF
      END IF
!*
      IF( NB.GE.NBMIN .AND. NB.LT.K .AND. NX.LT.K ) THEN
!*
!*        Use blocked code initially
!*
         DO 10 I = 1, K - NX, NB
            IB = MIN( K-I+1, NB )
!*
!*           Compute the QR factorization of the current block
!*           A(i:m,i:i+ib-1)
!*
            CALL DGEQR2( M-I+1, IB, A( I, I ), LDA, TAU( I ), WORK, &
                         IINFO )
            IF( I+IB.LE.N ) THEN
!*
!*              Form the triangular factor of the block reflector
!*              H = H(i) H(i+1) . . . H(i+ib-1)
!*
               CALL DLARFT( 'Forward', 'Columnwise', M-I+1, IB, &
                            A( I, I ), LDA, TAU( I ), WORK, LDWORK )
!*
!*              Apply H**T to A(i:m,i+ib:n) from the left
!*
               CALL DLARFB( 'Left', 'Transpose', 'Forward', &
                            'Columnwise', M-I+1, N-I-IB+1, IB, &
                            A( I, I ), LDA, WORK, LDWORK, A( I, I+IB ), &
                            LDA, WORK( IB+1 ), LDWORK )
            END IF
   10    CONTINUE
      ELSE
         I = 1
      END IF
!*
!*     Use unblocked code to factor the last or only block.
!*
      IF( I.LE.K ) &
         CALL DGEQR2( M-I+1, N-I+1, A( I, I ), LDA, TAU( I ), WORK, &
                      IINFO )
!*
      WORK( 1 ) = IWS
      RETURN
!*
!*     End of DGEQRF
!*
      END

!==========================================================================

!*> \brief \b DORGQR
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORGQR + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dorgqr.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dorgqr.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dorgqr.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORGQR( M, N, K, A, LDA, TAU, WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, K, LDA, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORGQR generates an M-by-N real matrix Q with orthonormal columns,
!*> which is defined as the first N columns of a product of K elementary
!*> reflectors of order M
!*>
!*>       Q  =  H(1) H(2) . . . H(k)
!*>
!*> as returned by DGEQRF.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix Q. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix Q. M >= N >= 0.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The number of elementary reflectors whose product defines the
!*>          matrix Q. N >= K >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the i-th column must contain the vector which
!*>          defines the elementary reflector H(i), for i = 1,2,...,k, as
!*>          returned by DGEQRF in the first k columns of its array
!*>          argument A.
!*>          On exit, the M-by-N matrix Q.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The first dimension of the array A. LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i), as returned by DGEQRF.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK. LWORK >= max(1,N).
!*>          For optimum performance LWORK >= N*NB, where NB is the
!*>          optimal blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument has an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORGQR( M, N, K, A, LDA, TAU, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, K, LDA, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LQUERY
      INTEGER            I, IB, IINFO, IWS, J, KI, KK, L, LDWORK, &
                         LWKOPT, NB, NBMIN, NX
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARFB, DLARFT, DORG2R, XERBLA
!*     ..
!*     .. External Functions ..
      INTEGER            ILAENV
      EXTERNAL           ILAENV
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      NB = ILAENV( 1, 'DORGQR', ' ', M, N, K, -1 )
      LWKOPT = MAX( 1, N )*NB
      WORK( 1 ) = LWKOPT
      LQUERY = ( LWORK.EQ.-1 )
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 .OR. N.GT.M ) THEN
         INFO = -2
      ELSE IF( K.LT.0 .OR. K.GT.N ) THEN
         INFO = -3
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -5
      ELSE IF( LWORK.LT.MAX( 1, N ) .AND. .NOT.LQUERY ) THEN
         INFO = -8
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORGQR', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( N.LE.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!*
      NBMIN = 2
      NX = 0
      IWS = N
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
!*
!*        Determine when to cross over from blocked to unblocked code.
!*
         NX = MAX( 0, ILAENV( 3, 'DORGQR', ' ', M, N, K, -1 ) )
         IF( NX.LT.K ) THEN
!*
!*           Determine if workspace is large enough for blocked code.
!*
            LDWORK = N
            IWS = LDWORK*NB
            IF( LWORK.LT.IWS ) THEN
!*
!*              Not enough workspace to use optimal NB:  reduce NB and
!*              determine the minimum value of NB.
!*
               NB = LWORK / LDWORK
               NBMIN = MAX( 2, ILAENV( 2, 'DORGQR', ' ', M, N, K, -1 ) )
            END IF
         END IF
      END IF
!*
      IF( NB.GE.NBMIN .AND. NB.LT.K .AND. NX.LT.K ) THEN
!*
!*        Use blocked code after the last block.
!*        The first kk columns are handled by the block method.
!*
         KI = ( ( K-NX-1 ) / NB )*NB
         KK = MIN( K, KI+NB )
!*
!*        Set A(1:kk,kk+1:n) to zero.
!*
         DO 20 J = KK + 1, N
            DO 10 I = 1, KK
               A( I, J ) = ZERO
   10       CONTINUE
   20    CONTINUE
      ELSE
         KK = 0
      END IF
!*
!*     Use unblocked code for the last or only block.
!*
      IF( KK.LT.N ) &
         CALL DORG2R( M-KK, N-KK, K-KK, A( KK+1, KK+1 ), LDA, &
                      TAU( KK+1 ), WORK, IINFO )
!*
      IF( KK.GT.0 ) THEN
!*
!*        Use blocked code
!*
         DO 50 I = KI + 1, 1, -NB
            IB = MIN( NB, K-I+1 )
            IF( I+IB.LE.N ) THEN
!*
!*              Form the triangular factor of the block reflector
!*              H = H(i) H(i+1) . . . H(i+ib-1)
!*
               CALL DLARFT( 'Forward', 'Columnwise', M-I+1, IB, &
                            A( I, I ), LDA, TAU( I ), WORK, LDWORK )
!*
!*              Apply H to A(i:m,i+ib:n) from the left
!*
               CALL DLARFB( 'Left', 'No transpose', 'Forward', &
                            'Columnwise', M-I+1, N-I-IB+1, IB, &
                            A( I, I ), LDA, WORK, LDWORK, A( I, I+IB ), &
                            LDA, WORK( IB+1 ), LDWORK )
            END IF
!*
!*           Apply H to rows i:m of current block
!*
            CALL DORG2R( M-I+1, IB, IB, A( I, I ), LDA, TAU( I ), WORK, &
                         IINFO )
!*
!*           Set rows 1:i-1 of current block to zero
!*
            DO 40 J = I, I + IB - 1
               DO 30 L = 1, I - 1
                  A( L, J ) = ZERO
   30          CONTINUE
   40       CONTINUE
   50    CONTINUE
      END IF
!*
      WORK( 1 ) = IWS
      RETURN
!*
!*     End of DORGQR
!*
      END

!==========================================================================

!*> \brief \b DLAMCH
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*      DOUBLE PRECISION FUNCTION DLAMCH( CMACH )
!
!       IMPLICIT NONE
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLAMCH determines double precision machine parameters.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] CMACH
!*> \verbatim
!*>          Specifies the value to be returned by DLAMCH:
!*>          = 'E' or 'e',   DLAMCH := eps
!*>          = 'S' or 's ,   DLAMCH := sfmin
!*>          = 'B' or 'b',   DLAMCH := base
!*>          = 'P' or 'p',   DLAMCH := eps*base
!*>          = 'N' or 'n',   DLAMCH := t
!*>          = 'R' or 'r',   DLAMCH := rnd
!*>          = 'M' or 'm',   DLAMCH := emin
!*>          = 'U' or 'u',   DLAMCH := rmin
!*>          = 'L' or 'l',   DLAMCH := emax
!*>          = 'O' or 'o',   DLAMCH := rmax
!*>          where
!*>          eps   = relative machine precision
!*>          sfmin = safe minimum, such that 1/sfmin does not overflow
!*>          base  = base of the machine
!*>          prec  = eps*base
!*>          t     = number of (base) digits in the mantissa
!*>          rnd   = 1.0 when rounding occurs in addition, 0.0 otherwise
!*>          emin  = minimum exponent before (gradual) underflow
!*>          rmin  = underflow threshold - base**(emin-1)
!*>          emax  = largest exponent before overflow
!*>          rmax  = overflow threshold  - (base**emax)*(1-eps)
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      DOUBLE PRECISION FUNCTION DLAMCH( CMACH )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      CHARACTER          CMACH
!*     ..
!*
!*     .. Scalar Arguments ..
!*    DOUBLE PRECISION   A, B
!*     ..
!*
!* =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE, ZERO
      PARAMETER          ( ONE = 1.0D+0, ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      DOUBLE PRECISION   RND, EPS, SFMIN, SMALL, RMACH
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     .. Executable Statements ..
!*
!*
!*     Assume rounding, not chopping. Always.
!*
      RND = ONE
!*
      IF( ONE.EQ.RND ) THEN
         EPS = EPSILON(ZERO) * 0.5
      ELSE
         EPS = EPSILON(ZERO)
      END IF
!*
      IF( LSAME( CMACH, 'E' ) ) THEN
         RMACH = EPS
      ELSE IF( LSAME( CMACH, 'S' ) ) THEN
         SFMIN = TINY(ZERO)
         SMALL = ONE / HUGE(ZERO)
         IF( SMALL.GE.SFMIN ) THEN
!*
!*           Use SMALL plus a bit, to avoid the possibility of rounding
!*           causing overflow when computing  1/sfmin.
!*
            SFMIN = SMALL*( ONE+EPS )
         END IF
         RMACH = SFMIN
      ELSE IF( LSAME( CMACH, 'B' ) ) THEN
         RMACH = RADIX(ZERO)
      ELSE IF( LSAME( CMACH, 'P' ) ) THEN
         RMACH = EPS * RADIX(ZERO)
      ELSE IF( LSAME( CMACH, 'N' ) ) THEN
         RMACH = DIGITS(ZERO)
      ELSE IF( LSAME( CMACH, 'R' ) ) THEN
         RMACH = RND
      ELSE IF( LSAME( CMACH, 'M' ) ) THEN
         RMACH = MINEXPONENT(ZERO)
      ELSE IF( LSAME( CMACH, 'U' ) ) THEN
         RMACH = tiny(zero)
      ELSE IF( LSAME( CMACH, 'L' ) ) THEN
         RMACH = MAXEXPONENT(ZERO)
      ELSE IF( LSAME( CMACH, 'O' ) ) THEN
         RMACH = HUGE(ZERO)
      ELSE
         RMACH = ZERO
      END IF
!*
      DLAMCH = RMACH
      RETURN
!*
!*     End of DLAMCH
!*
      END
!************************************************************************
!*> \brief \b DLAMC3
!*> \details
!*> \b Purpose:
!*> \verbatim
!*> DLAMC3  is intended to force  A  and  B  to be stored prior to doing
!*> the addition of  A  and  B ,  for use in situations where optimizers
!*> might hold one of these in a register.
!*> \endverbatim
!*> \author LAPACK is a software package provided by Univ. of Tennessee,
!*> Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..
!*> \date November 2011
!*> \ingroup auxOTHERauxiliary
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is a DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in] B
!*> \verbatim
!*>          B is a DOUBLE PRECISION
!*>          The values A and B.
!*> \endverbatim
!*>
      DOUBLE PRECISION FUNCTION DLAMC3( A, B )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.0) --
!*     Univ. of Tennessee, Univ. of California Berkeley and NAG Ltd..
!*     November 2010
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION   A, B
!*     ..
!* =====================================================================
!*
!*     .. Executable Statements ..
!*
      DLAMC3 = A + B
!*
      RETURN
!*
!*     End of DLAMC3
!*
      END
!*
!************************************************************************

!==========================================================================

!*> \brief \b DLACPY copies all or part of one two-dimensional array to another.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLACPY + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlacpy.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlacpy.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlacpy.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLACPY( UPLO, M, N, A, LDA, B, LDB )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          UPLO
!*       INTEGER            LDA, LDB, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), B( LDB, * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLACPY copies all or part of a two-dimensional matrix A to another
!*> matrix B.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] UPLO
!*> \verbatim
!*>          UPLO is CHARACTER*1
!*>          Specifies the part of the matrix A to be copied to B.
!*>          = 'U':      Upper triangular part
!*>          = 'L':      Lower triangular part
!*>          Otherwise:  All of the matrix A
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          The m by n matrix A.  If UPLO = 'U', only the upper triangle
!*>          or trapezoid is accessed; if UPLO = 'L', only the lower
!*>          triangle or trapezoid is accessed.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] B
!*> \verbatim
!*>          B is DOUBLE PRECISION array, dimension (LDB,N)
!*>          On exit, B = A in the locations specified by UPLO.
!*> \endverbatim
!*>
!*> \param[in] LDB
!*> \verbatim
!*>          LDB is INTEGER
!*>          The leading dimension of the array B.  LDB >= max(1,M).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      SUBROUTINE DLACPY( UPLO, M, N, A, LDA, B, LDB )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          UPLO
      INTEGER            LDA, LDB, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), B( LDB, * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Local Scalars ..
      INTEGER            I, J
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     .. Executable Statements ..
!*
      IF( LSAME( UPLO, 'U' ) ) THEN
         DO 20 J = 1, N
            DO 10 I = 1, MIN( J, M )
               B( I, J ) = A( I, J )
   10       CONTINUE
   20    CONTINUE
      ELSE IF( LSAME( UPLO, 'L' ) ) THEN
         DO 40 J = 1, N
            DO 30 I = J, M
               B( I, J ) = A( I, J )
   30       CONTINUE
   40    CONTINUE
      ELSE
         DO 60 J = 1, N
            DO 50 I = 1, M
               B( I, J ) = A( I, J )
   50       CONTINUE
   60    CONTINUE
      END IF
      RETURN
!*
!*     End of DLACPY
!*
      END

!==========================================================================

!*> \brief \b DLANGE returns the value of the 1-norm, Frobenius norm, infinity-norm,
!*> or the largest absolute value of any element of a general rectangular matrix.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLANGE + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlange.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlange.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlange.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       DOUBLE PRECISION FUNCTION DLANGE( NORM, M, N, A, LDA, WORK )
!
!        IMPLICIT NONE
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          NORM
!*       INTEGER            LDA, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLANGE  returns the value of the one norm,  or the Frobenius norm, or
!*> the  infinity norm,  or the  element of  largest absolute value  of a
!*> real matrix A.
!*> \endverbatim
!*>
!*> \return DLANGE
!*> \verbatim
!*>
!*>    DLANGE = ( max(abs(A(i,j))), NORM = 'M' or 'm'
!*>             (
!*>             ( norm1(A),         NORM = '1', 'O' or 'o'
!*>             (
!*>             ( normI(A),         NORM = 'I' or 'i'
!*>             (
!*>             ( normF(A),         NORM = 'F', 'f', 'E' or 'e'
!*>
!*> where  norm1  denotes the  one norm of a matrix (maximum column sum),
!*> normI  denotes the  infinity norm  of a matrix  (maximum row sum) and
!*> normF  denotes the  Frobenius norm of a matrix (square root of sum of
!*> squares).  Note that  max(abs(A(i,j)))  is not a consistent matrix norm.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] NORM
!*> \verbatim
!*>          NORM is CHARACTER*1
!*>          Specifies the value to be returned in DLANGE as described
!*>          above.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  M >= 0.  When M = 0,
!*>          DLANGE is set to zero.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  N >= 0.  When N = 0,
!*>          DLANGE is set to zero.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          The m by n matrix A.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(M,1).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK)),
!*>          where LWORK >= M when NORM = 'I'; otherwise, WORK is not
!*>          referenced.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleGEauxiliary
!*
!*  =====================================================================
      DOUBLE PRECISION FUNCTION DLANGE( NORM, M, N, A, LDA, WORK )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          NORM
      INTEGER            LDA, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), WORK( * )
!*     ..
!*
!* =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE, ZERO
      PARAMETER          ( ONE = 1.0D+0, ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I, J
      DOUBLE PRECISION   SCALE, SUM, VALUE, TEMP
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLASSQ
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME, DISNAN
      EXTERNAL           LSAME, DISNAN
!*     ..
!*     .. Executable Statements ..
!*
      IF( MIN( M, N ).EQ.0 ) THEN
         VALUE = ZERO
      ELSE IF( LSAME( NORM, 'M' ) ) THEN
!*
!*        Find max(abs(A(i,j))).
!*
         VALUE = ZERO
         DO 20 J = 1, N
            DO 10 I = 1, M
               TEMP = ABS( A( I, J ) )
               IF( VALUE.LT.TEMP .OR. DISNAN( TEMP ) ) VALUE = TEMP
   10       CONTINUE
   20    CONTINUE
      ELSE IF( ( LSAME( NORM, 'O' ) ) .OR. ( NORM.EQ.'1' ) ) THEN
!*
!*        Find norm1(A).
!*
         VALUE = ZERO
         DO 40 J = 1, N
            SUM = ZERO
            DO 30 I = 1, M
               SUM = SUM + ABS( A( I, J ) )
   30       CONTINUE
            IF( VALUE.LT.SUM .OR. DISNAN( SUM ) ) VALUE = SUM
   40    CONTINUE
      ELSE IF( LSAME( NORM, 'I' ) ) THEN
!*
!*        Find normI(A).
!*
         DO 50 I = 1, M
            WORK( I ) = ZERO
   50    CONTINUE
         DO 70 J = 1, N
            DO 60 I = 1, M
               WORK( I ) = WORK( I ) + ABS( A( I, J ) )
   60       CONTINUE
   70    CONTINUE
         VALUE = ZERO
         DO 80 I = 1, M
            TEMP = WORK( I )
            IF( VALUE.LT.TEMP .OR. DISNAN( TEMP ) ) VALUE = TEMP
   80    CONTINUE
      ELSE IF( ( LSAME( NORM, 'F' ) ) .OR. ( LSAME( NORM, 'E' ) ) ) THEN
!*
!*        Find normF(A).
!*
         SCALE = ZERO
         SUM = ONE
         DO 90 J = 1, N
            CALL DLASSQ( M, A( 1, J ), 1, SCALE, SUM )
   90    CONTINUE
         VALUE = SCALE*SQRT( SUM )
      END IF
!*
      DLANGE = VALUE
      RETURN
!*
!*     End of DLANGE
!*
      END

!==========================================================================

!*> \brief \b DLASCL multiplies a general rectangular matrix by a real scalar defined as cto/cfrom.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASCL + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlascl.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlascl.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlascl.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASCL( TYPE, KL, KU, CFROM, CTO, M, N, A, LDA, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          TYPE
!*       INTEGER            INFO, KL, KU, LDA, M, N
!*       DOUBLE PRECISION   CFROM, CTO
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASCL multiplies the M by N real matrix A by the real scalar
!*> CTO/CFROM.  This is done without over/underflow as long as the final
!*> result CTO*A(I,J)/CFROM does not over/underflow. TYPE specifies that
!*> A may be full, upper triangular, lower triangular, upper Hessenberg,
!*> or banded.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] TYPE
!*> \verbatim
!*>          TYPE is CHARACTER*1
!*>          TYPE indices the storage type of the input matrix.
!*>          = 'G':  A is a full matrix.
!*>          = 'L':  A is a lower triangular matrix.
!*>          = 'U':  A is an upper triangular matrix.
!*>          = 'H':  A is an upper Hessenberg matrix.
!*>          = 'B':  A is a symmetric band matrix with lower bandwidth KL
!*>                  and upper bandwidth KU and with the only the lower
!*>                  half stored.
!*>          = 'Q':  A is a symmetric band matrix with lower bandwidth KL
!*>                  and upper bandwidth KU and with the only the upper
!*>                  half stored.
!*>          = 'Z':  A is a band matrix with lower bandwidth KL and upper
!*>                  bandwidth KU. See DGBTRF for storage details.
!*> \endverbatim
!*>
!*> \param[in] KL
!*> \verbatim
!*>          KL is INTEGER
!*>          The lower bandwidth of A.  Referenced only if TYPE = 'B',
!*>          'Q' or 'Z'.
!*> \endverbatim
!*>
!*> \param[in] KU
!*> \verbatim
!*>          KU is INTEGER
!*>          The upper bandwidth of A.  Referenced only if TYPE = 'B',
!*>          'Q' or 'Z'.
!*> \endverbatim
!*>
!*> \param[in] CFROM
!*> \verbatim
!*>          CFROM is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in] CTO
!*> \verbatim
!*>          CTO is DOUBLE PRECISION
!*>
!*>          The matrix A is multiplied by CTO/CFROM. A(I,J) is computed
!*>          without over/underflow if the final result CTO*A(I,J)/CFROM
!*>          can be represented without over/underflow.  CFROM must be
!*>          nonzero.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          The matrix to be multiplied by CTO/CFROM.  See TYPE for the
!*>          storage type.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          0  - successful exit
!*>          <0 - if INFO = -i, the i-th argument had an illegal value.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      SUBROUTINE DLASCL( TYPE, KL, KU, CFROM, CTO, M, N, A, LDA, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          TYPE
      INTEGER            INFO, KL, KU, LDA, M, N
      DOUBLE PRECISION   CFROM, CTO
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO, ONE
      PARAMETER          ( ZERO = 0.0D0, ONE = 1.0D0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            DONE
      INTEGER            I, ITYPE, J, K1, K2, K3, K4
      DOUBLE PRECISION   BIGNUM, CFROM1, CFROMC, CTO1, CTOC, MUL, SMLNUM
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME, DISNAN
      DOUBLE PRECISION   DLAMCH
      EXTERNAL           LSAME, DLAMCH, DISNAN
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
!*
      IF( LSAME( TYPE, 'G' ) ) THEN
         ITYPE = 0
      ELSE IF( LSAME( TYPE, 'L' ) ) THEN
         ITYPE = 1
      ELSE IF( LSAME( TYPE, 'U' ) ) THEN
         ITYPE = 2
      ELSE IF( LSAME( TYPE, 'H' ) ) THEN
         ITYPE = 3
      ELSE IF( LSAME( TYPE, 'B' ) ) THEN
         ITYPE = 4
      ELSE IF( LSAME( TYPE, 'Q' ) ) THEN
         ITYPE = 5
      ELSE IF( LSAME( TYPE, 'Z' ) ) THEN
         ITYPE = 6
      ELSE
         ITYPE = -1
      END IF
!*
      IF( ITYPE.EQ.-1 ) THEN
         INFO = -1
      ELSE IF( CFROM.EQ.ZERO .OR. DISNAN(CFROM) ) THEN
         INFO = -4
      ELSE IF( DISNAN(CTO) ) THEN
         INFO = -5
      ELSE IF( M.LT.0 ) THEN
         INFO = -6
      ELSE IF( N.LT.0 .OR. ( ITYPE.EQ.4 .AND. N.NE.M ) .OR. &
               ( ITYPE.EQ.5 .AND. N.NE.M ) ) THEN
         INFO = -7
      ELSE IF( ITYPE.LE.3 .AND. LDA.LT.MAX( 1, M ) ) THEN
         INFO = -9
      ELSE IF( ITYPE.GE.4 ) THEN
         IF( KL.LT.0 .OR. KL.GT.MAX( M-1, 0 ) ) THEN
            INFO = -2
         ELSE IF( KU.LT.0 .OR. KU.GT.MAX( N-1, 0 ) .OR. &
                  ( ( ITYPE.EQ.4 .OR. ITYPE.EQ.5 ) .AND. KL.NE.KU ) ) &
                   THEN
            INFO = -3
         ELSE IF( ( ITYPE.EQ.4 .AND. LDA.LT.KL+1 ) .OR. &
                  ( ITYPE.EQ.5 .AND. LDA.LT.KU+1 ) .OR. &
                  ( ITYPE.EQ.6 .AND. LDA.LT.2*KL+KU+1 ) ) THEN
            INFO = -9
         END IF
      END IF
!*
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DLASCL', -INFO )
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( N.EQ.0 .OR. M.EQ.0 ) &
         RETURN
!*
!*     Get machine parameters
!*
      SMLNUM = DLAMCH( 'S' )
      BIGNUM = ONE / SMLNUM
!*
      CFROMC = CFROM
      CTOC = CTO
!*
   10 CONTINUE
      CFROM1 = CFROMC*SMLNUM
      IF( CFROM1.EQ.CFROMC ) THEN
!        CFROMC is an inf.  Multiply by a correctly signed zero for
!        finite CTOC, or a NaN if CTOC is infinite.
         MUL = CTOC / CFROMC
         DONE = .TRUE.
         CTO1 = CTOC
      ELSE
         CTO1 = CTOC / BIGNUM
         IF( CTO1.EQ.CTOC ) THEN
!           CTOC is either 0 or an inf.  In both cases, CTOC itself
!           serves as the correct multiplication factor.
            MUL = CTOC
            DONE = .TRUE.
            CFROMC = ONE
         ELSE IF( ABS( CFROM1 ).GT.ABS( CTOC ) .AND. CTOC.NE.ZERO ) THEN
            MUL = SMLNUM
            DONE = .FALSE.
            CFROMC = CFROM1
         ELSE IF( ABS( CTO1 ).GT.ABS( CFROMC ) ) THEN
            MUL = BIGNUM
            DONE = .FALSE.
            CTOC = CTO1
         ELSE
            MUL = CTOC / CFROMC
            DONE = .TRUE.
         END IF
      END IF
!*
      IF( ITYPE.EQ.0 ) THEN
!*
!*        Full matrix
!*
         DO 30 J = 1, N
            DO 20 I = 1, M
               A( I, J ) = A( I, J )*MUL
   20       CONTINUE
   30    CONTINUE
!*
      ELSE IF( ITYPE.EQ.1 ) THEN
!*
!*        Lower triangular matrix
!*
         DO 50 J = 1, N
            DO 40 I = J, M
               A( I, J ) = A( I, J )*MUL
   40       CONTINUE
   50    CONTINUE
!*
      ELSE IF( ITYPE.EQ.2 ) THEN
!*
!*        Upper triangular matrix
!*
         DO 70 J = 1, N
            DO 60 I = 1, MIN( J, M )
               A( I, J ) = A( I, J )*MUL
   60       CONTINUE
   70    CONTINUE
!*
      ELSE IF( ITYPE.EQ.3 ) THEN
!*
!*        Upper Hessenberg matrix
!*
         DO 90 J = 1, N
            DO 80 I = 1, MIN( J+1, M )
               A( I, J ) = A( I, J )*MUL
   80       CONTINUE
   90    CONTINUE
!*
      ELSE IF( ITYPE.EQ.4 ) THEN
!*
!*        Lower half of a symmetric band matrix
!*
         K3 = KL + 1
         K4 = N + 1
         DO 110 J = 1, N
            DO 100 I = 1, MIN( K3, K4-J )
               A( I, J ) = A( I, J )*MUL
  100       CONTINUE
  110    CONTINUE
!*
      ELSE IF( ITYPE.EQ.5 ) THEN
!*
!*        Upper half of a symmetric band matrix
!*
         K1 = KU + 2
         K3 = KU + 1
         DO 130 J = 1, N
            DO 120 I = MAX( K1-J, 1 ), K3
               A( I, J ) = A( I, J )*MUL
  120       CONTINUE
  130    CONTINUE
!*
      ELSE IF( ITYPE.EQ.6 ) THEN
!*
!*        Band matrix
!*
         K1 = KL + KU + 2
         K2 = KL + 1
         K3 = 2*KL + KU + 1
         K4 = KL + KU + 1 + M
         DO 150 J = 1, N
            DO 140 I = MAX( K1-J, K2 ), MIN( K3, K4-J )
               A( I, J ) = A( I, J )*MUL
  140       CONTINUE
  150    CONTINUE
!*
      END IF
!*
      IF( .NOT.DONE ) &
         GO TO 10
!*
      RETURN
!*
!*     End of DLASCL
!*
      END

!==========================================================================

!*> \brief \b DLASET initializes the off-diagonal elements and the diagonal elements of a matrix to given values.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASET + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlaset.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlaset.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlaset.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASET( UPLO, M, N, ALPHA, BETA, A, LDA )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          UPLO
!*       INTEGER            LDA, M, N
!*       DOUBLE PRECISION   ALPHA, BETA
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASET initializes an m-by-n matrix A to BETA on the diagonal and
!*> ALPHA on the offdiagonals.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] UPLO
!*> \verbatim
!*>          UPLO is CHARACTER*1
!*>          Specifies the part of the matrix A to be set.
!*>          = 'U':      Upper triangular part is set; the strictly lower
!*>                      triangular part of A is not changed.
!*>          = 'L':      Lower triangular part is set; the strictly upper
!*>                      triangular part of A is not changed.
!*>          Otherwise:  All of the matrix A is set.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in] ALPHA
!*> \verbatim
!*>          ALPHA is DOUBLE PRECISION
!*>          The constant to which the offdiagonal elements are to be set.
!*> \endverbatim
!*>
!*> \param[in] BETA
!*> \verbatim
!*>          BETA is DOUBLE PRECISION
!*>          The constant to which the diagonal elements are to be set.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On exit, the leading m-by-n submatrix of A is set as follows:
!*>
!*>          if UPLO = 'U', A(i,j) = ALPHA, 1<=i<=j-1, 1<=j<=n,
!*>          if UPLO = 'L', A(i,j) = ALPHA, j+1<=i<=m, 1<=j<=n,
!*>          otherwise,     A(i,j) = ALPHA, 1<=i<=m, 1<=j<=n, i.ne.j,
!*>
!*>          and, for all UPLO, A(i,i) = BETA, 1<=i<=min(m,n).
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      SUBROUTINE DLASET( UPLO, M, N, ALPHA, BETA, A, LDA )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          UPLO
      INTEGER            LDA, M, N
      DOUBLE PRECISION   ALPHA, BETA
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * )
!*     ..
!*
!* =====================================================================
!*
!*     .. Local Scalars ..
      INTEGER            I, J
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     ..
!*     .. Executable Statements ..
!*
      IF( LSAME( UPLO, 'U' ) ) THEN
!*
!*        Set the strictly upper triangular or trapezoidal part of the
!*        array to ALPHA.
!*
         DO 20 J = 2, N
            DO 10 I = 1, MIN( J-1, M )
               A( I, J ) = ALPHA
   10       CONTINUE
   20    CONTINUE
!*
      ELSE IF( LSAME( UPLO, 'L' ) ) THEN
!*
!*        Set the strictly lower triangular or trapezoidal part of the
!*        array to ALPHA.
!*
         DO 40 J = 1, MIN( M, N )
            DO 30 I = J + 1, M
               A( I, J ) = ALPHA
   30       CONTINUE
   40    CONTINUE
!*
      ELSE
!*
!*        Set the leading m-by-n submatrix to ALPHA.
!*
         DO 60 J = 1, N
            DO 50 I = 1, M
               A( I, J ) = ALPHA
   50       CONTINUE
   60    CONTINUE
      END IF
!*
!*     Set the first min(M,N) diagonal elements to BETA.
!*
      DO 70 I = 1, MIN( M, N )
         A( I, I ) = BETA
   70 CONTINUE
!*
      RETURN
!*
!*     End of DLASET
!*
      END

!==========================================================================

!*> \brief \b DORGBR
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORGBR + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dorgbr.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dorgbr.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dorgbr.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORGBR( VECT, M, N, K, A, LDA, TAU, WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          VECT
!*       INTEGER            INFO, K, LDA, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORGBR generates one of the real orthogonal matrices Q or P**T
!*> determined by DGEBRD when reducing a real matrix A to bidiagonal
!*> form: A = Q * B * P**T.  Q and P**T are defined as products of
!*> elementary reflectors H(i) or G(i) respectively.
!*>
!*> If VECT = 'Q', A is assumed to have been an M-by-K matrix, and Q
!*> is of order M:
!*> if m >= k, Q = H(1) H(2) . . . H(k) and DORGBR returns the first n
!*> columns of Q, where m >= n >= k;
!*> if m < k, Q = H(1) H(2) . . . H(m-1) and DORGBR returns Q as an
!*> M-by-M matrix.
!*>
!*> If VECT = 'P', A is assumed to have been a K-by-N matrix, and P**T
!*> is of order N:
!*> if k < n, P**T = G(k) . . . G(2) G(1) and DORGBR returns the first m
!*> rows of P**T, where n >= m >= k;
!*> if k >= n, P**T = G(n-1) . . . G(2) G(1) and DORGBR returns P**T as
!*> an N-by-N matrix.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] VECT
!*> \verbatim
!*>          VECT is CHARACTER*1
!*>          Specifies whether the matrix Q or the matrix P**T is
!*>          required, as defined in the transformation applied by DGEBRD:
!*>          = 'Q':  generate Q;
!*>          = 'P':  generate P**T.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix Q or P**T to be returned.
!*>          M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix Q or P**T to be returned.
!*>          N >= 0.
!*>          If VECT = 'Q', M >= N >= min(M,K);
!*>          if VECT = 'P', N >= M >= min(N,K).
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          If VECT = 'Q', the number of columns in the original M-by-K
!*>          matrix reduced by DGEBRD.
!*>          If VECT = 'P', the number of rows in the original K-by-N
!*>          matrix reduced by DGEBRD.
!*>          K >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the vectors which define the elementary reflectors,
!*>          as returned by DGEBRD.
!*>          On exit, the M-by-N matrix Q or P**T.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A. LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension
!*>                                (min(M,K)) if VECT = 'Q'
!*>                                (min(N,K)) if VECT = 'P'
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i) or G(i), which determines Q or P**T, as
!*>          returned by DGEBRD in its array argument TAUQ or TAUP.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK. LWORK >= max(1,min(M,N)).
!*>          For optimum performance LWORK >= min(M,N)*NB, where NB
!*>          is the optimal blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date April 2012
!*
!*> \ingroup doubleGBcomputational
!*
!*  =====================================================================
      SUBROUTINE DORGBR( VECT, M, N, K, A, LDA, TAU, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.1) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     April 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          VECT
      INTEGER            INFO, K, LDA, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO, ONE
      PARAMETER          ( ZERO = 0.0D+0, ONE = 1.0D+0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LQUERY, WANTQ
      INTEGER            I, IINFO, J, LWKOPT, MN
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      EXTERNAL           LSAME, ILAENV
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DORGLQ, DORGQR, XERBLA
!*     ..
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      WANTQ = LSAME( VECT, 'Q' )
      MN = MIN( M, N )
      LQUERY = ( LWORK.EQ.-1 )
      IF( .NOT.WANTQ .AND. .NOT.LSAME( VECT, 'P' ) ) THEN
         INFO = -1
      ELSE IF( M.LT.0 ) THEN
         INFO = -2
      ELSE IF( N.LT.0 .OR. ( WANTQ .AND. ( N.GT.M .OR. N.LT.MIN( M, &
               K ) ) ) .OR. ( .NOT.WANTQ .AND. ( M.GT.N .OR. M.LT. &
               MIN( N, K ) ) ) ) THEN
         INFO = -3
      ELSE IF( K.LT.0 ) THEN
         INFO = -4
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -6
      ELSE IF( LWORK.LT.MAX( 1, MN ) .AND. .NOT.LQUERY ) THEN
         INFO = -9
      END IF
!*
      IF( INFO.EQ.0 ) THEN
         WORK( 1 ) = 1
         IF( WANTQ ) THEN
            IF( M.GE.K ) THEN
               CALL DORGQR( M, N, K, A, LDA, TAU, WORK, -1, IINFO )
            ELSE
               IF( M.GT.1 ) THEN
                  CALL DORGQR( M-1, M-1, M-1, A( 2, 2 ), LDA, TAU, WORK, &
                               -1, IINFO )
               END IF
            END IF
         ELSE
            IF( K.LT.N ) THEN
               CALL DORGLQ( M, N, K, A, LDA, TAU, WORK, -1, IINFO )
            ELSE
               IF( N.GT.1 ) THEN
                  CALL DORGLQ( N-1, N-1, N-1, A( 2, 2 ), LDA, TAU, WORK, &
                               -1, IINFO )
               END IF
            END IF
         END IF
         LWKOPT = WORK( 1 )
         LWKOPT = MAX (LWKOPT, MN)
      END IF
!*
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORGBR', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         WORK( 1 ) = LWKOPT
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( M.EQ.0 .OR. N.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!*
      IF( WANTQ ) THEN
!*
!*        Form Q, determined by a call to DGEBRD to reduce an m-by-k
!*        matrix
!*
         IF( M.GE.K ) THEN
!*
!*           If m >= k, assume m >= n >= k
!*
            CALL DORGQR( M, N, K, A, LDA, TAU, WORK, LWORK, IINFO )
!*
         ELSE
!*
!*           If m < k, assume m = n
!*
!*           Shift the vectors which define the elementary reflectors one
!*           column to the right, and set the first row and column of Q
!*           to those of the unit matrix
!*
            DO 20 J = M, 2, -1
               A( 1, J ) = ZERO
               DO 10 I = J + 1, M
                  A( I, J ) = A( I, J-1 )
   10          CONTINUE
   20       CONTINUE
            A( 1, 1 ) = ONE
            DO 30 I = 2, M
               A( I, 1 ) = ZERO
   30       CONTINUE
            IF( M.GT.1 ) THEN
!*
!*              Form Q(2:m,2:m)
!*
               CALL DORGQR( M-1, M-1, M-1, A( 2, 2 ), LDA, TAU, WORK, &
                            LWORK, IINFO )
            END IF
         END IF
      ELSE
!*
!*        Form P**T, determined by a call to DGEBRD to reduce a k-by-n
!*        matrix
!*
         IF( K.LT.N ) THEN
!*
!*           If k < n, assume k <= m <= n
!*
            CALL DORGLQ( M, N, K, A, LDA, TAU, WORK, LWORK, IINFO )
!*
         ELSE
!*
!*           If k >= n, assume m = n
!*
!*           Shift the vectors which define the elementary reflectors one
!*           row downward, and set the first row and column of P**T to
!*           those of the unit matrix
!*
            A( 1, 1 ) = ONE
            DO 40 I = 2, N
               A( I, 1 ) = ZERO
   40       CONTINUE
            DO 60 J = 2, N
               DO 50 I = J - 1, 2, -1
                  A( I, J ) = A( I-1, J )
   50          CONTINUE
               A( 1, J ) = ZERO
   60       CONTINUE
            IF( N.GT.1 ) THEN
!*
!*              Form P**T(2:n,2:n)
!*
               CALL DORGLQ( N-1, N-1, N-1, A( 2, 2 ), LDA, TAU, WORK, &
                            LWORK, IINFO )
            END IF
         END IF
      END IF
      WORK( 1 ) = LWKOPT
      RETURN
!*
!*     End of DORGBR
!*
      END

!==========================================================================

!*> \brief \b DORGLQ
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORGLQ + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dorglq.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dorglq.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dorglq.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORGLQ( M, N, K, A, LDA, TAU, WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, K, LDA, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORGLQ generates an M-by-N real matrix Q with orthonormal rows,
!*> which is defined as the first M rows of a product of K elementary
!*> reflectors of order N
!*>
!*>       Q  =  H(k) . . . H(2) H(1)
!*>
!*> as returned by DGELQF.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix Q. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix Q. N >= M.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The number of elementary reflectors whose product defines the
!*>          matrix Q. M >= K >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the i-th row must contain the vector which defines
!*>          the elementary reflector H(i), for i = 1,2,...,k, as returned
!*>          by DGELQF in the first k rows of its array argument A.
!*>          On exit, the M-by-N matrix Q.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The first dimension of the array A. LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i), as returned by DGELQF.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK. LWORK >= max(1,M).
!*>          For optimum performance LWORK >= M*NB, where NB is
!*>          the optimal blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument has an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORGLQ( M, N, K, A, LDA, TAU, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, K, LDA, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LQUERY
      INTEGER            I, IB, IINFO, IWS, J, KI, KK, L, LDWORK, &
                         LWKOPT, NB, NBMIN, NX
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARFB, DLARFT, DORGL2, XERBLA
!*     ..
!*     ..
!*     .. External Functions ..
      INTEGER            ILAENV
      EXTERNAL           ILAENV
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      NB = ILAENV( 1, 'DORGLQ', ' ', M, N, K, -1 )
      LWKOPT = MAX( 1, M )*NB
      WORK( 1 ) = LWKOPT
      LQUERY = ( LWORK.EQ.-1 )
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.M ) THEN
         INFO = -2
      ELSE IF( K.LT.0 .OR. K.GT.M ) THEN
         INFO = -3
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -5
      ELSE IF( LWORK.LT.MAX( 1, M ) .AND. .NOT.LQUERY ) THEN
         INFO = -8
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORGLQ', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( M.LE.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!*
      NBMIN = 2
      NX = 0
      IWS = M
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
!*
!*        Determine when to cross over from blocked to unblocked code.
!*
         NX = MAX( 0, ILAENV( 3, 'DORGLQ', ' ', M, N, K, -1 ) )
         IF( NX.LT.K ) THEN
!*
!*           Determine if workspace is large enough for blocked code.
!*
            LDWORK = M
            IWS = LDWORK*NB
            IF( LWORK.LT.IWS ) THEN
!*
!*              Not enough workspace to use optimal NB:  reduce NB and
!*              determine the minimum value of NB.
!*
               NB = LWORK / LDWORK
               NBMIN = MAX( 2, ILAENV( 2, 'DORGLQ', ' ', M, N, K, -1 ) )
            END IF
         END IF
      END IF
!*
      IF( NB.GE.NBMIN .AND. NB.LT.K .AND. NX.LT.K ) THEN
!*
!*        Use blocked code after the last block.
!*        The first kk rows are handled by the block method.
!*
         KI = ( ( K-NX-1 ) / NB )*NB
         KK = MIN( K, KI+NB )
!*
!*        Set A(kk+1:m,1:kk) to zero.
!*
         DO 20 J = 1, KK
            DO 10 I = KK + 1, M
               A( I, J ) = ZERO
   10       CONTINUE
   20    CONTINUE
      ELSE
         KK = 0
      END IF
!*
!*     Use unblocked code for the last or only block.
!*
      IF( KK.LT.M ) &
         CALL DORGL2( M-KK, N-KK, K-KK, A( KK+1, KK+1 ), LDA, &
                      TAU( KK+1 ), WORK, IINFO )
!*
      IF( KK.GT.0 ) THEN
!*
!*        Use blocked code
!*
         DO 50 I = KI + 1, 1, -NB
            IB = MIN( NB, K-I+1 )
            IF( I+IB.LE.M ) THEN
!*
!*              Form the triangular factor of the block reflector
!*              H = H(i) H(i+1) . . . H(i+ib-1)
!*
               CALL DLARFT( 'Forward', 'Rowwise', N-I+1, IB, A( I, I ), &
                            LDA, TAU( I ), WORK, LDWORK )
!*
!*              Apply H**T to A(i+ib:m,i:n) from the right
!*
               CALL DLARFB( 'Right', 'Transpose', 'Forward', 'Rowwise', &
                            M-I-IB+1, N-I+1, IB, A( I, I ), LDA, WORK, &
                            LDWORK, A( I+IB, I ), LDA, WORK( IB+1 ), &
                            LDWORK )
            END IF
!*
!*           Apply H**T to columns i:n of current block
!*
            CALL DORGL2( IB, N-I+1, IB, A( I, I ), LDA, TAU( I ), WORK, &
                         IINFO )
!*
!*           Set columns 1:i-1 of current block to zero
!*
            DO 40 J = 1, I - 1
               DO 30 L = I, I + IB - 1
                  A( L, J ) = ZERO
   30          CONTINUE
   40       CONTINUE
   50    CONTINUE
      END IF
!*
      WORK( 1 ) = IWS
      RETURN
!*
!*     End of DORGLQ
!*
      END

!==========================================================================

!*> \brief \b DORMBR
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORMBR + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dormbr.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dormbr.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dormbr.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORMBR( VECT, SIDE, TRANS, M, N, K, A, LDA, TAU, C,
!*                          LDC, WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          SIDE, TRANS, VECT
!*       INTEGER            INFO, K, LDA, LDC, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> If VECT = 'Q', DORMBR overwrites the general real M-by-N matrix C
!*> with
!*>                 SIDE = 'L'     SIDE = 'R'
!*> TRANS = 'N':      Q * C          C * Q
!*> TRANS = 'T':      Q**T * C       C * Q**T
!*>
!*> If VECT = 'P', DORMBR overwrites the general real M-by-N matrix C
!*> with
!*>                 SIDE = 'L'     SIDE = 'R'
!*> TRANS = 'N':      P * C          C * P
!*> TRANS = 'T':      P**T * C       C * P**T
!*>
!*> Here Q and P**T are the orthogonal matrices determined by DGEBRD when
!*> reducing a real matrix A to bidiagonal form: A = Q * B * P**T. Q and
!*> P**T are defined as products of elementary reflectors H(i) and G(i)
!*> respectively.
!*>
!*> Let nq = m if SIDE = 'L' and nq = n if SIDE = 'R'. Thus nq is the
!*> order of the orthogonal matrix Q or P**T that is applied.
!*>
!*> If VECT = 'Q', A is assumed to have been an NQ-by-K matrix:
!*> if nq >= k, Q = H(1) H(2) . . . H(k);
!*> if nq < k, Q = H(1) H(2) . . . H(nq-1).
!*>
!*> If VECT = 'P', A is assumed to have been a K-by-NQ matrix:
!*> if k < nq, P = G(1) G(2) . . . G(k);
!*> if k >= nq, P = G(1) G(2) . . . G(nq-1).
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] VECT
!*> \verbatim
!*>          VECT is CHARACTER*1
!*>          = 'Q': apply Q or Q**T;
!*>          = 'P': apply P or P**T.
!*> \endverbatim
!*>
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>          = 'L': apply Q, Q**T, P or P**T from the Left;
!*>          = 'R': apply Q, Q**T, P or P**T from the Right.
!*> \endverbatim
!*>
!*> \param[in] TRANS
!*> \verbatim
!*>          TRANS is CHARACTER*1
!*>          = 'N':  No transpose, apply Q  or P;
!*>          = 'T':  Transpose, apply Q**T or P**T.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix C. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix C. N >= 0.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          If VECT = 'Q', the number of columns in the original
!*>          matrix reduced by DGEBRD.
!*>          If VECT = 'P', the number of rows in the original
!*>          matrix reduced by DGEBRD.
!*>          K >= 0.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension
!*>                                (LDA,min(nq,K)) if VECT = 'Q'
!*>                                (LDA,nq)        if VECT = 'P'
!*>          The vectors which define the elementary reflectors H(i) and
!*>          G(i), whose products determine the matrices Q and P, as
!*>          returned by DGEBRD.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.
!*>          If VECT = 'Q', LDA >= max(1,nq);
!*>          if VECT = 'P', LDA >= max(1,min(nq,K)).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (min(nq,K))
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i) or G(i) which determines Q or P, as returned
!*>          by DGEBRD in the array argument TAUQ or TAUP.
!*> \endverbatim
!*>
!*> \param[in,out] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension (LDC,N)
!*>          On entry, the M-by-N matrix C.
!*>          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q
!*>          or P*C or P**T*C or C*P or C*P**T.
!*> \endverbatim
!*>
!*> \param[in] LDC
!*> \verbatim
!*>          LDC is INTEGER
!*>          The leading dimension of the array C. LDC >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK.
!*>          If SIDE = 'L', LWORK >= max(1,N);
!*>          if SIDE = 'R', LWORK >= max(1,M).
!*>          For optimum performance LWORK >= N*NB if SIDE = 'L', and
!*>          LWORK >= M*NB if SIDE = 'R', where NB is the optimal
!*>          blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORMBR( VECT, SIDE, TRANS, M, N, K, A, LDA, TAU, C, &
                         LDC, WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS, VECT
      INTEGER            INFO, K, LDA, LDC, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Local Scalars ..
      LOGICAL            APPLYQ, LEFT, LQUERY, NOTRAN
      CHARACTER          TRANST
      INTEGER            I1, I2, IINFO, LWKOPT, MI, NB, NI, NQ, NW
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      EXTERNAL           LSAME, ILAENV
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DORMLQ, DORMQR, XERBLA
!*     ..
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      APPLYQ = LSAME( VECT, 'Q' )
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
      LQUERY = ( LWORK.EQ.-1 )
!*
!*     NQ is the order of Q or P and NW is the minimum dimension of WORK
!*
      IF( LEFT ) THEN
         NQ = M
         NW = N
      ELSE
         NQ = N
         NW = M
      END IF
      IF( .NOT.APPLYQ .AND. .NOT.LSAME( VECT, 'P' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -2
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -3
      ELSE IF( M.LT.0 ) THEN
         INFO = -4
      ELSE IF( N.LT.0 ) THEN
         INFO = -5
      ELSE IF( K.LT.0 ) THEN
         INFO = -6
      ELSE IF( ( APPLYQ .AND. LDA.LT.MAX( 1, NQ ) ) .OR. &
               ( .NOT.APPLYQ .AND. LDA.LT.MAX( 1, MIN( NQ, K ) ) ) ) &
                THEN
         INFO = -8
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -11
      ELSE IF( LWORK.LT.MAX( 1, NW ) .AND. .NOT.LQUERY ) THEN
         INFO = -13
      END IF
!*
      IF( INFO.EQ.0 ) THEN
         IF( APPLYQ ) THEN
            IF( LEFT ) THEN
               NB = ILAENV( 1, 'DORMQR', SIDE // TRANS, M-1, N, M-1, &
                    -1 )
            ELSE
               NB = ILAENV( 1, 'DORMQR', SIDE // TRANS, M, N-1, N-1, &
                    -1 )
            END IF
         ELSE
            IF( LEFT ) THEN
               NB = ILAENV( 1, 'DORMLQ', SIDE // TRANS, M-1, N, M-1, &
                    -1 )
            ELSE
               NB = ILAENV( 1, 'DORMLQ', SIDE // TRANS, M, N-1, N-1, &
                    -1 )
            END IF
         END IF
         LWKOPT = MAX( 1, NW )*NB
         WORK( 1 ) = LWKOPT
      END IF
!*
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORMBR', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      WORK( 1 ) = 1
      IF( M.EQ.0 .OR. N.EQ.0 ) &
         RETURN
!*
      IF( APPLYQ ) THEN
!*
!*        Apply Q
!*
         IF( NQ.GE.K ) THEN
!*
!*           Q was determined by a call to DGEBRD with nq >= k
!*
            CALL DORMQR( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, LWORK, IINFO )
         ELSE IF( NQ.GT.1 ) THEN
!*
!*           Q was determined by a call to DGEBRD with nq < k
!*
            IF( LEFT ) THEN
               MI = M - 1
               NI = N
               I1 = 2
               I2 = 1
            ELSE
               MI = M
               NI = N - 1
               I1 = 1
               I2 = 2
            END IF
            CALL DORMQR( SIDE, TRANS, MI, NI, NQ-1, A( 2, 1 ), LDA, TAU, &
                         C( I1, I2 ), LDC, WORK, LWORK, IINFO )
         END IF
      ELSE
!*
!*        Apply P
!*
         IF( NOTRAN ) THEN
            TRANST = 'T'
         ELSE
            TRANST = 'N'
         END IF
         IF( NQ.GT.K ) THEN
!*
!*           P was determined by a call to DGEBRD with nq > k
!*
            CALL DORMLQ( SIDE, TRANST, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, LWORK, IINFO )
         ELSE IF( NQ.GT.1 ) THEN
!*
!*           P was determined by a call to DGEBRD with nq <= k
!*
            IF( LEFT ) THEN
               MI = M - 1
               NI = N
               I1 = 2
               I2 = 1
            ELSE
               MI = M
               NI = N - 1
               I1 = 1
               I2 = 2
            END IF
            CALL DORMLQ( SIDE, TRANST, MI, NI, NQ-1, A( 1, 2 ), LDA, &
                         TAU, C( I1, I2 ), LDC, WORK, LWORK, IINFO )
         END IF
      END IF
      WORK( 1 ) = LWKOPT
      RETURN
!*
!*     End of DORMBR
!*
      END

!==========================================================================

!*> \brief \b DISNAN tests input for NaN.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DISNAN + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/disnan.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/disnan.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/disnan.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       LOGICAL FUNCTION DISNAN( DIN )
!
!        IMPLICIT NONE
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION   DIN
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DISNAN returns .TRUE. if its argument is NaN, and .FALSE.
!*> otherwise.  To be replaced by the Fortran 2003 intrinsic in the
!*> future.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] DIN
!*> \verbatim
!*>          DIN is DOUBLE PRECISION
!*>          Input to test for NaN.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      LOGICAL FUNCTION DISNAN( DIN )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION   DIN
!*     ..
!*
!*  =====================================================================
!*
!*  .. External Functions ..
      LOGICAL DLAISNAN
      EXTERNAL DLAISNAN
!*  ..
!*  .. Executable Statements ..
      DISNAN = DLAISNAN(DIN,DIN)
      RETURN
      END

!==========================================================================

!*> \brief \b DORGL2
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORGL2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dorgl2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dorgl2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dorgl2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORGL2( M, N, K, A, LDA, TAU, WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, K, LDA, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORGL2 generates an m by n real matrix Q with orthonormal rows,
!*> which is defined as the first m rows of a product of k elementary
!*> reflectors of order n
!*>
!*>       Q  =  H(k) . . . H(2) H(1)
!*>
!*> as returned by DGELQF.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix Q. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix Q. N >= M.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The number of elementary reflectors whose product defines the
!*>          matrix Q. M >= K >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the i-th row must contain the vector which defines
!*>          the elementary reflector H(i), for i = 1,2,...,k, as returned
!*>          by DGELQF in the first k rows of its array argument A.
!*>          On exit, the m-by-n matrix Q.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The first dimension of the array A. LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i), as returned by DGELQF.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (M)
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0: successful exit
!*>          < 0: if INFO = -i, the i-th argument has an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORGL2( M, N, K, A, LDA, TAU, WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, K, LDA, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE, ZERO
      PARAMETER          ( ONE = 1.0D+0, ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I, J, L
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARF, DSCAL, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.M ) THEN
         INFO = -2
      ELSE IF( K.LT.0 .OR. K.GT.M ) THEN
         INFO = -3
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -5
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORGL2', -INFO )
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( M.LE.0 ) &
         RETURN
!*
      IF( K.LT.M ) THEN
!*
!*        Initialise rows k+1:m to rows of the unit matrix
!*
         DO 20 J = 1, N
            DO 10 L = K + 1, M
               A( L, J ) = ZERO
   10       CONTINUE
            IF( J.GT.K .AND. J.LE.M ) &
               A( J, J ) = ONE
   20    CONTINUE
      END IF
!*
      DO 40 I = K, 1, -1
!*
!*        Apply H(i) to A(i:m,i:n) from the right
!*
         IF( I.LT.N ) THEN
            IF( I.LT.M ) THEN
               A( I, I ) = ONE
               CALL DLARF( 'Right', M-I, N-I+1, A( I, I ), LDA, &
                           TAU( I ), A( I+1, I ), LDA, WORK )
            END IF
            CALL DSCAL( N-I, -TAU( I ), A( I, I+1 ), LDA )
         END IF
         A( I, I ) = ONE - TAU( I )
!*
!*        Set A(i,1:i-1) to zero
!*
         DO 30 L = 1, I - 1
            A( I, L ) = ZERO
   30    CONTINUE
   40 CONTINUE
      RETURN
!*
!*     End of DORGL2
!*
      END

!==========================================================================

!*> \brief \b DLARFT forms the triangular factor T of a block reflector H = I - vtvH
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLARFT + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlarft.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlarft.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlarft.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLARFT( DIRECT, STOREV, N, K, V, LDV, TAU, T, LDT )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          DIRECT, STOREV
!*       INTEGER            K, LDT, LDV, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   T( LDT, * ), TAU( * ), V( LDV, * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLARFT forms the triangular factor T of a real block reflector H
!*> of order n, which is defined as a product of k elementary reflectors.
!*>
!*> If DIRECT = 'F', H = H(1) H(2) . . . H(k) and T is upper triangular;
!*>
!*> If DIRECT = 'B', H = H(k) . . . H(2) H(1) and T is lower triangular.
!*>
!*> If STOREV = 'C', the vector which defines the elementary reflector
!*> H(i) is stored in the i-th column of the array V, and
!*>
!*>    H  =  I - V * T * V**T
!*>
!*> If STOREV = 'R', the vector which defines the elementary reflector
!*> H(i) is stored in the i-th row of the array V, and
!*>
!*>    H  =  I - V**T * T * V
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] DIRECT
!*> \verbatim
!*>          DIRECT is CHARACTER*1
!*>          Specifies the order in which the elementary reflectors are
!*>          multiplied to form the block reflector:
!*>          = 'F': H = H(1) H(2) . . . H(k) (Forward)
!*>          = 'B': H = H(k) . . . H(2) H(1) (Backward)
!*> \endverbatim
!*>
!*> \param[in] STOREV
!*> \verbatim
!*>          STOREV is CHARACTER*1
!*>          Specifies how the vectors which define the elementary
!*>          reflectors are stored (see also Further Details):
!*>          = 'C': columnwise
!*>          = 'R': rowwise
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The order of the block reflector H. N >= 0.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The order of the triangular factor T (= the number of
!*>          elementary reflectors). K >= 1.
!*> \endverbatim
!*>
!*> \param[in] V
!*> \verbatim
!*>          V is DOUBLE PRECISION array, dimension
!*>                               (LDV,K) if STOREV = 'C'
!*>                               (LDV,N) if STOREV = 'R'
!*>          The matrix V. See further details.
!*> \endverbatim
!*>
!*> \param[in] LDV
!*> \verbatim
!*>          LDV is INTEGER
!*>          The leading dimension of the array V.
!*>          If STOREV = 'C', LDV >= max(1,N); if STOREV = 'R', LDV >= K.
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i).
!*> \endverbatim
!*>
!*> \param[out] T
!*> \verbatim
!*>          T is DOUBLE PRECISION array, dimension (LDT,K)
!*>          The k by k triangular factor T of the block reflector.
!*>          If DIRECT = 'F', T is upper triangular; if DIRECT = 'B', T is
!*>          lower triangular. The rest of the array is not used.
!*> \endverbatim
!*>
!*> \param[in] LDT
!*> \verbatim
!*>          LDT is INTEGER
!*>          The leading dimension of the array T. LDT >= K.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleOTHERauxiliary
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The shape of the matrix V and the storage of the vectors which define
!*>  the H(i) is best illustrated by the following example with n = 5 and
!*>  k = 3. The elements equal to 1 are not stored.
!*>
!*>  DIRECT = 'F' and STOREV = 'C':         DIRECT = 'F' and STOREV = 'R':
!*>
!*>               V = (  1       )                 V = (  1 v1 v1 v1 v1 )
!*>                   ( v1  1    )                     (     1 v2 v2 v2 )
!*>                   ( v1 v2  1 )                     (        1 v3 v3 )
!*>                   ( v1 v2 v3 )
!*>                   ( v1 v2 v3 )
!*>
!*>  DIRECT = 'B' and STOREV = 'C':         DIRECT = 'B' and STOREV = 'R':
!*>
!*>               V = ( v1 v2 v3 )                 V = ( v1 v1  1       )
!*>                   ( v1 v2 v3 )                     ( v2 v2 v2  1    )
!*>                   (  1 v2 v3 )                     ( v3 v3 v3 v3  1 )
!*>                   (     1 v3 )
!*>                   (        1 )
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DLARFT( DIRECT, STOREV, N, K, V, LDV, TAU, T, LDT )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          DIRECT, STOREV
      INTEGER            K, LDT, LDV, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   T( LDT, * ), TAU( * ), V( LDV, * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE, ZERO
      PARAMETER          ( ONE = 1.0D+0, ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I, J, PREVLASTV, LASTV
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DGEMV, DTRMV
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     .. Executable Statements ..
!*
!*     Quick return if possible
!*
      IF( N.EQ.0 ) &
         RETURN
!*
      IF( LSAME( DIRECT, 'F' ) ) THEN
         PREVLASTV = N
         DO I = 1, K
            PREVLASTV = MAX( I, PREVLASTV )
            IF( TAU( I ).EQ.ZERO ) THEN
!*
!*              H(i)  =  I
!*
               DO J = 1, I
                  T( J, I ) = ZERO
               END DO
            ELSE
!*
!*              general case
!*
               IF( LSAME( STOREV, 'C' ) ) THEN
!*                 Skip any trailing zeros.
                  DO LASTV = N, I+1, -1
                     IF( V( LASTV, I ).NE.ZERO ) EXIT
                  END DO
                  DO J = 1, I-1
                     T( J, I ) = -TAU( I ) * V( I , J )
                  END DO
                  J = MIN( LASTV, PREVLASTV )
!*
!*                 T(1:i-1,i) := - tau(i) * V(i:j,1:i-1)**T * V(i:j,i)
!*
                  CALL DGEMV( 'Transpose', J-I, I-1, -TAU( I ),  &
                              V( I+1, 1 ), LDV, V( I+1, I ), 1, ONE,  &
                              T( 1, I ), 1 )
               ELSE
!*                 Skip any trailing zeros.
                  DO LASTV = N, I+1, -1
                     IF( V( I, LASTV ).NE.ZERO ) EXIT
                  END DO
                  DO J = 1, I-1
                     T( J, I ) = -TAU( I ) * V( J , I )
                  END DO
                  J = MIN( LASTV, PREVLASTV )
!*
!*                 T(1:i-1,i) := - tau(i) * V(1:i-1,i:j) * V(i,i:j)**T
!*
                  CALL DGEMV( 'No transpose', I-1, J-I, -TAU( I ), &
                              V( 1, I+1 ), LDV, V( I, I+1 ), LDV, ONE, &
                              T( 1, I ), 1 )
               END IF
!*
!*              T(1:i-1,i) := T(1:i-1,1:i-1) * T(1:i-1,i)
!*
               CALL DTRMV( 'Upper', 'No transpose', 'Non-unit', I-1, T, &
                           LDT, T( 1, I ), 1 )
               T( I, I ) = TAU( I )
               IF( I.GT.1 ) THEN
                  PREVLASTV = MAX( PREVLASTV, LASTV )
               ELSE
                  PREVLASTV = LASTV
               END IF
            END IF
         END DO
      ELSE
         PREVLASTV = 1
         DO I = K, 1, -1
            IF( TAU( I ).EQ.ZERO ) THEN
!*
!*              H(i)  =  I
!*
               DO J = I, K
                  T( J, I ) = ZERO
               END DO
            ELSE
!*
!*              general case
!*
               IF( I.LT.K ) THEN
                  IF( LSAME( STOREV, 'C' ) ) THEN
!*                    Skip any leading zeros.
                     DO LASTV = 1, I-1
                        IF( V( LASTV, I ).NE.ZERO ) EXIT
                     END DO
                     DO J = I+1, K
                        T( J, I ) = -TAU( I ) * V( N-K+I , J )
                     END DO
                     J = MAX( LASTV, PREVLASTV )
!*
!*                    T(i+1:k,i) = -tau(i) * V(j:n-k+i,i+1:k)**T * V(j:n-k+i,i)
!*
                     CALL DGEMV( 'Transpose', N-K+I-J, K-I, -TAU( I ), &
                                 V( J, I+1 ), LDV, V( J, I ), 1, ONE, &
                                 T( I+1, I ), 1 )
                  ELSE
!*                    Skip any leading zeros.
                     DO LASTV = 1, I-1
                        IF( V( I, LASTV ).NE.ZERO ) EXIT
                     END DO
                     DO J = I+1, K
                        T( J, I ) = -TAU( I ) * V( J, N-K+I )
                     END DO
                     J = MAX( LASTV, PREVLASTV )
!*
!*                    T(i+1:k,i) = -tau(i) * V(i+1:k,j:n-k+i) * V(i,j:n-k+i)**T
!*
                     CALL DGEMV( 'No transpose', K-I, N-K+I-J, &
                          -TAU( I ), V( I+1, J ), LDV, V( I, J ), LDV, &
                          ONE, T( I+1, I ), 1 )
                  END IF
!*
!*                 T(i+1:k,i) := T(i+1:k,i+1:k) * T(i+1:k,i)
!*
                  CALL DTRMV( 'Lower', 'No transpose', 'Non-unit', K-I, &
                              T( I+1, I+1 ), LDT, T( I+1, I ), 1 )
                  IF( I.GT.1 ) THEN
                     PREVLASTV = MIN( PREVLASTV, LASTV )
                  ELSE
                     PREVLASTV = LASTV
                  END IF
               END IF
               T( I, I ) = TAU( I )
            END IF
         END DO
      END IF
      RETURN
!*
!*     End of DLARFT
!*
      END

!==========================================================================

!*> \brief \b DLARFB applies a block reflector or its transpose to a general rectangular matrix.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLARFB + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlarfb.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlarfb.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlarfb.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLARFB( SIDE, TRANS, DIRECT, STOREV, M, N, K, V, LDV,
!*                          T, LDT, C, LDC, WORK, LDWORK )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          DIRECT, SIDE, STOREV, TRANS
!*       INTEGER            K, LDC, LDT, LDV, LDWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   C( LDC, * ), T( LDT, * ), V( LDV, * ), &
!*                          WORK( LDWORK, * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLARFB applies a real block reflector H or its transpose H**T to a
!*> real m by n matrix C, from either the left or the right.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>          = 'L': apply H or H**T from the Left
!*>          = 'R': apply H or H**T from the Right
!*> \endverbatim
!*>
!*> \param[in] TRANS
!*> \verbatim
!*>          TRANS is CHARACTER*1
!*>          = 'N': apply H (No transpose)
!*>          = 'T': apply H**T (Transpose)
!*> \endverbatim
!*>
!*> \param[in] DIRECT
!*> \verbatim
!*>          DIRECT is CHARACTER*1
!*>          Indicates how H is formed from a product of elementary
!*>          reflectors
!*>          = 'F': H = H(1) H(2) . . . H(k) (Forward)
!*>          = 'B': H = H(k) . . . H(2) H(1) (Backward)
!*> \endverbatim
!*>
!*> \param[in] STOREV
!*> \verbatim
!*>          STOREV is CHARACTER*1
!*>          Indicates how the vectors which define the elementary
!*>          reflectors are stored:
!*>          = 'C': Columnwise
!*>          = 'R': Rowwise
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix C.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix C.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The order of the matrix T (= the number of elementary
!*>          reflectors whose product defines the block reflector).
!*> \endverbatim
!*>
!*> \param[in] V
!*> \verbatim
!*>          V is DOUBLE PRECISION array, dimension
!*>                                (LDV,K) if STOREV = 'C'
!*>                                (LDV,M) if STOREV = 'R' and SIDE = 'L'
!*>                                (LDV,N) if STOREV = 'R' and SIDE = 'R'
!*>          The matrix V. See Further Details.
!*> \endverbatim
!*>
!*> \param[in] LDV
!*> \verbatim
!*>          LDV is INTEGER
!*>          The leading dimension of the array V.
!*>          If STOREV = 'C' and SIDE = 'L', LDV >= max(1,M);
!*>          if STOREV = 'C' and SIDE = 'R', LDV >= max(1,N);
!*>          if STOREV = 'R', LDV >= K.
!*> \endverbatim
!*>
!*> \param[in] T
!*> \verbatim
!*>          T is DOUBLE PRECISION array, dimension (LDT,K)
!*>          The triangular k by k matrix T in the representation of the
!*>          block reflector.
!*> \endverbatim
!*>
!*> \param[in] LDT
!*> \verbatim
!*>          LDT is INTEGER
!*>          The leading dimension of the array T. LDT >= K.
!*> \endverbatim
!*>
!*> \param[in,out] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension (LDC,N)
!*>          On entry, the m by n matrix C.
!*>          On exit, C is overwritten by H*C or H**T*C or C*H or C*H**T.
!*> \endverbatim
!*>
!*> \param[in] LDC
!*> \verbatim
!*>          LDC is INTEGER
!*>          The leading dimension of the array C. LDC >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (LDWORK,K)
!*> \endverbatim
!*>
!*> \param[in] LDWORK
!*> \verbatim
!*>          LDWORK is INTEGER
!*>          The leading dimension of the array WORK.
!*>          If SIDE = 'L', LDWORK >= max(1,N);
!*>          if SIDE = 'R', LDWORK >= max(1,M).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date June 2013
!*
!*> \ingroup doubleOTHERauxiliary
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The shape of the matrix V and the storage of the vectors which define
!*>  the H(i) is best illustrated by the following example with n = 5 and
!*>  k = 3. The elements equal to 1 are not stored; the corresponding
!*>  array elements are modified but restored on exit. The rest of the
!*>  array is not used.
!*>
!*>  DIRECT = 'F' and STOREV = 'C':         DIRECT = 'F' and STOREV = 'R':
!*>
!*>               V = (  1       )                 V = (  1 v1 v1 v1 v1 )
!*>                   ( v1  1    )                     (     1 v2 v2 v2 )
!*>                   ( v1 v2  1 )                     (        1 v3 v3 )
!*>                   ( v1 v2 v3 )
!*>                   ( v1 v2 v3 )
!*>
!*>  DIRECT = 'B' and STOREV = 'C':         DIRECT = 'B' and STOREV = 'R':
!*>
!*>               V = ( v1 v2 v3 )                 V = ( v1 v1  1       )
!*>                   ( v1 v2 v3 )                     ( v2 v2 v2  1    )
!*>                   (  1 v2 v3 )                     ( v3 v3 v3 v3  1 )
!*>                   (     1 v3 )
!*>                   (        1 )
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DLARFB( SIDE, TRANS, DIRECT, STOREV, M, N, K, V, LDV, &
                         T, LDT, C, LDC, WORK, LDWORK )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.5.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     June 2013
!*
!*     .. Scalar Arguments ..
      CHARACTER          DIRECT, SIDE, STOREV, TRANS
      INTEGER            K, LDC, LDT, LDV, LDWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   C( LDC, * ), T( LDT, * ), V( LDV, * ), &
                         WORK( LDWORK, * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D+0 )
!*     ..
!*     .. Local Scalars ..
      CHARACTER          TRANST
      INTEGER            I, J
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DCOPY, DGEMM, DTRMM
!*     ..
!*     .. Executable Statements ..
!*
!*     Quick return if possible
!*
      IF( M.LE.0 .OR. N.LE.0 ) &
         RETURN
!*
      IF( LSAME( TRANS, 'N' ) ) THEN
         TRANST = 'T'
      ELSE
         TRANST = 'N'
      END IF
!*
      IF( LSAME( STOREV, 'C' ) ) THEN
!*
         IF( LSAME( DIRECT, 'F' ) ) THEN
!*
!*           Let  V =  ( V1 )    (first K rows)
!*                     ( V2 )
!*           where  V1  is unit lower triangular.
!*
            IF( LSAME( SIDE, 'L' ) ) THEN
!*
!*              Form  H * C  or  H**T * C  where  C = ( C1 )
!*                                                    ( C2 )
!*
!*              W := C**T * V  =  (C1**T * V1 + C2**T * V2)  (stored in WORK)
!*
!*              W := C1**T
!*
               DO 10 J = 1, K
                  CALL DCOPY( N, C( J, 1 ), LDC, WORK( 1, J ), 1 )
   10          CONTINUE
!*
!*              W := W * V1
!*
               CALL DTRMM( 'Right', 'Lower', 'No transpose', 'Unit', N, &
                           K, ONE, V, LDV, WORK, LDWORK )
               IF( M.GT.K ) THEN
!*
!*                 W := W + C2**T * V2
!*
                  CALL DGEMM( 'Transpose', 'No transpose', N, K, M-K, &
                              ONE, C( K+1, 1 ), LDC, V( K+1, 1 ), LDV, &
                              ONE, WORK, LDWORK )
               END IF
!*
!*              W := W * T**T  or  W * T
!*
               CALL DTRMM( 'Right', 'Upper', TRANST, 'Non-unit', N, K, &
                           ONE, T, LDT, WORK, LDWORK )
!*
!*              C := C - V * W**T
!*
               IF( M.GT.K ) THEN
!*
!*                 C2 := C2 - V2 * W**T
!*
                  CALL DGEMM( 'No transpose', 'Transpose', M-K, N, K, &
                              -ONE, V( K+1, 1 ), LDV, WORK, LDWORK, ONE, &
                              C( K+1, 1 ), LDC )
               END IF
!*
!*              W := W * V1**T
!*
               CALL DTRMM( 'Right', 'Lower', 'Transpose', 'Unit', N, K, &
                           ONE, V, LDV, WORK, LDWORK )
!*
!*              C1 := C1 - W**T
!*
               DO 30 J = 1, K
                  DO 20 I = 1, N
                     C( J, I ) = C( J, I ) - WORK( I, J )
   20             CONTINUE
   30          CONTINUE
!*
            ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!*
!*              Form  C * H  or  C * H**T  where  C = ( C1  C2 )
!*
!*              W := C * V  =  (C1*V1 + C2*V2)  (stored in WORK)
!*
!*              W := C1
!*
               DO 40 J = 1, K
                  CALL DCOPY( M, C( 1, J ), 1, WORK( 1, J ), 1 )
   40          CONTINUE
!*
!*              W := W * V1
!*
               CALL DTRMM( 'Right', 'Lower', 'No transpose', 'Unit', M, &
                           K, ONE, V, LDV, WORK, LDWORK )
               IF( N.GT.K ) THEN
!*
!*                 W := W + C2 * V2
!*
                  CALL DGEMM( 'No transpose', 'No transpose', M, K, N-K, &
                              ONE, C( 1, K+1 ), LDC, V( K+1, 1 ), LDV, &
                              ONE, WORK, LDWORK )
               END IF
!*
!*              W := W * T  or  W * T**T
!*
               CALL DTRMM( 'Right', 'Upper', TRANS, 'Non-unit', M, K, &
                           ONE, T, LDT, WORK, LDWORK )
!*
!*              C := C - W * V**T
!*
               IF( N.GT.K ) THEN
!*
!*                 C2 := C2 - W * V2**T
!*
                  CALL DGEMM( 'No transpose', 'Transpose', M, N-K, K, &
                              -ONE, WORK, LDWORK, V( K+1, 1 ), LDV, ONE, &
                              C( 1, K+1 ), LDC )
               END IF
!*
!*              W := W * V1**T
!*
               CALL DTRMM( 'Right', 'Lower', 'Transpose', 'Unit', M, K, &
                           ONE, V, LDV, WORK, LDWORK )
!*
!*              C1 := C1 - W
!*
               DO 60 J = 1, K
                  DO 50 I = 1, M
                     C( I, J ) = C( I, J ) - WORK( I, J )
   50             CONTINUE
   60          CONTINUE
            END IF
!*
         ELSE
!*
!*           Let  V =  ( V1 )
!*                     ( V2 )    (last K rows)
!*           where  V2  is unit upper triangular.
!*
            IF( LSAME( SIDE, 'L' ) ) THEN
!*
!*              Form  H * C  or  H**T * C  where  C = ( C1 )
!*                                                    ( C2 )
!*
!*              W := C**T * V  =  (C1**T * V1 + C2**T * V2)  (stored in WORK)
!*
!*              W := C2**T
!*
               DO 70 J = 1, K
                  CALL DCOPY( N, C( M-K+J, 1 ), LDC, WORK( 1, J ), 1 )
   70          CONTINUE
!*
!*              W := W * V2
!*
               CALL DTRMM( 'Right', 'Upper', 'No transpose', 'Unit', N, &
                           K, ONE, V( M-K+1, 1 ), LDV, WORK, LDWORK )
               IF( M.GT.K ) THEN
!*
!*                 W := W + C1**T * V1
!*
                  CALL DGEMM( 'Transpose', 'No transpose', N, K, M-K, &
                              ONE, C, LDC, V, LDV, ONE, WORK, LDWORK )
               END IF
!*
!*              W := W * T**T  or  W * T
!*
               CALL DTRMM( 'Right', 'Lower', TRANST, 'Non-unit', N, K, &
                           ONE, T, LDT, WORK, LDWORK )
!*
!*              C := C - V * W**T
!*
               IF( M.GT.K ) THEN
!*
!*                 C1 := C1 - V1 * W**T
!*
                  CALL DGEMM( 'No transpose', 'Transpose', M-K, N, K, &
                              -ONE, V, LDV, WORK, LDWORK, ONE, C, LDC )
               END IF
!*
!*              W := W * V2**T
!*
               CALL DTRMM( 'Right', 'Upper', 'Transpose', 'Unit', N, K, &
                           ONE, V( M-K+1, 1 ), LDV, WORK, LDWORK )
!*
!*              C2 := C2 - W**T
!*
               DO 90 J = 1, K
                  DO 80 I = 1, N
                     C( M-K+J, I ) = C( M-K+J, I ) - WORK( I, J )
   80             CONTINUE
   90          CONTINUE
!*
            ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!*
!*              Form  C * H  or  C * H**T  where  C = ( C1  C2 )
!*
!*              W := C * V  =  (C1*V1 + C2*V2)  (stored in WORK)
!*
!*              W := C2
!*
               DO 100 J = 1, K
                  CALL DCOPY( M, C( 1, N-K+J ), 1, WORK( 1, J ), 1 )
  100          CONTINUE
!*
!*              W := W * V2
!*
               CALL DTRMM( 'Right', 'Upper', 'No transpose', 'Unit', M, &
                           K, ONE, V( N-K+1, 1 ), LDV, WORK, LDWORK )
               IF( N.GT.K ) THEN
!*
!*                 W := W + C1 * V1
!*
                  CALL DGEMM( 'No transpose', 'No transpose', M, K, N-K, &
                              ONE, C, LDC, V, LDV, ONE, WORK, LDWORK )
               END IF
!*
!*              W := W * T  or  W * T**T
!*
               CALL DTRMM( 'Right', 'Lower', TRANS, 'Non-unit', M, K, &
                           ONE, T, LDT, WORK, LDWORK )
!*
!*              C := C - W * V**T
!*
               IF( N.GT.K ) THEN
!*
!*                 C1 := C1 - W * V1**T
!*
                  CALL DGEMM( 'No transpose', 'Transpose', M, N-K, K, &
                              -ONE, WORK, LDWORK, V, LDV, ONE, C, LDC )
               END IF
!*
!*              W := W * V2**T
!*
               CALL DTRMM( 'Right', 'Upper', 'Transpose', 'Unit', M, K, &
                           ONE, V( N-K+1, 1 ), LDV, WORK, LDWORK )
!*
!*              C2 := C2 - W
!*
               DO 120 J = 1, K
                  DO 110 I = 1, M
                     C( I, N-K+J ) = C( I, N-K+J ) - WORK( I, J )
  110             CONTINUE
  120          CONTINUE
            END IF
         END IF
!*
      ELSE IF( LSAME( STOREV, 'R' ) ) THEN
!*
         IF( LSAME( DIRECT, 'F' ) ) THEN
!*
!*           Let  V =  ( V1  V2 )    (V1: first K columns)
!*           where  V1  is unit upper triangular.
!*
            IF( LSAME( SIDE, 'L' ) ) THEN
!*
!*              Form  H * C  or  H**T * C  where  C = ( C1 )
!*                                                    ( C2 )
!*
!*              W := C**T * V**T  =  (C1**T * V1**T + C2**T * V2**T) (stored in WORK)
!*
!*              W := C1**T
!*
               DO 130 J = 1, K
                  CALL DCOPY( N, C( J, 1 ), LDC, WORK( 1, J ), 1 )
  130          CONTINUE
!*
!*              W := W * V1**T
!*
               CALL DTRMM( 'Right', 'Upper', 'Transpose', 'Unit', N, K, &
                           ONE, V, LDV, WORK, LDWORK )
               IF( M.GT.K ) THEN
!*
!*                 W := W + C2**T * V2**T
!*
                  CALL DGEMM( 'Transpose', 'Transpose', N, K, M-K, ONE, &
                              C( K+1, 1 ), LDC, V( 1, K+1 ), LDV, ONE, &
                              WORK, LDWORK )
               END IF
!*
!*              W := W * T**T  or  W * T
!*
               CALL DTRMM( 'Right', 'Upper', TRANST, 'Non-unit', N, K, &
                           ONE, T, LDT, WORK, LDWORK )
!*
!*              C := C - V**T * W**T
!*
               IF( M.GT.K ) THEN
!*
!*                 C2 := C2 - V2**T * W**T
!*
                  CALL DGEMM( 'Transpose', 'Transpose', M-K, N, K, -ONE, &
                              V( 1, K+1 ), LDV, WORK, LDWORK, ONE, &
                              C( K+1, 1 ), LDC )
               END IF
!*
!*              W := W * V1
!*
               CALL DTRMM( 'Right', 'Upper', 'No transpose', 'Unit', N, &
                           K, ONE, V, LDV, WORK, LDWORK )
!*
!*              C1 := C1 - W**T
!*
               DO 150 J = 1, K
                  DO 140 I = 1, N
                     C( J, I ) = C( J, I ) - WORK( I, J )
  140             CONTINUE
  150          CONTINUE
!*
            ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!*
!*              Form  C * H  or  C * H**T  where  C = ( C1  C2 )
!*
!*              W := C * V**T  =  (C1*V1**T + C2*V2**T)  (stored in WORK)
!*
!*              W := C1
!*
               DO 160 J = 1, K
                  CALL DCOPY( M, C( 1, J ), 1, WORK( 1, J ), 1 )
  160          CONTINUE
!*
!*              W := W * V1**T
!*
               CALL DTRMM( 'Right', 'Upper', 'Transpose', 'Unit', M, K, &
                           ONE, V, LDV, WORK, LDWORK )
               IF( N.GT.K ) THEN
!*
!*                 W := W + C2 * V2**T
!*
                  CALL DGEMM( 'No transpose', 'Transpose', M, K, N-K, &
                              ONE, C( 1, K+1 ), LDC, V( 1, K+1 ), LDV, &
                              ONE, WORK, LDWORK )
               END IF
!*
!*              W := W * T  or  W * T**T
!*
               CALL DTRMM( 'Right', 'Upper', TRANS, 'Non-unit', M, K, &
                           ONE, T, LDT, WORK, LDWORK )
!*
!*              C := C - W * V
!*
               IF( N.GT.K ) THEN
!*
!*                 C2 := C2 - W * V2
!*
                  CALL DGEMM( 'No transpose', 'No transpose', M, N-K, K, &
                              -ONE, WORK, LDWORK, V( 1, K+1 ), LDV, ONE, &
                              C( 1, K+1 ), LDC )
               END IF
!*
!*              W := W * V1
!*
               CALL DTRMM( 'Right', 'Upper', 'No transpose', 'Unit', M, &
                           K, ONE, V, LDV, WORK, LDWORK )
!*
!*              C1 := C1 - W
!*
               DO 180 J = 1, K
                  DO 170 I = 1, M
                     C( I, J ) = C( I, J ) - WORK( I, J )
  170             CONTINUE
  180          CONTINUE
!*
            END IF
!*
         ELSE
!*
!*           Let  V =  ( V1  V2 )    (V2: last K columns)
!*           where  V2  is unit lower triangular.
!*
            IF( LSAME( SIDE, 'L' ) ) THEN
!*
!*              Form  H * C  or  H**T * C  where  C = ( C1 )
!*                                                    ( C2 )
!*
!*              W := C**T * V**T  =  (C1**T * V1**T + C2**T * V2**T) (stored in WORK)
!*
!*              W := C2**T
!*
               DO 190 J = 1, K
                  CALL DCOPY( N, C( M-K+J, 1 ), LDC, WORK( 1, J ), 1 )
  190          CONTINUE
!*
!*              W := W * V2**T
!*
               CALL DTRMM( 'Right', 'Lower', 'Transpose', 'Unit', N, K, &
                           ONE, V( 1, M-K+1 ), LDV, WORK, LDWORK )
               IF( M.GT.K ) THEN
!*
!*                 W := W + C1**T * V1**T
!*
                  CALL DGEMM( 'Transpose', 'Transpose', N, K, M-K, ONE, &
                              C, LDC, V, LDV, ONE, WORK, LDWORK )
               END IF
!*
!*              W := W * T**T  or  W * T
!*
               CALL DTRMM( 'Right', 'Lower', TRANST, 'Non-unit', N, K, &
                           ONE, T, LDT, WORK, LDWORK )
!*
!*              C := C - V**T * W**T
!*
               IF( M.GT.K ) THEN
!*
!*                 C1 := C1 - V1**T * W**T
!*
                  CALL DGEMM( 'Transpose', 'Transpose', M-K, N, K, -ONE, &
                              V, LDV, WORK, LDWORK, ONE, C, LDC )
               END IF
!*
!*              W := W * V2
!*
               CALL DTRMM( 'Right', 'Lower', 'No transpose', 'Unit', N, &
                           K, ONE, V( 1, M-K+1 ), LDV, WORK, LDWORK )
!*
!*              C2 := C2 - W**T
!*
               DO 210 J = 1, K
                  DO 200 I = 1, N
                     C( M-K+J, I ) = C( M-K+J, I ) - WORK( I, J )
  200             CONTINUE
  210          CONTINUE
!*
            ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!*
!*              Form  C * H  or  C * H'  where  C = ( C1  C2 )
!*
!*              W := C * V**T  =  (C1*V1**T + C2*V2**T)  (stored in WORK)
!*
!*              W := C2
!*
               DO 220 J = 1, K
                  CALL DCOPY( M, C( 1, N-K+J ), 1, WORK( 1, J ), 1 )
  220          CONTINUE
!*
!*              W := W * V2**T
!*
               CALL DTRMM( 'Right', 'Lower', 'Transpose', 'Unit', M, K, &
                           ONE, V( 1, N-K+1 ), LDV, WORK, LDWORK )
               IF( N.GT.K ) THEN
!*
!*                 W := W + C1 * V1**T
!*
                  CALL DGEMM( 'No transpose', 'Transpose', M, K, N-K, &
                              ONE, C, LDC, V, LDV, ONE, WORK, LDWORK )
               END IF
!*
!*              W := W * T  or  W * T**T
!*
               CALL DTRMM( 'Right', 'Lower', TRANS, 'Non-unit', M, K, &
                           ONE, T, LDT, WORK, LDWORK )
!*
!*              C := C - W * V
!*
               IF( N.GT.K ) THEN
!*
!*                 C1 := C1 - W * V1
!*
                  CALL DGEMM( 'No transpose', 'No transpose', M, N-K, K, &
                              -ONE, WORK, LDWORK, V, LDV, ONE, C, LDC )
               END IF
!*
!*              W := W * V2
!*
               CALL DTRMM( 'Right', 'Lower', 'No transpose', 'Unit', M, &
                           K, ONE, V( 1, N-K+1 ), LDV, WORK, LDWORK )
!*
!*              C1 := C1 - W
!*
               DO 240 J = 1, K
                  DO 230 I = 1, M
                     C( I, N-K+J ) = C( I, N-K+J ) - WORK( I, J )
  230             CONTINUE
  240          CONTINUE
!*
            END IF
!*
         END IF
      END IF
!*
      RETURN
!*
!*     End of DLARFB
!*
      END

!==========================================================================

!*> \brief \b DGELQ2 computes the LQ factorization of a general rectangular matrix using an unblocked algorithm.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DGELQ2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dgelq2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dgelq2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dgelq2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGELQ2( M, N, A, LDA, TAU, WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, LDA, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGELQ2 computes an LQ factorization of a real m by n matrix A:
!*> A = L * Q.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the m by n matrix A.
!*>          On exit, the elements on and below the diagonal of the array
!*>          contain the m by min(m,n) lower trapezoidal matrix L (L is
!*>          lower triangular if m <= n); the elements above the diagonal,
!*>          with the array TAU, represent the orthogonal matrix Q as a
!*>          product of elementary reflectors (see Further Details).
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The scalar factors of the elementary reflectors (see Further
!*>          Details).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (M)
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0: successful exit
!*>          < 0: if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleGEcomputational
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The matrix Q is represented as a product of elementary reflectors
!*>
!*>     Q = H(k) . . . H(2) H(1), where k = min(m,n).
!*>
!*>  Each H(i) has the form
!*>
!*>     H(i) = I - tau * v * v**T
!*>
!*>  where tau is a real scalar, and v is a real vector with
!*>  v(1:i-1) = 0 and v(i) = 1; v(i+1:n) is stored on exit in A(i,i+1:n),
!*>  and tau in TAU(i).
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DGELQ2( M, N, A, LDA, TAU, WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, LDA, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I, K
      DOUBLE PRECISION   AII
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARF, DLARFG, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DGELQ2', -INFO )
         RETURN
      END IF
!*
      K = MIN( M, N )
!*
      DO 10 I = 1, K
!*
!*        Generate elementary reflector H(i) to annihilate A(i,i+1:n)
!*
         CALL DLARFG( N-I+1, A( I, I ), A( I, MIN( I+1, N ) ), LDA, &
                      TAU( I ) )
         IF( I.LT.M ) THEN
!*
!*           Apply H(i) to A(i+1:m,i:n) from the right
!*
            AII = A( I, I )
            A( I, I ) = ONE
            CALL DLARF( 'Right', M-I, N-I+1, A( I, I ), LDA, TAU( I ), &
                        A( I+1, I ), LDA, WORK )
            A( I, I ) = AII
         END IF
   10 CONTINUE
      RETURN
!*
!*     End of DGELQ2
!*
      END

!==========================================================================

!*> \brief \b DLASSQ updates a sum of squares represented in scaled form.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASSQ + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlassq.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlassq.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlassq.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASSQ( N, X, INCX, SCALE, SUMSQ )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INCX, N
!*       DOUBLE PRECISION   SCALE, SUMSQ
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   X( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASSQ  returns the values  scl  and  smsq  such that
!*>
!*>    ( scl**2 )*smsq = x( 1 )**2 +...+ x( n )**2 + ( scale**2 )*sumsq,
!*>
!*> where  x( i ) = X( 1 + ( i - 1 )*INCX ). The value of  sumsq  is
!*> assumed to be non-negative and  scl  returns the value
!*>
!*>    scl = max( scale, abs( x( i ) ) ).
!*>
!*> scale and sumsq must be supplied in SCALE and SUMSQ and
!*> scl and smsq are overwritten on SCALE and SUMSQ respectively.
!*>
!*> The routine makes only one pass through the vector x.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of elements to be used from the vector X.
!*> \endverbatim
!*>
!*> \param[in] X
!*> \verbatim
!*>          X is DOUBLE PRECISION array, dimension (N)
!*>          The vector for which a scaled sum of squares is computed.
!*>             x( i )  = X( 1 + ( i - 1 )*INCX ), 1 <= i <= n.
!*> \endverbatim
!*>
!*> \param[in] INCX
!*> \verbatim
!*>          INCX is INTEGER
!*>          The increment between successive values of the vector X.
!*>          INCX > 0.
!*> \endverbatim
!*>
!*> \param[in,out] SCALE
!*> \verbatim
!*>          SCALE is DOUBLE PRECISION
!*>          On entry, the value  scale  in the equation above.
!*>          On exit, SCALE is overwritten with  scl , the scaling factor
!*>          for the sum of squares.
!*> \endverbatim
!*>
!*> \param[in,out] SUMSQ
!*> \verbatim
!*>          SUMSQ is DOUBLE PRECISION
!*>          On entry, the value  sumsq  in the equation above.
!*>          On exit, SUMSQ is overwritten with  smsq , the basic sum of
!*>          squares from which  scl  has been factored out.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      SUBROUTINE DLASSQ( N, X, INCX, SCALE, SUMSQ )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            INCX, N
      DOUBLE PRECISION   SCALE, SUMSQ
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   X( * )
!*     ..
!*
!* =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            IX
      DOUBLE PRECISION   ABSXI
!*     ..
!*     .. External Functions ..
      LOGICAL            DISNAN
      EXTERNAL           DISNAN
!*     ..
!*     .. Executable Statements ..
!*
      IF( N.GT.0 ) THEN
         DO 10 IX = 1, 1 + ( N-1 )*INCX, INCX
            ABSXI = ABS( X( IX ) )
            IF( ABSXI.GT.ZERO.OR.DISNAN( ABSXI ) ) THEN
               IF( SCALE.LT.ABSXI ) THEN
                  SUMSQ = 1 + SUMSQ*( SCALE / ABSXI )**2
                  SCALE = ABSXI
               ELSE
                  SUMSQ = SUMSQ + ( ABSXI / SCALE )**2
               END IF
            END IF
   10    CONTINUE
      END IF
      RETURN
!*
!*     End of DLASSQ
!*
      END

!==========================================================================

!*> \brief \b DORMQR
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORMQR + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dormqr.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dormqr.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dormqr.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORMQR( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC,
!*                          WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          SIDE, TRANS
!*       INTEGER            INFO, K, LDA, LDC, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORMQR overwrites the general real M-by-N matrix C with
!*>
!*>                 SIDE = 'L'     SIDE = 'R'
!*> TRANS = 'N':      Q * C          C * Q
!*> TRANS = 'T':      Q**T * C       C * Q**T
!*>
!*> where Q is a real orthogonal matrix defined as the product of k
!*> elementary reflectors
!*>
!*>       Q = H(1) H(2) . . . H(k)
!*>
!*> as returned by DGEQRF. Q is of order M if SIDE = 'L' and of order N
!*> if SIDE = 'R'.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>          = 'L': apply Q or Q**T from the Left;
!*>          = 'R': apply Q or Q**T from the Right.
!*> \endverbatim
!*>
!*> \param[in] TRANS
!*> \verbatim
!*>          TRANS is CHARACTER*1
!*>          = 'N':  No transpose, apply Q;
!*>          = 'T':  Transpose, apply Q**T.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix C. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix C. N >= 0.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The number of elementary reflectors whose product defines
!*>          the matrix Q.
!*>          If SIDE = 'L', M >= K >= 0;
!*>          if SIDE = 'R', N >= K >= 0.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,K)
!*>          The i-th column must contain the vector which defines the
!*>          elementary reflector H(i), for i = 1,2,...,k, as returned by
!*>          DGEQRF in the first k columns of its array argument A.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.
!*>          If SIDE = 'L', LDA >= max(1,M);
!*>          if SIDE = 'R', LDA >= max(1,N).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i), as returned by DGEQRF.
!*> \endverbatim
!*>
!*> \param[in,out] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension (LDC,N)
!*>          On entry, the M-by-N matrix C.
!*>          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q.
!*> \endverbatim
!*>
!*> \param[in] LDC
!*> \verbatim
!*>          LDC is INTEGER
!*>          The leading dimension of the array C. LDC >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK.
!*>          If SIDE = 'L', LWORK >= max(1,N);
!*>          if SIDE = 'R', LWORK >= max(1,M).
!*>          For optimum performance LWORK >= N*NB if SIDE = 'L', and
!*>          LWORK >= M*NB if SIDE = 'R', where NB is the optimal
!*>          blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORMQR( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS
      INTEGER            INFO, K, LDA, LDC, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      INTEGER            NBMAX, LDT
      PARAMETER          ( NBMAX = 64, LDT = NBMAX+1 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LEFT, LQUERY, NOTRAN
      INTEGER            I, I1, I2, I3, IB, IC, IINFO, IWS, JC, LDWORK, &
                         LWKOPT, MI, NB, NBMIN, NI, NQ, NW
!*     ..
!*     .. Local Arrays ..
      DOUBLE PRECISION   T( LDT, NBMAX )
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      EXTERNAL           LSAME, ILAENV
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARFB, DLARFT, DORM2R, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
      LQUERY = ( LWORK.EQ.-1 )
!*
!*     NQ is the order of Q and NW is the minimum dimension of WORK
!*
      IF( LEFT ) THEN
         NQ = M
         NW = N
      ELSE
         NQ = N
         NW = M
      END IF
      IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( K.LT.0 .OR. K.GT.NQ ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, NQ ) ) THEN
         INFO = -7
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -10
      ELSE IF( LWORK.LT.MAX( 1, NW ) .AND. .NOT.LQUERY ) THEN
         INFO = -12
      END IF
!*
      IF( INFO.EQ.0 ) THEN
!*
!*        Determine the block size.  NB may be at most NBMAX, where NBMAX
!*        is used to define the local array T.
!*
         NB = MIN( NBMAX, ILAENV( 1, 'DORMQR', SIDE // TRANS, M, N, K, &
              -1 ) )
         LWKOPT = MAX( 1, NW )*NB
         WORK( 1 ) = LWKOPT
      END IF
!*
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORMQR', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( M.EQ.0 .OR. N.EQ.0 .OR. K.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!*
      NBMIN = 2
      LDWORK = NW
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
         IWS = NW*NB
         IF( LWORK.LT.IWS ) THEN
            NB = LWORK / LDWORK
            NBMIN = MAX( 2, ILAENV( 2, 'DORMQR', SIDE // TRANS, M, N, K, &
                    -1 ) )
         END IF
      ELSE
         IWS = NW
      END IF
!*
      IF( NB.LT.NBMIN .OR. NB.GE.K ) THEN
!*
!*        Use unblocked code
!*
         CALL DORM2R( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, &
                      IINFO )
      ELSE
!*
!*        Use blocked code
!*
         IF( ( LEFT .AND. .NOT.NOTRAN ) .OR. &
             ( .NOT.LEFT .AND. NOTRAN ) ) THEN
            I1 = 1
            I2 = K
            I3 = NB
         ELSE
            I1 = ( ( K-1 ) / NB )*NB + 1
            I2 = 1
            I3 = -NB
         END IF
!*
         IF( LEFT ) THEN
            NI = N
            JC = 1
         ELSE
            MI = M
            IC = 1
         END IF
!*
         DO 10 I = I1, I2, I3
            IB = MIN( NB, K-I+1 )
!*
!*           Form the triangular factor of the block reflector
!*           H = H(i) H(i+1) . . . H(i+ib-1)
!*
            CALL DLARFT( 'Forward', 'Columnwise', NQ-I+1, IB, A( I, I ), &
                         LDA, TAU( I ), T, LDT )
            IF( LEFT ) THEN
!*
!*              H or H**T is applied to C(i:m,1:n)
!*
               MI = M - I + 1
               IC = I
            ELSE
!*
!*              H or H**T is applied to C(1:m,i:n)
!*
               NI = N - I + 1
               JC = I
            END IF
!*
!*           Apply H or H**T
!*
            CALL DLARFB( SIDE, TRANS, 'Forward', 'Columnwise', MI, NI, &
                         IB, A( I, I ), LDA, T, LDT, C( IC, JC ), LDC, &
                         WORK, LDWORK )
   10    CONTINUE
      END IF
      WORK( 1 ) = LWKOPT
      RETURN
!*
!*     End of DORMQR
!*
      END

!==========================================================================

!*> \brief \b DORMLQ
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORMLQ + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dormlq.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dormlq.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dormlq.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORMLQ( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC,
!*                          WORK, LWORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          SIDE, TRANS
!*       INTEGER            INFO, K, LDA, LDC, LWORK, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORMLQ overwrites the general real M-by-N matrix C with
!*>
!*>                 SIDE = 'L'     SIDE = 'R'
!*> TRANS = 'N':      Q * C          C * Q
!*> TRANS = 'T':      Q**T * C       C * Q**T
!*>
!*> where Q is a real orthogonal matrix defined as the product of k
!*> elementary reflectors
!*>
!*>       Q = H(k) . . . H(2) H(1)
!*>
!*> as returned by DGELQF. Q is of order M if SIDE = 'L' and of order N
!*> if SIDE = 'R'.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>          = 'L': apply Q or Q**T from the Left;
!*>          = 'R': apply Q or Q**T from the Right.
!*> \endverbatim
!*>
!*> \param[in] TRANS
!*> \verbatim
!*>          TRANS is CHARACTER*1
!*>          = 'N':  No transpose, apply Q;
!*>          = 'T':  Transpose, apply Q**T.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix C. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix C. N >= 0.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The number of elementary reflectors whose product defines
!*>          the matrix Q.
!*>          If SIDE = 'L', M >= K >= 0;
!*>          if SIDE = 'R', N >= K >= 0.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension
!*>                               (LDA,M) if SIDE = 'L',
!*>                               (LDA,N) if SIDE = 'R'
!*>          The i-th row must contain the vector which defines the
!*>          elementary reflector H(i), for i = 1,2,...,k, as returned by
!*>          DGELQF in the first k rows of its array argument A.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A. LDA >= max(1,K).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i), as returned by DGELQF.
!*> \endverbatim
!*>
!*> \param[in,out] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension (LDC,N)
!*>          On entry, the M-by-N matrix C.
!*>          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q.
!*> \endverbatim
!*>
!*> \param[in] LDC
!*> \verbatim
!*>          LDC is INTEGER
!*>          The leading dimension of the array C. LDC >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!*>          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!*> \endverbatim
!*>
!*> \param[in] LWORK
!*> \verbatim
!*>          LWORK is INTEGER
!*>          The dimension of the array WORK.
!*>          If SIDE = 'L', LWORK >= max(1,N);
!*>          if SIDE = 'R', LWORK >= max(1,M).
!*>          For optimum performance LWORK >= N*NB if SIDE = 'L', and
!*>          LWORK >= M*NB if SIDE = 'R', where NB is the optimal
!*>          blocksize.
!*>
!*>          If LWORK = -1, then a workspace query is assumed; the routine
!*>          only calculates the optimal size of the WORK array, returns
!*>          this value as the first entry of the WORK array, and no error
!*>          message related to LWORK is issued by XERBLA.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORMLQ( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, LWORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.0) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS
      INTEGER            INFO, K, LDA, LDC, LWORK, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      INTEGER            NBMAX, LDT
      PARAMETER          ( NBMAX = 64, LDT = NBMAX+1 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LEFT, LQUERY, NOTRAN
      CHARACTER          TRANST
      INTEGER            I, I1, I2, I3, IB, IC, IINFO, IWS, JC, LDWORK, &
                         LWKOPT, MI, NB, NBMIN, NI, NQ, NW
!*     ..
!*     .. Local Arrays ..
      DOUBLE PRECISION   T( LDT, NBMAX )
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      EXTERNAL           LSAME, ILAENV
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARFB, DLARFT, DORML2, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
      LQUERY = ( LWORK.EQ.-1 )
!*
!*     NQ is the order of Q and NW is the minimum dimension of WORK
!*
      IF( LEFT ) THEN
         NQ = M
         NW = N
      ELSE
         NQ = N
         NW = M
      END IF
      IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( K.LT.0 .OR. K.GT.NQ ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, K ) ) THEN
         INFO = -7
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -10
      ELSE IF( LWORK.LT.MAX( 1, NW ) .AND. .NOT.LQUERY ) THEN
         INFO = -12
      END IF
!*
      IF( INFO.EQ.0 ) THEN
!*
!*        Determine the block size.  NB may be at most NBMAX, where NBMAX
!*        is used to define the local array T.
!*
         NB = MIN( NBMAX, ILAENV( 1, 'DORMLQ', SIDE // TRANS, M, N, K, &
              -1 ) )
         LWKOPT = MAX( 1, NW )*NB
         WORK( 1 ) = LWKOPT
      END IF
!*
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORMLQ', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( M.EQ.0 .OR. N.EQ.0 .OR. K.EQ.0 ) THEN
         WORK( 1 ) = 1
         RETURN
      END IF
!*
      NBMIN = 2
      LDWORK = NW
      IF( NB.GT.1 .AND. NB.LT.K ) THEN
         IWS = NW*NB
         IF( LWORK.LT.IWS ) THEN
            NB = LWORK / LDWORK
            NBMIN = MAX( 2, ILAENV( 2, 'DORMLQ', SIDE // TRANS, M, N, K, &
                    -1 ) )
         END IF
      ELSE
         IWS = NW
      END IF
!*
      IF( NB.LT.NBMIN .OR. NB.GE.K ) THEN
!*
!*        Use unblocked code
!*
         CALL DORML2( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, WORK, &
                      IINFO )
      ELSE
!*
!*        Use blocked code
!*
         IF( ( LEFT .AND. NOTRAN ) .OR. &
             ( .NOT.LEFT .AND. .NOT.NOTRAN ) ) THEN
            I1 = 1
            I2 = K
            I3 = NB
         ELSE
            I1 = ( ( K-1 ) / NB )*NB + 1
            I2 = 1
            I3 = -NB
         END IF
!*
         IF( LEFT ) THEN
            NI = N
            JC = 1
         ELSE
            MI = M
            IC = 1
         END IF
!*
         IF( NOTRAN ) THEN
            TRANST = 'T'
         ELSE
            TRANST = 'N'
         END IF
!*
         DO 10 I = I1, I2, I3
            IB = MIN( NB, K-I+1 )
!*
!*           Form the triangular factor of the block reflector
!*           H = H(i) H(i+1) . . . H(i+ib-1)
!*
            CALL DLARFT( 'Forward', 'Rowwise', NQ-I+1, IB, A( I, I ), &
                         LDA, TAU( I ), T, LDT )
            IF( LEFT ) THEN
!*
!*              H or H**T is applied to C(i:m,1:n)
!*
               MI = M - I + 1
               IC = I
            ELSE
!*
!*              H or H**T is applied to C(1:m,i:n)
!*
               NI = N - I + 1
               JC = I
            END IF
!*
!*           Apply H or H**T
!*
            CALL DLARFB( SIDE, TRANST, 'Forward', 'Rowwise', MI, NI, IB, &
                         A( I, I ), LDA, T, LDT, C( IC, JC ), LDC, WORK, &
                         LDWORK )
   10    CONTINUE
      END IF
      WORK( 1 ) = LWKOPT
      RETURN
!*
!*     End of DORMLQ
!*
      END

!==========================================================================

!*> \brief \b DGEQR2 computes the QR factorization of a general rectangular matrix using an unblocked algorithm.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DGEQR2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dgeqr2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dgeqr2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dgeqr2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGEQR2( M, N, A, LDA, TAU, WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, LDA, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGEQR2 computes a QR factorization of a real m by n matrix A:
!*> A = Q * R.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the m by n matrix A.
!*>          On exit, the elements on and above the diagonal of the array
!*>          contain the min(m,n) by n upper trapezoidal matrix R (R is
!*>          upper triangular if m >= n); the elements below the diagonal,
!*>          with the array TAU, represent the orthogonal matrix Q as a
!*>          product of elementary reflectors (see Further Details).
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The scalar factors of the elementary reflectors (see Further
!*>          Details).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (N)
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0: successful exit
!*>          < 0: if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleGEcomputational
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The matrix Q is represented as a product of elementary reflectors
!*>
!*>     Q = H(1) H(2) . . . H(k), where k = min(m,n).
!*>
!*>  Each H(i) has the form
!*>
!*>     H(i) = I - tau * v * v**T
!*>
!*>  where tau is a real scalar, and v is a real vector with
!*>  v(1:i-1) = 0 and v(i) = 1; v(i+1:m) is stored on exit in A(i+1:m,i),
!*>  and tau in TAU(i).
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DGEQR2( M, N, A, LDA, TAU, WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, LDA, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I, K
      DOUBLE PRECISION   AII
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARF, DLARFG, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DGEQR2', -INFO )
         RETURN
      END IF
!*
      K = MIN( M, N )
!*
      DO 10 I = 1, K
!*
!*        Generate elementary reflector H(i) to annihilate A(i+1:m,i)
!*
         CALL DLARFG( M-I+1, A( I, I ), A( MIN( I+1, M ), I ), 1, &
                      TAU( I ) )
         IF( I.LT.N ) THEN
!*
!*           Apply H(i) to A(i:m,i+1:n) from the left
!*
            AII = A( I, I )
            A( I, I ) = ONE
            CALL DLARF( 'Left', M-I+1, N-I, A( I, I ), 1, TAU( I ), &
                        A( I, I+1 ), LDA, WORK )
            A( I, I ) = AII
         END IF
   10 CONTINUE
      RETURN
!*
!*     End of DGEQR2
!*
      END

!==========================================================================

!*> \brief \b DORG2R generates all or part of the orthogonal matrix Q from a QR
!*> factorization determined by sgeqrf (unblocked algorithm).
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORG2R + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dorg2r.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dorg2r.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dorg2r.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORG2R( M, N, K, A, LDA, TAU, WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, K, LDA, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORG2R generates an m by n real matrix Q with orthonormal columns,
!*> which is defined as the first n columns of a product of k elementary
!*> reflectors of order m
!*>
!*>       Q  =  H(1) H(2) . . . H(k)
!*>
!*> as returned by DGEQRF.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix Q. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix Q. M >= N >= 0.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The number of elementary reflectors whose product defines the
!*>          matrix Q. N >= K >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the i-th column must contain the vector which
!*>          defines the elementary reflector H(i), for i = 1,2,...,k, as
!*>          returned by DGEQRF in the first k columns of its array
!*>          argument A.
!*>          On exit, the m-by-n matrix Q.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The first dimension of the array A. LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i), as returned by DGEQRF.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (N)
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0: successful exit
!*>          < 0: if INFO = -i, the i-th argument has an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORG2R( M, N, K, A, LDA, TAU, WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, K, LDA, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE, ZERO
      PARAMETER          ( ONE = 1.0D+0, ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I, J, L
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARF, DSCAL, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 .OR. N.GT.M ) THEN
         INFO = -2
      ELSE IF( K.LT.0 .OR. K.GT.N ) THEN
         INFO = -3
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -5
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORG2R', -INFO )
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( N.LE.0 ) &
         RETURN
!*
!*     Initialise columns k+1:n to columns of the unit matrix
!*
      DO 20 J = K + 1, N
         DO 10 L = 1, M
            A( L, J ) = ZERO
   10    CONTINUE
         A( J, J ) = ONE
   20 CONTINUE
!*
      DO 40 I = K, 1, -1
!*
!*        Apply H(i) to A(i:m,i:n) from the left
!*
         IF( I.LT.N ) THEN
            A( I, I ) = ONE
            CALL DLARF( 'Left', M-I+1, N-I, A( I, I ), 1, TAU( I ), &
                        A( I, I+1 ), LDA, WORK )
         END IF
         IF( I.LT.M ) &
            CALL DSCAL( M-I, -TAU( I ), A( I+1, I ), 1 )
         A( I, I ) = ONE - TAU( I )
!*
!*        Set A(1:i-1,i) to zero
!*
         DO 30 L = 1, I - 1
            A( L, I ) = ZERO
   30    CONTINUE
   40 CONTINUE
      RETURN
!*
!*     End of DORG2R
!*
      END

!==========================================================================

!*> \brief \b DLABRD reduces the first nb rows and columns of a general matrix to a bidiagonal form.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLABRD + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlabrd.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlabrd.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlabrd.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLABRD( M, N, NB, A, LDA, D, E, TAUQ, TAUP, X, LDX, Y,
!*                          LDY )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            LDA, LDX, LDY, M, N, NB
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), D( * ), E( * ), TAUP( * ), &
!*                          TAUQ( * ), X( LDX, * ), Y( LDY, * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLABRD reduces the first NB rows and columns of a real general
!*> m by n matrix A to upper or lower bidiagonal form by an orthogonal
!*> transformation Q**T * A * P, and returns the matrices X and Y which
!*> are needed to apply the transformation to the unreduced part of A.
!*>
!*> If m >= n, A is reduced to upper bidiagonal form; if m < n, to lower
!*> bidiagonal form.
!*>
!*> This is an auxiliary routine called by DGEBRD
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows in the matrix A.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns in the matrix A.
!*> \endverbatim
!*>
!*> \param[in] NB
!*> \verbatim
!*>          NB is INTEGER
!*>          The number of leading rows and columns of A to be reduced.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the m by n general matrix to be reduced.
!*>          On exit, the first NB rows and columns of the matrix are
!*>          overwritten; the rest of the array is unchanged.
!*>          If m >= n, elements on and below the diagonal in the first NB
!*>            columns, with the array TAUQ, represent the orthogonal
!*>            matrix Q as a product of elementary reflectors; and
!*>            elements above the diagonal in the first NB rows, with the
!*>            array TAUP, represent the orthogonal matrix P as a product
!*>            of elementary reflectors.
!*>          If m < n, elements below the diagonal in the first NB
!*>            columns, with the array TAUQ, represent the orthogonal
!*>            matrix Q as a product of elementary reflectors, and
!*>            elements on and above the diagonal in the first NB rows,
!*>            with the array TAUP, represent the orthogonal matrix P as
!*>            a product of elementary reflectors.
!*>          See Further Details.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] D
!*> \verbatim
!*>          D is DOUBLE PRECISION array, dimension (NB)
!*>          The diagonal elements of the first NB rows and columns of
!*>          the reduced matrix.  D(i) = A(i,i).
!*> \endverbatim
!*>
!*> \param[out] E
!*> \verbatim
!*>          E is DOUBLE PRECISION array, dimension (NB)
!*>          The off-diagonal elements of the first NB rows and columns of
!*>          the reduced matrix.
!*> \endverbatim
!*>
!*> \param[out] TAUQ
!*> \verbatim
!*>          TAUQ is DOUBLE PRECISION array dimension (NB)
!*>          The scalar factors of the elementary reflectors which
!*>          represent the orthogonal matrix Q. See Further Details.
!*> \endverbatim
!*>
!*> \param[out] TAUP
!*> \verbatim
!*>          TAUP is DOUBLE PRECISION array, dimension (NB)
!*>          The scalar factors of the elementary reflectors which
!*>          represent the orthogonal matrix P. See Further Details.
!*> \endverbatim
!*>
!*> \param[out] X
!*> \verbatim
!*>          X is DOUBLE PRECISION array, dimension (LDX,NB)
!*>          The m-by-nb matrix X required to update the unreduced part
!*>          of A.
!*> \endverbatim
!*>
!*> \param[in] LDX
!*> \verbatim
!*>          LDX is INTEGER
!*>          The leading dimension of the array X. LDX >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] Y
!*> \verbatim
!*>          Y is DOUBLE PRECISION array, dimension (LDY,NB)
!*>          The n-by-nb matrix Y required to update the unreduced part
!*>          of A.
!*> \endverbatim
!*>
!*> \param[in] LDY
!*> \verbatim
!*>          LDY is INTEGER
!*>          The leading dimension of the array Y. LDY >= max(1,N).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleOTHERauxiliary
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The matrices Q and P are represented as products of elementary
!*>  reflectors:
!*>
!*>     Q = H(1) H(2) . . . H(nb)  and  P = G(1) G(2) . . . G(nb)
!*>
!*>  Each H(i) and G(i) has the form:
!*>
!*>     H(i) = I - tauq * v * v**T  and G(i) = I - taup * u * u**T
!*>
!*>  where tauq and taup are real scalars, and v and u are real vectors.
!*>
!*>  If m >= n, v(1:i-1) = 0, v(i) = 1, and v(i:m) is stored on exit in
!*>  A(i:m,i); u(1:i) = 0, u(i+1) = 1, and u(i+1:n) is stored on exit in
!*>  A(i,i+1:n); tauq is stored in TAUQ(i) and taup in TAUP(i).
!*>
!*>  If m < n, v(1:i) = 0, v(i+1) = 1, and v(i+1:m) is stored on exit in
!*>  A(i+2:m,i); u(1:i-1) = 0, u(i) = 1, and u(i:n) is stored on exit in
!*>  A(i,i+1:n); tauq is stored in TAUQ(i) and taup in TAUP(i).
!*>
!*>  The elements of the vectors v and u together form the m-by-nb matrix
!*>  V and the nb-by-n matrix U**T which are needed, with X and Y, to apply
!*>  the transformation to the unreduced part of the matrix, using a block
!*>  update of the form:  A := A - V*Y**T - X*U**T.
!*>
!*>  The contents of A on exit are illustrated by the following examples
!*>  with nb = 2:
!*>
!*>  m = 6 and n = 5 (m > n):          m = 5 and n = 6 (m < n):
!*>
!*>    (  1   1   u1  u1  u1 )           (  1   u1  u1  u1  u1  u1 )
!*>    (  v1  1   1   u2  u2 )           (  1   1   u2  u2  u2  u2 )
!*>    (  v1  v2  a   a   a  )           (  v1  1   a   a   a   a  )
!*>    (  v1  v2  a   a   a  )           (  v1  v2  a   a   a   a  )
!*>    (  v1  v2  a   a   a  )           (  v1  v2  a   a   a   a  )
!*>    (  v1  v2  a   a   a  )
!*>
!*>  where a denotes an element of the original matrix which is unchanged,
!*>  vi denotes an element of the vector defining H(i), and ui an element
!*>  of the vector defining G(i).
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DLABRD( M, N, NB, A, LDA, D, E, TAUQ, TAUP, X, LDX, Y, &
                         LDY )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            LDA, LDX, LDY, M, N, NB
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), D( * ), E( * ), TAUP( * ), &
                         TAUQ( * ), X( LDX, * ), Y( LDY, * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO, ONE
      PARAMETER          ( ZERO = 0.0D0, ONE = 1.0D0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DGEMV, DLARFG, DSCAL
!*     ..
!*     .. Executable Statements ..
!*
!*     Quick return if possible
!*
      IF( M.LE.0 .OR. N.LE.0 ) &
         RETURN
!*
      IF( M.GE.N ) THEN
!*
!*        Reduce to upper bidiagonal form
!*
         DO 10 I = 1, NB
!*
!*           Update A(i:m,i)
!*
            CALL DGEMV( 'No transpose', M-I+1, I-1, -ONE, A( I, 1 ), &
                        LDA, Y( I, 1 ), LDY, ONE, A( I, I ), 1 )
            CALL DGEMV( 'No transpose', M-I+1, I-1, -ONE, X( I, 1 ), &
                        LDX, A( 1, I ), 1, ONE, A( I, I ), 1 )
!*
!*           Generate reflection Q(i) to annihilate A(i+1:m,i)
!*
            CALL DLARFG( M-I+1, A( I, I ), A( MIN( I+1, M ), I ), 1, &
                         TAUQ( I ) )
            D( I ) = A( I, I )
            IF( I.LT.N ) THEN
               A( I, I ) = ONE
!*
!*              Compute Y(i+1:n,i)
!*
               CALL DGEMV( 'Transpose', M-I+1, N-I, ONE, A( I, I+1 ), &
                           LDA, A( I, I ), 1, ZERO, Y( I+1, I ), 1 )
               CALL DGEMV( 'Transpose', M-I+1, I-1, ONE, A( I, 1 ), LDA, &
                           A( I, I ), 1, ZERO, Y( 1, I ), 1 )
               CALL DGEMV( 'No transpose', N-I, I-1, -ONE, Y( I+1, 1 ), &
                           LDY, Y( 1, I ), 1, ONE, Y( I+1, I ), 1 )
               CALL DGEMV( 'Transpose', M-I+1, I-1, ONE, X( I, 1 ), LDX, &
                           A( I, I ), 1, ZERO, Y( 1, I ), 1 )
               CALL DGEMV( 'Transpose', I-1, N-I, -ONE, A( 1, I+1 ), &
                           LDA, Y( 1, I ), 1, ONE, Y( I+1, I ), 1 )
               CALL DSCAL( N-I, TAUQ( I ), Y( I+1, I ), 1 )
!*
!*              Update A(i,i+1:n)
!*
               CALL DGEMV( 'No transpose', N-I, I, -ONE, Y( I+1, 1 ), &
                           LDY, A( I, 1 ), LDA, ONE, A( I, I+1 ), LDA )
               CALL DGEMV( 'Transpose', I-1, N-I, -ONE, A( 1, I+1 ), &
                           LDA, X( I, 1 ), LDX, ONE, A( I, I+1 ), LDA )
!*
!*              Generate reflection P(i) to annihilate A(i,i+2:n)
!*
               CALL DLARFG( N-I, A( I, I+1 ), A( I, MIN( I+2, N ) ), &
                            LDA, TAUP( I ) )
               E( I ) = A( I, I+1 )
               A( I, I+1 ) = ONE
!*
!*              Compute X(i+1:m,i)
!*
               CALL DGEMV( 'No transpose', M-I, N-I, ONE, A( I+1, I+1 ), &
                           LDA, A( I, I+1 ), LDA, ZERO, X( I+1, I ), 1 )
               CALL DGEMV( 'Transpose', N-I, I, ONE, Y( I+1, 1 ), LDY, &
                           A( I, I+1 ), LDA, ZERO, X( 1, I ), 1 )
               CALL DGEMV( 'No transpose', M-I, I, -ONE, A( I+1, 1 ), &
                           LDA, X( 1, I ), 1, ONE, X( I+1, I ), 1 )
               CALL DGEMV( 'No transpose', I-1, N-I, ONE, A( 1, I+1 ), &
                           LDA, A( I, I+1 ), LDA, ZERO, X( 1, I ), 1 )
               CALL DGEMV( 'No transpose', M-I, I-1, -ONE, X( I+1, 1 ), &
                           LDX, X( 1, I ), 1, ONE, X( I+1, I ), 1 )
               CALL DSCAL( M-I, TAUP( I ), X( I+1, I ), 1 )
            END IF
   10    CONTINUE
      ELSE
!*
!*        Reduce to lower bidiagonal form
!*
         DO 20 I = 1, NB
!*
!*           Update A(i,i:n)
!*
            CALL DGEMV( 'No transpose', N-I+1, I-1, -ONE, Y( I, 1 ), &
                        LDY, A( I, 1 ), LDA, ONE, A( I, I ), LDA )
            CALL DGEMV( 'Transpose', I-1, N-I+1, -ONE, A( 1, I ), LDA, &
                        X( I, 1 ), LDX, ONE, A( I, I ), LDA )
!*
!*           Generate reflection P(i) to annihilate A(i,i+1:n)
!*
            CALL DLARFG( N-I+1, A( I, I ), A( I, MIN( I+1, N ) ), LDA, &
                         TAUP( I ) )
            D( I ) = A( I, I )
            IF( I.LT.M ) THEN
               A( I, I ) = ONE
!*
!*              Compute X(i+1:m,i)
!*
               CALL DGEMV( 'No transpose', M-I, N-I+1, ONE, A( I+1, I ), &
                           LDA, A( I, I ), LDA, ZERO, X( I+1, I ), 1 )
               CALL DGEMV( 'Transpose', N-I+1, I-1, ONE, Y( I, 1 ), LDY, &
                           A( I, I ), LDA, ZERO, X( 1, I ), 1 )
               CALL DGEMV( 'No transpose', M-I, I-1, -ONE, A( I+1, 1 ), &
                           LDA, X( 1, I ), 1, ONE, X( I+1, I ), 1 )
               CALL DGEMV( 'No transpose', I-1, N-I+1, ONE, A( 1, I ), &
                           LDA, A( I, I ), LDA, ZERO, X( 1, I ), 1 )
               CALL DGEMV( 'No transpose', M-I, I-1, -ONE, X( I+1, 1 ), &
                           LDX, X( 1, I ), 1, ONE, X( I+1, I ), 1 )
               CALL DSCAL( M-I, TAUP( I ), X( I+1, I ), 1 )
!*
!*              Update A(i+1:m,i)
!*
               CALL DGEMV( 'No transpose', M-I, I-1, -ONE, A( I+1, 1 ), &
                           LDA, Y( I, 1 ), LDY, ONE, A( I+1, I ), 1 )
               CALL DGEMV( 'No transpose', M-I, I, -ONE, X( I+1, 1 ), &
                           LDX, A( 1, I ), 1, ONE, A( I+1, I ), 1 )
!*
!*              Generate reflection Q(i) to annihilate A(i+2:m,i)
!*
               CALL DLARFG( M-I, A( I+1, I ), A( MIN( I+2, M ), I ), 1, &
                            TAUQ( I ) )
               E( I ) = A( I+1, I )
               A( I+1, I ) = ONE
!*
!*              Compute Y(i+1:n,i)
!*
               CALL DGEMV( 'Transpose', M-I, N-I, ONE, A( I+1, I+1 ), &
                           LDA, A( I+1, I ), 1, ZERO, Y( I+1, I ), 1 )
               CALL DGEMV( 'Transpose', M-I, I-1, ONE, A( I+1, 1 ), LDA, &
                           A( I+1, I ), 1, ZERO, Y( 1, I ), 1 )
               CALL DGEMV( 'No transpose', N-I, I-1, -ONE, Y( I+1, 1 ), &
                           LDY, Y( 1, I ), 1, ONE, Y( I+1, I ), 1 )
               CALL DGEMV( 'Transpose', M-I, I, ONE, X( I+1, 1 ), LDX, &
                           A( I+1, I ), 1, ZERO, Y( 1, I ), 1 )
               CALL DGEMV( 'Transpose', I, N-I, -ONE, A( 1, I+1 ), LDA, &
                           Y( 1, I ), 1, ONE, Y( I+1, I ), 1 )
               CALL DSCAL( N-I, TAUQ( I ), Y( I+1, I ), 1 )
            END IF
   20    CONTINUE
      END IF
      RETURN
!*
!*     End of DLABRD
!*
      END

!==========================================================================

!*> \brief \b DGEBD2 reduces a general matrix to bidiagonal form using an unblocked algorithm.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DGEBD2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dgebd2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dgebd2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dgebd2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGEBD2( M, N, A, LDA, D, E, TAUQ, TAUP, WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, LDA, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), D( * ), E( * ), TAUP( * ), &
!*                          TAUQ( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGEBD2 reduces a real general m by n matrix A to upper or lower
!*> bidiagonal form B by an orthogonal transformation: Q**T * A * P = B.
!*>
!*> If m >= n, B is upper bidiagonal; if m < n, B is lower bidiagonal.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows in the matrix A.  M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns in the matrix A.  N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          On entry, the m by n general matrix to be reduced.
!*>          On exit,
!*>          if m >= n, the diagonal and the first superdiagonal are
!*>            overwritten with the upper bidiagonal matrix B; the
!*>            elements below the diagonal, with the array TAUQ, represent
!*>            the orthogonal matrix Q as a product of elementary
!*>            reflectors, and the elements above the first superdiagonal,
!*>            with the array TAUP, represent the orthogonal matrix P as
!*>            a product of elementary reflectors;
!*>          if m < n, the diagonal and the first subdiagonal are
!*>            overwritten with the lower bidiagonal matrix B; the
!*>            elements below the first subdiagonal, with the array TAUQ,
!*>            represent the orthogonal matrix Q as a product of
!*>            elementary reflectors, and the elements above the diagonal,
!*>            with the array TAUP, represent the orthogonal matrix P as
!*>            a product of elementary reflectors.
!*>          See Further Details.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] D
!*> \verbatim
!*>          D is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The diagonal elements of the bidiagonal matrix B:
!*>          D(i) = A(i,i).
!*> \endverbatim
!*>
!*> \param[out] E
!*> \verbatim
!*>          E is DOUBLE PRECISION array, dimension (min(M,N)-1)
!*>          The off-diagonal elements of the bidiagonal matrix B:
!*>          if m >= n, E(i) = A(i,i+1) for i = 1,2,...,n-1;
!*>          if m < n, E(i) = A(i+1,i) for i = 1,2,...,m-1.
!*> \endverbatim
!*>
!*> \param[out] TAUQ
!*> \verbatim
!*>          TAUQ is DOUBLE PRECISION array dimension (min(M,N))
!*>          The scalar factors of the elementary reflectors which
!*>          represent the orthogonal matrix Q. See Further Details.
!*> \endverbatim
!*>
!*> \param[out] TAUP
!*> \verbatim
!*>          TAUP is DOUBLE PRECISION array, dimension (min(M,N))
!*>          The scalar factors of the elementary reflectors which
!*>          represent the orthogonal matrix P. See Further Details.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (max(M,N))
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0: successful exit.
!*>          < 0: if INFO = -i, the i-th argument had an illegal value.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleGEcomputational
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  The matrices Q and P are represented as products of elementary
!*>  reflectors:
!*>
!*>  If m >= n,
!*>
!*>     Q = H(1) H(2) . . . H(n)  and  P = G(1) G(2) . . . G(n-1)
!*>
!*>  Each H(i) and G(i) has the form:
!*>
!*>     H(i) = I - tauq * v * v**T  and G(i) = I - taup * u * u**T
!*>
!*>  where tauq and taup are real scalars, and v and u are real vectors;
!*>  v(1:i-1) = 0, v(i) = 1, and v(i+1:m) is stored on exit in A(i+1:m,i);
!*>  u(1:i) = 0, u(i+1) = 1, and u(i+2:n) is stored on exit in A(i,i+2:n);
!*>  tauq is stored in TAUQ(i) and taup in TAUP(i).
!*>
!*>  If m < n,
!*>
!*>     Q = H(1) H(2) . . . H(m-1)  and  P = G(1) G(2) . . . G(m)
!*>
!*>  Each H(i) and G(i) has the form:
!*>
!*>     H(i) = I - tauq * v * v**T  and G(i) = I - taup * u * u**T
!*>
!*>  where tauq and taup are real scalars, and v and u are real vectors;
!*>  v(1:i) = 0, v(i+1) = 1, and v(i+2:m) is stored on exit in A(i+2:m,i);
!*>  u(1:i-1) = 0, u(i) = 1, and u(i+1:n) is stored on exit in A(i,i+1:n);
!*>  tauq is stored in TAUQ(i) and taup in TAUP(i).
!*>
!*>  The contents of A on exit are illustrated by the following examples:
!*>
!*>  m = 6 and n = 5 (m > n):          m = 5 and n = 6 (m < n):
!*>
!*>    (  d   e   u1  u1  u1 )           (  d   u1  u1  u1  u1  u1 )
!*>    (  v1  d   e   u2  u2 )           (  e   d   u2  u2  u2  u2 )
!*>    (  v1  v2  d   e   u3 )           (  v1  e   d   u3  u3  u3 )
!*>    (  v1  v2  v3  d   e  )           (  v1  v2  e   d   u4  u4 )
!*>    (  v1  v2  v3  v4  d  )           (  v1  v2  v3  e   d   u5 )
!*>    (  v1  v2  v3  v4  v5 )
!*>
!*>  where d and e denote diagonal and off-diagonal elements of B, vi
!*>  denotes an element of the vector defining H(i), and ui an element of
!*>  the vector defining G(i).
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DGEBD2( M, N, A, LDA, D, E, TAUQ, TAUP, WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, LDA, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), D( * ), E( * ), TAUP( * ), &
                         TAUQ( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO, ONE
      PARAMETER          ( ZERO = 0.0D+0, ONE = 1.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARF, DLARFG, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input parameters
!*
      INFO = 0
      IF( M.LT.0 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -4
      END IF
      IF( INFO.LT.0 ) THEN
         CALL XERBLA( 'DGEBD2', -INFO )
         RETURN
      END IF
!*
      IF( M.GE.N ) THEN
!*
!*        Reduce to upper bidiagonal form
!*
         DO 10 I = 1, N
!*
!*           Generate elementary reflector H(i) to annihilate A(i+1:m,i)
!*
            CALL DLARFG( M-I+1, A( I, I ), A( MIN( I+1, M ), I ), 1, &
                         TAUQ( I ) )
            D( I ) = A( I, I )
            A( I, I ) = ONE
!*
!*           Apply H(i) to A(i:m,i+1:n) from the left
!*
            IF( I.LT.N ) &
               CALL DLARF( 'Left', M-I+1, N-I, A( I, I ), 1, TAUQ( I ), &
                           A( I, I+1 ), LDA, WORK )
            A( I, I ) = D( I )
!*
            IF( I.LT.N ) THEN
!*
!*              Generate elementary reflector G(i) to annihilate
!*              A(i,i+2:n)
!*
               CALL DLARFG( N-I, A( I, I+1 ), A( I, MIN( I+2, N ) ), &
                            LDA, TAUP( I ) )
               E( I ) = A( I, I+1 )
               A( I, I+1 ) = ONE
!*
!*              Apply G(i) to A(i+1:m,i+1:n) from the right
!*
               CALL DLARF( 'Right', M-I, N-I, A( I, I+1 ), LDA, &
                           TAUP( I ), A( I+1, I+1 ), LDA, WORK )
               A( I, I+1 ) = E( I )
            ELSE
               TAUP( I ) = ZERO
            END IF
   10    CONTINUE
      ELSE
!*
!*        Reduce to lower bidiagonal form
!*
         DO 20 I = 1, M
!*
!*           Generate elementary reflector G(i) to annihilate A(i,i+1:n)
!*
            CALL DLARFG( N-I+1, A( I, I ), A( I, MIN( I+1, N ) ), LDA, &
                         TAUP( I ) )
            D( I ) = A( I, I )
            A( I, I ) = ONE
!*
!*           Apply G(i) to A(i+1:m,i:n) from the right
!*
            IF( I.LT.M ) &
               CALL DLARF( 'Right', M-I, N-I+1, A( I, I ), LDA, &
                           TAUP( I ), A( I+1, I ), LDA, WORK )
            A( I, I ) = D( I )
!*
            IF( I.LT.M ) THEN
!*
!*              Generate elementary reflector H(i) to annihilate
!*              A(i+2:m,i)
!*
               CALL DLARFG( M-I, A( I+1, I ), A( MIN( I+2, M ), I ), 1, &
                            TAUQ( I ) )
               E( I ) = A( I+1, I )
               A( I+1, I ) = ONE
!*
!*              Apply H(i) to A(i+1:m,i+1:n) from the left
!*
               CALL DLARF( 'Left', M-I, N-I, A( I+1, I ), 1, TAUQ( I ), &
                           A( I+1, I+1 ), LDA, WORK )
               A( I+1, I ) = E( I )
            ELSE
               TAUQ( I ) = ZERO
            END IF
   20    CONTINUE
      END IF
      RETURN
!*
!*     End of DGEBD2
!*
      END

!==========================================================================

!*> \brief \b DLARTG generates a plane rotation with real cosine and real sine.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLARTG + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlartg.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlartg.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlartg.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLARTG( F, G, CS, SN, R )
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION   CS, F, G, R, SN
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLARTG generate a plane rotation so that
!*>
!*>    [  CS  SN  ]  .  [ F ]  =  [ R ]   where CS**2 + SN**2 = 1.
!*>    [ -SN  CS  ]     [ G ]     [ 0 ]
!*>
!*> This is a slower, more accurate version of the BLAS1 routine DROTG,
!*> with the following other differences:
!*>    F and G are unchanged on return.
!*>    If G=0, then CS=1 and SN=0.
!*>    If F=0 and (G .ne. 0), then CS=0 and SN=1 without doing any
!*>       floating point operations (saves work in DBDSQR when
!*>       there are zeros on the diagonal).
!*>
!*> If F exceeds G in magnitude, CS will be positive.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] F
!*> \verbatim
!*>          F is DOUBLE PRECISION
!*>          The first component of vector to be rotated.
!*> \endverbatim
!*>
!*> \param[in] G
!*> \verbatim
!*>          G is DOUBLE PRECISION
!*>          The second component of vector to be rotated.
!*> \endverbatim
!*>
!*> \param[out] CS
!*> \verbatim
!*>          CS is DOUBLE PRECISION
!*>          The cosine of the rotation.
!*> \endverbatim
!*>
!*> \param[out] SN
!*> \verbatim
!*>          SN is DOUBLE PRECISION
!*>          The sine of the rotation.
!*> \endverbatim
!*>
!*> \param[out] R
!*> \verbatim
!*>          R is DOUBLE PRECISION
!*>          The nonzero component of the rotated vector.
!*>
!*>  This version has a few statements commented out for thread safety
!*>  (machine parameters are computed on each entry). 10 feb 03, SJH.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      SUBROUTINE DLARTG( F, G, CS, SN, R )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION   CS, F, G, R, SN
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D0 )
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D0 )
      DOUBLE PRECISION   TWO
      PARAMETER          ( TWO = 2.0D0 )
!*     ..
!*     .. Local Scalars ..
!*     LOGICAL            FIRST
      INTEGER            COUNT, I
      DOUBLE PRECISION   EPS, F1, G1, SAFMIN, SAFMN2, SAFMX2, SCALE
!*     ..
!*     .. External Functions ..
      DOUBLE PRECISION   DLAMCH
      EXTERNAL           DLAMCH
!*     ..
!*     .. Save statement ..
!*     SAVE               FIRST, SAFMX2, SAFMIN, SAFMN2
!*     ..
!*     .. Data statements ..
!*     DATA               FIRST / .TRUE. /
!*     ..
!*     .. Executable Statements ..
!*
!*     IF( FIRST ) THEN
         SAFMIN = DLAMCH( 'S' )
         EPS = DLAMCH( 'E' )
         SAFMN2 = DLAMCH( 'B' )**INT( LOG( SAFMIN / EPS ) / &
                  LOG( DLAMCH( 'B' ) ) / TWO )
         SAFMX2 = ONE / SAFMN2
!*        FIRST = .FALSE.
!*     END IF
      IF( G.EQ.ZERO ) THEN
         CS = ONE
         SN = ZERO
         R = F
      ELSE IF( F.EQ.ZERO ) THEN
         CS = ZERO
         SN = ONE
         R = G
      ELSE
         F1 = F
         G1 = G
         SCALE = MAX( ABS( F1 ), ABS( G1 ) )
         IF( SCALE.GE.SAFMX2 ) THEN
            COUNT = 0
   10       CONTINUE
            COUNT = COUNT + 1
            F1 = F1*SAFMN2
            G1 = G1*SAFMN2
            SCALE = MAX( ABS( F1 ), ABS( G1 ) )
            IF( SCALE.GE.SAFMX2 ) &
               GO TO 10
            R = SQRT( F1**2+G1**2 )
            CS = F1 / R
            SN = G1 / R
            DO 20 I = 1, COUNT
               R = R*SAFMX2
   20       CONTINUE
         ELSE IF( SCALE.LE.SAFMN2 ) THEN
            COUNT = 0
   30       CONTINUE
            COUNT = COUNT + 1
            F1 = F1*SAFMX2
            G1 = G1*SAFMX2
            SCALE = MAX( ABS( F1 ), ABS( G1 ) )
            IF( SCALE.LE.SAFMN2 ) &
               GO TO 30
            R = SQRT( F1**2+G1**2 )
            CS = F1 / R
            SN = G1 / R
            DO 40 I = 1, COUNT
               R = R*SAFMN2
   40       CONTINUE
         ELSE
            R = SQRT( F1**2+G1**2 )
            CS = F1 / R
            SN = G1 / R
         END IF
         IF( ABS( F ).GT.ABS( G ) .AND. CS.LT.ZERO ) THEN
            CS = -CS
            SN = -SN
            R = -R
         END IF
      END IF
      RETURN
!*
!*     End of DLARTG
!*
      END

!==========================================================================

!*> \brief \b DLASR applies a sequence of plane rotations to a general rectangular matrix.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASR + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasr.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasr.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasr.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASR( SIDE, PIVOT, DIRECT, M, N, C, S, A, LDA )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          DIRECT, PIVOT, SIDE
!*       INTEGER            LDA, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), C( * ), S( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASR applies a sequence of plane rotations to a real matrix A,
!*> from either the left or the right.
!*>
!*> When SIDE = 'L', the transformation takes the form
!*>
!*>    A := P*A
!*>
!*> and when SIDE = 'R', the transformation takes the form
!*>
!*>    A := A*P**T
!*>
!*> where P is an orthogonal matrix consisting of a sequence of z plane
!*> rotations, with z = M when SIDE = 'L' and z = N when SIDE = 'R',
!*> and P**T is the transpose of P.
!*>
!*> When DIRECT = 'F' (Forward sequence), then
!*>
!*>    P = P(z-1) * ... * P(2) * P(1)
!*>
!*> and when DIRECT = 'B' (Backward sequence), then
!*>
!*>    P = P(1) * P(2) * ... * P(z-1)
!*>
!*> where P(k) is a plane rotation matrix defined by the 2-by-2 rotation
!*>
!*>    R(k) = (  c(k)  s(k) )
!*>         = ( -s(k)  c(k) ).
!*>
!*> When PIVOT = 'V' (Variable pivot), the rotation is performed
!*> for the plane (k,k+1), i.e., P(k) has the form
!*>
!*>    P(k) = (  1                                            )
!*>           (       ...                                     )
!*>           (              1                                )
!*>           (                   c(k)  s(k)                  )
!*>           (                  -s(k)  c(k)                  )
!*>           (                                1              )
!*>           (                                     ...       )
!*>           (                                            1  )
!*>
!*> where R(k) appears as a rank-2 modification to the identity matrix in
!*> rows and columns k and k+1.
!*>
!*> When PIVOT = 'T' (Top pivot), the rotation is performed for the
!*> plane (1,k+1), so P(k) has the form
!*>
!*>    P(k) = (  c(k)                    s(k)                 )
!*>           (         1                                     )
!*>           (              ...                              )
!*>           (                     1                         )
!*>           ( -s(k)                    c(k)                 )
!*>           (                                 1             )
!*>           (                                      ...      )
!*>           (                                             1 )
!*>
!*> where R(k) appears in rows and columns 1 and k+1.
!*>
!*> Similarly, when PIVOT = 'B' (Bottom pivot), the rotation is
!*> performed for the plane (k,z), giving P(k) the form
!*>
!*>    P(k) = ( 1                                             )
!*>           (      ...                                      )
!*>           (             1                                 )
!*>           (                  c(k)                    s(k) )
!*>           (                         1                     )
!*>           (                              ...              )
!*>           (                                     1         )
!*>           (                 -s(k)                    c(k) )
!*>
!*> where R(k) appears in rows and columns k and z.  The rotations are
!*> performed without ever forming P(k) explicitly.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>          Specifies whether the plane rotation matrix P is applied to
!*>          A on the left or the right.
!*>          = 'L':  Left, compute A := P*A
!*>          = 'R':  Right, compute A:= A*P**T
!*> \endverbatim
!*>
!*> \param[in] PIVOT
!*> \verbatim
!*>          PIVOT is CHARACTER*1
!*>          Specifies the plane for which P(k) is a plane rotation
!*>          matrix.
!*>          = 'V':  Variable pivot, the plane (k,k+1)
!*>          = 'T':  Top pivot, the plane (1,k+1)
!*>          = 'B':  Bottom pivot, the plane (k,z)
!*> \endverbatim
!*>
!*> \param[in] DIRECT
!*> \verbatim
!*>          DIRECT is CHARACTER*1
!*>          Specifies whether P is a forward or backward sequence of
!*>          plane rotations.
!*>          = 'F':  Forward, P = P(z-1)*...*P(2)*P(1)
!*>          = 'B':  Backward, P = P(1)*P(2)*...*P(z-1)
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.  If m <= 1, an immediate
!*>          return is effected.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.  If n <= 1, an
!*>          immediate return is effected.
!*> \endverbatim
!*>
!*> \param[in] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension
!*>                  (M-1) if SIDE = 'L'
!*>                  (N-1) if SIDE = 'R'
!*>          The cosines c(k) of the plane rotations.
!*> \endverbatim
!*>
!*> \param[in] S
!*> \verbatim
!*>          S is DOUBLE PRECISION array, dimension
!*>                  (M-1) if SIDE = 'L'
!*>                  (N-1) if SIDE = 'R'
!*>          The sines s(k) of the plane rotations.  The 2-by-2 plane
!*>          rotation part of the matrix P(k), R(k), has the form
!*>          R(k) = (  c(k)  s(k) )
!*>                 ( -s(k)  c(k) ).
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          The M-by-N matrix A.  On exit, A is overwritten by P*A if
!*>          SIDE = 'R' or by A*P**T if SIDE = 'L'.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.  LDA >= max(1,M).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      SUBROUTINE DLASR( SIDE, PIVOT, DIRECT, M, N, C, S, A, LDA )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          DIRECT, PIVOT, SIDE
      INTEGER            LDA, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), C( * ), S( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE, ZERO
      PARAMETER          ( ONE = 1.0D+0, ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I, INFO, J
      DOUBLE PRECISION   CTEMP, STEMP, TEMP
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input parameters
!*
      INFO = 0
      IF( .NOT.( LSAME( SIDE, 'L' ) .OR. LSAME( SIDE, 'R' ) ) ) THEN
         INFO = 1
      ELSE IF( .NOT.( LSAME( PIVOT, 'V' ) .OR. LSAME( PIVOT, &
               'T' ) .OR. LSAME( PIVOT, 'B' ) ) ) THEN
         INFO = 2
      ELSE IF( .NOT.( LSAME( DIRECT, 'F' ) .OR. LSAME( DIRECT, 'B' ) ) ) &
                THEN
         INFO = 3
      ELSE IF( M.LT.0 ) THEN
         INFO = 4
      ELSE IF( N.LT.0 ) THEN
         INFO = 5
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = 9
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DLASR ', INFO )
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( ( M.EQ.0 ) .OR. ( N.EQ.0 ) ) &
         RETURN
      IF( LSAME( SIDE, 'L' ) ) THEN
!*
!*        Form  P * A
!*
         IF( LSAME( PIVOT, 'V' ) ) THEN
            IF( LSAME( DIRECT, 'F' ) ) THEN
               DO 20 J = 1, M - 1
                  CTEMP = C( J )
                  STEMP = S( J )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 10 I = 1, N
                        TEMP = A( J+1, I )
                        A( J+1, I ) = CTEMP*TEMP - STEMP*A( J, I )
                        A( J, I ) = STEMP*TEMP + CTEMP*A( J, I )
   10                CONTINUE
                  END IF
   20          CONTINUE
            ELSE IF( LSAME( DIRECT, 'B' ) ) THEN
               DO 40 J = M - 1, 1, -1
                  CTEMP = C( J )
                  STEMP = S( J )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 30 I = 1, N
                        TEMP = A( J+1, I )
                        A( J+1, I ) = CTEMP*TEMP - STEMP*A( J, I )
                        A( J, I ) = STEMP*TEMP + CTEMP*A( J, I )
   30                CONTINUE
                  END IF
   40          CONTINUE
            END IF
         ELSE IF( LSAME( PIVOT, 'T' ) ) THEN
            IF( LSAME( DIRECT, 'F' ) ) THEN
               DO 60 J = 2, M
                  CTEMP = C( J-1 )
                  STEMP = S( J-1 )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 50 I = 1, N
                        TEMP = A( J, I )
                        A( J, I ) = CTEMP*TEMP - STEMP*A( 1, I )
                        A( 1, I ) = STEMP*TEMP + CTEMP*A( 1, I )
   50                CONTINUE
                  END IF
   60          CONTINUE
            ELSE IF( LSAME( DIRECT, 'B' ) ) THEN
               DO 80 J = M, 2, -1
                  CTEMP = C( J-1 )
                  STEMP = S( J-1 )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 70 I = 1, N
                        TEMP = A( J, I )
                        A( J, I ) = CTEMP*TEMP - STEMP*A( 1, I )
                        A( 1, I ) = STEMP*TEMP + CTEMP*A( 1, I )
   70                CONTINUE
                  END IF
   80          CONTINUE
            END IF
         ELSE IF( LSAME( PIVOT, 'B' ) ) THEN
            IF( LSAME( DIRECT, 'F' ) ) THEN
               DO 100 J = 1, M - 1
                  CTEMP = C( J )
                  STEMP = S( J )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 90 I = 1, N
                        TEMP = A( J, I )
                        A( J, I ) = STEMP*A( M, I ) + CTEMP*TEMP
                        A( M, I ) = CTEMP*A( M, I ) - STEMP*TEMP
   90                CONTINUE
                  END IF
  100          CONTINUE
            ELSE IF( LSAME( DIRECT, 'B' ) ) THEN
               DO 120 J = M - 1, 1, -1
                  CTEMP = C( J )
                  STEMP = S( J )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 110 I = 1, N
                        TEMP = A( J, I )
                        A( J, I ) = STEMP*A( M, I ) + CTEMP*TEMP
                        A( M, I ) = CTEMP*A( M, I ) - STEMP*TEMP
  110                CONTINUE
                  END IF
  120          CONTINUE
            END IF
         END IF
      ELSE IF( LSAME( SIDE, 'R' ) ) THEN
!*
!*        Form A * P**T
!*
         IF( LSAME( PIVOT, 'V' ) ) THEN
            IF( LSAME( DIRECT, 'F' ) ) THEN
               DO 140 J = 1, N - 1
                  CTEMP = C( J )
                  STEMP = S( J )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 130 I = 1, M
                        TEMP = A( I, J+1 )
                        A( I, J+1 ) = CTEMP*TEMP - STEMP*A( I, J )
                        A( I, J ) = STEMP*TEMP + CTEMP*A( I, J )
  130                CONTINUE
                  END IF
  140          CONTINUE
            ELSE IF( LSAME( DIRECT, 'B' ) ) THEN
               DO 160 J = N - 1, 1, -1
                  CTEMP = C( J )
                  STEMP = S( J )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 150 I = 1, M
                        TEMP = A( I, J+1 )
                        A( I, J+1 ) = CTEMP*TEMP - STEMP*A( I, J )
                        A( I, J ) = STEMP*TEMP + CTEMP*A( I, J )
  150                CONTINUE
                  END IF
  160          CONTINUE
            END IF
         ELSE IF( LSAME( PIVOT, 'T' ) ) THEN
            IF( LSAME( DIRECT, 'F' ) ) THEN
               DO 180 J = 2, N
                  CTEMP = C( J-1 )
                  STEMP = S( J-1 )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 170 I = 1, M
                        TEMP = A( I, J )
                        A( I, J ) = CTEMP*TEMP - STEMP*A( I, 1 )
                        A( I, 1 ) = STEMP*TEMP + CTEMP*A( I, 1 )
  170                CONTINUE
                  END IF
  180          CONTINUE
            ELSE IF( LSAME( DIRECT, 'B' ) ) THEN
               DO 200 J = N, 2, -1
                  CTEMP = C( J-1 )
                  STEMP = S( J-1 )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 190 I = 1, M
                        TEMP = A( I, J )
                        A( I, J ) = CTEMP*TEMP - STEMP*A( I, 1 )
                        A( I, 1 ) = STEMP*TEMP + CTEMP*A( I, 1 )
  190                CONTINUE
                  END IF
  200          CONTINUE
            END IF
         ELSE IF( LSAME( PIVOT, 'B' ) ) THEN
            IF( LSAME( DIRECT, 'F' ) ) THEN
               DO 220 J = 1, N - 1
                  CTEMP = C( J )
                  STEMP = S( J )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 210 I = 1, M
                        TEMP = A( I, J )
                        A( I, J ) = STEMP*A( I, N ) + CTEMP*TEMP
                        A( I, N ) = CTEMP*A( I, N ) - STEMP*TEMP
  210                CONTINUE
                  END IF
  220          CONTINUE
            ELSE IF( LSAME( DIRECT, 'B' ) ) THEN
               DO 240 J = N - 1, 1, -1
                  CTEMP = C( J )
                  STEMP = S( J )
                  IF( ( CTEMP.NE.ONE ) .OR. ( STEMP.NE.ZERO ) ) THEN
                     DO 230 I = 1, M
                        TEMP = A( I, J )
                        A( I, J ) = STEMP*A( I, N ) + CTEMP*TEMP
                        A( I, N ) = CTEMP*A( I, N ) - STEMP*TEMP
  230                CONTINUE
                  END IF
  240          CONTINUE
            END IF
         END IF
      END IF
!*
      RETURN
!*
!*     End of DLASR
!*
      END

!==========================================================================

!*> \brief \b DLASV2 computes the singular value decomposition of a 2-by-2 triangular matrix.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASV2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasv2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasv2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasv2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASV2( F, G, H, SSMIN, SSMAX, SNR, CSR, SNL, CSL )
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION   CSL, CSR, F, G, H, SNL, SNR, SSMAX, SSMIN
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASV2 computes the singular value decomposition of a 2-by-2
!*> triangular matrix
!*>    [  F   G  ]
!*>    [  0   H  ].
!*> On return, abs(SSMAX) is the larger singular value, abs(SSMIN) is the
!*> smaller singular value, and (CSL,SNL) and (CSR,SNR) are the left and
!*> right singular vectors for abs(SSMAX), giving the decomposition
!*>
!*>    [ CSL  SNL ] [  F   G  ] [ CSR -SNR ]  =  [ SSMAX   0   ]
!*>    [-SNL  CSL ] [  0   H  ] [ SNR  CSR ]     [  0    SSMIN ].
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] F
!*> \verbatim
!*>          F is DOUBLE PRECISION
!*>          The (1,1) element of the 2-by-2 matrix.
!*> \endverbatim
!*>
!*> \param[in] G
!*> \verbatim
!*>          G is DOUBLE PRECISION
!*>          The (1,2) element of the 2-by-2 matrix.
!*> \endverbatim
!*>
!*> \param[in] H
!*> \verbatim
!*>          H is DOUBLE PRECISION
!*>          The (2,2) element of the 2-by-2 matrix.
!*> \endverbatim
!*>
!*> \param[out] SSMIN
!*> \verbatim
!*>          SSMIN is DOUBLE PRECISION
!*>          abs(SSMIN) is the smaller singular value.
!*> \endverbatim
!*>
!*> \param[out] SSMAX
!*> \verbatim
!*>          SSMAX is DOUBLE PRECISION
!*>          abs(SSMAX) is the larger singular value.
!*> \endverbatim
!*>
!*> \param[out] SNL
!*> \verbatim
!*>          SNL is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[out] CSL
!*> \verbatim
!*>          CSL is DOUBLE PRECISION
!*>          The vector (CSL, SNL) is a unit left singular vector for the
!*>          singular value abs(SSMAX).
!*> \endverbatim
!*>
!*> \param[out] SNR
!*> \verbatim
!*>          SNR is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[out] CSR
!*> \verbatim
!*>          CSR is DOUBLE PRECISION
!*>          The vector (CSR, SNR) is a unit right singular vector for the
!*>          singular value abs(SSMAX).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  Any input parameter may be aliased with any output parameter.
!*>
!*>  Barring over/underflow and assuming a guard digit in subtraction, all
!*>  output quantities are correct to within a few units in the last
!*>  place (ulps).
!*>
!*>  In IEEE arithmetic, the code works correctly if one matrix element is
!*>  infinite.
!*>
!*>  Overflow will not occur unless the largest singular value itself
!*>  overflows or is within a few ulps of overflow. (On machines with
!*>  partial overflow, like the Cray, overflow may occur if the largest
!*>  singular value is within a factor of 2 of overflow.)
!*>
!*>  Underflow is harmless if underflow is gradual. Otherwise, results
!*>  may correspond to a matrix modified by perturbations of size near
!*>  the underflow threshold.
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DLASV2( F, G, H, SSMIN, SSMAX, SNR, CSR, SNL, CSL )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION   CSL, CSR, F, G, H, SNL, SNR, SSMAX, SSMIN
!*     ..
!*
!* =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D0 )
      DOUBLE PRECISION   HALF
      PARAMETER          ( HALF = 0.5D0 )
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D0 )
      DOUBLE PRECISION   TWO
      PARAMETER          ( TWO = 2.0D0 )
      DOUBLE PRECISION   FOUR
      PARAMETER          ( FOUR = 4.0D0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            GASMAL, SWAP
      INTEGER            PMAX
      DOUBLE PRECISION   A, CLT, CRT, D, FA, FT, GA, GT, HA, HT, L, M, &
                         MM, R, S, SLT, SRT, T, TEMP, TSIGN, TT
!*     ..
!*     .. External Functions ..
      DOUBLE PRECISION   DLAMCH
      EXTERNAL           DLAMCH
!*     ..
!*     .. Executable Statements ..
!*
      FT = F
      FA = ABS( FT )
      HT = H
      HA = ABS( H )
!*
!*     PMAX points to the maximum absolute element of matrix
!*       PMAX = 1 if F largest in absolute values
!*       PMAX = 2 if G largest in absolute values
!*       PMAX = 3 if H largest in absolute values
!*
      PMAX = 1
      SWAP = ( HA.GT.FA )
      IF( SWAP ) THEN
         PMAX = 3
         TEMP = FT
         FT = HT
         HT = TEMP
         TEMP = FA
         FA = HA
         HA = TEMP
!*
!*        Now FA .ge. HA
!*
      END IF
      GT = G
      GA = ABS( GT )
      IF( GA.EQ.ZERO ) THEN
!*
!*        Diagonal matrix
!*
         SSMIN = HA
         SSMAX = FA
         CLT = ONE
         CRT = ONE
         SLT = ZERO
         SRT = ZERO
      ELSE
         GASMAL = .TRUE.
         IF( GA.GT.FA ) THEN
            PMAX = 2
            IF( ( FA / GA ).LT.DLAMCH( 'EPS' ) ) THEN
!*
!*              Case of very large GA
!*
               GASMAL = .FALSE.
               SSMAX = GA
               IF( HA.GT.ONE ) THEN
                  SSMIN = FA / ( GA / HA )
               ELSE
                  SSMIN = ( FA / GA )*HA
               END IF
               CLT = ONE
               SLT = HT / GT
               SRT = ONE
               CRT = FT / GT
            END IF
         END IF
         IF( GASMAL ) THEN
!*
!*           Normal case
!*
            D = FA - HA
            IF( D.EQ.FA ) THEN
!*
!*              Copes with infinite F or H
!*
               L = ONE
            ELSE
               L = D / FA
            END IF
!*
!*           Note that 0 .le. L .le. 1
!*
            M = GT / FT
!*
!*           Note that abs(M) .le. 1/macheps
!*
            T = TWO - L
!*
!*           Note that T .ge. 1
!*
            MM = M*M
            TT = T*T
            S = SQRT( TT+MM )
!*
!*           Note that 1 .le. S .le. 1 + 1/macheps
!*
            IF( L.EQ.ZERO ) THEN
               R = ABS( M )
            ELSE
               R = SQRT( L*L+MM )
            END IF
!*
!*           Note that 0 .le. R .le. 1 + 1/macheps
!*
            A = HALF*( S+R )
!*
!*           Note that 1 .le. A .le. 1 + abs(M)
!*
            SSMIN = HA / A
            SSMAX = FA*A
            IF( MM.EQ.ZERO ) THEN
!*
!*              Note that M is very tiny
!*
               IF( L.EQ.ZERO ) THEN
                  T = SIGN( TWO, FT )*SIGN( ONE, GT )
               ELSE
                  T = GT / SIGN( D, FT ) + M / T
               END IF
            ELSE
               T = ( M / ( S+T )+M / ( R+L ) )*( ONE+A )
            END IF
            L = SQRT( T*T+FOUR )
            CRT = TWO / L
            SRT = T / L
            CLT = ( CRT+SRT*M ) / A
            SLT = ( HT / FT )*SRT / A
         END IF
      END IF
      IF( SWAP ) THEN
         CSL = SRT
         SNL = CRT
         CSR = SLT
         SNR = CLT
      ELSE
         CSL = CLT
         SNL = SLT
         CSR = CRT
         SNR = SRT
      END IF
!*
!*     Correct signs of SSMAX and SSMIN
!*
      IF( PMAX.EQ.1 ) &
         TSIGN = SIGN( ONE, CSR )*SIGN( ONE, CSL )*SIGN( ONE, F )
      IF( PMAX.EQ.2 ) &
         TSIGN = SIGN( ONE, SNR )*SIGN( ONE, CSL )*SIGN( ONE, G )
      IF( PMAX.EQ.3 ) &
         TSIGN = SIGN( ONE, SNR )*SIGN( ONE, SNL )*SIGN( ONE, H )
      SSMAX = SIGN( SSMAX, TSIGN )
      SSMIN = SIGN( SSMIN, TSIGN*SIGN( ONE, F )*SIGN( ONE, H ) )
      RETURN
!*
!*     End of DLASV2
!*
      END

!==========================================================================

!*> \brief \b DROT
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DROT(N,DX,INCX,DY,INCY,C,S)
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION C,S
!*       INTEGER INCX,INCY,N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION DX(*),DY(*)
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*>    DROT applies a plane rotation.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup double_blas_level1
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>     jack dongarra, linpack, 3/11/78.
!*>     modified 12/3/93, array(1) declarations changed to array(*)
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DROT(N,DX,INCX,DY,INCY,C,S)
!
      IMPLICIT NONE
!*
!*  -- Reference BLAS level1 routine (version 3.4.0) --
!*  -- Reference BLAS is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION C,S
      INTEGER INCX,INCY,N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION DX(*),DY(*)
!*     ..
!*
!*  =====================================================================
!*
!*     .. Local Scalars ..
      DOUBLE PRECISION DTEMP
      INTEGER I,IX,IY
!*     ..
      IF (N.LE.0) RETURN
      IF (INCX.EQ.1 .AND. INCY.EQ.1) THEN
!*
!*       code for both increments equal to 1
!*
         DO I = 1,N
            DTEMP = C*DX(I) + S*DY(I)
            DY(I) = C*DY(I) - S*DX(I)
            DX(I) = DTEMP
         END DO
      ELSE
!*
!*       code for unequal increments or equal increments not equal
!*         to 1
!*
         IX = 1
         IY = 1
         IF (INCX.LT.0) IX = (-N+1)*INCX + 1
         IF (INCY.LT.0) IY = (-N+1)*INCY + 1
         DO I = 1,N
            DTEMP = C*DX(IX) + S*DY(IY)
            DY(IY) = C*DY(IY) - S*DX(IX)
            DX(IX) = DTEMP
            IX = IX + INCX
            IY = IY + INCY
         END DO
      END IF
      RETURN
      END

!==========================================================================

!*> \brief \b DLASQ1 computes the singular values of a real square bidiagonal matrix. Used by sbdsqr.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASQ1 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasq1.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasq1.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasq1.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASQ1( N, D, E, WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   D( * ), E( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASQ1 computes the singular values of a real N-by-N bidiagonal
!*> matrix with diagonal D and off-diagonal E. The singular values
!*> are computed to high relative accuracy, in the absence of
!*> denormalization, underflow and overflow. The algorithm was first
!*> presented in
!*>
!*> "Accurate singular values and differential qd algorithms" by K. V.
!*> Fernando and B. N. Parlett, Numer. Math., Vol-67, No. 2, pp. 191-230,
!*> 1994,
!*>
!*> and the present implementation is described in "An implementation of
!*> the dqds Algorithm (Positive Case)", LAPACK Working Note.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>        The number of rows and columns in the matrix. N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] D
!*> \verbatim
!*>          D is DOUBLE PRECISION array, dimension (N)
!*>        On entry, D contains the diagonal elements of the
!*>        bidiagonal matrix whose SVD is desired. On normal exit,
!*>        D contains the singular values in decreasing order.
!*> \endverbatim
!*>
!*> \param[in,out] E
!*> \verbatim
!*>          E is DOUBLE PRECISION array, dimension (N)
!*>        On entry, elements E(1:N-1) contain the off-diagonal elements
!*>        of the bidiagonal matrix whose SVD is desired.
!*>        On exit, E is overwritten.
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension (4*N)
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>        = 0: successful exit
!*>        < 0: if INFO = -i, the i-th argument had an illegal value
!*>        > 0: the algorithm failed
!*>             = 1, a split was marked by a positive value in E
!*>             = 2, current block of Z not diagonalized after 100*N
!*>                  iterations (in inner while loop)  On exit D and E
!*>                  represent a matrix with the same singular values
!*>                  which the calling subroutine could use to finish the
!*>                  computation, or even feed back into DLASQ1
!*>             = 3, termination criterion of outer while loop not met
!*>                  (program created more than N unreduced blocks)
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DLASQ1( N, D, E, WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   D( * ), E( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I, IINFO
      DOUBLE PRECISION   EPS, SCALE, SAFMIN, SIGMN, SIGMX
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DCOPY, DLAS2, DLASCL, DLASQ2, DLASRT, XERBLA
!*     ..
!*     .. External Functions ..
      DOUBLE PRECISION   DLAMCH
      EXTERNAL           DLAMCH
!*     ..
!*     .. Executable Statements ..
!*
      INFO = 0
      IF( N.LT.0 ) THEN
         INFO = -2
         CALL XERBLA( 'DLASQ1', -INFO )
         RETURN
      ELSE IF( N.EQ.0 ) THEN
         RETURN
      ELSE IF( N.EQ.1 ) THEN
         D( 1 ) = ABS( D( 1 ) )
         RETURN
      ELSE IF( N.EQ.2 ) THEN
         CALL DLAS2( D( 1 ), E( 1 ), D( 2 ), SIGMN, SIGMX )
         D( 1 ) = SIGMX
         D( 2 ) = SIGMN
         RETURN
      END IF
!*
!*     Estimate the largest singular value.
!*
      SIGMX = ZERO
      DO 10 I = 1, N - 1
         D( I ) = ABS( D( I ) )
         SIGMX = MAX( SIGMX, ABS( E( I ) ) )
   10 CONTINUE
      D( N ) = ABS( D( N ) )
!*
!*     Early return if SIGMX is zero (matrix is already diagonal).
!*
      IF( SIGMX.EQ.ZERO ) THEN
         CALL DLASRT( 'D', N, D, IINFO )
         RETURN
      END IF
!*
      DO 20 I = 1, N
         SIGMX = MAX( SIGMX, D( I ) )
   20 CONTINUE
!*
!*     Copy D and E into WORK (in the Z format) and scale (squaring the
!*     input data makes scaling by a power of the radix pointless).
!*
      EPS = DLAMCH( 'Precision' )
      SAFMIN = DLAMCH( 'Safe minimum' )
      SCALE = SQRT( EPS / SAFMIN )
      CALL DCOPY( N, D, 1, WORK( 1 ), 2 )
      CALL DCOPY( N-1, E, 1, WORK( 2 ), 2 )
      CALL DLASCL( 'G', 0, 0, SIGMX, SCALE, 2*N-1, 1, WORK, 2*N-1, &
                   IINFO )
!*
!*     Compute the q's and e's.
!*
      DO 30 I = 1, 2*N - 1
         WORK( I ) = WORK( I )**2
   30 CONTINUE
      WORK( 2*N ) = ZERO
!*
      CALL DLASQ2( N, WORK, INFO )
!*
      IF( INFO.EQ.0 ) THEN
         DO 40 I = 1, N
            D( I ) = SQRT( WORK( I ) )
   40    CONTINUE
         CALL DLASCL( 'G', 0, 0, SCALE, SIGMX, N, 1, D, N, IINFO )
      ELSE IF( INFO.EQ.2 ) THEN
!*
!*     Maximum number of iterations exceeded.  Move data from WORK
!*     into D and E so the calling subroutine can try to finish
!*
         DO I = 1, N
            D( I ) = SQRT( WORK( 2*I-1 ) )
            E( I ) = SQRT( WORK( 2*I ) )
         END DO
         CALL DLASCL( 'G', 0, 0, SCALE, SIGMX, N, 1, D, N, IINFO )
         CALL DLASCL( 'G', 0, 0, SCALE, SIGMX, N, 1, E, N, IINFO )
      END IF
!*
      RETURN
!*
!*     End of DLASQ1
!*
      END

!==========================================================================

!*> \brief \b DLAS2 computes singular values of a 2-by-2 triangular matrix.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLAS2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlas2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlas2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlas2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLAS2( F, G, H, SSMIN, SSMAX )
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION   F, G, H, SSMAX, SSMIN
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLAS2  computes the singular values of the 2-by-2 matrix
!*>    [  F   G  ]
!*>    [  0   H  ].
!*> On return, SSMIN is the smaller singular value and SSMAX is the
!*> larger singular value.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] F
!*> \verbatim
!*>          F is DOUBLE PRECISION
!*>          The (1,1) element of the 2-by-2 matrix.
!*> \endverbatim
!*>
!*> \param[in] G
!*> \verbatim
!*>          G is DOUBLE PRECISION
!*>          The (1,2) element of the 2-by-2 matrix.
!*> \endverbatim
!*>
!*> \param[in] H
!*> \verbatim
!*>          H is DOUBLE PRECISION
!*>          The (2,2) element of the 2-by-2 matrix.
!*> \endverbatim
!*>
!*> \param[out] SSMIN
!*> \verbatim
!*>          SSMIN is DOUBLE PRECISION
!*>          The smaller singular value.
!*> \endverbatim
!*>
!*> \param[out] SSMAX
!*> \verbatim
!*>          SSMAX is DOUBLE PRECISION
!*>          The larger singular value.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  Barring over/underflow, all output quantities are correct to within
!*>  a few units in the last place (ulps), even in the absence of a guard
!*>  digit in addition/subtraction.
!*>
!*>  In IEEE arithmetic, the code works correctly if one matrix element is
!*>  infinite.
!*>
!*>  Overflow will not occur unless the largest singular value itself
!*>  overflows, or is within a few ulps of overflow. (On machines with
!*>  partial overflow, like the Cray, overflow may occur if the largest
!*>  singular value is within a factor of 2 of overflow.)
!*>
!*>  Underflow is harmless if underflow is gradual. Otherwise, results
!*>  may correspond to a matrix modified by perturbations of size near
!*>  the underflow threshold.
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DLAS2( F, G, H, SSMIN, SSMAX )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION   F, G, H, SSMAX, SSMIN
!*     ..
!*
!*  ====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D0 )
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D0 )
      DOUBLE PRECISION   TWO
      PARAMETER          ( TWO = 2.0D0 )
!*     ..
!*     .. Local Scalars ..
      DOUBLE PRECISION   AS, AT, AU, C, FA, FHMN, FHMX, GA, HA
!*     ..
!*     .. Executable Statements ..
!*
      FA = ABS( F )
      GA = ABS( G )
      HA = ABS( H )
      FHMN = MIN( FA, HA )
      FHMX = MAX( FA, HA )
      IF( FHMN.EQ.ZERO ) THEN
         SSMIN = ZERO
         IF( FHMX.EQ.ZERO ) THEN
            SSMAX = GA
         ELSE
            SSMAX = MAX( FHMX, GA )*SQRT( ONE+ &
                    ( MIN( FHMX, GA ) / MAX( FHMX, GA ) )**2 )
         END IF
      ELSE
         IF( GA.LT.FHMX ) THEN
            AS = ONE + FHMN / FHMX
            AT = ( FHMX-FHMN ) / FHMX
            AU = ( GA / FHMX )**2
            C = TWO / ( SQRT( AS*AS+AU )+SQRT( AT*AT+AU ) )
            SSMIN = FHMN*C
            SSMAX = FHMX / C
         ELSE
            AU = FHMX / GA
            IF( AU.EQ.ZERO ) THEN
!*
!*              Avoid possible harmful underflow if exponent range
!*              asymmetric (true SSMIN may not underflow even if
!*              AU underflows)
!*
               SSMIN = ( FHMN*FHMX ) / GA
               SSMAX = GA
            ELSE
               AS = ONE + FHMN / FHMX
               AT = ( FHMX-FHMN ) / FHMX
               C = ONE / ( SQRT( ONE+( AS*AU )**2 )+ &
                   SQRT( ONE+( AT*AU )**2 ) )
               SSMIN = ( FHMN*C )*AU
               SSMIN = SSMIN + SSMIN
               SSMAX = GA / ( C+C )
            END IF
         END IF
      END IF
      RETURN
!*
!*     End of DLAS2
!*
      END

!==========================================================================

!*> \brief \b DSWAP
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DSWAP(N,DX,INCX,DY,INCY)
!*
!*       .. Scalar Arguments ..
!*       INTEGER INCX,INCY,N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION DX(*),DY(*)
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*>    interchanges two vectors.
!*>    uses unrolled loops for increments equal one.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup double_blas_level1
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>     jack dongarra, linpack, 3/11/78.
!*>     modified 12/3/93, array(1) declarations changed to array(*)
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DSWAP(N,DX,INCX,DY,INCY)
!
      IMPLICIT NONE
!*
!*  -- Reference BLAS level1 routine (version 3.4.0) --
!*  -- Reference BLAS is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER INCX,INCY,N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION DX(*),DY(*)
!*     ..
!*
!*  =====================================================================
!*
!*     .. Local Scalars ..
      DOUBLE PRECISION DTEMP
      INTEGER I,IX,IY,M,MP1
!*     ..
      IF (N.LE.0) RETURN
      IF (INCX.EQ.1 .AND. INCY.EQ.1) THEN
!*
!*       code for both increments equal to 1
!*
!*
!*       clean-up loop
!*
         M = MOD(N,3)
         IF (M.NE.0) THEN
            DO I = 1,M
               DTEMP = DX(I)
               DX(I) = DY(I)
               DY(I) = DTEMP
            END DO
            IF (N.LT.3) RETURN
         END IF
         MP1 = M + 1
         DO I = MP1,N,3
            DTEMP = DX(I)
            DX(I) = DY(I)
            DY(I) = DTEMP
            DTEMP = DX(I+1)
            DX(I+1) = DY(I+1)
            DY(I+1) = DTEMP
            DTEMP = DX(I+2)
            DX(I+2) = DY(I+2)
            DY(I+2) = DTEMP
         END DO
      ELSE
!*
!*       code for unequal increments or equal increments not equal
!*         to 1
!*
         IX = 1
         IY = 1
         IF (INCX.LT.0) IX = (-N+1)*INCX + 1
         IF (INCY.LT.0) IY = (-N+1)*INCY + 1
         DO I = 1,N
            DTEMP = DX(IX)
            DX(IX) = DY(IY)
            DY(IY) = DTEMP
            IX = IX + INCX
            IY = IY + INCY
         END DO
      END IF
      RETURN
      END

!==========================================================================

!*> \brief \b DLAISNAN tests input for NaN by comparing two arguments for inequality.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLAISNAN + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlaisnan.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlaisnan.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlaisnan.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       LOGICAL FUNCTION DLAISNAN( DIN1, DIN2 )
!
!        IMPLICIT NONE
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION   DIN1, DIN2
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> This routine is not for general use.  It exists solely to avoid
!*> over-optimization in DISNAN.
!*>
!*> DLAISNAN checks for NaNs by comparing its two arguments for
!*> inequality.  NaN is the only floating-point value where NaN != NaN
!*> returns .TRUE.  To check for NaNs, pass the same variable as both
!*> arguments.
!*>
!*> A compiler must assume that the two arguments are
!*> not the same variable, and the test will not be optimized away.
!*> Interprocedural or whole-program optimization may delete this
!*> test.  The ISNAN functions will be replaced by the correct
!*> Fortran 03 intrinsic once the intrinsic is widely available.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] DIN1
!*> \verbatim
!*>          DIN1 is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in] DIN2
!*> \verbatim
!*>          DIN2 is DOUBLE PRECISION
!*>          Two numbers to compare for inequality.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      LOGICAL FUNCTION DLAISNAN( DIN1, DIN2 )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION   DIN1, DIN2
!*     ..
!*
!*  =====================================================================
!*
!*  .. Executable Statements ..
      DLAISNAN = (DIN1.NE.DIN2)
      RETURN
      END

!==========================================================================

!*> \brief \b DLARF applies an elementary reflector to a general rectangular matrix.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLARF + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlarf.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlarf.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlarf.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLARF( SIDE, M, N, V, INCV, TAU, C, LDC, WORK )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          SIDE
!*       INTEGER            INCV, LDC, M, N
!*       DOUBLE PRECISION   TAU
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   C( LDC, * ), V( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLARF applies a real elementary reflector H to a real m by n matrix
!*> C, from either the left or the right. H is represented in the form
!*>
!*>       H = I - tau * v * v**T
!*>
!*> where tau is a real scalar and v is a real vector.
!*>
!*> If tau = 0, then H is taken to be the unit matrix.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>          = 'L': form  H * C
!*>          = 'R': form  C * H
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix C.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix C.
!*> \endverbatim
!*>
!*> \param[in] V
!*> \verbatim
!*>          V is DOUBLE PRECISION array, dimension
!*>                     (1 + (M-1)*abs(INCV)) if SIDE = 'L'
!*>                  or (1 + (N-1)*abs(INCV)) if SIDE = 'R'
!*>          The vector v in the representation of H. V is not used if
!*>          TAU = 0.
!*> \endverbatim
!*>
!*> \param[in] INCV
!*> \verbatim
!*>          INCV is INTEGER
!*>          The increment between elements of v. INCV <> 0.
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION
!*>          The value tau in the representation of H.
!*> \endverbatim
!*>
!*> \param[in,out] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension (LDC,N)
!*>          On entry, the m by n matrix C.
!*>          On exit, C is overwritten by the matrix H * C if SIDE = 'L',
!*>          or C * H if SIDE = 'R'.
!*> \endverbatim
!*>
!*> \param[in] LDC
!*> \verbatim
!*>          LDC is INTEGER
!*>          The leading dimension of the array C. LDC >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension
!*>                         (N) if SIDE = 'L'
!*>                      or (M) if SIDE = 'R'
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleOTHERauxiliary
!*
!*  =====================================================================
      SUBROUTINE DLARF( SIDE, M, N, V, INCV, TAU, C, LDC, WORK )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          SIDE
      INTEGER            INCV, LDC, M, N
      DOUBLE PRECISION   TAU
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   C( LDC, * ), V( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE, ZERO
      PARAMETER          ( ONE = 1.0D+0, ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            APPLYLEFT
      INTEGER            I, LASTV, LASTC
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DGEMV, DGER
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILADLR, ILADLC
      EXTERNAL           LSAME, ILADLR, ILADLC
!*     ..
!*     .. Executable Statements ..
!*
      APPLYLEFT = LSAME( SIDE, 'L' )
      LASTV = 0
      LASTC = 0
      IF( TAU.NE.ZERO ) THEN
!     Set up variables for scanning V.  LASTV begins pointing to the end
!     of V.
         IF( APPLYLEFT ) THEN
            LASTV = M
         ELSE
            LASTV = N
         END IF
         IF( INCV.GT.0 ) THEN
            I = 1 + (LASTV-1) * INCV
         ELSE
            I = 1
         END IF
!     Look for the last non-zero row in V.
         DO WHILE( LASTV.GT.0 .AND. V( I ).EQ.ZERO )
            LASTV = LASTV - 1
            I = I - INCV
         END DO
         IF( APPLYLEFT ) THEN
!     Scan for the last non-zero column in C(1:lastv,:).
            LASTC = ILADLC(LASTV, N, C, LDC)
         ELSE
!     Scan for the last non-zero row in C(:,1:lastv).
            LASTC = ILADLR(M, LASTV, C, LDC)
         END IF
      END IF
!     Note that lastc.eq.0 renders the BLAS operations null; no special
!     case is needed at this level.
      IF( APPLYLEFT ) THEN
!*
!*        Form  H * C
!*
         IF( LASTV.GT.0 ) THEN
!*
!*           w(1:lastc,1) := C(1:lastv,1:lastc)**T * v(1:lastv,1)
!*
            CALL DGEMV( 'Transpose', LASTV, LASTC, ONE, C, LDC, V, INCV, &
                 ZERO, WORK, 1 )
!*
!*           C(1:lastv,1:lastc) := C(...) - v(1:lastv,1) * w(1:lastc,1)**T
!*
            CALL DGER( LASTV, LASTC, -TAU, V, INCV, WORK, 1, C, LDC )
         END IF
      ELSE
!*
!*        Form  C * H
!*
         IF( LASTV.GT.0 ) THEN
!*
!*           w(1:lastc,1) := C(1:lastc,1:lastv) * v(1:lastv,1)
!*
            CALL DGEMV( 'No transpose', LASTC, LASTV, ONE, C, LDC, &
                 V, INCV, ZERO, WORK, 1 )
!*
!*           C(1:lastc,1:lastv) := C(...) - w(1:lastc,1) * v(1:lastv,1)**T
!*
            CALL DGER( LASTC, LASTV, -TAU, WORK, 1, V, INCV, C, LDC )
         END IF
      END IF
      RETURN
!*
!*     End of DLARF
!*
      END

!==========================================================================

!*> \brief \b DCOPY
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DCOPY(N,DX,INCX,DY,INCY)
!*
!*       .. Scalar Arguments ..
!*       INTEGER INCX,INCY,N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION DX(*),DY(*)
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*>    DCOPY copies a vector, x, to a vector, y.
!*>    uses unrolled loops for increments equal to one.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup double_blas_level1
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>     jack dongarra, linpack, 3/11/78.
!*>     modified 12/3/93, array(1) declarations changed to array(*)
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DCOPY(N,DX,INCX,DY,INCY)
!
      IMPLICIT NONE
!*
!*  -- Reference BLAS level1 routine (version 3.4.0) --
!*  -- Reference BLAS is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER INCX,INCY,N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION DX(*),DY(*)
!*     ..
!*
!*  =====================================================================
!*
!*     .. Local Scalars ..
      INTEGER I,IX,IY,M,MP1
!*     ..
      IF (N.LE.0) RETURN
      IF (INCX.EQ.1 .AND. INCY.EQ.1) THEN
!*
!*        code for both increments equal to 1
!*
!*
!*        clean-up loop
!*
         M = MOD(N,7)
         IF (M.NE.0) THEN
            DO I = 1,M
               DY(I) = DX(I)
            END DO
            IF (N.LT.7) RETURN
         END IF
         MP1 = M + 1
         DO I = MP1,N,7
            DY(I) = DX(I)
            DY(I+1) = DX(I+1)
            DY(I+2) = DX(I+2)
            DY(I+3) = DX(I+3)
            DY(I+4) = DX(I+4)
            DY(I+5) = DX(I+5)
            DY(I+6) = DX(I+6)
         END DO
      ELSE
!*
!*        code for unequal increments or equal increments
!*          not equal to 1
!*
         IX = 1
         IY = 1
         IF (INCX.LT.0) IX = (-N+1)*INCX + 1
         IF (INCY.LT.0) IY = (-N+1)*INCY + 1
         DO I = 1,N
            DY(IY) = DX(IX)
            IX = IX + INCX
            IY = IY + INCY
         END DO
      END IF
      RETURN
      END

!==========================================================================

!*> \brief \b DTRMM
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DTRMM(SIDE,UPLO,TRANSA,DIAG,M,N,ALPHA,A,LDA,B,LDB)
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION ALPHA
!*       INTEGER LDA,LDB,M,N
!*       CHARACTER DIAG,SIDE,TRANSA,UPLO
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION A(LDA,*),B(LDB,*)
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DTRMM  performs one of the matrix-matrix operations
!*>
!*>    B := alpha*op( A )*B,   or   B := alpha*B*op( A ),
!*>
!*> where  alpha  is a scalar,  B  is an m by n matrix,  A  is a unit, or
!*> non-unit,  upper or lower triangular matrix  and  op( A )  is one  of
!*>
!*>    op( A ) = A   or   op( A ) = A**T.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>           On entry,  SIDE specifies whether  op( A ) multiplies B from
!*>           the left or right as follows:
!*>
!*>              SIDE = 'L' or 'l'   B := alpha*op( A )*B.
!*>
!*>              SIDE = 'R' or 'r'   B := alpha*B*op( A ).
!*> \endverbatim
!*>
!*> \param[in] UPLO
!*> \verbatim
!*>          UPLO is CHARACTER*1
!*>           On entry, UPLO specifies whether the matrix A is an upper or
!*>           lower triangular matrix as follows:
!*>
!*>              UPLO = 'U' or 'u'   A is an upper triangular matrix.
!*>
!*>              UPLO = 'L' or 'l'   A is a lower triangular matrix.
!*> \endverbatim
!*>
!*> \param[in] TRANSA
!*> \verbatim
!*>          TRANSA is CHARACTER*1
!*>           On entry, TRANSA specifies the form of op( A ) to be used in
!*>           the matrix multiplication as follows:
!*>
!*>              TRANSA = 'N' or 'n'   op( A ) = A.
!*>
!*>              TRANSA = 'T' or 't'   op( A ) = A**T.
!*>
!*>              TRANSA = 'C' or 'c'   op( A ) = A**T.
!*> \endverbatim
!*>
!*> \param[in] DIAG
!*> \verbatim
!*>          DIAG is CHARACTER*1
!*>           On entry, DIAG specifies whether or not A is unit triangular
!*>           as follows:
!*>
!*>              DIAG = 'U' or 'u'   A is assumed to be unit triangular.
!*>
!*>              DIAG = 'N' or 'n'   A is not assumed to be unit
!*>                                  triangular.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>           On entry, M specifies the number of rows of B. M must be at
!*>           least zero.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>           On entry, N specifies the number of columns of B.  N must be
!*>           at least zero.
!*> \endverbatim
!*>
!*> \param[in] ALPHA
!*> \verbatim
!*>          ALPHA is DOUBLE PRECISION.
!*>           On entry,  ALPHA specifies the scalar  alpha. When  alpha is
!*>           zero then  A is not referenced and  B need not be set before
!*>           entry.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>           A is DOUBLE PRECISION array of DIMENSION ( LDA, k ), where k is m
!*>           when  SIDE = 'L' or 'l'  and is  n  when  SIDE = 'R' or 'r'.
!*>           Before entry  with  UPLO = 'U' or 'u',  the  leading  k by k
!*>           upper triangular part of the array  A must contain the upper
!*>           triangular matrix  and the strictly lower triangular part of
!*>           A is not referenced.
!*>           Before entry  with  UPLO = 'L' or 'l',  the  leading  k by k
!*>           lower triangular part of the array  A must contain the lower
!*>           triangular matrix  and the strictly upper triangular part of
!*>           A is not referenced.
!*>           Note that when  DIAG = 'U' or 'u',  the diagonal elements of
!*>           A  are not referenced either,  but are assumed to be  unity.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>           On entry, LDA specifies the first dimension of A as declared
!*>           in the calling (sub) program.  When  SIDE = 'L' or 'l'  then
!*>           LDA  must be at least  max( 1, m ),  when  SIDE = 'R' or 'r'
!*>           then LDA must be at least max( 1, n ).
!*> \endverbatim
!*>
!*> \param[in,out] B
!*> \verbatim
!*>          B is DOUBLE PRECISION array of DIMENSION ( LDB, n ).
!*>           Before entry,  the leading  m by n part of the array  B must
!*>           contain the matrix  B,  and  on exit  is overwritten  by the
!*>           transformed matrix.
!*> \endverbatim
!*>
!*> \param[in] LDB
!*> \verbatim
!*>          LDB is INTEGER
!*>           On entry, LDB specifies the first dimension of B as declared
!*>           in  the  calling  (sub)  program.   LDB  must  be  at  least
!*>           max( 1, m ).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup double_blas_level3
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  Level 3 Blas routine.
!*>
!*>  -- Written on 8-February-1989.
!*>     Jack Dongarra, Argonne National Laboratory.
!*>     Iain Duff, AERE Harwell.
!*>     Jeremy Du Croz, Numerical Algorithms Group Ltd.
!*>     Sven Hammarling, Numerical Algorithms Group Ltd.
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DTRMM(SIDE,UPLO,TRANSA,DIAG,M,N,ALPHA,A,LDA,B,LDB)
!
      IMPLICIT NONE
!*
!*  -- Reference BLAS level3 routine (version 3.4.0) --
!*  -- Reference BLAS is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION ALPHA
      INTEGER LDA,LDB,M,N
      CHARACTER DIAG,SIDE,TRANSA,UPLO
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION A(LDA,*),B(LDB,*)
!*     ..
!*
!*  =====================================================================
!*
!*     .. External Functions ..
      LOGICAL LSAME
      EXTERNAL LSAME
!*     ..
!*     .. External Subroutines ..
      EXTERNAL XERBLA
!*     ..
!*     .. Local Scalars ..
      DOUBLE PRECISION TEMP
      INTEGER I,INFO,J,K,NROWA
      LOGICAL LSIDE,NOUNIT,UPPER
!*     ..
!*     .. Parameters ..
      DOUBLE PRECISION ONE,ZERO
      PARAMETER (ONE=1.0D+0,ZERO=0.0D+0)
!*     ..
!*
!*     Test the input parameters.
!*
      LSIDE = LSAME(SIDE,'L')
      IF (LSIDE) THEN
          NROWA = M
      ELSE
          NROWA = N
      END IF
      NOUNIT = LSAME(DIAG,'N')
      UPPER = LSAME(UPLO,'U')
!*
      INFO = 0
      IF ((.NOT.LSIDE) .AND. (.NOT.LSAME(SIDE,'R'))) THEN
          INFO = 1
      ELSE IF ((.NOT.UPPER) .AND. (.NOT.LSAME(UPLO,'L'))) THEN
          INFO = 2
      ELSE IF ((.NOT.LSAME(TRANSA,'N')) .AND. &
               (.NOT.LSAME(TRANSA,'T')) .AND. &
               (.NOT.LSAME(TRANSA,'C'))) THEN
          INFO = 3
      ELSE IF ((.NOT.LSAME(DIAG,'U')) .AND. (.NOT.LSAME(DIAG,'N'))) THEN
          INFO = 4
      ELSE IF (M.LT.0) THEN
          INFO = 5
      ELSE IF (N.LT.0) THEN
          INFO = 6
      ELSE IF (LDA.LT.MAX(1,NROWA)) THEN
          INFO = 9
      ELSE IF (LDB.LT.MAX(1,M)) THEN
          INFO = 11
      END IF
      IF (INFO.NE.0) THEN
          CALL XERBLA('DTRMM ',INFO)
          RETURN
      END IF
!*
!*     Quick return if possible.
!*
      IF (M.EQ.0 .OR. N.EQ.0) RETURN
!*
!*     And when  alpha.eq.zero.
!*
      IF (ALPHA.EQ.ZERO) THEN
          DO 20 J = 1,N
              DO 10 I = 1,M
                  B(I,J) = ZERO
   10         CONTINUE
   20     CONTINUE
          RETURN
      END IF
!*
!*     Start the operations.
!*
      IF (LSIDE) THEN
          IF (LSAME(TRANSA,'N')) THEN
!*
!*           Form  B := alpha*A*B.
!*
              IF (UPPER) THEN
                  DO 50 J = 1,N
                      DO 40 K = 1,M
                          IF (B(K,J).NE.ZERO) THEN
                              TEMP = ALPHA*B(K,J)
                              DO 30 I = 1,K - 1
                                  B(I,J) = B(I,J) + TEMP*A(I,K)
   30                         CONTINUE
                              IF (NOUNIT) TEMP = TEMP*A(K,K)
                              B(K,J) = TEMP
                          END IF
   40                 CONTINUE
   50             CONTINUE
              ELSE
                  DO 80 J = 1,N
                      DO 70 K = M,1,-1
                          IF (B(K,J).NE.ZERO) THEN
                              TEMP = ALPHA*B(K,J)
                              B(K,J) = TEMP
                              IF (NOUNIT) B(K,J) = B(K,J)*A(K,K)
                              DO 60 I = K + 1,M
                                  B(I,J) = B(I,J) + TEMP*A(I,K)
   60                         CONTINUE
                          END IF
   70                 CONTINUE
   80             CONTINUE
              END IF
          ELSE
!*
!*           Form  B := alpha*A**T*B.
!*
              IF (UPPER) THEN
                  DO 110 J = 1,N
                      DO 100 I = M,1,-1
                          TEMP = B(I,J)
                          IF (NOUNIT) TEMP = TEMP*A(I,I)
                          DO 90 K = 1,I - 1
                              TEMP = TEMP + A(K,I)*B(K,J)
   90                     CONTINUE
                          B(I,J) = ALPHA*TEMP
  100                 CONTINUE
  110             CONTINUE
              ELSE
                  DO 140 J = 1,N
                      DO 130 I = 1,M
                          TEMP = B(I,J)
                          IF (NOUNIT) TEMP = TEMP*A(I,I)
                          DO 120 K = I + 1,M
                              TEMP = TEMP + A(K,I)*B(K,J)
  120                     CONTINUE
                          B(I,J) = ALPHA*TEMP
  130                 CONTINUE
  140             CONTINUE
              END IF
          END IF
      ELSE
          IF (LSAME(TRANSA,'N')) THEN
!*
!*           Form  B := alpha*B*A.
!*
              IF (UPPER) THEN
                  DO 180 J = N,1,-1
                      TEMP = ALPHA
                      IF (NOUNIT) TEMP = TEMP*A(J,J)
                      DO 150 I = 1,M
                          B(I,J) = TEMP*B(I,J)
  150                 CONTINUE
                      DO 170 K = 1,J - 1
                          IF (A(K,J).NE.ZERO) THEN
                              TEMP = ALPHA*A(K,J)
                              DO 160 I = 1,M
                                  B(I,J) = B(I,J) + TEMP*B(I,K)
  160                         CONTINUE
                          END IF
  170                 CONTINUE
  180             CONTINUE
              ELSE
                  DO 220 J = 1,N
                      TEMP = ALPHA
                      IF (NOUNIT) TEMP = TEMP*A(J,J)
                      DO 190 I = 1,M
                          B(I,J) = TEMP*B(I,J)
  190                 CONTINUE
                      DO 210 K = J + 1,N
                          IF (A(K,J).NE.ZERO) THEN
                              TEMP = ALPHA*A(K,J)
                              DO 200 I = 1,M
                                  B(I,J) = B(I,J) + TEMP*B(I,K)
  200                         CONTINUE
                          END IF
  210                 CONTINUE
  220             CONTINUE
              END IF
          ELSE
!*
!*           Form  B := alpha*B*A**T.
!*
              IF (UPPER) THEN
                  DO 260 K = 1,N
                      DO 240 J = 1,K - 1
                          IF (A(J,K).NE.ZERO) THEN
                              TEMP = ALPHA*A(J,K)
                              DO 230 I = 1,M
                                  B(I,J) = B(I,J) + TEMP*B(I,K)
  230                         CONTINUE
                          END IF
  240                 CONTINUE
                      TEMP = ALPHA
                      IF (NOUNIT) TEMP = TEMP*A(K,K)
                      IF (TEMP.NE.ONE) THEN
                          DO 250 I = 1,M
                              B(I,K) = TEMP*B(I,K)
  250                     CONTINUE
                      END IF
  260             CONTINUE
              ELSE
                  DO 300 K = N,1,-1
                      DO 280 J = K + 1,N
                          IF (A(J,K).NE.ZERO) THEN
                              TEMP = ALPHA*A(J,K)
                              DO 270 I = 1,M
                                  B(I,J) = B(I,J) + TEMP*B(I,K)
  270                         CONTINUE
                          END IF
  280                 CONTINUE
                      TEMP = ALPHA
                      IF (NOUNIT) TEMP = TEMP*A(K,K)
                      IF (TEMP.NE.ONE) THEN
                          DO 290 I = 1,M
                              B(I,K) = TEMP*B(I,K)
  290                     CONTINUE
                      END IF
  300             CONTINUE
              END IF
          END IF
      END IF
!*
      RETURN
!*
!*     End of DTRMM .
!*
      END

!==========================================================================

!*> \brief \b DGEMV
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGEMV(TRANS,M,N,ALPHA,A,LDA,X,INCX,BETA,Y,INCY)
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION ALPHA,BETA
!*       INTEGER INCX,INCY,LDA,M,N
!*       CHARACTER TRANS
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION A(LDA,*),X(*),Y(*)
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGEMV  performs one of the matrix-vector operations
!*>
!*>    y := alpha*A*x + beta*y,   or   y := alpha*A**T*x + beta*y,
!*>
!*> where alpha and beta are scalars, x and y are vectors and A is an
!*> m by n matrix.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] TRANS
!*> \verbatim
!*>          TRANS is CHARACTER*1
!*>           On entry, TRANS specifies the operation to be performed as
!*>           follows:
!*>
!*>              TRANS = 'N' or 'n'   y := alpha*A*x + beta*y.
!*>
!*>              TRANS = 'T' or 't'   y := alpha*A**T*x + beta*y.
!*>
!*>              TRANS = 'C' or 'c'   y := alpha*A**T*x + beta*y.
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>           On entry, M specifies the number of rows of the matrix A.
!*>           M must be at least zero.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>           On entry, N specifies the number of columns of the matrix A.
!*>           N must be at least zero.
!*> \endverbatim
!*>
!*> \param[in] ALPHA
!*> \verbatim
!*>          ALPHA is DOUBLE PRECISION.
!*>           On entry, ALPHA specifies the scalar alpha.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array of DIMENSION ( LDA, n ).
!*>           Before entry, the leading m by n part of the array A must
!*>           contain the matrix of coefficients.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>           On entry, LDA specifies the first dimension of A as declared
!*>           in the calling (sub) program. LDA must be at least
!*>           max( 1, m ).
!*> \endverbatim
!*>
!*> \param[in] X
!*> \verbatim
!*>          X is DOUBLE PRECISION array of DIMENSION at least
!*>           ( 1 + ( n - 1 )*abs( INCX ) ) when TRANS = 'N' or 'n'
!*>           and at least
!*>           ( 1 + ( m - 1 )*abs( INCX ) ) otherwise.
!*>           Before entry, the incremented array X must contain the
!*>           vector x.
!*> \endverbatim
!*>
!*> \param[in] INCX
!*> \verbatim
!*>          INCX is INTEGER
!*>           On entry, INCX specifies the increment for the elements of
!*>           X. INCX must not be zero.
!*> \endverbatim
!*>
!*> \param[in] BETA
!*> \verbatim
!*>          BETA is DOUBLE PRECISION.
!*>           On entry, BETA specifies the scalar beta. When BETA is
!*>           supplied as zero then Y need not be set on input.
!*> \endverbatim
!*>
!*> \param[in,out] Y
!*> \verbatim
!*>          Y is DOUBLE PRECISION array of DIMENSION at least
!*>           ( 1 + ( m - 1 )*abs( INCY ) ) when TRANS = 'N' or 'n'
!*>           and at least
!*>           ( 1 + ( n - 1 )*abs( INCY ) ) otherwise.
!*>           Before entry with BETA non-zero, the incremented array Y
!*>           must contain the vector y. On exit, Y is overwritten by the
!*>           updated vector y.
!*> \endverbatim
!*>
!*> \param[in] INCY
!*> \verbatim
!*>          INCY is INTEGER
!*>           On entry, INCY specifies the increment for the elements of
!*>           Y. INCY must not be zero.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup double_blas_level2
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  Level 2 Blas routine.
!*>  The vector and matrix arguments are not referenced when N = 0, or M = 0
!*>
!*>  -- Written on 22-October-1986.
!*>     Jack Dongarra, Argonne National Lab.
!*>     Jeremy Du Croz, Nag Central Office.
!*>     Sven Hammarling, Nag Central Office.
!*>     Richard Hanson, Sandia National Labs.
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DGEMV(TRANS,M,N,ALPHA,A,LDA,X,INCX,BETA,Y,INCY)
!
      IMPLICIT NONE
!*
!*  -- Reference BLAS level2 routine (version 3.4.0) --
!*  -- Reference BLAS is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION ALPHA,BETA
      INTEGER INCX,INCY,LDA,M,N
      CHARACTER TRANS
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION A(LDA,*),X(*),Y(*)
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION ONE,ZERO
      PARAMETER (ONE=1.0D+0,ZERO=0.0D+0)
!*     ..
!*     .. Local Scalars ..
      DOUBLE PRECISION TEMP
      INTEGER I,INFO,IX,IY,J,JX,JY,KX,KY,LENX,LENY
!*     ..
!*     .. External Functions ..
      LOGICAL LSAME
      EXTERNAL LSAME
!*     ..
!*     .. External Subroutines ..
      EXTERNAL XERBLA
!*     ..
!*
!*     Test the input parameters.
!*
      INFO = 0
      IF (.NOT.LSAME(TRANS,'N') .AND. .NOT.LSAME(TRANS,'T') .AND. &
          .NOT.LSAME(TRANS,'C')) THEN
          INFO = 1
      ELSE IF (M.LT.0) THEN
          INFO = 2
      ELSE IF (N.LT.0) THEN
          INFO = 3
      ELSE IF (LDA.LT.MAX(1,M)) THEN
          INFO = 6
      ELSE IF (INCX.EQ.0) THEN
          INFO = 8
      ELSE IF (INCY.EQ.0) THEN
          INFO = 11
      END IF
      IF (INFO.NE.0) THEN
          CALL XERBLA('DGEMV ',INFO)
          RETURN
      END IF
!*
!*     Quick return if possible.
!*
      IF ((M.EQ.0) .OR. (N.EQ.0) .OR. &
          ((ALPHA.EQ.ZERO).AND. (BETA.EQ.ONE))) RETURN
!*
!*     Set  LENX  and  LENY, the lengths of the vectors x and y, and set
!*     up the start points in  X  and  Y.
!*
      IF (LSAME(TRANS,'N')) THEN
          LENX = N
          LENY = M
      ELSE
          LENX = M
          LENY = N
      END IF
      IF (INCX.GT.0) THEN
          KX = 1
      ELSE
          KX = 1 - (LENX-1)*INCX
      END IF
      IF (INCY.GT.0) THEN
          KY = 1
      ELSE
          KY = 1 - (LENY-1)*INCY
      END IF
!*
!*     Start the operations. In this version the elements of A are
!*     accessed sequentially with one pass through A.
!*
!*     First form  y := beta*y.
!*
      IF (BETA.NE.ONE) THEN
          IF (INCY.EQ.1) THEN
              IF (BETA.EQ.ZERO) THEN
                  DO 10 I = 1,LENY
                      Y(I) = ZERO
   10             CONTINUE
              ELSE
                  DO 20 I = 1,LENY
                      Y(I) = BETA*Y(I)
   20             CONTINUE
              END IF
          ELSE
              IY = KY
              IF (BETA.EQ.ZERO) THEN
                  DO 30 I = 1,LENY
                      Y(IY) = ZERO
                      IY = IY + INCY
   30             CONTINUE
              ELSE
                  DO 40 I = 1,LENY
                      Y(IY) = BETA*Y(IY)
                      IY = IY + INCY
   40             CONTINUE
              END IF
          END IF
      END IF
      IF (ALPHA.EQ.ZERO) RETURN
      IF (LSAME(TRANS,'N')) THEN
!*
!*        Form  y := alpha*A*x + y.
!*
          JX = KX
          IF (INCY.EQ.1) THEN
              DO 60 J = 1,N
                  IF (X(JX).NE.ZERO) THEN
                      TEMP = ALPHA*X(JX)
                      DO 50 I = 1,M
                          Y(I) = Y(I) + TEMP*A(I,J)
   50                 CONTINUE
                  END IF
                  JX = JX + INCX
   60         CONTINUE
          ELSE
              DO 80 J = 1,N
                  IF (X(JX).NE.ZERO) THEN
                      TEMP = ALPHA*X(JX)
                      IY = KY
                      DO 70 I = 1,M
                          Y(IY) = Y(IY) + TEMP*A(I,J)
                          IY = IY + INCY
   70                 CONTINUE
                  END IF
                  JX = JX + INCX
   80         CONTINUE
          END IF
      ELSE
!*
!*        Form  y := alpha*A**T*x + y.
!*
          JY = KY
          IF (INCX.EQ.1) THEN
              DO 100 J = 1,N
                  TEMP = ZERO
                  DO 90 I = 1,M
                      TEMP = TEMP + A(I,J)*X(I)
   90             CONTINUE
                  Y(JY) = Y(JY) + ALPHA*TEMP
                  JY = JY + INCY
  100         CONTINUE
          ELSE
              DO 120 J = 1,N
                  TEMP = ZERO
                  IX = KX
                  DO 110 I = 1,M
                      TEMP = TEMP + A(I,J)*X(IX)
                      IX = IX + INCX
  110             CONTINUE
                  Y(JY) = Y(JY) + ALPHA*TEMP
                  JY = JY + INCY
  120         CONTINUE
          END IF
      END IF
!*
      RETURN
!*
!*     End of DGEMV .
!*
      END

!==========================================================================

!*> \brief \b DTRMV
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DTRMV(UPLO,TRANS,DIAG,N,A,LDA,X,INCX)
!*
!*       .. Scalar Arguments ..
!*       INTEGER INCX,LDA,N
!*       CHARACTER DIAG,TRANS,UPLO
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION A(LDA,*),X(*)
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DTRMV  performs one of the matrix-vector operations
!*>
!*>    x := A*x,   or   x := A**T*x,
!*>
!*> where x is an n element vector and  A is an n by n unit, or non-unit,
!*> upper or lower triangular matrix.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] UPLO
!*> \verbatim
!*>          UPLO is CHARACTER*1
!*>           On entry, UPLO specifies whether the matrix is an upper or
!*>           lower triangular matrix as follows:
!*>
!*>              UPLO = 'U' or 'u'   A is an upper triangular matrix.
!*>
!*>              UPLO = 'L' or 'l'   A is a lower triangular matrix.
!*> \endverbatim
!*>
!*> \param[in] TRANS
!*> \verbatim
!*>          TRANS is CHARACTER*1
!*>           On entry, TRANS specifies the operation to be performed as
!*>           follows:
!*>
!*>              TRANS = 'N' or 'n'   x := A*x.
!*>
!*>              TRANS = 'T' or 't'   x := A**T*x.
!*>
!*>              TRANS = 'C' or 'c'   x := A**T*x.
!*> \endverbatim
!*>
!*> \param[in] DIAG
!*> \verbatim
!*>          DIAG is CHARACTER*1
!*>           On entry, DIAG specifies whether or not A is unit
!*>           triangular as follows:
!*>
!*>              DIAG = 'U' or 'u'   A is assumed to be unit triangular.
!*>
!*>              DIAG = 'N' or 'n'   A is not assumed to be unit
!*>                                  triangular.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>           On entry, N specifies the order of the matrix A.
!*>           N must be at least zero.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array of DIMENSION ( LDA, n ).
!*>           Before entry with  UPLO = 'U' or 'u', the leading n by n
!*>           upper triangular part of the array A must contain the upper
!*>           triangular matrix and the strictly lower triangular part of
!*>           A is not referenced.
!*>           Before entry with UPLO = 'L' or 'l', the leading n by n
!*>           lower triangular part of the array A must contain the lower
!*>           triangular matrix and the strictly upper triangular part of
!*>           A is not referenced.
!*>           Note that when  DIAG = 'U' or 'u', the diagonal elements of
!*>           A are not referenced either, but are assumed to be unity.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>           On entry, LDA specifies the first dimension of A as declared
!*>           in the calling (sub) program. LDA must be at least
!*>           max( 1, n ).
!*> \endverbatim
!*>
!*> \param[in,out] X
!*> \verbatim
!*>          X is DOUBLE PRECISION array of dimension at least
!*>           ( 1 + ( n - 1 )*abs( INCX ) ).
!*>           Before entry, the incremented array X must contain the n
!*>           element vector x. On exit, X is overwritten with the
!*>           tranformed vector x.
!*> \endverbatim
!*>
!*> \param[in] INCX
!*> \verbatim
!*>          INCX is INTEGER
!*>           On entry, INCX specifies the increment for the elements of
!*>           X. INCX must not be zero.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup double_blas_level2
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  Level 2 Blas routine.
!*>  The vector and matrix arguments are not referenced when N = 0, or M = 0
!*>
!*>  -- Written on 22-October-1986.
!*>     Jack Dongarra, Argonne National Lab.
!*>     Jeremy Du Croz, Nag Central Office.
!*>     Sven Hammarling, Nag Central Office.
!*>     Richard Hanson, Sandia National Labs.
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DTRMV(UPLO,TRANS,DIAG,N,A,LDA,X,INCX)
!
      IMPLICIT NONE
!*
!*  -- Reference BLAS level2 routine (version 3.4.0) --
!*  -- Reference BLAS is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER INCX,LDA,N
      CHARACTER DIAG,TRANS,UPLO
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION A(LDA,*),X(*)
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION ZERO
      PARAMETER (ZERO=0.0D+0)
!*     ..
!*     .. Local Scalars ..
      DOUBLE PRECISION TEMP
      INTEGER I,INFO,IX,J,JX,KX
      LOGICAL NOUNIT
!*     ..
!*     .. External Functions ..
      LOGICAL LSAME
      EXTERNAL LSAME
!*     ..
!*     .. External Subroutines ..
      EXTERNAL XERBLA
!*     ..
!*
!*     Test the input parameters.
!*
      INFO = 0
      IF (.NOT.LSAME(UPLO,'U') .AND. .NOT.LSAME(UPLO,'L')) THEN
          INFO = 1
      ELSE IF (.NOT.LSAME(TRANS,'N') .AND. .NOT.LSAME(TRANS,'T') .AND. &
               .NOT.LSAME(TRANS,'C')) THEN
          INFO = 2
      ELSE IF (.NOT.LSAME(DIAG,'U') .AND. .NOT.LSAME(DIAG,'N')) THEN
          INFO = 3
      ELSE IF (N.LT.0) THEN
          INFO = 4
      ELSE IF (LDA.LT.MAX(1,N)) THEN
          INFO = 6
      ELSE IF (INCX.EQ.0) THEN
          INFO = 8
      END IF
      IF (INFO.NE.0) THEN
          CALL XERBLA('DTRMV ',INFO)
          RETURN
      END IF
!*
!*     Quick return if possible.
!*
      IF (N.EQ.0) RETURN
!*
      NOUNIT = LSAME(DIAG,'N')
!*
!*     Set up the start point in X if the increment is not unity. This
!*     will be  ( N - 1 )*INCX  too small for descending loops.
!*
      IF (INCX.LE.0) THEN
          KX = 1 - (N-1)*INCX
      ELSE IF (INCX.NE.1) THEN
          KX = 1
      END IF
!*
!*     Start the operations. In this version the elements of A are
!*     accessed sequentially with one pass through A.
!*
      IF (LSAME(TRANS,'N')) THEN
!*
!*        Form  x := A*x.
!*
          IF (LSAME(UPLO,'U')) THEN
              IF (INCX.EQ.1) THEN
                  DO 20 J = 1,N
                      IF (X(J).NE.ZERO) THEN
                          TEMP = X(J)
                          DO 10 I = 1,J - 1
                              X(I) = X(I) + TEMP*A(I,J)
   10                     CONTINUE
                          IF (NOUNIT) X(J) = X(J)*A(J,J)
                      END IF
   20             CONTINUE
              ELSE
                  JX = KX
                  DO 40 J = 1,N
                      IF (X(JX).NE.ZERO) THEN
                          TEMP = X(JX)
                          IX = KX
                          DO 30 I = 1,J - 1
                              X(IX) = X(IX) + TEMP*A(I,J)
                              IX = IX + INCX
   30                     CONTINUE
                          IF (NOUNIT) X(JX) = X(JX)*A(J,J)
                      END IF
                      JX = JX + INCX
   40             CONTINUE
              END IF
          ELSE
              IF (INCX.EQ.1) THEN
                  DO 60 J = N,1,-1
                      IF (X(J).NE.ZERO) THEN
                          TEMP = X(J)
                          DO 50 I = N,J + 1,-1
                              X(I) = X(I) + TEMP*A(I,J)
   50                     CONTINUE
                          IF (NOUNIT) X(J) = X(J)*A(J,J)
                      END IF
   60             CONTINUE
              ELSE
                  KX = KX + (N-1)*INCX
                  JX = KX
                  DO 80 J = N,1,-1
                      IF (X(JX).NE.ZERO) THEN
                          TEMP = X(JX)
                          IX = KX
                          DO 70 I = N,J + 1,-1
                              X(IX) = X(IX) + TEMP*A(I,J)
                              IX = IX - INCX
   70                     CONTINUE
                          IF (NOUNIT) X(JX) = X(JX)*A(J,J)
                      END IF
                      JX = JX - INCX
   80             CONTINUE
              END IF
          END IF
      ELSE
!*
!*        Form  x := A**T*x.
!*
          IF (LSAME(UPLO,'U')) THEN
              IF (INCX.EQ.1) THEN
                  DO 100 J = N,1,-1
                      TEMP = X(J)
                      IF (NOUNIT) TEMP = TEMP*A(J,J)
                      DO 90 I = J - 1,1,-1
                          TEMP = TEMP + A(I,J)*X(I)
   90                 CONTINUE
                      X(J) = TEMP
  100             CONTINUE
              ELSE
                  JX = KX + (N-1)*INCX
                  DO 120 J = N,1,-1
                      TEMP = X(JX)
                      IX = JX
                      IF (NOUNIT) TEMP = TEMP*A(J,J)
                      DO 110 I = J - 1,1,-1
                          IX = IX - INCX
                          TEMP = TEMP + A(I,J)*X(IX)
  110                 CONTINUE
                      X(JX) = TEMP
                      JX = JX - INCX
  120             CONTINUE
              END IF
          ELSE
              IF (INCX.EQ.1) THEN
                  DO 140 J = 1,N
                      TEMP = X(J)
                      IF (NOUNIT) TEMP = TEMP*A(J,J)
                      DO 130 I = J + 1,N
                          TEMP = TEMP + A(I,J)*X(I)
  130                 CONTINUE
                      X(J) = TEMP
  140             CONTINUE
              ELSE
                  JX = KX
                  DO 160 J = 1,N
                      TEMP = X(JX)
                      IX = JX
                      IF (NOUNIT) TEMP = TEMP*A(J,J)
                      DO 150 I = J + 1,N
                          IX = IX + INCX
                          TEMP = TEMP + A(I,J)*X(IX)
  150                 CONTINUE
                      X(JX) = TEMP
                      JX = JX + INCX
  160             CONTINUE
              END IF
          END IF
      END IF
!*
      RETURN
!*
!*     End of DTRMV .
!*
      END

!==========================================================================

!*> \brief \b DLARFG generates an elementary reflector (Householder matrix).
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLARFG + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlarfg.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlarfg.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlarfg.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLARFG( N, ALPHA, X, INCX, TAU )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INCX, N
!*       DOUBLE PRECISION   ALPHA, TAU
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   X( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLARFG generates a real elementary reflector H of order n, such
!*> that
!*>
!*>       H * ( alpha ) = ( beta ),   H**T * H = I.
!*>           (   x   )   (   0  )
!*>
!*> where alpha and beta are scalars, and x is an (n-1)-element real
!*> vector. H is represented in the form
!*>
!*>       H = I - tau * ( 1 ) * ( 1 v**T ) ,
!*>                     ( v )
!*>
!*> where tau is a real scalar and v is a real (n-1)-element
!*> vector.
!*>
!*> If the elements of x are all zero, then tau = 0 and H is taken to be
!*> the unit matrix.
!*>
!*> Otherwise  1 <= tau <= 2.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The order of the elementary reflector.
!*> \endverbatim
!*>
!*> \param[in,out] ALPHA
!*> \verbatim
!*>          ALPHA is DOUBLE PRECISION
!*>          On entry, the value alpha.
!*>          On exit, it is overwritten with the value beta.
!*> \endverbatim
!*>
!*> \param[in,out] X
!*> \verbatim
!*>          X is DOUBLE PRECISION array, dimension
!*>                         (1+(N-2)*abs(INCX))
!*>          On entry, the vector x.
!*>          On exit, it is overwritten with the vector v.
!*> \endverbatim
!*>
!*> \param[in] INCX
!*> \verbatim
!*>          INCX is INTEGER
!*>          The increment between elements of X. INCX > 0.
!*> \endverbatim
!*>
!*> \param[out] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION
!*>          The value tau.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleOTHERauxiliary
!*
!*  =====================================================================
      SUBROUTINE DLARFG( N, ALPHA, X, INCX, TAU )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            INCX, N
      DOUBLE PRECISION   ALPHA, TAU
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   X( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE, ZERO
      PARAMETER          ( ONE = 1.0D+0, ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            J, KNT
      DOUBLE PRECISION   BETA, RSAFMN, SAFMIN, XNORM
!*     ..
!*     .. External Functions ..
      DOUBLE PRECISION   DLAMCH, DLAPY2, DNRM2
      EXTERNAL           DLAMCH, DLAPY2, DNRM2
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DSCAL
!*     ..
!*     .. Executable Statements ..
!*
      IF( N.LE.1 ) THEN
         TAU = ZERO
         RETURN
      END IF
!*
      XNORM = DNRM2( N-1, X, INCX )
!*
      IF( XNORM.EQ.ZERO ) THEN
!*
!*        H  =  I
!*
         TAU = ZERO
      ELSE
!*
!*        general case
!*
         BETA = -SIGN( DLAPY2( ALPHA, XNORM ), ALPHA )
         SAFMIN = DLAMCH( 'S' ) / DLAMCH( 'E' )
         KNT = 0
         IF( ABS( BETA ).LT.SAFMIN ) THEN
!*
!*           XNORM, BETA may be inaccurate; scale X and recompute them
!*
            RSAFMN = ONE / SAFMIN
   10       CONTINUE
            KNT = KNT + 1
            CALL DSCAL( N-1, RSAFMN, X, INCX )
            BETA = BETA*RSAFMN
            ALPHA = ALPHA*RSAFMN
            IF( ABS( BETA ).LT.SAFMIN ) &
               GO TO 10
!*
!*           New BETA is at most 1, at least SAFMIN
!*
            XNORM = DNRM2( N-1, X, INCX )
            BETA = -SIGN( DLAPY2( ALPHA, XNORM ), ALPHA )
         END IF
         TAU = ( BETA-ALPHA ) / BETA
         CALL DSCAL( N-1, ONE / ( ALPHA-BETA ), X, INCX )
!*
!*        If ALPHA is subnormal, it may lose relative accuracy
!*
         DO 20 J = 1, KNT
            BETA = BETA*SAFMIN
 20      CONTINUE
         ALPHA = BETA
      END IF
!*
      RETURN
!*
!*     End of DLARFG
!*
      END

!==========================================================================

!*> \brief \b DORM2R multiplies a general matrix by the orthogonal matrix from a
!*> QR factorization determined by sgeqrf (unblocked algorithm).
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORM2R + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dorm2r.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dorm2r.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dorm2r.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORM2R( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC,
!*                          WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          SIDE, TRANS
!*       INTEGER            INFO, K, LDA, LDC, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORM2R overwrites the general real m by n matrix C with
!*>
!*>       Q * C  if SIDE = 'L' and TRANS = 'N', or
!*>
!*>       Q**T* C  if SIDE = 'L' and TRANS = 'T', or
!*>
!*>       C * Q  if SIDE = 'R' and TRANS = 'N', or
!*>
!*>       C * Q**T if SIDE = 'R' and TRANS = 'T',
!*>
!*> where Q is a real orthogonal matrix defined as the product of k
!*> elementary reflectors
!*>
!*>       Q = H(1) H(2) . . . H(k)
!*>
!*> as returned by DGEQRF. Q is of order m if SIDE = 'L' and of order n
!*> if SIDE = 'R'.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>          = 'L': apply Q or Q**T from the Left
!*>          = 'R': apply Q or Q**T from the Right
!*> \endverbatim
!*>
!*> \param[in] TRANS
!*> \verbatim
!*>          TRANS is CHARACTER*1
!*>          = 'N': apply Q  (No transpose)
!*>          = 'T': apply Q**T (Transpose)
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix C. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix C. N >= 0.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The number of elementary reflectors whose product defines
!*>          the matrix Q.
!*>          If SIDE = 'L', M >= K >= 0;
!*>          if SIDE = 'R', N >= K >= 0.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,K)
!*>          The i-th column must contain the vector which defines the
!*>          elementary reflector H(i), for i = 1,2,...,k, as returned by
!*>          DGEQRF in the first k columns of its array argument A.
!*>          A is modified by the routine but restored on exit.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A.
!*>          If SIDE = 'L', LDA >= max(1,M);
!*>          if SIDE = 'R', LDA >= max(1,N).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i), as returned by DGEQRF.
!*> \endverbatim
!*>
!*> \param[in,out] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension (LDC,N)
!*>          On entry, the m by n matrix C.
!*>          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q.
!*> \endverbatim
!*>
!*> \param[in] LDC
!*> \verbatim
!*>          LDC is INTEGER
!*>          The leading dimension of the array C. LDC >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension
!*>                                   (N) if SIDE = 'L',
!*>                                   (M) if SIDE = 'R'
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0: successful exit
!*>          < 0: if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORM2R( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS
      INTEGER            INFO, K, LDA, LDC, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D+0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LEFT, NOTRAN
      INTEGER            I, I1, I2, I3, IC, JC, MI, NI, NQ
      DOUBLE PRECISION   AII
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARF, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
!*
!*     NQ is the order of Q
!*
      IF( LEFT ) THEN
         NQ = M
      ELSE
         NQ = N
      END IF
      IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( K.LT.0 .OR. K.GT.NQ ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, NQ ) ) THEN
         INFO = -7
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -10
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORM2R', -INFO )
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( M.EQ.0 .OR. N.EQ.0 .OR. K.EQ.0 ) &
         RETURN
!*
      IF( ( LEFT .AND. .NOT.NOTRAN ) .OR. ( .NOT.LEFT .AND. NOTRAN ) ) &
           THEN
         I1 = 1
         I2 = K
         I3 = 1
      ELSE
         I1 = K
         I2 = 1
         I3 = -1
      END IF
!*
      IF( LEFT ) THEN
         NI = N
         JC = 1
      ELSE
         MI = M
         IC = 1
      END IF
!*
      DO 10 I = I1, I2, I3
         IF( LEFT ) THEN
!*
!*           H(i) is applied to C(i:m,1:n)
!*
            MI = M - I + 1
            IC = I
         ELSE
!*
!*           H(i) is applied to C(1:m,i:n)
!*
            NI = N - I + 1
            JC = I
         END IF
!*
!*        Apply H(i)
!*
         AII = A( I, I )
         A( I, I ) = ONE
         CALL DLARF( SIDE, MI, NI, A( I, I ), 1, TAU( I ), C( IC, JC ), &
                     LDC, WORK )
         A( I, I ) = AII
   10 CONTINUE
      RETURN
!*
!*     End of DORM2R
!*
      END

!==========================================================================

!*> \brief \b DORML2 multiplies a general matrix by the orthogonal matrix from a
!*> LQ factorization determined by sgelqf (unblocked algorithm).
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DORML2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dorml2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dorml2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dorml2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DORML2( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC,
!*                          WORK, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          SIDE, TRANS
!*       INTEGER            INFO, K, LDA, LDC, M, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DORML2 overwrites the general real m by n matrix C with
!*>
!*>       Q * C  if SIDE = 'L' and TRANS = 'N', or
!*>
!*>       Q**T* C  if SIDE = 'L' and TRANS = 'T', or
!*>
!*>       C * Q  if SIDE = 'R' and TRANS = 'N', or
!*>
!*>       C * Q**T if SIDE = 'R' and TRANS = 'T',
!*>
!*> where Q is a real orthogonal matrix defined as the product of k
!*> elementary reflectors
!*>
!*>       Q = H(k) . . . H(2) H(1)
!*>
!*> as returned by DGELQF. Q is of order m if SIDE = 'L' and of order n
!*> if SIDE = 'R'.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] SIDE
!*> \verbatim
!*>          SIDE is CHARACTER*1
!*>          = 'L': apply Q or Q**T from the Left
!*>          = 'R': apply Q or Q**T from the Right
!*> \endverbatim
!*>
!*> \param[in] TRANS
!*> \verbatim
!*>          TRANS is CHARACTER*1
!*>          = 'N': apply Q  (No transpose)
!*>          = 'T': apply Q**T (Transpose)
!*> \endverbatim
!*>
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix C. M >= 0.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix C. N >= 0.
!*> \endverbatim
!*>
!*> \param[in] K
!*> \verbatim
!*>          K is INTEGER
!*>          The number of elementary reflectors whose product defines
!*>          the matrix Q.
!*>          If SIDE = 'L', M >= K >= 0;
!*>          if SIDE = 'R', N >= K >= 0.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension
!*>                               (LDA,M) if SIDE = 'L',
!*>                               (LDA,N) if SIDE = 'R'
!*>          The i-th row must contain the vector which defines the
!*>          elementary reflector H(i), for i = 1,2,...,k, as returned by
!*>          DGELQF in the first k rows of its array argument A.
!*>          A is modified by the routine but restored on exit.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A. LDA >= max(1,K).
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION array, dimension (K)
!*>          TAU(i) must contain the scalar factor of the elementary
!*>          reflector H(i), as returned by DGELQF.
!*> \endverbatim
!*>
!*> \param[in,out] C
!*> \verbatim
!*>          C is DOUBLE PRECISION array, dimension (LDC,N)
!*>          On entry, the m by n matrix C.
!*>          On exit, C is overwritten by Q*C or Q**T*C or C*Q**T or C*Q.
!*> \endverbatim
!*>
!*> \param[in] LDC
!*> \verbatim
!*>          LDC is INTEGER
!*>          The leading dimension of the array C. LDC >= max(1,M).
!*> \endverbatim
!*>
!*> \param[out] WORK
!*> \verbatim
!*>          WORK is DOUBLE PRECISION array, dimension
!*>                                   (N) if SIDE = 'L',
!*>                                   (M) if SIDE = 'R'
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0: successful exit
!*>          < 0: if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup doubleOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DORML2( SIDE, TRANS, M, N, K, A, LDA, TAU, C, LDC, &
                         WORK, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          SIDE, TRANS
      INTEGER            INFO, K, LDA, LDC, M, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), C( LDC, * ), TAU( * ), WORK( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D+0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            LEFT, NOTRAN
      INTEGER            I, I1, I2, I3, IC, JC, MI, NI, NQ
      DOUBLE PRECISION   AII
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLARF, XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments
!*
      INFO = 0
      LEFT = LSAME( SIDE, 'L' )
      NOTRAN = LSAME( TRANS, 'N' )
!*
!*     NQ is the order of Q
!*
      IF( LEFT ) THEN
         NQ = M
      ELSE
         NQ = N
      END IF
      IF( .NOT.LEFT .AND. .NOT.LSAME( SIDE, 'R' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.NOTRAN .AND. .NOT.LSAME( TRANS, 'T' ) ) THEN
         INFO = -2
      ELSE IF( M.LT.0 ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( K.LT.0 .OR. K.GT.NQ ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, K ) ) THEN
         INFO = -7
      ELSE IF( LDC.LT.MAX( 1, M ) ) THEN
         INFO = -10
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DORML2', -INFO )
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( M.EQ.0 .OR. N.EQ.0 .OR. K.EQ.0 ) &
         RETURN
!*
      IF( ( LEFT .AND. NOTRAN ) .OR. ( .NOT.LEFT .AND. .NOT.NOTRAN ) ) &
           THEN
         I1 = 1
         I2 = K
         I3 = 1
      ELSE
         I1 = K
         I2 = 1
         I3 = -1
      END IF
!*
      IF( LEFT ) THEN
         NI = N
         JC = 1
      ELSE
         MI = M
         IC = 1
      END IF
!*
      DO 10 I = I1, I2, I3
         IF( LEFT ) THEN
!*
!*           H(i) is applied to C(i:m,1:n)
!*
            MI = M - I + 1
            IC = I
         ELSE
!*
!*           H(i) is applied to C(1:m,i:n)
!*
            NI = N - I + 1
            JC = I
         END IF
!*
!*        Apply H(i)
!*
         AII = A( I, I )
         A( I, I ) = ONE
         CALL DLARF( SIDE, MI, NI, A( I, I ), LDA, TAU( I ), &
                     C( IC, JC ), LDC, WORK )
         A( I, I ) = AII
   10 CONTINUE
      RETURN
!*
!*     End of DORML2
!*
      END

!==========================================================================

!*> \brief \b DLASQ2 computes all the eigenvalues of the symmetric positive definite
!*> tridiagonal matrix associated with the qd Array Z to high relative accuracy.
!*> Used by sbdsqr and sstegr.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASQ2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasq2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasq2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasq2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASQ2( N, Z, INFO )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            INFO, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   Z( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASQ2 computes all the eigenvalues of the symmetric positive
!*> definite tridiagonal matrix associated with the qd array Z to high
!*> relative accuracy are computed to high relative accuracy, in the
!*> absence of denormalization, underflow and overflow.
!*>
!*> To see the relation of Z to the tridiagonal matrix, let L be a
!*> unit lower bidiagonal matrix with subdiagonals Z(2,4,6,,..) and
!*> let U be an upper bidiagonal matrix with 1's above and diagonal
!*> Z(1,3,5,,..). The tridiagonal is L*U or, if you prefer, the
!*> symmetric tridiagonal to which it is similar.
!*>
!*> Note : DLASQ2 defines a logical variable, IEEE, which is true
!*> on machines which follow ieee-754 floating-point standard in their
!*> handling of infinities and NaNs, and false otherwise. This variable
!*> is passed to DLASQ3.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>        The number of rows and columns in the matrix. N >= 0.
!*> \endverbatim
!*>
!*> \param[in,out] Z
!*> \verbatim
!*>          Z is DOUBLE PRECISION array, dimension ( 4*N )
!*>        On entry Z holds the qd array. On exit, entries 1 to N hold
!*>        the eigenvalues in decreasing order, Z( 2*N+1 ) holds the
!*>        trace, and Z( 2*N+2 ) holds the sum of the eigenvalues. If
!*>        N > 2, then Z( 2*N+3 ) holds the iteration count, Z( 2*N+4 )
!*>        holds NDIVS/NIN^2, and Z( 2*N+5 ) holds the percentage of
!*>        shifts that failed.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>        = 0: successful exit
!*>        < 0: if the i-th argument is a scalar and had an illegal
!*>             value, then INFO = -i, if the i-th argument is an
!*>             array and the j-entry had an illegal value, then
!*>             INFO = -(i*100+j)
!*>        > 0: the algorithm failed
!*>              = 1, a split was marked by a positive value in E
!*>              = 2, current block of Z not diagonalized after 100*N
!*>                   iterations (in inner while loop).  On exit Z holds
!*>                   a qd array with the same eigenvalues as the given Z.
!*>              = 3, termination criterion of outer while loop not met
!*>                   (program created more than N unreduced blocks)
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERcomputational
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  Local Variables: I0:N0 defines a current unreduced segment of Z.
!*>  The shifts are accumulated in SIGMA. Iteration count is in ITER.
!*>  Ping-pong is controlled by PP (alternates between 0 and 1).
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DLASQ2( N, Z, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            INFO, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   Z( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   CBIAS
      PARAMETER          ( CBIAS = 1.50D0 )
      DOUBLE PRECISION   ZERO, HALF, ONE, TWO, FOUR, HUNDRD
      PARAMETER          ( ZERO = 0.0D0, HALF = 0.5D0, ONE = 1.0D0, &
                           TWO = 2.0D0, FOUR = 4.0D0, HUNDRD = 100.0D0 )
!*     ..
!*     .. Local Scalars ..
      LOGICAL            IEEE
      INTEGER            I0, I1, I4, IINFO, IPN4, ITER, IWHILA, IWHILB, &
                         K, KMIN, N0, N1, NBIG, NDIV, NFAIL, PP, SPLT,  &
                         TTYPE
      DOUBLE PRECISION   D, DEE, DEEMIN, DESIG, DMIN, DMIN1, DMIN2, DN, &
                         DN1, DN2, E, EMAX, EMIN, EPS, G, OLDEMN, QMAX, &
                         QMIN, S, SAFMIN, SIGMA, T, TAU, TEMP, TOL, &
                         TOL2, TRACE, ZMAX, TEMPE, TEMPQ
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLASQ3, DLASRT, XERBLA
!*     ..
!*     .. External Functions ..
      INTEGER            ILAENV
      DOUBLE PRECISION   DLAMCH
      EXTERNAL           DLAMCH, ILAENV
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input arguments.
!*     (in case DLASQ2 is not called by DLASQ1)
!*
      INFO = 0
      EPS = DLAMCH( 'Precision' )
      SAFMIN = DLAMCH( 'Safe minimum' )
      TOL = EPS*HUNDRD
      TOL2 = TOL**2
!*
      IF( N.LT.0 ) THEN
         INFO = -1
         CALL XERBLA( 'DLASQ2', 1 )
         RETURN
      ELSE IF( N.EQ.0 ) THEN
         RETURN
      ELSE IF( N.EQ.1 ) THEN
!*
!*        1-by-1 case.
!*
         IF( Z( 1 ).LT.ZERO ) THEN
            INFO = -201
            CALL XERBLA( 'DLASQ2', 2 )
         END IF
         RETURN
      ELSE IF( N.EQ.2 ) THEN
!*
!*        2-by-2 case.
!*
         IF( Z( 2 ).LT.ZERO .OR. Z( 3 ).LT.ZERO ) THEN
            INFO = -2
            CALL XERBLA( 'DLASQ2', 2 )
            RETURN
         ELSE IF( Z( 3 ).GT.Z( 1 ) ) THEN
            D = Z( 3 )
            Z( 3 ) = Z( 1 )
            Z( 1 ) = D
         END IF
         Z( 5 ) = Z( 1 ) + Z( 2 ) + Z( 3 )
         IF( Z( 2 ).GT.Z( 3 )*TOL2 ) THEN
            T = HALF*( ( Z( 1 )-Z( 3 ) )+Z( 2 ) )
            S = Z( 3 )*( Z( 2 ) / T )
            IF( S.LE.T ) THEN
               S = Z( 3 )*( Z( 2 ) / ( T*( ONE+SQRT( ONE+S / T ) ) ) )
            ELSE
               S = Z( 3 )*( Z( 2 ) / ( T+SQRT( T )*SQRT( T+S ) ) )
            END IF
            T = Z( 1 ) + ( S+Z( 2 ) )
            Z( 3 ) = Z( 3 )*( Z( 1 ) / T )
            Z( 1 ) = T
         END IF
         Z( 2 ) = Z( 3 )
         Z( 6 ) = Z( 2 ) + Z( 1 )
         RETURN
      END IF
!*
!*     Check for negative data and compute sums of q's and e's.
!*
      Z( 2*N ) = ZERO
      EMIN = Z( 2 )
      QMAX = ZERO
      ZMAX = ZERO
      D = ZERO
      E = ZERO
!*
      DO 10 K = 1, 2*( N-1 ), 2
         IF( Z( K ).LT.ZERO ) THEN
            INFO = -( 200+K )
            CALL XERBLA( 'DLASQ2', 2 )
            RETURN
         ELSE IF( Z( K+1 ).LT.ZERO ) THEN
            INFO = -( 200+K+1 )
            CALL XERBLA( 'DLASQ2', 2 )
            RETURN
         END IF
         D = D + Z( K )
         E = E + Z( K+1 )
         QMAX = MAX( QMAX, Z( K ) )
         EMIN = MIN( EMIN, Z( K+1 ) )
         ZMAX = MAX( QMAX, ZMAX, Z( K+1 ) )
   10 CONTINUE
      IF( Z( 2*N-1 ).LT.ZERO ) THEN
         INFO = -( 200+2*N-1 )
         CALL XERBLA( 'DLASQ2', 2 )
         RETURN
      END IF
      D = D + Z( 2*N-1 )
      QMAX = MAX( QMAX, Z( 2*N-1 ) )
      ZMAX = MAX( QMAX, ZMAX )
!*
!*     Check for diagonality.
!*
      IF( E.EQ.ZERO ) THEN
         DO 20 K = 2, N
            Z( K ) = Z( 2*K-1 )
   20    CONTINUE
         CALL DLASRT( 'D', N, Z, IINFO )
         Z( 2*N-1 ) = D
         RETURN
      END IF
!*
      TRACE = D + E
!*
!*     Check for zero data.
!*
      IF( TRACE.EQ.ZERO ) THEN
         Z( 2*N-1 ) = ZERO
         RETURN
      END IF
!*
!*     Check whether the machine is IEEE conformable.
!*
      IEEE = ILAENV( 10, 'DLASQ2', 'N', 1, 2, 3, 4 ).EQ.1 .AND. &
             ILAENV( 11, 'DLASQ2', 'N', 1, 2, 3, 4 ).EQ.1
!*
!*     Rearrange data for locality: Z=(q1,qq1,e1,ee1,q2,qq2,e2,ee2,...).
!*
      DO 30 K = 2*N, 2, -2
         Z( 2*K ) = ZERO
         Z( 2*K-1 ) = Z( K )
         Z( 2*K-2 ) = ZERO
         Z( 2*K-3 ) = Z( K-1 )
   30 CONTINUE
!*
      I0 = 1
      N0 = N
!*
!*     Reverse the qd-array, if warranted.
!*
      IF( CBIAS*Z( 4*I0-3 ).LT.Z( 4*N0-3 ) ) THEN
         IPN4 = 4*( I0+N0 )
         DO 40 I4 = 4*I0, 2*( I0+N0-1 ), 4
            TEMP = Z( I4-3 )
            Z( I4-3 ) = Z( IPN4-I4-3 )
            Z( IPN4-I4-3 ) = TEMP
            TEMP = Z( I4-1 )
            Z( I4-1 ) = Z( IPN4-I4-5 )
            Z( IPN4-I4-5 ) = TEMP
   40    CONTINUE
      END IF
!*
!*     Initial split checking via dqd and Li's test.
!*
      PP = 0
!*
      DO 80 K = 1, 2
!*
         D = Z( 4*N0+PP-3 )
         DO 50 I4 = 4*( N0-1 ) + PP, 4*I0 + PP, -4
            IF( Z( I4-1 ).LE.TOL2*D ) THEN
               Z( I4-1 ) = -ZERO
               D = Z( I4-3 )
            ELSE
               D = Z( I4-3 )*( D / ( D+Z( I4-1 ) ) )
            END IF
   50    CONTINUE
!*
!*        dqd maps Z to ZZ plus Li's test.
!*
         EMIN = Z( 4*I0+PP+1 )
         D = Z( 4*I0+PP-3 )
         DO 60 I4 = 4*I0 + PP, 4*( N0-1 ) + PP, 4
            Z( I4-2*PP-2 ) = D + Z( I4-1 )
            IF( Z( I4-1 ).LE.TOL2*D ) THEN
               Z( I4-1 ) = -ZERO
               Z( I4-2*PP-2 ) = D
               Z( I4-2*PP ) = ZERO
               D = Z( I4+1 )
            ELSE IF( SAFMIN*Z( I4+1 ).LT.Z( I4-2*PP-2 ) .AND. &
                     SAFMIN*Z( I4-2*PP-2 ).LT.Z( I4+1 ) ) THEN
               TEMP = Z( I4+1 ) / Z( I4-2*PP-2 )
               Z( I4-2*PP ) = Z( I4-1 )*TEMP
               D = D*TEMP
            ELSE
               Z( I4-2*PP ) = Z( I4+1 )*( Z( I4-1 ) / Z( I4-2*PP-2 ) )
               D = Z( I4+1 )*( D / Z( I4-2*PP-2 ) )
            END IF
            EMIN = MIN( EMIN, Z( I4-2*PP ) )
   60    CONTINUE
         Z( 4*N0-PP-2 ) = D
!*
!*        Now find qmax.
!*
         QMAX = Z( 4*I0-PP-2 )
         DO 70 I4 = 4*I0 - PP + 2, 4*N0 - PP - 2, 4
            QMAX = MAX( QMAX, Z( I4 ) )
   70    CONTINUE
!*
!*        Prepare for the next iteration on K.
!*
         PP = 1 - PP
   80 CONTINUE
!*
!*     Initialise variables to pass to DLASQ3.
!*
      TTYPE = 0
      DMIN1 = ZERO
      DMIN2 = ZERO
      DN    = ZERO
      DN1   = ZERO
      DN2   = ZERO
      G     = ZERO
      TAU   = ZERO
!*
      ITER = 2
      NFAIL = 0
      NDIV = 2*( N0-I0 )
!*
      DO 160 IWHILA = 1, N + 1
         IF( N0.LT.1 )  &
            GO TO 170
!*
!*        While array unfinished do
!*
!*        E(N0) holds the value of SIGMA when submatrix in I0:N0
!*        splits from the rest of the array, but is negated.
!*
         DESIG = ZERO
         IF( N0.EQ.N ) THEN
            SIGMA = ZERO
         ELSE
            SIGMA = -Z( 4*N0-1 )
         END IF
         IF( SIGMA.LT.ZERO ) THEN
            INFO = 1
            RETURN
         END IF
!*
!*        Find last unreduced submatrix's top index I0, find QMAX and
!*        EMIN. Find Gershgorin-type bound if Q's much greater than E's.
!*
         EMAX = ZERO
         IF( N0.GT.I0 ) THEN
            EMIN = ABS( Z( 4*N0-5 ) )
         ELSE
            EMIN = ZERO
         END IF
         QMIN = Z( 4*N0-3 )
         QMAX = QMIN
         DO 90 I4 = 4*N0, 8, -4
            IF( Z( I4-5 ).LE.ZERO ) &
               GO TO 100
            IF( QMIN.GE.FOUR*EMAX ) THEN
               QMIN = MIN( QMIN, Z( I4-3 ) )
               EMAX = MAX( EMAX, Z( I4-5 ) )
            END IF
            QMAX = MAX( QMAX, Z( I4-7 )+Z( I4-5 ) )
            EMIN = MIN( EMIN, Z( I4-5 ) )
   90    CONTINUE
         I4 = 4
!*
  100    CONTINUE
         I0 = I4 / 4
         PP = 0
!*
         IF( N0-I0.GT.1 ) THEN
            DEE = Z( 4*I0-3 )
            DEEMIN = DEE
            KMIN = I0
            DO 110 I4 = 4*I0+1, 4*N0-3, 4
               DEE = Z( I4 )*( DEE /( DEE+Z( I4-2 ) ) )
               IF( DEE.LE.DEEMIN ) THEN
                  DEEMIN = DEE
                  KMIN = ( I4+3 )/4
               END IF
  110       CONTINUE
            IF( (KMIN-I0)*2.LT.N0-KMIN .AND.  &
               DEEMIN.LE.HALF*Z(4*N0-3) ) THEN
               IPN4 = 4*( I0+N0 )
               PP = 2
               DO 120 I4 = 4*I0, 2*( I0+N0-1 ), 4
                  TEMP = Z( I4-3 )
                  Z( I4-3 ) = Z( IPN4-I4-3 )
                  Z( IPN4-I4-3 ) = TEMP
                  TEMP = Z( I4-2 )
                  Z( I4-2 ) = Z( IPN4-I4-2 )
                  Z( IPN4-I4-2 ) = TEMP
                  TEMP = Z( I4-1 )
                  Z( I4-1 ) = Z( IPN4-I4-5 )
                  Z( IPN4-I4-5 ) = TEMP
                  TEMP = Z( I4 )
                  Z( I4 ) = Z( IPN4-I4-4 )
                  Z( IPN4-I4-4 ) = TEMP
  120          CONTINUE
            END IF
         END IF
!*
!*        Put -(initial shift) into DMIN.
!*
         DMIN = -MAX( ZERO, QMIN-TWO*SQRT( QMIN )*SQRT( EMAX ) )
!*
!*        Now I0:N0 is unreduced.
!*        PP = 0 for ping, PP = 1 for pong.
!*        PP = 2 indicates that flipping was applied to the Z array and
!*               and that the tests for deflation upon entry in DLASQ3
!*               should not be performed.
!*
         NBIG = 100*( N0-I0+1 )
         DO 140 IWHILB = 1, NBIG
            IF( I0.GT.N0 )  &
               GO TO 150
!*
!*           While submatrix unfinished take a good dqds step.
!*
            CALL DLASQ3( I0, N0, Z, PP, DMIN, SIGMA, DESIG, QMAX, NFAIL, &
                         ITER, NDIV, IEEE, TTYPE, DMIN1, DMIN2, DN, DN1, &
                         DN2, G, TAU )
!*
            PP = 1 - PP
!*
!*           When EMIN is very small check for splits.
!*
            IF( PP.EQ.0 .AND. N0-I0.GE.3 ) THEN
               IF( Z( 4*N0 ).LE.TOL2*QMAX .OR. &
                   Z( 4*N0-1 ).LE.TOL2*SIGMA ) THEN
                  SPLT = I0 - 1
                  QMAX = Z( 4*I0-3 )
                  EMIN = Z( 4*I0-1 )
                  OLDEMN = Z( 4*I0 )
                  DO 130 I4 = 4*I0, 4*( N0-3 ), 4
                     IF( Z( I4 ).LE.TOL2*Z( I4-3 ) .OR. &
                         Z( I4-1 ).LE.TOL2*SIGMA ) THEN
                        Z( I4-1 ) = -SIGMA
                        SPLT = I4 / 4
                        QMAX = ZERO
                        EMIN = Z( I4+3 )
                        OLDEMN = Z( I4+4 )
                     ELSE
                        QMAX = MAX( QMAX, Z( I4+1 ) )
                        EMIN = MIN( EMIN, Z( I4-1 ) )
                        OLDEMN = MIN( OLDEMN, Z( I4 ) )
                     END IF
  130             CONTINUE
                  Z( 4*N0-1 ) = EMIN
                  Z( 4*N0 ) = OLDEMN
                  I0 = SPLT + 1
               END IF
            END IF
!*
  140    CONTINUE
!*
         INFO = 2
!*
!*        Maximum number of iterations exceeded, restore the shift
!*        SIGMA and place the new d's and e's in a qd array.
!*        This might need to be done for several blocks
!*
         I1 = I0
         N1 = N0
 145     CONTINUE
         TEMPQ = Z( 4*I0-3 )
         Z( 4*I0-3 ) = Z( 4*I0-3 ) + SIGMA
         DO K = I0+1, N0
            TEMPE = Z( 4*K-5 )
            Z( 4*K-5 ) = Z( 4*K-5 ) * (TEMPQ / Z( 4*K-7 ))
            TEMPQ = Z( 4*K-3 )
            Z( 4*K-3 ) = Z( 4*K-3 ) + SIGMA + TEMPE - Z( 4*K-5 )
         END DO
!*
!*        Prepare to do this on the previous block if there is one
!*
         IF( I1.GT.1 ) THEN
            N1 = I1-1
            DO WHILE( ( I1.GE.2 ) .AND. ( Z(4*I1-5).GE.ZERO ) )
               I1 = I1 - 1
            END DO
            SIGMA = -Z(4*N1-1)
            GO TO 145
         END IF

         DO K = 1, N
            Z( 2*K-1 ) = Z( 4*K-3 )
!*
!*        Only the block 1..N0 is unfinished.  The rest of the e's
!*        must be essentially zero, although sometimes other data
!*        has been stored in them.
!*
            IF( K.LT.N0 ) THEN
               Z( 2*K ) = Z( 4*K-1 )
            ELSE
               Z( 2*K ) = 0
            END IF
         END DO
         RETURN
!*
!*        end IWHILB
!*
  150    CONTINUE
!*
  160 CONTINUE
!*
      INFO = 3
      RETURN
!*
!*     end IWHILA
!*
  170 CONTINUE
!*
!*     Move q's to the front.
!*
      DO 180 K = 2, N
         Z( K ) = Z( 4*K-3 )
  180 CONTINUE
!*
!*     Sort and compute sum of eigenvalues.
!*
      CALL DLASRT( 'D', N, Z, IINFO )
!*
      E = ZERO
      DO 190 K = N, 1, -1
         E = E + Z( K )
  190 CONTINUE
!*
!*     Store trace, sum(eigenvalues) and information on performance.
!*
      Z( 2*N+1 ) = TRACE
      Z( 2*N+2 ) = E
      Z( 2*N+3 ) = DBLE( ITER )
      Z( 2*N+4 ) = DBLE( NDIV ) / DBLE( N**2 )
      Z( 2*N+5 ) = HUNDRD*NFAIL / DBLE( ITER )
      RETURN
!*
!*     End of DLASQ2
!*
      END

!==========================================================================

!*> \brief \b DLASRT sorts numbers in increasing or decreasing order.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASRT + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasrt.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasrt.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasrt.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASRT( ID, N, D, INFO )
!*
!*       .. Scalar Arguments ..
!*       CHARACTER          ID
!*       INTEGER            INFO, N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   D( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> Sort the numbers in D in increasing order (if ID = 'I') or
!*> in decreasing order (if ID = 'D' ).
!*>
!*> Use Quick Sort, reverting to Insertion sort on arrays of
!*> size <= 20. Dimension of STACK limits N to about 2**32.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] ID
!*> \verbatim
!*>          ID is CHARACTER*1
!*>          = 'I': sort D in increasing order;
!*>          = 'D': sort D in decreasing order.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The length of the array D.
!*> \endverbatim
!*>
!*> \param[in,out] D
!*> \verbatim
!*>          D is DOUBLE PRECISION array, dimension (N)
!*>          On entry, the array to be sorted.
!*>          On exit, D has been sorted into increasing order
!*>          (D(1) <= ... <= D(N) ) or into decreasing order
!*>          (D(1) >= ... >= D(N) ), depending on ID.
!*> \endverbatim
!*>
!*> \param[out] INFO
!*> \verbatim
!*>          INFO is INTEGER
!*>          = 0:  successful exit
!*>          < 0:  if INFO = -i, the i-th argument had an illegal value
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DLASRT( ID, N, D, INFO )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      CHARACTER          ID
      INTEGER            INFO, N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   D( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      INTEGER            SELECT
      PARAMETER          ( SELECT = 20 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            DIR, ENDD, I, J, START, STKPNT
      DOUBLE PRECISION   D1, D2, D3, DMNMX, TMP
!*     ..
!*     .. Local Arrays ..
      INTEGER            STACK( 2, 32 )
!*     ..
!*     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           XERBLA
!*     ..
!*     .. Executable Statements ..
!*
!*     Test the input paramters.
!*
      INFO = 0
      DIR = -1
      IF( LSAME( ID, 'D' ) ) THEN
         DIR = 0
      ELSE IF( LSAME( ID, 'I' ) ) THEN
         DIR = 1
      END IF
      IF( DIR.EQ.-1 ) THEN
         INFO = -1
      ELSE IF( N.LT.0 ) THEN
         INFO = -2
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DLASRT', -INFO )
         RETURN
      END IF
!*
!*     Quick return if possible
!*
      IF( N.LE.1 ) &
         RETURN
!*
      STKPNT = 1
      STACK( 1, 1 ) = 1
      STACK( 2, 1 ) = N
   10 CONTINUE
      START = STACK( 1, STKPNT )
      ENDD = STACK( 2, STKPNT )
      STKPNT = STKPNT - 1
      IF( ENDD-START.LE.SELECT .AND. ENDD-START.GT.0 ) THEN
!*
!*        Do Insertion sort on D( START:ENDD )
!*
         IF( DIR.EQ.0 ) THEN
!*
!*           Sort into decreasing order
!*
            DO 30 I = START + 1, ENDD
               DO 20 J = I, START + 1, -1
                  IF( D( J ).GT.D( J-1 ) ) THEN
                     DMNMX = D( J )
                     D( J ) = D( J-1 )
                     D( J-1 ) = DMNMX
                  ELSE
                     GO TO 30
                  END IF
   20          CONTINUE
   30       CONTINUE
!*
         ELSE
!*
!*           Sort into increasing order
!*
            DO 50 I = START + 1, ENDD
               DO 40 J = I, START + 1, -1
                  IF( D( J ).LT.D( J-1 ) ) THEN
                     DMNMX = D( J )
                     D( J ) = D( J-1 )
                     D( J-1 ) = DMNMX
                  ELSE
                     GO TO 50
                  END IF
   40          CONTINUE
   50       CONTINUE
!*
         END IF
!*
      ELSE IF( ENDD-START.GT.SELECT ) THEN
!*
!*        Partition D( START:ENDD ) and stack parts, largest one first
!*
!*        Choose partition entry as median of 3
!*
         D1 = D( START )
         D2 = D( ENDD )
         I = ( START+ENDD ) / 2
         D3 = D( I )
         IF( D1.LT.D2 ) THEN
            IF( D3.LT.D1 ) THEN
               DMNMX = D1
            ELSE IF( D3.LT.D2 ) THEN
               DMNMX = D3
            ELSE
               DMNMX = D2
            END IF
         ELSE
            IF( D3.LT.D2 ) THEN
               DMNMX = D2
            ELSE IF( D3.LT.D1 ) THEN
               DMNMX = D3
            ELSE
               DMNMX = D1
            END IF
         END IF
!*
         IF( DIR.EQ.0 ) THEN
!*
!*           Sort into decreasing order
!*
            I = START - 1
            J = ENDD + 1
   60       CONTINUE
   70       CONTINUE
            J = J - 1
            IF( D( J ).LT.DMNMX ) &
               GO TO 70
   80       CONTINUE
            I = I + 1
            IF( D( I ).GT.DMNMX ) &
               GO TO 80
            IF( I.LT.J ) THEN
               TMP = D( I )
               D( I ) = D( J )
               D( J ) = TMP
               GO TO 60
            END IF
            IF( J-START.GT.ENDD-J-1 ) THEN
               STKPNT = STKPNT + 1
               STACK( 1, STKPNT ) = START
               STACK( 2, STKPNT ) = J
               STKPNT = STKPNT + 1
               STACK( 1, STKPNT ) = J + 1
               STACK( 2, STKPNT ) = ENDD
            ELSE
               STKPNT = STKPNT + 1
               STACK( 1, STKPNT ) = J + 1
               STACK( 2, STKPNT ) = ENDD
               STKPNT = STKPNT + 1
               STACK( 1, STKPNT ) = START
               STACK( 2, STKPNT ) = J
            END IF
         ELSE
!*
!*           Sort into increasing order
!*
            I = START - 1
            J = ENDD + 1
   90       CONTINUE
  100       CONTINUE
            J = J - 1
            IF( D( J ).GT.DMNMX ) &
               GO TO 100
  110       CONTINUE
            I = I + 1
            IF( D( I ).LT.DMNMX ) &
               GO TO 110
            IF( I.LT.J ) THEN
               TMP = D( I )
               D( I ) = D( J )
               D( J ) = TMP
               GO TO 90
            END IF
            IF( J-START.GT.ENDD-J-1 ) THEN
               STKPNT = STKPNT + 1
               STACK( 1, STKPNT ) = START
               STACK( 2, STKPNT ) = J
               STKPNT = STKPNT + 1
               STACK( 1, STKPNT ) = J + 1
               STACK( 2, STKPNT ) = ENDD
            ELSE
               STKPNT = STKPNT + 1
               STACK( 1, STKPNT ) = J + 1
               STACK( 2, STKPNT ) = ENDD
               STKPNT = STKPNT + 1
               STACK( 1, STKPNT ) = START
               STACK( 2, STKPNT ) = J
            END IF
         END IF
      END IF
      IF( STKPNT.GT.0 ) &
         GO TO 10
      RETURN
!*
!*     End of DLASRT
!*
      END

!==========================================================================

!*> \brief \b ILADLR scans a matrix for its last non-zero row.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download ILADLR + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/iladlr.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/iladlr.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/iladlr.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       INTEGER FUNCTION ILADLR( M, N, A, LDA )
!
!        IMPLICIT NONE
!*
!*       .. Scalar Arguments ..
!*       INTEGER            M, N, LDA
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> ILADLR scans A for its last non-zero row.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          The m by n matrix A.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A. LDA >= max(1,M).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      INTEGER FUNCTION ILADLR( M, N, A, LDA )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            M, N, LDA
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION ZERO
      PARAMETER ( ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER I, J
!*     ..
!*     .. Executable Statements ..
!*
!*     Quick test for the common case where one corner is non-zero.
      IF( M.EQ.0 ) THEN
         ILADLR = M
      ELSE IF( A(M, 1).NE.ZERO .OR. A(M, N).NE.ZERO ) THEN
         ILADLR = M
      ELSE
!*     Scan up each column tracking the last zero row seen.
         ILADLR = 0
         DO J = 1, N
            I=M
            DO WHILE((A(MAX(I,1),J).EQ.ZERO).AND.(I.GE.1))
               I=I-1
            ENDDO
            ILADLR = MAX( ILADLR, I )
         END DO
      END IF
      RETURN
      END

!==========================================================================

!*> \brief \b DGER
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DGER(M,N,ALPHA,X,INCX,Y,INCY,A,LDA)
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION ALPHA
!*       INTEGER INCX,INCY,LDA,M,N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION A(LDA,*),X(*),Y(*)
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DGER   performs the rank 1 operation
!*>
!*>    A := alpha*x*y**T + A,
!*>
!*> where alpha is a scalar, x is an m element vector, y is an n element
!*> vector and A is an m by n matrix.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>           On entry, M specifies the number of rows of the matrix A.
!*>           M must be at least zero.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>           On entry, N specifies the number of columns of the matrix A.
!*>           N must be at least zero.
!*> \endverbatim
!*>
!*> \param[in] ALPHA
!*> \verbatim
!*>          ALPHA is DOUBLE PRECISION.
!*>           On entry, ALPHA specifies the scalar alpha.
!*> \endverbatim
!*>
!*> \param[in] X
!*> \verbatim
!*>          X is DOUBLE PRECISION array of dimension at least
!*>           ( 1 + ( m - 1 )*abs( INCX ) ).
!*>           Before entry, the incremented array X must contain the m
!*>           element vector x.
!*> \endverbatim
!*>
!*> \param[in] INCX
!*> \verbatim
!*>          INCX is INTEGER
!*>           On entry, INCX specifies the increment for the elements of
!*>           X. INCX must not be zero.
!*> \endverbatim
!*>
!*> \param[in] Y
!*> \verbatim
!*>          Y is DOUBLE PRECISION array of dimension at least
!*>           ( 1 + ( n - 1 )*abs( INCY ) ).
!*>           Before entry, the incremented array Y must contain the n
!*>           element vector y.
!*> \endverbatim
!*>
!*> \param[in] INCY
!*> \verbatim
!*>          INCY is INTEGER
!*>           On entry, INCY specifies the increment for the elements of
!*>           Y. INCY must not be zero.
!*> \endverbatim
!*>
!*> \param[in,out] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array of DIMENSION ( LDA, n ).
!*>           Before entry, the leading m by n part of the array A must
!*>           contain the matrix of coefficients. On exit, A is
!*>           overwritten by the updated matrix.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>           On entry, LDA specifies the first dimension of A as declared
!*>           in the calling (sub) program. LDA must be at least
!*>           max( 1, m ).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup double_blas_level2
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  Level 2 Blas routine.
!*>
!*>  -- Written on 22-October-1986.
!*>     Jack Dongarra, Argonne National Lab.
!*>     Jeremy Du Croz, Nag Central Office.
!*>     Sven Hammarling, Nag Central Office.
!*>     Richard Hanson, Sandia National Labs.
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DGER(M,N,ALPHA,X,INCX,Y,INCY,A,LDA)
!
      IMPLICIT NONE
!*
!*  -- Reference BLAS level2 routine (version 3.4.0) --
!*  -- Reference BLAS is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION ALPHA
      INTEGER INCX,INCY,LDA,M,N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION A(LDA,*),X(*),Y(*)
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION ZERO
      PARAMETER (ZERO=0.0D+0)
!*     ..
!*     .. Local Scalars ..
      DOUBLE PRECISION TEMP
      INTEGER I,INFO,IX,J,JY,KX
!*     ..
!*     .. External Subroutines ..
      EXTERNAL XERBLA
!*     ..
!*
!*     Test the input parameters.
!*
      INFO = 0
      IF (M.LT.0) THEN
          INFO = 1
      ELSE IF (N.LT.0) THEN
          INFO = 2
      ELSE IF (INCX.EQ.0) THEN
          INFO = 5
      ELSE IF (INCY.EQ.0) THEN
          INFO = 7
      ELSE IF (LDA.LT.MAX(1,M)) THEN
          INFO = 9
      END IF
      IF (INFO.NE.0) THEN
          CALL XERBLA('DGER  ',INFO)
          RETURN
      END IF
!*
!*     Quick return if possible.
!*
      IF ((M.EQ.0) .OR. (N.EQ.0) .OR. (ALPHA.EQ.ZERO)) RETURN
!*
!*     Start the operations. In this version the elements of A are
!*     accessed sequentially with one pass through A.
!*
      IF (INCY.GT.0) THEN
          JY = 1
      ELSE
          JY = 1 - (N-1)*INCY
      END IF
      IF (INCX.EQ.1) THEN
          DO 20 J = 1,N
              IF (Y(JY).NE.ZERO) THEN
                  TEMP = ALPHA*Y(JY)
                  DO 10 I = 1,M
                      A(I,J) = A(I,J) + X(I)*TEMP
   10             CONTINUE
              END IF
              JY = JY + INCY
   20     CONTINUE
      ELSE
          IF (INCX.GT.0) THEN
              KX = 1
          ELSE
              KX = 1 - (M-1)*INCX
          END IF
          DO 40 J = 1,N
              IF (Y(JY).NE.ZERO) THEN
                  TEMP = ALPHA*Y(JY)
                  IX = KX
                  DO 30 I = 1,M
                      A(I,J) = A(I,J) + X(IX)*TEMP
                      IX = IX + INCX
   30             CONTINUE
              END IF
              JY = JY + INCY
   40     CONTINUE
      END IF
!*
      RETURN
!*
!*     End of DGER  .
!*
      END

!==========================================================================

!*> \brief \b DNRM2
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*  Definition:
!*  ===========
!*
!*       DOUBLE PRECISION FUNCTION DNRM2(N,X,INCX)
!
!        IMPLICIT NONE
!*
!*       .. Scalar Arguments ..
!*       INTEGER INCX,N
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION X(*)
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DNRM2 returns the euclidean norm of a vector via the function
!*> name, so that
!*>
!*>    DNRM2 := sqrt( x'*x )
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date November 2011
!*
!*> \ingroup double_blas_level1
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  -- This version written on 25-October-1982.
!*>     Modified on 14-October-1993 to inline the call to DLASSQ.
!*>     Sven Hammarling, Nag Ltd.
!*> \endverbatim
!*>
!*  =====================================================================
      DOUBLE PRECISION FUNCTION DNRM2(N,X,INCX)
!
      IMPLICIT NONE
!*
!*  -- Reference BLAS level1 routine (version 3.4.0) --
!*  -- Reference BLAS is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     November 2011
!*
!*     .. Scalar Arguments ..
      INTEGER INCX,N
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION X(*)
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION ONE,ZERO
      PARAMETER (ONE=1.0D+0,ZERO=0.0D+0)
!*     ..
!*     .. Local Scalars ..
      DOUBLE PRECISION ABSXI,NORM,SCALE,SSQ
      INTEGER IX
!*     ..
      IF (N.LT.1 .OR. INCX.LT.1) THEN
          NORM = ZERO
      ELSE IF (N.EQ.1) THEN
          NORM = ABS(X(1))
      ELSE
          SCALE = ZERO
          SSQ = ONE
!*        The following loop is equivalent to this call to the LAPACK
!*        auxiliary routine:
!*        CALL DLASSQ( N, X, INCX, SCALE, SSQ )
!*
          DO 10 IX = 1,1 + (N-1)*INCX,INCX
              IF (X(IX).NE.ZERO) THEN
                  ABSXI = ABS(X(IX))
                  IF (SCALE.LT.ABSXI) THEN
                      SSQ = ONE + SSQ* (SCALE/ABSXI)**2
                      SCALE = ABSXI
                  ELSE
                      SSQ = SSQ + (ABSXI/SCALE)**2
                  END IF
              END IF
   10     CONTINUE
          NORM = SCALE*SQRT(SSQ)
      END IF
!*
      DNRM2 = NORM
      RETURN
!*
!*     End of DNRM2.
!*
      END

!==========================================================================

!*> \brief \b DLAPY2 returns sqrt(x2+y2).
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLAPY2 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlapy2.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlapy2.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlapy2.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       DOUBLE PRECISION FUNCTION DLAPY2( X, Y )
!
!        IMPLICIT NONE
!*
!*       .. Scalar Arguments ..
!*       DOUBLE PRECISION   X, Y
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLAPY2 returns sqrt(x**2+y**2), taking care not to cause unnecessary
!*> overflow.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] X
!*> \verbatim
!*>          X is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in] Y
!*> \verbatim
!*>          Y is DOUBLE PRECISION
!*>          X and Y specify the values x and y.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      DOUBLE PRECISION FUNCTION DLAPY2( X, Y )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      DOUBLE PRECISION   X, Y
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D0 )
      DOUBLE PRECISION   ONE
      PARAMETER          ( ONE = 1.0D0 )
!*     ..
!*     .. Local Scalars ..
      DOUBLE PRECISION   W, XABS, YABS, Z
!*     ..
!*     .. Executable Statements ..
!*
      XABS = ABS( X )
      YABS = ABS( Y )
      W = MAX( XABS, YABS )
      Z = MIN( XABS, YABS )
      IF( Z.EQ.ZERO ) THEN
         DLAPY2 = W
      ELSE
         DLAPY2 = W*SQRT( ONE+( Z / W )**2 )
      END IF
      RETURN
!*
!*     End of DLAPY2
!*
      END

!==========================================================================

!*> \brief \b ILADLC scans a matrix for its last non-zero column.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download ILADLC + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/iladlc.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/iladlc.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/iladlc.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       INTEGER FUNCTION ILADLC( M, N, A, LDA )
!
!        IMPLICIT NONE
!*
!*       .. Scalar Arguments ..
!*       INTEGER            M, N, LDA
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   A( LDA, * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> ILADLC scans A for its last non-zero column.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] M
!*> \verbatim
!*>          M is INTEGER
!*>          The number of rows of the matrix A.
!*> \endverbatim
!*>
!*> \param[in] N
!*> \verbatim
!*>          N is INTEGER
!*>          The number of columns of the matrix A.
!*> \endverbatim
!*>
!*> \param[in] A
!*> \verbatim
!*>          A is DOUBLE PRECISION array, dimension (LDA,N)
!*>          The m by n matrix A.
!*> \endverbatim
!*>
!*> \param[in] LDA
!*> \verbatim
!*>          LDA is INTEGER
!*>          The leading dimension of the array A. LDA >= max(1,M).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERauxiliary
!*
!*  =====================================================================
      INTEGER FUNCTION ILADLC( M, N, A, LDA )
!
      IMPLICIT NONE
!*
!*  -- LAPACK auxiliary routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            M, N, LDA
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION ZERO
      PARAMETER ( ZERO = 0.0D+0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER I
!*     ..
!*     .. Executable Statements ..
!*
!*     Quick test for the common case where one corner is non-zero.
      IF( N.EQ.0 ) THEN
         ILADLC = N
      ELSE IF( A(1, N).NE.ZERO .OR. A(M, N).NE.ZERO ) THEN
         ILADLC = N
      ELSE
!*     Now scan each column from the end, returning with the first non-zero.
         DO ILADLC = N, 1, -1
            DO I = 1, M
               IF( A(I, ILADLC).NE.ZERO ) RETURN
            END DO
         END DO
      END IF
      RETURN
      END

!==========================================================================

!*> \brief \b DLASQ3 checks for deflation, computes a shift and calls dqds. Used by sbdsqr.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASQ3 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasq3.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasq3.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasq3.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASQ3( I0, N0, Z, PP, DMIN, SIGMA, DESIG, QMAX, NFAIL,
!*                          ITER, NDIV, IEEE, TTYPE, DMIN1, DMIN2, DN, DN1,
!*                          DN2, G, TAU )
!*
!*       .. Scalar Arguments ..
!*       LOGICAL            IEEE
!*       INTEGER            I0, ITER, N0, NDIV, NFAIL, PP
!*       DOUBLE PRECISION   DESIG, DMIN, DMIN1, DMIN2, DN, DN1, DN2, G, &
!*                          QMAX, SIGMA, TAU
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   Z( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASQ3 checks for deflation, computes a shift (TAU) and calls dqds.
!*> In case of failure it changes shifts, and tries again until output
!*> is positive.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] I0
!*> \verbatim
!*>          I0 is INTEGER
!*>         First index.
!*> \endverbatim
!*>
!*> \param[in,out] N0
!*> \verbatim
!*>          N0 is INTEGER
!*>         Last index.
!*> \endverbatim
!*>
!*> \param[in] Z
!*> \verbatim
!*>          Z is DOUBLE PRECISION array, dimension ( 4*N )
!*>         Z holds the qd array.
!*> \endverbatim
!*>
!*> \param[in,out] PP
!*> \verbatim
!*>          PP is INTEGER
!*>         PP=0 for ping, PP=1 for pong.
!*>         PP=2 indicates that flipping was applied to the Z array
!*>         and that the initial tests for deflation should not be
!*>         performed.
!*> \endverbatim
!*>
!*> \param[out] DMIN
!*> \verbatim
!*>          DMIN is DOUBLE PRECISION
!*>         Minimum value of d.
!*> \endverbatim
!*>
!*> \param[out] SIGMA
!*> \verbatim
!*>          SIGMA is DOUBLE PRECISION
!*>         Sum of shifts used in current segment.
!*> \endverbatim
!*>
!*> \param[in,out] DESIG
!*> \verbatim
!*>          DESIG is DOUBLE PRECISION
!*>         Lower order part of SIGMA
!*> \endverbatim
!*>
!*> \param[in] QMAX
!*> \verbatim
!*>          QMAX is DOUBLE PRECISION
!*>         Maximum value of q.
!*> \endverbatim
!*>
!*> \param[out] NFAIL
!*> \verbatim
!*>          NFAIL is INTEGER
!*>         Number of times shift was too big.
!*> \endverbatim
!*>
!*> \param[out] ITER
!*> \verbatim
!*>          ITER is INTEGER
!*>         Number of iterations.
!*> \endverbatim
!*>
!*> \param[out] NDIV
!*> \verbatim
!*>          NDIV is INTEGER
!*>         Number of divisions.
!*> \endverbatim
!*>
!*> \param[in] IEEE
!*> \verbatim
!*>          IEEE is LOGICAL
!*>         Flag for IEEE or non IEEE arithmetic (passed to DLASQ5).
!*> \endverbatim
!*>
!*> \param[in,out] TTYPE
!*> \verbatim
!*>          TTYPE is INTEGER
!*>         Shift type.
!*> \endverbatim
!*>
!*> \param[in,out] DMIN1
!*> \verbatim
!*>          DMIN1 is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in,out] DMIN2
!*> \verbatim
!*>          DMIN2 is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in,out] DN
!*> \verbatim
!*>          DN is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in,out] DN1
!*> \verbatim
!*>          DN1 is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in,out] DN2
!*> \verbatim
!*>          DN2 is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in,out] G
!*> \verbatim
!*>          G is DOUBLE PRECISION
!*> \endverbatim
!*>
!*> \param[in,out] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION
!*>
!*>         These are passed as arguments in order to save their values
!*>         between calls to DLASQ3.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DLASQ3( I0, N0, Z, PP, DMIN, SIGMA, DESIG, QMAX, NFAIL, &
                         ITER, NDIV, IEEE, TTYPE, DMIN1, DMIN2, DN, DN1, &
                         DN2, G, TAU )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      LOGICAL            IEEE
      INTEGER            I0, ITER, N0, NDIV, NFAIL, PP
      DOUBLE PRECISION   DESIG, DMIN, DMIN1, DMIN2, DN, DN1, DN2, G, &
                         QMAX, SIGMA, TAU
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   Z( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   CBIAS
      PARAMETER          ( CBIAS = 1.50D0 )
      DOUBLE PRECISION   ZERO, QURTR, HALF, ONE, TWO, HUNDRD
      PARAMETER          ( ZERO = 0.0D0, QURTR = 0.250D0, HALF = 0.5D0, &
                           ONE = 1.0D0, TWO = 2.0D0, HUNDRD = 100.0D0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            IPN4, J4, N0IN, NN, TTYPE
      DOUBLE PRECISION   EPS, S, T, TEMP, TOL, TOL2
!*     ..
!*     .. External Subroutines ..
      EXTERNAL           DLASQ4, DLASQ5, DLASQ6
!*     ..
!*     .. External Function ..
      DOUBLE PRECISION   DLAMCH
      LOGICAL            DISNAN
      EXTERNAL           DISNAN, DLAMCH
!*     ..
!*     .. Executable Statements ..
!*
      N0IN = N0
      EPS = DLAMCH( 'Precision' )
      TOL = EPS*HUNDRD
      TOL2 = TOL**2
!*
!*     Check for deflation.
!*
   10 CONTINUE
!*
      IF( N0.LT.I0 ) &
         RETURN
      IF( N0.EQ.I0 ) &
         GO TO 20
      NN = 4*N0 + PP
      IF( N0.EQ.( I0+1 ) ) &
         GO TO 40
!*
!*     Check whether E(N0-1) is negligible, 1 eigenvalue.
!*
      IF( Z( NN-5 ).GT.TOL2*( SIGMA+Z( NN-3 ) ) .AND. &
          Z( NN-2*PP-4 ).GT.TOL2*Z( NN-7 ) ) &
         GO TO 30
!*
   20 CONTINUE
!*
      Z( 4*N0-3 ) = Z( 4*N0+PP-3 ) + SIGMA
      N0 = N0 - 1
      GO TO 10
!*
!*     Check  whether E(N0-2) is negligible, 2 eigenvalues.
!*
   30 CONTINUE
!*
      IF( Z( NN-9 ).GT.TOL2*SIGMA .AND. &
          Z( NN-2*PP-8 ).GT.TOL2*Z( NN-11 ) ) &
         GO TO 50
!*
   40 CONTINUE
!*
      IF( Z( NN-3 ).GT.Z( NN-7 ) ) THEN
         S = Z( NN-3 )
         Z( NN-3 ) = Z( NN-7 )
         Z( NN-7 ) = S
      END IF
      T = HALF*( ( Z( NN-7 )-Z( NN-3 ) )+Z( NN-5 ) )
      IF( Z( NN-5 ).GT.Z( NN-3 )*TOL2.AND.T.NE.ZERO ) THEN
         S = Z( NN-3 )*( Z( NN-5 ) / T )
         IF( S.LE.T ) THEN
            S = Z( NN-3 )*( Z( NN-5 ) / &
                ( T*( ONE+SQRT( ONE+S / T ) ) ) )
         ELSE
            S = Z( NN-3 )*( Z( NN-5 ) / ( T+SQRT( T )*SQRT( T+S ) ) )
         END IF
         T = Z( NN-7 ) + ( S+Z( NN-5 ) )
         Z( NN-3 ) = Z( NN-3 )*( Z( NN-7 ) / T )
         Z( NN-7 ) = T
      END IF
      Z( 4*N0-7 ) = Z( NN-7 ) + SIGMA
      Z( 4*N0-3 ) = Z( NN-3 ) + SIGMA
      N0 = N0 - 2
      GO TO 10
!*
   50 CONTINUE
      IF( PP.EQ.2 )  &
         PP = 0
!*
!*     Reverse the qd-array, if warranted.
!*
      IF( DMIN.LE.ZERO .OR. N0.LT.N0IN ) THEN
         IF( CBIAS*Z( 4*I0+PP-3 ).LT.Z( 4*N0+PP-3 ) ) THEN
            IPN4 = 4*( I0+N0 )
            DO 60 J4 = 4*I0, 2*( I0+N0-1 ), 4
               TEMP = Z( J4-3 )
               Z( J4-3 ) = Z( IPN4-J4-3 )
               Z( IPN4-J4-3 ) = TEMP
               TEMP = Z( J4-2 )
               Z( J4-2 ) = Z( IPN4-J4-2 )
               Z( IPN4-J4-2 ) = TEMP
               TEMP = Z( J4-1 )
               Z( J4-1 ) = Z( IPN4-J4-5 )
               Z( IPN4-J4-5 ) = TEMP
               TEMP = Z( J4 )
               Z( J4 ) = Z( IPN4-J4-4 )
               Z( IPN4-J4-4 ) = TEMP
   60       CONTINUE
            IF( N0-I0.LE.4 ) THEN
               Z( 4*N0+PP-1 ) = Z( 4*I0+PP-1 )
               Z( 4*N0-PP ) = Z( 4*I0-PP )
            END IF
            DMIN2 = MIN( DMIN2, Z( 4*N0+PP-1 ) )
            Z( 4*N0+PP-1 ) = MIN( Z( 4*N0+PP-1 ), Z( 4*I0+PP-1 ), &
                                  Z( 4*I0+PP+3 ) )
            Z( 4*N0-PP ) = MIN( Z( 4*N0-PP ), Z( 4*I0-PP ), &
                                Z( 4*I0-PP+4 ) )
            QMAX = MAX( QMAX, Z( 4*I0+PP-3 ), Z( 4*I0+PP+1 ) )
            DMIN = -ZERO
         END IF
      END IF
!*
!*     Choose a shift.
!*
      CALL DLASQ4( I0, N0, Z, PP, N0IN, DMIN, DMIN1, DMIN2, DN, DN1, &
                   DN2, TAU, TTYPE, G )
!*
!*     Call dqds until DMIN > 0.
!*
   70 CONTINUE
!*
      CALL DLASQ5( I0, N0, Z, PP, TAU, SIGMA, DMIN, DMIN1, DMIN2, DN, &
                   DN1, DN2, IEEE, EPS )
!*
      NDIV = NDIV + ( N0-I0+2 )
      ITER = ITER + 1
!*
!*     Check status.
!*
      IF( DMIN.GE.ZERO .AND. DMIN1.GE.ZERO ) THEN
!*
!*        Success.
!*
         GO TO 90
!*
      ELSE IF( DMIN.LT.ZERO .AND. DMIN1.GT.ZERO .AND.  &
               Z( 4*( N0-1 )-PP ).LT.TOL*( SIGMA+DN1 ) .AND. &
               ABS( DN ).LT.TOL*SIGMA ) THEN
!*
!*        Convergence hidden by negative DN.
!*
         Z( 4*( N0-1 )-PP+2 ) = ZERO
         DMIN = ZERO
         GO TO 90
      ELSE IF( DMIN.LT.ZERO ) THEN
!*
!*        TAU too big. Select new TAU and try again.
!*
         NFAIL = NFAIL + 1
         IF( TTYPE.LT.-22 ) THEN
!*
!*           Failed twice. Play it safe.
!*
            TAU = ZERO
         ELSE IF( DMIN1.GT.ZERO ) THEN
!*
!*           Late failure. Gives excellent shift.
!*
            TAU = ( TAU+DMIN )*( ONE-TWO*EPS )
            TTYPE = TTYPE - 11
         ELSE
!*
!*           Early failure. Divide by 4.
!*
            TAU = QURTR*TAU
            TTYPE = TTYPE - 12
         END IF
         GO TO 70
      ELSE IF( DISNAN( DMIN ) ) THEN
!*
!*        NaN.
!*
         IF( TAU.EQ.ZERO ) THEN
            GO TO 80
         ELSE
            TAU = ZERO
            GO TO 70
         END IF
      ELSE
!*
!*        Possible underflow. Play it safe.
!*
         GO TO 80
      END IF
!*
!*     Risk of underflow.
!*
   80 CONTINUE
      CALL DLASQ6( I0, N0, Z, PP, DMIN, DMIN1, DMIN2, DN, DN1, DN2 )
      NDIV = NDIV + ( N0-I0+2 )
      ITER = ITER + 1
      TAU = ZERO
!*
   90 CONTINUE
      IF( TAU.LT.SIGMA ) THEN
         DESIG = DESIG + TAU
         T = SIGMA + DESIG
         DESIG = DESIG - ( T-SIGMA )
      ELSE
         T = SIGMA + TAU
         DESIG = SIGMA - ( T-TAU ) + DESIG
      END IF
      SIGMA = T
!*
      RETURN
!*
!*     End of DLASQ3
!*
      END

!==========================================================================

!*> \brief \b DLASQ4 computes an approximation to the smallest eigenvalue using values
!*> of d from the previous transform. Used by sbdsqr.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASQ4 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasq4.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasq4.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasq4.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASQ4( I0, N0, Z, PP, N0IN, DMIN, DMIN1, DMIN2, DN,
!*                          DN1, DN2, TAU, TTYPE, G )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            I0, N0, N0IN, PP, TTYPE
!*       DOUBLE PRECISION   DMIN, DMIN1, DMIN2, DN, DN1, DN2, G, TAU
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   Z( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASQ4 computes an approximation TAU to the smallest eigenvalue
!*> using values of d from the previous transform.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] I0
!*> \verbatim
!*>          I0 is INTEGER
!*>        First index.
!*> \endverbatim
!*>
!*> \param[in] N0
!*> \verbatim
!*>          N0 is INTEGER
!*>        Last index.
!*> \endverbatim
!*>
!*> \param[in] Z
!*> \verbatim
!*>          Z is DOUBLE PRECISION array, dimension ( 4*N )
!*>        Z holds the qd array.
!*> \endverbatim
!*>
!*> \param[in] PP
!*> \verbatim
!*>          PP is INTEGER
!*>        PP=0 for ping, PP=1 for pong.
!*> \endverbatim
!*>
!*> \param[in] N0IN
!*> \verbatim
!*>          N0IN is INTEGER
!*>        The value of N0 at start of EIGTEST.
!*> \endverbatim
!*>
!*> \param[in] DMIN
!*> \verbatim
!*>          DMIN is DOUBLE PRECISION
!*>        Minimum value of d.
!*> \endverbatim
!*>
!*> \param[in] DMIN1
!*> \verbatim
!*>          DMIN1 is DOUBLE PRECISION
!*>        Minimum value of d, excluding D( N0 ).
!*> \endverbatim
!*>
!*> \param[in] DMIN2
!*> \verbatim
!*>          DMIN2 is DOUBLE PRECISION
!*>        Minimum value of d, excluding D( N0 ) and D( N0-1 ).
!*> \endverbatim
!*>
!*> \param[in] DN
!*> \verbatim
!*>          DN is DOUBLE PRECISION
!*>        d(N)
!*> \endverbatim
!*>
!*> \param[in] DN1
!*> \verbatim
!*>          DN1 is DOUBLE PRECISION
!*>        d(N-1)
!*> \endverbatim
!*>
!*> \param[in] DN2
!*> \verbatim
!*>          DN2 is DOUBLE PRECISION
!*>        d(N-2)
!*> \endverbatim
!*>
!*> \param[out] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION
!*>        This is the shift.
!*> \endverbatim
!*>
!*> \param[out] TTYPE
!*> \verbatim
!*>          TTYPE is INTEGER
!*>        Shift type.
!*> \endverbatim
!*>
!*> \param[in,out] G
!*> \verbatim
!*>          G is REAL
!*>        G is passed as an argument in order to save its value between
!*>        calls to DLASQ4.
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERcomputational
!*
!*> \par Further Details:
!*  =====================
!*>
!*> \verbatim
!*>
!*>  CNST1 = 9/16
!*> \endverbatim
!*>
!*  =====================================================================
      SUBROUTINE DLASQ4( I0, N0, Z, PP, N0IN, DMIN, DMIN1, DMIN2, DN, &
                         DN1, DN2, TAU, TTYPE, G )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            I0, N0, N0IN, PP, TTYPE
      DOUBLE PRECISION   DMIN, DMIN1, DMIN2, DN, DN1, DN2, G, TAU
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   Z( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameters ..
      DOUBLE PRECISION   CNST1, CNST2, CNST3
      PARAMETER          ( CNST1 = 0.5630D0, CNST2 = 1.010D0, &
                         CNST3 = 1.050D0 )
      DOUBLE PRECISION   QURTR, THIRD, HALF, ZERO, ONE, TWO, HUNDRD
      PARAMETER          ( QURTR = 0.250D0, THIRD = 0.3330D0, &
                         HALF = 0.50D0, ZERO = 0.0D0, ONE = 1.0D0, &
                         TWO = 2.0D0, HUNDRD = 100.0D0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            I4, NN, NP
      DOUBLE PRECISION   A2, B1, B2, GAM, GAP1, GAP2, S
!*     ..
!*     .. Executable Statements ..
!*
!*     A negative DMIN forces the shift to take that absolute value
!*     TTYPE records the type of shift.
!*
      IF( DMIN.LE.ZERO ) THEN
         TAU = -DMIN
         TTYPE = -1
         RETURN
      END IF
!*
      NN = 4*N0 + PP
      IF( N0IN.EQ.N0 ) THEN
!*
!*        No eigenvalues deflated.
!*
         IF( DMIN.EQ.DN .OR. DMIN.EQ.DN1 ) THEN
!*
            B1 = SQRT( Z( NN-3 ) )*SQRT( Z( NN-5 ) )
            B2 = SQRT( Z( NN-7 ) )*SQRT( Z( NN-9 ) )
            A2 = Z( NN-7 ) + Z( NN-5 )
!*
!*           Cases 2 and 3.
!*
            IF( DMIN.EQ.DN .AND. DMIN1.EQ.DN1 ) THEN
               GAP2 = DMIN2 - A2 - DMIN2*QURTR
               IF( GAP2.GT.ZERO .AND. GAP2.GT.B2 ) THEN
                  GAP1 = A2 - DN - ( B2 / GAP2 )*B2
               ELSE
                  GAP1 = A2 - DN - ( B1+B2 )
               END IF
               IF( GAP1.GT.ZERO .AND. GAP1.GT.B1 ) THEN
                  S = MAX( DN-( B1 / GAP1 )*B1, HALF*DMIN )
                  TTYPE = -2
               ELSE
                  S = ZERO
                  IF( DN.GT.B1 ) &
                     S = DN - B1
                  IF( A2.GT.( B1+B2 ) ) &
                     S = MIN( S, A2-( B1+B2 ) )
                  S = MAX( S, THIRD*DMIN )
                  TTYPE = -3
               END IF
            ELSE
!*
!*              Case 4.
!*
               TTYPE = -4
               S = QURTR*DMIN
               IF( DMIN.EQ.DN ) THEN
                  GAM = DN
                  A2 = ZERO
                  IF( Z( NN-5 ) .GT. Z( NN-7 ) ) &
                     RETURN
                  B2 = Z( NN-5 ) / Z( NN-7 )
                  NP = NN - 9
               ELSE
                  NP = NN - 2*PP
                  B2 = Z( NP-2 )
                  GAM = DN1
                  IF( Z( NP-4 ) .GT. Z( NP-2 ) ) &
                     RETURN
                  A2 = Z( NP-4 ) / Z( NP-2 )
                  IF( Z( NN-9 ) .GT. Z( NN-11 ) ) &
                     RETURN
                  B2 = Z( NN-9 ) / Z( NN-11 )
                  NP = NN - 13
               END IF
!*
!*              Approximate contribution to norm squared from I < NN-1.
!*
               A2 = A2 + B2
               DO 10 I4 = NP, 4*I0 - 1 + PP, -4
                  IF( B2.EQ.ZERO ) &
                     GO TO 20
                  B1 = B2
                  IF( Z( I4 ) .GT. Z( I4-2 ) ) &
                     RETURN
                  B2 = B2*( Z( I4 ) / Z( I4-2 ) )
                  A2 = A2 + B2
                  IF( HUNDRD*MAX( B2, B1 ).LT.A2 .OR. CNST1.LT.A2 )  &
                     GO TO 20
   10          CONTINUE
   20          CONTINUE
               A2 = CNST3*A2
!*
!*              Rayleigh quotient residual bound.
!*
               IF( A2.LT.CNST1 ) &
                  S = GAM*( ONE-SQRT( A2 ) ) / ( ONE+A2 )
            END IF
         ELSE IF( DMIN.EQ.DN2 ) THEN
!*
!*           Case 5.
!*
            TTYPE = -5
            S = QURTR*DMIN
!*
!*           Compute contribution to norm squared from I > NN-2.
!*
            NP = NN - 2*PP
            B1 = Z( NP-2 )
            B2 = Z( NP-6 )
            GAM = DN2
            IF( Z( NP-8 ).GT.B2 .OR. Z( NP-4 ).GT.B1 ) &
               RETURN
            A2 = ( Z( NP-8 ) / B2 )*( ONE+Z( NP-4 ) / B1 )
!*
!*           Approximate contribution to norm squared from I < NN-2.
!*
            IF( N0-I0.GT.2 ) THEN
               B2 = Z( NN-13 ) / Z( NN-15 )
               A2 = A2 + B2
               DO 30 I4 = NN - 17, 4*I0 - 1 + PP, -4
                  IF( B2.EQ.ZERO ) &
                     GO TO 40
                  B1 = B2
                  IF( Z( I4 ) .GT. Z( I4-2 ) ) &
                     RETURN
                  B2 = B2*( Z( I4 ) / Z( I4-2 ) )
                  A2 = A2 + B2
                  IF( HUNDRD*MAX( B2, B1 ).LT.A2 .OR. CNST1.LT.A2 )  &
                     GO TO 40
   30          CONTINUE
   40          CONTINUE
               A2 = CNST3*A2
            END IF
!*
            IF( A2.LT.CNST1 ) &
               S = GAM*( ONE-SQRT( A2 ) ) / ( ONE+A2 )
         ELSE
!*
!*           Case 6, no information to guide us.
!*
            IF( TTYPE.EQ.-6 ) THEN
               G = G + THIRD*( ONE-G )
            ELSE IF( TTYPE.EQ.-18 ) THEN
               G = QURTR*THIRD
            ELSE
               G = QURTR
            END IF
            S = G*DMIN
            TTYPE = -6
         END IF
!*
      ELSE IF( N0IN.EQ.( N0+1 ) ) THEN
!*
!*        One eigenvalue just deflated. Use DMIN1, DN1 for DMIN and DN.
!*
         IF( DMIN1.EQ.DN1 .AND. DMIN2.EQ.DN2 ) THEN
!*
!*           Cases 7 and 8.
!*
            TTYPE = -7
            S = THIRD*DMIN1
            IF( Z( NN-5 ).GT.Z( NN-7 ) ) &
               RETURN
            B1 = Z( NN-5 ) / Z( NN-7 )
            B2 = B1
            IF( B2.EQ.ZERO ) &
               GO TO 60
            DO 50 I4 = 4*N0 - 9 + PP, 4*I0 - 1 + PP, -4
               A2 = B1
               IF( Z( I4 ).GT.Z( I4-2 ) ) &
                  RETURN
               B1 = B1*( Z( I4 ) / Z( I4-2 ) )
               B2 = B2 + B1
               IF( HUNDRD*MAX( B1, A2 ).LT.B2 )  &
                  GO TO 60
   50       CONTINUE
   60       CONTINUE
            B2 = SQRT( CNST3*B2 )
            A2 = DMIN1 / ( ONE+B2**2 )
            GAP2 = HALF*DMIN2 - A2
            IF( GAP2.GT.ZERO .AND. GAP2.GT.B2*A2 ) THEN
               S = MAX( S, A2*( ONE-CNST2*A2*( B2 / GAP2 )*B2 ) )
            ELSE
               S = MAX( S, A2*( ONE-CNST2*B2 ) )
               TTYPE = -8
            END IF
         ELSE
!*
!*           Case 9.
!*
            S = QURTR*DMIN1
            IF( DMIN1.EQ.DN1 ) &
               S = HALF*DMIN1
            TTYPE = -9
         END IF
!*
      ELSE IF( N0IN.EQ.( N0+2 ) ) THEN
!*
!*        Two eigenvalues deflated. Use DMIN2, DN2 for DMIN and DN.
!*
!*        Cases 10 and 11.
!*
         IF( DMIN2.EQ.DN2 .AND. TWO*Z( NN-5 ).LT.Z( NN-7 ) ) THEN
            TTYPE = -10
            S = THIRD*DMIN2
            IF( Z( NN-5 ).GT.Z( NN-7 ) ) &
               RETURN
            B1 = Z( NN-5 ) / Z( NN-7 )
            B2 = B1
            IF( B2.EQ.ZERO ) &
               GO TO 80
            DO 70 I4 = 4*N0 - 9 + PP, 4*I0 - 1 + PP, -4
               IF( Z( I4 ).GT.Z( I4-2 ) ) &
                  RETURN
               B1 = B1*( Z( I4 ) / Z( I4-2 ) )
               B2 = B2 + B1
               IF( HUNDRD*B1.LT.B2 ) &
                  GO TO 80
   70       CONTINUE
   80       CONTINUE
            B2 = SQRT( CNST3*B2 )
            A2 = DMIN2 / ( ONE+B2**2 )
            GAP2 = Z( NN-7 ) + Z( NN-9 ) - &
                   SQRT( Z( NN-11 ) )*SQRT( Z( NN-9 ) ) - A2
            IF( GAP2.GT.ZERO .AND. GAP2.GT.B2*A2 ) THEN
               S = MAX( S, A2*( ONE-CNST2*A2*( B2 / GAP2 )*B2 ) )
            ELSE
               S = MAX( S, A2*( ONE-CNST2*B2 ) )
            END IF
         ELSE
            S = QURTR*DMIN2
            TTYPE = -11
         END IF
      ELSE IF( N0IN.GT.( N0+2 ) ) THEN
!*
!*        Case 12, more than two eigenvalues deflated. No information.
!*
         S = ZERO
         TTYPE = -12
      END IF
!*
      TAU = S
      RETURN
!*
!*     End of DLASQ4
!*
      END

!==========================================================================

!*> \brief \b DLASQ5 computes one dqds transform in ping-pong form. Used by sbdsqr and sstegr.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASQ5 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasq5.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasq5.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasq5.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASQ5( I0, N0, Z, PP, TAU, SIGMA, DMIN, DMIN1, DMIN2, DN,
!*                          DNM1, DNM2, IEEE, EPS )
!*
!*       .. Scalar Arguments ..
!*       LOGICAL            IEEE
!*       INTEGER            I0, N0, PP
!*       DOUBLE PRECISION   DMIN, DMIN1, DMIN2, DN, DNM1, DNM2, TAU, SIGMA, EPS
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   Z( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASQ5 computes one dqds transform in ping-pong form, one
!*> version for IEEE machines another for non IEEE machines.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] I0
!*> \verbatim
!*>          I0 is INTEGER
!*>        First index.
!*> \endverbatim
!*>
!*> \param[in] N0
!*> \verbatim
!*>          N0 is INTEGER
!*>        Last index.
!*> \endverbatim
!*>
!*> \param[in] Z
!*> \verbatim
!*>          Z is DOUBLE PRECISION array, dimension ( 4*N )
!*>        Z holds the qd array. EMIN is stored in Z(4*N0) to avoid
!*>        an extra argument.
!*> \endverbatim
!*>
!*> \param[in] PP
!*> \verbatim
!*>          PP is INTEGER
!*>        PP=0 for ping, PP=1 for pong.
!*> \endverbatim
!*>
!*> \param[in] TAU
!*> \verbatim
!*>          TAU is DOUBLE PRECISION
!*>        This is the shift.
!*> \endverbatim
!*>
!*> \param[in] SIGMA
!*> \verbatim
!*>          SIGMA is DOUBLE PRECISION
!*>        This is the accumulated shift up to this step.
!*> \endverbatim
!*>
!*> \param[out] DMIN
!*> \verbatim
!*>          DMIN is DOUBLE PRECISION
!*>        Minimum value of d.
!*> \endverbatim
!*>
!*> \param[out] DMIN1
!*> \verbatim
!*>          DMIN1 is DOUBLE PRECISION
!*>        Minimum value of d, excluding D( N0 ).
!*> \endverbatim
!*>
!*> \param[out] DMIN2
!*> \verbatim
!*>          DMIN2 is DOUBLE PRECISION
!*>        Minimum value of d, excluding D( N0 ) and D( N0-1 ).
!*> \endverbatim
!*>
!*> \param[out] DN
!*> \verbatim
!*>          DN is DOUBLE PRECISION
!*>        d(N0), the last value of d.
!*> \endverbatim
!*>
!*> \param[out] DNM1
!*> \verbatim
!*>          DNM1 is DOUBLE PRECISION
!*>        d(N0-1).
!*> \endverbatim
!*>
!*> \param[out] DNM2
!*> \verbatim
!*>          DNM2 is DOUBLE PRECISION
!*>        d(N0-2).
!*> \endverbatim
!*>
!*> \param[in] IEEE
!*> \verbatim
!*>          IEEE is LOGICAL
!*>        Flag for IEEE or non IEEE arithmetic.
!*> \endverbatim
!*
!*> \param[in] EPS
!*> \verbatim
!*>          EPS is DOUBLE PRECISION
!*>        This is the value of epsilon used.
!*> \endverbatim
!*>
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DLASQ5( I0, N0, Z, PP, TAU, SIGMA, DMIN, DMIN1, DMIN2, &
                         DN, DNM1, DNM2, IEEE, EPS )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      LOGICAL            IEEE
      INTEGER            I0, N0, PP
      DOUBLE PRECISION   DMIN, DMIN1, DMIN2, DN, DNM1, DNM2, TAU, &
                         SIGMA, EPS
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   Z( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameter ..
      DOUBLE PRECISION   ZERO, HALF
      PARAMETER          ( ZERO = 0.0D0, HALF = 0.5 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            J4, J4P2
      DOUBLE PRECISION   D, EMIN, TEMP, DTHRESH
!*     ..
!*     .. Executable Statements ..
!*
      IF( ( N0-I0-1 ).LE.0 ) &
         RETURN
!*
      DTHRESH = EPS*(SIGMA+TAU)
      IF( TAU.LT.DTHRESH*HALF ) TAU = ZERO
      IF( TAU.NE.ZERO ) THEN
      J4 = 4*I0 + PP - 3
      EMIN = Z( J4+4 )
      D = Z( J4 ) - TAU
      DMIN = D
      DMIN1 = -Z( J4 )
!*
      IF( IEEE ) THEN
!*
!*        Code for IEEE arithmetic.
!*
         IF( PP.EQ.0 ) THEN
            DO 10 J4 = 4*I0, 4*( N0-3 ), 4
               Z( J4-2 ) = D + Z( J4-1 )
               TEMP = Z( J4+1 ) / Z( J4-2 )
               D = D*TEMP - TAU
               DMIN = MIN( DMIN, D )
               Z( J4 ) = Z( J4-1 )*TEMP
               EMIN = MIN( Z( J4 ), EMIN )
   10       CONTINUE
         ELSE
            DO 20 J4 = 4*I0, 4*( N0-3 ), 4
               Z( J4-3 ) = D + Z( J4 )
               TEMP = Z( J4+2 ) / Z( J4-3 )
               D = D*TEMP - TAU
               DMIN = MIN( DMIN, D )
               Z( J4-1 ) = Z( J4 )*TEMP
               EMIN = MIN( Z( J4-1 ), EMIN )
   20       CONTINUE
         END IF
!*
!*        Unroll last two steps.
!*
         DNM2 = D
         DMIN2 = DMIN
         J4 = 4*( N0-2 ) - PP
         J4P2 = J4 + 2*PP - 1
         Z( J4-2 ) = DNM2 + Z( J4P2 )
         Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
         DNM1 = Z( J4P2+2 )*( DNM2 / Z( J4-2 ) ) - TAU
         DMIN = MIN( DMIN, DNM1 )
!*
         DMIN1 = DMIN
         J4 = J4 + 4
         J4P2 = J4 + 2*PP - 1
         Z( J4-2 ) = DNM1 + Z( J4P2 )
         Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
         DN = Z( J4P2+2 )*( DNM1 / Z( J4-2 ) ) - TAU
         DMIN = MIN( DMIN, DN )
!*
      ELSE
!*
!*        Code for non IEEE arithmetic.
!*
         IF( PP.EQ.0 ) THEN
            DO 30 J4 = 4*I0, 4*( N0-3 ), 4
               Z( J4-2 ) = D + Z( J4-1 )
               IF( D.LT.ZERO ) THEN
                  RETURN
               ELSE
                  Z( J4 ) = Z( J4+1 )*( Z( J4-1 ) / Z( J4-2 ) )
                  D = Z( J4+1 )*( D / Z( J4-2 ) ) - TAU
               END IF
               DMIN = MIN( DMIN, D )
               EMIN = MIN( EMIN, Z( J4 ) )
   30       CONTINUE
         ELSE
            DO 40 J4 = 4*I0, 4*( N0-3 ), 4
               Z( J4-3 ) = D + Z( J4 )
               IF( D.LT.ZERO ) THEN
                  RETURN
               ELSE
                  Z( J4-1 ) = Z( J4+2 )*( Z( J4 ) / Z( J4-3 ) )
                  D = Z( J4+2 )*( D / Z( J4-3 ) ) - TAU
               END IF
               DMIN = MIN( DMIN, D )
               EMIN = MIN( EMIN, Z( J4-1 ) )
   40       CONTINUE
         END IF
!*
!*        Unroll last two steps.
!*
         DNM2 = D
         DMIN2 = DMIN
         J4 = 4*( N0-2 ) - PP
         J4P2 = J4 + 2*PP - 1
         Z( J4-2 ) = DNM2 + Z( J4P2 )
         IF( DNM2.LT.ZERO ) THEN
            RETURN
         ELSE
            Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
            DNM1 = Z( J4P2+2 )*( DNM2 / Z( J4-2 ) ) - TAU
         END IF
         DMIN = MIN( DMIN, DNM1 )
!*
         DMIN1 = DMIN
         J4 = J4 + 4
         J4P2 = J4 + 2*PP - 1
         Z( J4-2 ) = DNM1 + Z( J4P2 )
         IF( DNM1.LT.ZERO ) THEN
            RETURN
         ELSE
            Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
            DN = Z( J4P2+2 )*( DNM1 / Z( J4-2 ) ) - TAU
         END IF
         DMIN = MIN( DMIN, DN )
!*
      END IF
      ELSE
!*     This is the version that sets d's to zero if they are small enough
         J4 = 4*I0 + PP - 3
         EMIN = Z( J4+4 )
         D = Z( J4 ) - TAU
         DMIN = D
         DMIN1 = -Z( J4 )
         IF( IEEE ) THEN
!*
!*     Code for IEEE arithmetic.
!*
            IF( PP.EQ.0 ) THEN
               DO 50 J4 = 4*I0, 4*( N0-3 ), 4
                  Z( J4-2 ) = D + Z( J4-1 )
                  TEMP = Z( J4+1 ) / Z( J4-2 )
                  D = D*TEMP - TAU
                  IF( D.LT.DTHRESH ) D = ZERO
                  DMIN = MIN( DMIN, D )
                  Z( J4 ) = Z( J4-1 )*TEMP
                  EMIN = MIN( Z( J4 ), EMIN )
 50            CONTINUE
            ELSE
               DO 60 J4 = 4*I0, 4*( N0-3 ), 4
                  Z( J4-3 ) = D + Z( J4 )
                  TEMP = Z( J4+2 ) / Z( J4-3 )
                  D = D*TEMP - TAU
                  IF( D.LT.DTHRESH ) D = ZERO
                  DMIN = MIN( DMIN, D )
                  Z( J4-1 ) = Z( J4 )*TEMP
                  EMIN = MIN( Z( J4-1 ), EMIN )
 60            CONTINUE
            END IF
!*
!*     Unroll last two steps.
!*
            DNM2 = D
            DMIN2 = DMIN
            J4 = 4*( N0-2 ) - PP
            J4P2 = J4 + 2*PP - 1
            Z( J4-2 ) = DNM2 + Z( J4P2 )
            Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
            DNM1 = Z( J4P2+2 )*( DNM2 / Z( J4-2 ) ) - TAU
            DMIN = MIN( DMIN, DNM1 )
!*
            DMIN1 = DMIN
            J4 = J4 + 4
            J4P2 = J4 + 2*PP - 1
            Z( J4-2 ) = DNM1 + Z( J4P2 )
            Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
            DN = Z( J4P2+2 )*( DNM1 / Z( J4-2 ) ) - TAU
            DMIN = MIN( DMIN, DN )
!*
         ELSE
!*
!*     Code for non IEEE arithmetic.
!*
            IF( PP.EQ.0 ) THEN
               DO 70 J4 = 4*I0, 4*( N0-3 ), 4
                  Z( J4-2 ) = D + Z( J4-1 )
                  IF( D.LT.ZERO ) THEN
                     RETURN
                  ELSE
                     Z( J4 ) = Z( J4+1 )*( Z( J4-1 ) / Z( J4-2 ) )
                     D = Z( J4+1 )*( D / Z( J4-2 ) ) - TAU
                  END IF
                  IF( D.LT.DTHRESH) D = ZERO
                  DMIN = MIN( DMIN, D )
                  EMIN = MIN( EMIN, Z( J4 ) )
 70            CONTINUE
            ELSE
               DO 80 J4 = 4*I0, 4*( N0-3 ), 4
                  Z( J4-3 ) = D + Z( J4 )
                  IF( D.LT.ZERO ) THEN
                     RETURN
                  ELSE
                     Z( J4-1 ) = Z( J4+2 )*( Z( J4 ) / Z( J4-3 ) )
                     D = Z( J4+2 )*( D / Z( J4-3 ) ) - TAU
                  END IF
                  IF( D.LT.DTHRESH) D = ZERO
                  DMIN = MIN( DMIN, D )
                  EMIN = MIN( EMIN, Z( J4-1 ) )
 80            CONTINUE
            END IF
!*
!*     Unroll last two steps.
!*
            DNM2 = D
            DMIN2 = DMIN
            J4 = 4*( N0-2 ) - PP
            J4P2 = J4 + 2*PP - 1
            Z( J4-2 ) = DNM2 + Z( J4P2 )
            IF( DNM2.LT.ZERO ) THEN
               RETURN
            ELSE
               Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
               DNM1 = Z( J4P2+2 )*( DNM2 / Z( J4-2 ) ) - TAU
            END IF
            DMIN = MIN( DMIN, DNM1 )
!*
            DMIN1 = DMIN
            J4 = J4 + 4
            J4P2 = J4 + 2*PP - 1
            Z( J4-2 ) = DNM1 + Z( J4P2 )
            IF( DNM1.LT.ZERO ) THEN
               RETURN
            ELSE
               Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
               DN = Z( J4P2+2 )*( DNM1 / Z( J4-2 ) ) - TAU
            END IF
            DMIN = MIN( DMIN, DN )
!*
         END IF
      END IF
!*
      Z( J4+2 ) = DN
      Z( 4*N0-PP ) = EMIN
      RETURN
!*
!*     End of DLASQ5
!*
      END

!==========================================================================

!*> \brief \b DLASQ6 computes one dqd transform in ping-pong form. Used by sbdsqr and sstegr.
!*
!*  =========== DOCUMENTATION ===========
!*
!* Online html documentation available at
!*            http://www.netlib.org/lapack/explore-html/
!*
!*> \htmlonly
!*> Download DLASQ6 + dependencies
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.tgz?format=tgz&filename=/lapack/lapack_routine/dlasq6.f">
!*> [TGZ]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.zip?format=zip&filename=/lapack/lapack_routine/dlasq6.f">
!*> [ZIP]</a>
!*> <a href="http://www.netlib.org/cgi-bin/netlibfiles.txt?format=txt&filename=/lapack/lapack_routine/dlasq6.f">
!*> [TXT]</a>
!*> \endhtmlonly
!*
!*  Definition:
!*  ===========
!*
!*       SUBROUTINE DLASQ6( I0, N0, Z, PP, DMIN, DMIN1, DMIN2, DN,
!*                          DNM1, DNM2 )
!*
!*       .. Scalar Arguments ..
!*       INTEGER            I0, N0, PP
!*       DOUBLE PRECISION   DMIN, DMIN1, DMIN2, DN, DNM1, DNM2
!*       ..
!*       .. Array Arguments ..
!*       DOUBLE PRECISION   Z( * )
!*       ..
!*
!*
!*> \par Purpose:
!*  =============
!*>
!*> \verbatim
!*>
!*> DLASQ6 computes one dqd (shift equal to zero) transform in
!*> ping-pong form, with protection against underflow and overflow.
!*> \endverbatim
!*
!*  Arguments:
!*  ==========
!*
!*> \param[in] I0
!*> \verbatim
!*>          I0 is INTEGER
!*>        First index.
!*> \endverbatim
!*>
!*> \param[in] N0
!*> \verbatim
!*>          N0 is INTEGER
!*>        Last index.
!*> \endverbatim
!*>
!*> \param[in] Z
!*> \verbatim
!*>          Z is DOUBLE PRECISION array, dimension ( 4*N )
!*>        Z holds the qd array. EMIN is stored in Z(4*N0) to avoid
!*>        an extra argument.
!*> \endverbatim
!*>
!*> \param[in] PP
!*> \verbatim
!*>          PP is INTEGER
!*>        PP=0 for ping, PP=1 for pong.
!*> \endverbatim
!*>
!*> \param[out] DMIN
!*> \verbatim
!*>          DMIN is DOUBLE PRECISION
!*>        Minimum value of d.
!*> \endverbatim
!*>
!*> \param[out] DMIN1
!*> \verbatim
!*>          DMIN1 is DOUBLE PRECISION
!*>        Minimum value of d, excluding D( N0 ).
!*> \endverbatim
!*>
!*> \param[out] DMIN2
!*> \verbatim
!*>          DMIN2 is DOUBLE PRECISION
!*>        Minimum value of d, excluding D( N0 ) and D( N0-1 ).
!*> \endverbatim
!*>
!*> \param[out] DN
!*> \verbatim
!*>          DN is DOUBLE PRECISION
!*>        d(N0), the last value of d.
!*> \endverbatim
!*>
!*> \param[out] DNM1
!*> \verbatim
!*>          DNM1 is DOUBLE PRECISION
!*>        d(N0-1).
!*> \endverbatim
!*>
!*> \param[out] DNM2
!*> \verbatim
!*>          DNM2 is DOUBLE PRECISION
!*>        d(N0-2).
!*> \endverbatim
!*
!*  Authors:
!*  ========
!*
!*> \author Univ. of Tennessee
!*> \author Univ. of California Berkeley
!*> \author Univ. of Colorado Denver
!*> \author NAG Ltd.
!*
!*> \date September 2012
!*
!*> \ingroup auxOTHERcomputational
!*
!*  =====================================================================
      SUBROUTINE DLASQ6( I0, N0, Z, PP, DMIN, DMIN1, DMIN2, DN, &
                         DNM1, DNM2 )
!
      IMPLICIT NONE
!*
!*  -- LAPACK computational routine (version 3.4.2) --
!*  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!*  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!*     September 2012
!*
!*     .. Scalar Arguments ..
      INTEGER            I0, N0, PP
      DOUBLE PRECISION   DMIN, DMIN1, DMIN2, DN, DNM1, DNM2
!*     ..
!*     .. Array Arguments ..
      DOUBLE PRECISION   Z( * )
!*     ..
!*
!*  =====================================================================
!*
!*     .. Parameter ..
      DOUBLE PRECISION   ZERO
      PARAMETER          ( ZERO = 0.0D0 )
!*     ..
!*     .. Local Scalars ..
      INTEGER            J4, J4P2
      DOUBLE PRECISION   D, EMIN, SAFMIN, TEMP
!*     ..
!*     .. External Function ..
      DOUBLE PRECISION   DLAMCH
      EXTERNAL           DLAMCH
!*     ..
!*     .. Executable Statements ..
!*
      IF( ( N0-I0-1 ).LE.0 ) &
         RETURN
!*
      SAFMIN = DLAMCH( 'Safe minimum' )
      J4 = 4*I0 + PP - 3
      EMIN = Z( J4+4 )
      D = Z( J4 )
      DMIN = D
!*
      IF( PP.EQ.0 ) THEN
         DO 10 J4 = 4*I0, 4*( N0-3 ), 4
            Z( J4-2 ) = D + Z( J4-1 )
            IF( Z( J4-2 ).EQ.ZERO ) THEN
               Z( J4 ) = ZERO
               D = Z( J4+1 )
               DMIN = D
               EMIN = ZERO
            ELSE IF( SAFMIN*Z( J4+1 ).LT.Z( J4-2 ) .AND. &
                     SAFMIN*Z( J4-2 ).LT.Z( J4+1 ) ) THEN
               TEMP = Z( J4+1 ) / Z( J4-2 )
               Z( J4 ) = Z( J4-1 )*TEMP
               D = D*TEMP
            ELSE
               Z( J4 ) = Z( J4+1 )*( Z( J4-1 ) / Z( J4-2 ) )
               D = Z( J4+1 )*( D / Z( J4-2 ) )
            END IF
            DMIN = MIN( DMIN, D )
            EMIN = MIN( EMIN, Z( J4 ) )
   10    CONTINUE
      ELSE
         DO 20 J4 = 4*I0, 4*( N0-3 ), 4
            Z( J4-3 ) = D + Z( J4 )
            IF( Z( J4-3 ).EQ.ZERO ) THEN
               Z( J4-1 ) = ZERO
               D = Z( J4+2 )
               DMIN = D
               EMIN = ZERO
            ELSE IF( SAFMIN*Z( J4+2 ).LT.Z( J4-3 ) .AND. &
                     SAFMIN*Z( J4-3 ).LT.Z( J4+2 ) ) THEN
               TEMP = Z( J4+2 ) / Z( J4-3 )
               Z( J4-1 ) = Z( J4 )*TEMP
               D = D*TEMP
            ELSE
               Z( J4-1 ) = Z( J4+2 )*( Z( J4 ) / Z( J4-3 ) )
               D = Z( J4+2 )*( D / Z( J4-3 ) )
            END IF
            DMIN = MIN( DMIN, D )
            EMIN = MIN( EMIN, Z( J4-1 ) )
   20    CONTINUE
      END IF
!*
!*     Unroll last two steps.
!*
      DNM2 = D
      DMIN2 = DMIN
      J4 = 4*( N0-2 ) - PP
      J4P2 = J4 + 2*PP - 1
      Z( J4-2 ) = DNM2 + Z( J4P2 )
      IF( Z( J4-2 ).EQ.ZERO ) THEN
         Z( J4 ) = ZERO
         DNM1 = Z( J4P2+2 )
         DMIN = DNM1
         EMIN = ZERO
      ELSE IF( SAFMIN*Z( J4P2+2 ).LT.Z( J4-2 ) .AND. &
               SAFMIN*Z( J4-2 ).LT.Z( J4P2+2 ) ) THEN
         TEMP = Z( J4P2+2 ) / Z( J4-2 )
         Z( J4 ) = Z( J4P2 )*TEMP
         DNM1 = DNM2*TEMP
      ELSE
         Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
         DNM1 = Z( J4P2+2 )*( DNM2 / Z( J4-2 ) )
      END IF
      DMIN = MIN( DMIN, DNM1 )
!*
      DMIN1 = DMIN
      J4 = J4 + 4
      J4P2 = J4 + 2*PP - 1
      Z( J4-2 ) = DNM1 + Z( J4P2 )
      IF( Z( J4-2 ).EQ.ZERO ) THEN
         Z( J4 ) = ZERO
         DN = Z( J4P2+2 )
         DMIN = DN
         EMIN = ZERO
      ELSE IF( SAFMIN*Z( J4P2+2 ).LT.Z( J4-2 ) .AND. &
               SAFMIN*Z( J4-2 ).LT.Z( J4P2+2 ) ) THEN
         TEMP = Z( J4P2+2 ) / Z( J4-2 )
         Z( J4 ) = Z( J4P2 )*TEMP
         DN = DNM1*TEMP
      ELSE
         Z( J4 ) = Z( J4P2+2 )*( Z( J4P2 ) / Z( J4-2 ) )
         DN = Z( J4P2+2 )*( DNM1 / Z( J4-2 ) )
      END IF
      DMIN = MIN( DMIN, DN )
!*
      Z( J4+2 ) = DN
      Z( 4*N0-PP ) = EMIN
      RETURN
!*
!*     End of DLASQ6
!*
      END





      SUBROUTINE DGELS( TRANS, M, N, NRHS, A, LDA, B, LDB, WORK, LWORK, &
                        INFO )

      IMPLICIT NONE

!  -- LAPACK driver routine (version 3.3.1) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!  -- April 2011                                                      --
!
!     .. Scalar Arguments ..
      CHARACTER          TRANS
      INTEGER            INFO, LDA, LDB, LWORK, M, N, NRHS
!     ..
!     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), B( LDB, * ), WORK( * )
!     ..
!
!  Purpose
!  =======
!
!  DGELS solves overdetermined or underdetermined real linear systems
!  involving an M-by-N matrix A, or its transpose, using a QR or LQ
!  factorization of A.  It is assumed that A has full rank.
!
!  The following options are provided:
!
!  1. If TRANS = 'N' and m >= n:  find the least squares solution of
!     an overdetermined system, i.e., solve the least squares problem
!                  minimize || B - A*X ||.
!
!  2. If TRANS = 'N' and m < n:  find the minimum norm solution of
!     an underdetermined system A * X = B.
!
!  3. If TRANS = 'T' and m >= n:  find the minimum norm solution of
!     an undetermined system A**T * X = B.
!
!  4. If TRANS = 'T' and m < n:  find the least squares solution of
!     an overdetermined system, i.e., solve the least squares problem
!                  minimize || B - A**T * X ||.
!
!  Several right hand side vectors b and solution vectors x can be
!  handled in a single call; they are stored as the columns of the
!  M-by-NRHS right hand side matrix B and the N-by-NRHS solution
!  matrix X.
!
!  Arguments
!  =========
!
!  TRANS   (input) CHARACTER*1
!          = 'N': the linear system involves A;
!          = 'T': the linear system involves A**T.
!
!  M       (input) INTEGER
!          The number of rows of the matrix A.  M >= 0.
!
!  N       (input) INTEGER
!          The number of columns of the matrix A.  N >= 0.
!
!  NRHS    (input) INTEGER
!          The number of right hand sides, i.e., the number of
!          columns of the matrices B and X. NRHS >=0.
!
!  A       (input/output) DOUBLE PRECISION array, dimension (LDA,N)
!          On entry, the M-by-N matrix A.
!          On exit,
!            if M >= N, A is overwritten by details of its QR
!                       factorization as returned by DGEQRF;
!            if M <  N, A is overwritten by details of its LQ
!                       factorization as returned by DGELQF.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,M).
!
!  B       (input/output) DOUBLE PRECISION array, dimension (LDB,NRHS)
!          On entry, the matrix B of right hand side vectors, stored
!          columnwise; B is M-by-NRHS if TRANS = 'N', or N-by-NRHS
!          if TRANS = 'T'.
!          On exit, if INFO = 0, B is overwritten by the solution
!          vectors, stored columnwise:
!          if TRANS = 'N' and m >= n, rows 1 to n of B contain the least
!          squares solution vectors; the residual sum of squares for the
!          solution in each column is given by the sum of squares of
!          elements N+1 to M in that column;
!          if TRANS = 'N' and m < n, rows 1 to N of B contain the
!          minimum norm solution vectors;
!          if TRANS = 'T' and m >= n, rows 1 to M of B contain the
!          minimum norm solution vectors;
!          if TRANS = 'T' and m < n, rows 1 to M of B contain the
!          least squares solution vectors; the residual sum of squares
!          for the solution in each column is given by the sum of
!          squares of elements M+1 to N in that column.
!
!  LDB     (input) INTEGER
!          The leading dimension of the array B. LDB >= MAX(1,M,N).
!
!  WORK    (workspace/output) DOUBLE PRECISION array, dimension (MAX(1,LWORK))
!          On exit, if INFO = 0, WORK(1) returns the optimal LWORK.
!
!  LWORK   (input) INTEGER
!          The dimension of the array WORK.
!          LWORK >= max( 1, MN + max( MN, NRHS ) ).
!          For optimal performance,
!          LWORK >= max( 1, MN + max( MN, NRHS )*NB ).
!          where MN = min(M,N) and NB is the optimum block size.
!
!          If LWORK = -1, then a workspace query is assumed; the routine
!          only calculates the optimal size of the WORK array, returns
!          this value as the first entry of the WORK array, and no error
!          message related to LWORK is issued by XERBLA.
!
!  INFO    (output) INTEGER
!          = 0:  successful exit
!          < 0:  if INFO = -i, the i-th argument had an illegal value
!          > 0:  if INFO =  i, the i-th diagonal element of the
!                triangular factor of A is zero, so that A does not have
!                full rank; the least squares solution could not be
!                computed.
!
!  =====================================================================
!
!     .. Parameters ..
      DOUBLE PRECISION   ZERO, ONE
      PARAMETER          ( ZERO = 0.0D0, ONE = 1.0D0 )
!     ..
!     .. Local Scalars ..
      LOGICAL            LQUERY, TPSD
      INTEGER            BROW, I, IASCL, IBSCL, J, MN, NB, SCLLEN, WSIZE
      DOUBLE PRECISION   ANRM, BIGNUM, BNRM, SMLNUM
!     ..
!     .. Local Arrays ..
      DOUBLE PRECISION   RWORK( 1 )
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      INTEGER            ILAENV
      DOUBLE PRECISION   DLAMCH, DLANGE
      EXTERNAL           LSAME, ILAENV, DLABAD, DLAMCH, DLANGE
!     ..
!     .. External Subroutines ..
      EXTERNAL           DGELQF, DGEQRF, DLASCL, DLASET, DORMLQ, DORMQR, &
                         DTRTRS, XERBLA
!     ..
!     .. Intrinsic Functions ..
      INTRINSIC          DBLE, MAX, MIN
!     ..
!     .. Executable Statements ..
!
!     Test the input arguments.
!
      INFO = 0
      MN = MIN( M, N )
      LQUERY = ( LWORK.EQ.-1 )
      IF( .NOT.( LSAME( TRANS, 'N' ) .OR. LSAME( TRANS, 'T' ) ) ) THEN
         INFO = -1
      ELSE IF( M.LT.0 ) THEN
         INFO = -2
      ELSE IF( N.LT.0 ) THEN
         INFO = -3
      ELSE IF( NRHS.LT.0 ) THEN
         INFO = -4
      ELSE IF( LDA.LT.MAX( 1, M ) ) THEN
         INFO = -6
      ELSE IF( LDB.LT.MAX( 1, M, N ) ) THEN
         INFO = -8
      ELSE IF( LWORK.LT.MAX( 1, MN+MAX( MN, NRHS ) ) .AND. .NOT.LQUERY ) &
                THEN
         INFO = -10
      END IF
!
!     Figure out optimal block size
!
      IF( INFO.EQ.0 .OR. INFO.EQ.-10 ) THEN
!
         TPSD = .TRUE.
         IF( LSAME( TRANS, 'N' ) ) &
            TPSD = .FALSE.
!
         IF( M.GE.N ) THEN
            NB = ILAENV( 1, 'DGEQRF', ' ', M, N, -1, -1 )
            IF( TPSD ) THEN
               NB = MAX( NB, ILAENV( 1, 'DORMQR', 'LN', M, NRHS, N, &
                    -1 ) )
            ELSE
               NB = MAX( NB, ILAENV( 1, 'DORMQR', 'LT', M, NRHS, N, &
                    -1 ) )
            END IF
         ELSE
            NB = ILAENV( 1, 'DGELQF', ' ', M, N, -1, -1 )
            IF( TPSD ) THEN
               NB = MAX( NB, ILAENV( 1, 'DORMLQ', 'LT', N, NRHS, M, &
                    -1 ) )
            ELSE
               NB = MAX( NB, ILAENV( 1, 'DORMLQ', 'LN', N, NRHS, M, &
                    -1 ) )
            END IF
         END IF
!
         WSIZE = MAX( 1, MN+MAX( MN, NRHS )*NB )
         WORK( 1 ) = DBLE( WSIZE )
!
      END IF
!
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DGELS ', -INFO )
         RETURN
      ELSE IF( LQUERY ) THEN
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( MIN( M, N, NRHS ).EQ.0 ) THEN
         CALL DLASET( 'Full', MAX( M, N ), NRHS, ZERO, ZERO, B, LDB )
         RETURN
      END IF
!
!     Get machine parameters
!
      SMLNUM = DLAMCH( 'S' ) / DLAMCH( 'P' )
      BIGNUM = ONE / SMLNUM
      CALL DLABAD( SMLNUM, BIGNUM )
!
!     Scale A, B if max element outside range [SMLNUM,BIGNUM]
!
      ANRM = DLANGE( 'M', M, N, A, LDA, RWORK )
      IASCL = 0
      IF( ANRM.GT.ZERO .AND. ANRM.LT.SMLNUM ) THEN
!
!        Scale matrix norm up to SMLNUM
!
         CALL DLASCL( 'G', 0, 0, ANRM, SMLNUM, M, N, A, LDA, INFO )
         IASCL = 1
      ELSE IF( ANRM.GT.BIGNUM ) THEN
!
!        Scale matrix norm down to BIGNUM
!
         CALL DLASCL( 'G', 0, 0, ANRM, BIGNUM, M, N, A, LDA, INFO )
         IASCL = 2
      ELSE IF( ANRM.EQ.ZERO ) THEN
!
!        Matrix all zero. Return zero solution.
!
         CALL DLASET( 'F', MAX( M, N ), NRHS, ZERO, ZERO, B, LDB )

         WORK( 1 ) = DBLE( WSIZE )
         RETURN
      END IF
!
      BROW = M
      IF( TPSD ) BROW = N
      BNRM = DLANGE( 'M', BROW, NRHS, B, LDB, RWORK )
      IBSCL = 0
      IF( BNRM.GT.ZERO .AND. BNRM.LT.SMLNUM ) THEN
!
!        Scale matrix norm up to SMLNUM
!
         CALL DLASCL( 'G', 0, 0, BNRM, SMLNUM, BROW, NRHS, B, LDB, &
                      INFO )
         IBSCL = 1
      ELSE IF( BNRM.GT.BIGNUM ) THEN
!
!        Scale matrix norm down to BIGNUM
!
         CALL DLASCL( 'G', 0, 0, BNRM, BIGNUM, BROW, NRHS, B, LDB, &
                      INFO )
         IBSCL = 2
      END IF
!
      IF( M.GE.N ) THEN
!
!        compute QR factorization of A
!
         CALL DGEQRF( M, N, A, LDA, WORK( 1 ), WORK( MN+1 ), LWORK-MN, &
                      INFO )
!
!        workspace at least N, optimally N*NB
!
         IF( .NOT.TPSD ) THEN
!
!           Least-Squares Problem min || A * X - B ||
!
!           B(1:M,1:NRHS) := Q**T * B(1:M,1:NRHS)
!
            CALL DORMQR( 'Left', 'Transpose', M, NRHS, N, A, LDA, &
                         WORK( 1 ), B, LDB, WORK( MN+1 ), LWORK-MN, &
                         INFO )
!
!           workspace at least NRHS, optimally NRHS*NB
!
!           B(1:N,1:NRHS) := inv(R) * B(1:N,1:NRHS)
!
            CALL DTRTRS( 'Upper', 'No transpose', 'Non-unit', N, NRHS, &
                         A, LDA, B, LDB, INFO )
!
            IF( INFO.GT.0 ) THEN
               RETURN
            END IF
!
            SCLLEN = N
!
         ELSE
!
!           Overdetermined system of equations A**T * X = B
!
!           B(1:N,1:NRHS) := inv(R**T) * B(1:N,1:NRHS)
!
            CALL DTRTRS( 'Upper', 'Transpose', 'Non-unit', N, NRHS, &
                         A, LDA, B, LDB, INFO )
!
            IF( INFO.GT.0 ) THEN
               RETURN
            END IF
!
!           B(N+1:M,1:NRHS) = ZERO
!
            DO J = 1, NRHS
               DO I = N + 1, M
                  B( I, J ) = ZERO
               END DO
            END DO
!
!           B(1:M,1:NRHS) := Q(1:N,:) * B(1:N,1:NRHS)
!
            CALL DORMQR( 'Left', 'No transpose', M, NRHS, N, A, LDA, &
                         WORK( 1 ), B, LDB, WORK( MN+1 ), LWORK-MN, &
                         INFO )
!
!           workspace at least NRHS, optimally NRHS*NB
!
            SCLLEN = M
!
         END IF
!
      ELSE
!
!        Compute LQ factorization of A
!
         CALL DGELQF( M, N, A, LDA, WORK( 1 ), WORK( MN+1 ), LWORK-MN, &
                      INFO )
!
!        workspace at least M, optimally M*NB.
!
         IF( .NOT.TPSD ) THEN
!
!           underdetermined system of equations A * X = B
!
!           B(1:M,1:NRHS) := inv(L) * B(1:M,1:NRHS)
!
            CALL DTRTRS( 'Lower', 'No transpose', 'Non-unit', M, NRHS, &
                         A, LDA, B, LDB, INFO )
!
            IF( INFO.GT.0 ) THEN
               RETURN
            END IF
!
!           B(M+1:N,1:NRHS) = 0
!
            DO J = 1, NRHS
               DO I = M + 1, N
                  B( I, J ) = ZERO
               END DO
            END DO
!
!           B(1:N,1:NRHS) := Q(1:N,:)**T * B(1:M,1:NRHS)
!
            CALL DORMLQ( 'Left', 'Transpose', N, NRHS, M, A, LDA, &
                         WORK( 1 ), B, LDB, WORK( MN+1 ), LWORK-MN, &
                         INFO )
!
!           workspace at least NRHS, optimally NRHS*NB
!
            SCLLEN = N
!
         ELSE
!
!           overdetermined system min || A**T * X - B ||
!
!           B(1:N,1:NRHS) := Q * B(1:N,1:NRHS)
!
            CALL DORMLQ( 'Left', 'No transpose', N, NRHS, M, A, LDA, &
                         WORK( 1 ), B, LDB, WORK( MN+1 ), LWORK-MN, &
                         INFO )
!
!           workspace at least NRHS, optimally NRHS*NB
!
!           B(1:M,1:NRHS) := inv(L**T) * B(1:M,1:NRHS)
!
            CALL DTRTRS( 'Lower', 'Transpose', 'Non-unit', M, NRHS, &
                         A, LDA, B, LDB, INFO )
!
            IF( INFO.GT.0 ) THEN
               RETURN
            END IF
!
            SCLLEN = M
!
         END IF
!
      END IF
!
!     Undo scaling
!
      IF( IASCL.EQ.1 ) THEN
         CALL DLASCL( 'G', 0, 0, ANRM, SMLNUM, SCLLEN, NRHS, B, LDB, &
                      INFO )
      ELSE IF( IASCL.EQ.2 ) THEN
         CALL DLASCL( 'G', 0, 0, ANRM, BIGNUM, SCLLEN, NRHS, B, LDB, &
                      INFO )
      END IF
      IF( IBSCL.EQ.1 ) THEN
         CALL DLASCL( 'G', 0, 0, SMLNUM, BNRM, SCLLEN, NRHS, B, LDB, &
                      INFO )
      ELSE IF( IBSCL.EQ.2 ) THEN
         CALL DLASCL( 'G', 0, 0, BIGNUM, BNRM, SCLLEN, NRHS, B, LDB, &
                      INFO )
      END IF
!
      WORK( 1 ) = DBLE( WSIZE )
!
!     End of DGELS
!
      END SUBROUTINE DGELS

      SUBROUTINE DLABAD( SMALL, LARGE )

      IMPLICIT NONE

!  -- LAPACK auxiliary routine (version 3.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2006
!
!     .. Scalar Arguments ..
      DOUBLE PRECISION   LARGE, SMALL
!     ..
!
!  Purpose
!  =======
!
!  DLABAD takes as input the values computed by DLAMCH for underflow and
!  overflow, and returns the square root of each of these values if the
!  log of LARGE is sufficiently large.  This subroutine is intended to
!  identify machines with a large exponent range, such as the Crays, and
!  redefine the underflow and overflow limits to be the square roots of
!  the values computed by DLAMCH.  This subroutine is needed because
!  DLAMCH does not compensate for poor arithmetic in the upper half of
!  the exponent range, as is found on a Cray.
!
!  Arguments
!  =========
!
!  SMALL   (input/output) DOUBLE PRECISION
!          On entry, the underflow threshold as computed by DLAMCH.
!          On exit, if LOG10(LARGE) is sufficiently large, the square
!          root of SMALL, otherwise unchanged.
!
!  LARGE   (input/output) DOUBLE PRECISION
!          On entry, the overflow threshold as computed by DLAMCH.
!          On exit, if LOG10(LARGE) is sufficiently large, the square
!          root of LARGE, otherwise unchanged.
!
!  =====================================================================
!
!     ..
!     .. Executable Statements ..
!
!     If it looks like we're on a Cray, take the square root of
!     SMALL and LARGE to avoid overflow and underflow problems.
!
      IF( LOG10( LARGE ).GT.2000.D0 ) THEN
         SMALL = SQRT( SMALL )
         LARGE = SQRT( LARGE )
      END IF

      END SUBROUTINE DLABAD

      SUBROUTINE DTRTRS( UPLO, TRANS, DIAG, N, NRHS, A, LDA, B, LDB, &
                         INFO )

       IMPLICIT NONE

!  -- LAPACK routine (version 3.2) --
!  -- LAPACK is a software package provided by Univ. of Tennessee,    --
!  -- Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd..--
!     November 2006
!
!     .. Scalar Arguments ..
      CHARACTER          DIAG, TRANS, UPLO
      INTEGER            INFO, LDA, LDB, N, NRHS
!     ..
!     .. Array Arguments ..
      DOUBLE PRECISION   A( LDA, * ), B( LDB, * )
!     ..
!
!  Purpose
!  =======
!
!  DTRTRS solves a triangular system of the form
!
!     A * X = B  or  A**T * X = B,
!
!  where A is a triangular matrix of order N, and B is an N-by-NRHS
!  matrix.  A check is made to verify that A is nonsingular.
!
!  Arguments
!  =========
!
!  UPLO    (input) CHARACTER*1
!          = 'U':  A is upper triangular;
!          = 'L':  A is lower triangular.
!
!  TRANS   (input) CHARACTER*1
!          Specifies the form of the system of equations:
!          = 'N':  A * X = B  (No transpose)
!          = 'T':  A**T * X = B  (Transpose)
!          = 'C':  A**H * X = B  (Conjugate transpose = Transpose)
!
!  DIAG    (input) CHARACTER*1
!          = 'N':  A is non-unit triangular;
!          = 'U':  A is unit triangular.
!
!  N       (input) INTEGER
!          The order of the matrix A.  N >= 0.
!
!  NRHS    (input) INTEGER
!          The number of right hand sides, i.e., the number of columns
!          of the matrix B.  NRHS >= 0.
!
!  A       (input) DOUBLE PRECISION array, dimension (LDA,N)
!          The triangular matrix A.  If UPLO = 'U', the leading N-by-N
!          upper triangular part of the array A contains the upper
!          triangular matrix, and the strictly lower triangular part of
!          A is not referenced.  If UPLO = 'L', the leading N-by-N lower
!          triangular part of the array A contains the lower triangular
!          matrix, and the strictly upper triangular part of A is not
!          referenced.  If DIAG = 'U', the diagonal elements of A are
!          also not referenced and are assumed to be 1.
!
!  LDA     (input) INTEGER
!          The leading dimension of the array A.  LDA >= max(1,N).
!
!  B       (input/output) DOUBLE PRECISION array, dimension (LDB,NRHS)
!          On entry, the right hand side matrix B.
!          On exit, if INFO = 0, the solution matrix X.
!
!  LDB     (input) INTEGER
!          The leading dimension of the array B.  LDB >= max(1,N).
!
!  INFO    (output) INTEGER
!          = 0:  successful exit
!          < 0: if INFO = -i, the i-th argument had an illegal value
!          > 0: if INFO = i, the i-th diagonal element of A is zero,
!               indicating that the matrix is singular and the solutions
!               X have not been computed.
!
!  =====================================================================
!
!     .. Parameters ..
      DOUBLE PRECISION   ZERO, ONE
      PARAMETER          ( ZERO = 0.0D+0, ONE = 1.0D+0 )
!     ..
!     .. Local Scalars ..
      LOGICAL            NOUNIT
!     ..
!     .. External Functions ..
      LOGICAL            LSAME
      EXTERNAL           LSAME
!     ..
!     .. External Subroutines ..
      EXTERNAL           DTRSM, XERBLA
!     ..
!     .. Executable Statements ..
!
!     Test the input parameters.
!
      INFO = 0
      NOUNIT = LSAME( DIAG, 'N' )
      IF( .NOT.LSAME( UPLO, 'U' ) .AND. .NOT.LSAME( UPLO, 'L' ) ) THEN
         INFO = -1
      ELSE IF( .NOT.LSAME( TRANS, 'N' ) .AND. .NOT. &
               LSAME( TRANS, 'T' ) .AND. .NOT.LSAME( TRANS, 'C' ) ) THEN
         INFO = -2
      ELSE IF( .NOT.NOUNIT .AND. .NOT.LSAME( DIAG, 'U' ) ) THEN
         INFO = -3
      ELSE IF( N.LT.0 ) THEN
         INFO = -4
      ELSE IF( NRHS.LT.0 ) THEN
         INFO = -5
      ELSE IF( LDA.LT.MAX( 1, N ) ) THEN
         INFO = -7
      ELSE IF( LDB.LT.MAX( 1, N ) ) THEN
         INFO = -9
      END IF
      IF( INFO.NE.0 ) THEN
         CALL XERBLA( 'DTRTRS', -INFO )
         RETURN
      END IF
!
!     Quick return if possible
!
      IF( N.EQ.0 ) RETURN
!
!     Check for singularity.
!
      IF( NOUNIT ) THEN
         DO INFO = 1, N
            IF( A( INFO, INFO ).EQ.ZERO ) RETURN
         END DO
      END IF
      INFO = 0
!
!     Solve A * x = b  or  A**T * x = b.
!
      CALL DTRSM( 'Left', UPLO, TRANS, DIAG, N, NRHS, ONE, A, LDA, B, &
                  LDB )

      END SUBROUTINE DTRTRS

      SUBROUTINE DTRSM(SIDE,UPLO,TRANSA,DIAG,M,N,ALPHA,A,LDA,B,LDB)

      IMPLICIT NONE

!     .. Scalar Arguments ..
      DOUBLE PRECISION ALPHA
      INTEGER LDA,LDB,M,N
      CHARACTER DIAG,SIDE,TRANSA,UPLO
!     ..
!     .. Array Arguments ..
      DOUBLE PRECISION A(LDA,*),B(LDB,*)
!     ..
!
!  Purpose
!  =======
!
!  DTRSM  solves one of the matrix equations
!
!     op( A )*X = alpha*B,   or   X*op( A ) = alpha*B,
!
!  where alpha is a scalar, X and B are m by n matrices, A is a unit, or
!  non-unit,  upper or lower triangular matrix  and  op( A )  is one  of
!
!     op( A ) = A   or   op( A ) = A**T.
!
!  The matrix X is overwritten on B.
!
!  Arguments
!  ==========
!
!  SIDE   - CHARACTER*1.
!           On entry, SIDE specifies whether op( A ) appears on the left
!           or right of X as follows:
!
!              SIDE = 'L' or 'l'   op( A )*X = alpha*B.
!
!              SIDE = 'R' or 'r'   X*op( A ) = alpha*B.
!
!           Unchanged on exit.
!
!  UPLO   - CHARACTER*1.
!           On entry, UPLO specifies whether the matrix A is an upper or
!           lower triangular matrix as follows:
!
!              UPLO = 'U' or 'u'   A is an upper triangular matrix.
!
!              UPLO = 'L' or 'l'   A is a lower triangular matrix.
!
!           Unchanged on exit.
!
!  TRANSA - CHARACTER*1.
!           On entry, TRANSA specifies the form of op( A ) to be used in
!           the matrix multiplication as follows:
!
!              TRANSA = 'N' or 'n'   op( A ) = A.
!
!              TRANSA = 'T' or 't'   op( A ) = A**T.
!
!              TRANSA = 'C' or 'c'   op( A ) = A**T.
!
!           Unchanged on exit.
!
!  DIAG   - CHARACTER*1.
!           On entry, DIAG specifies whether or not A is unit triangular
!           as follows:
!
!              DIAG = 'U' or 'u'   A is assumed to be unit triangular.
!
!              DIAG = 'N' or 'n'   A is not assumed to be unit
!                                  triangular.
!
!           Unchanged on exit.
!
!  M      - INTEGER.
!           On entry, M specifies the number of rows of B. M must be at
!           least zero.
!           Unchanged on exit.
!
!  N      - INTEGER.
!           On entry, N specifies the number of columns of B.  N must be
!           at least zero.
!           Unchanged on exit.
!
!  ALPHA  - DOUBLE PRECISION.
!           On entry,  ALPHA specifies the scalar  alpha. When  alpha is
!           zero then  A is not referenced and  B need not be set before
!           entry.
!           Unchanged on exit.
!
!  A      - DOUBLE PRECISION array of DIMENSION ( LDA, k ), where k is m
!           when  SIDE = 'L' or 'l'  and is  n  when  SIDE = 'R' or 'r'.
!           Before entry  with  UPLO = 'U' or 'u',  the  leading  k by k
!           upper triangular part of the array  A must contain the upper
!           triangular matrix  and the strictly lower triangular part of
!           A is not referenced.
!           Before entry  with  UPLO = 'L' or 'l',  the  leading  k by k
!           lower triangular part of the array  A must contain the lower
!           triangular matrix  and the strictly upper triangular part of
!           A is not referenced.
!           Note that when  DIAG = 'U' or 'u',  the diagonal elements of
!           A  are not referenced either,  but are assumed to be  unity.
!           Unchanged on exit.
!
!  LDA    - INTEGER.
!           On entry, LDA specifies the first dimension of A as declared
!           in the calling (sub) program.  When  SIDE = 'L' or 'l'  then
!           LDA  must be at least  max( 1, m ),  when  SIDE = 'R' or 'r'
!           then LDA must be at least max( 1, n ).
!           Unchanged on exit.
!
!  B      - DOUBLE PRECISION array of DIMENSION ( LDB, n ).
!           Before entry,  the leading  m by n part of the array  B must
!           contain  the  right-hand  side  matrix  B,  and  on exit  is
!           overwritten by the solution matrix  X.
!
!  LDB    - INTEGER.
!           On entry, LDB specifies the first dimension of B as declared
!           in  the  calling  (sub)  program.   LDB  must  be  at  least
!           max( 1, m ).
!           Unchanged on exit.
!
!  Further Details
!  ===============
!
!  Level 3 Blas routine.
!
!
!  -- Written on 8-February-1989.
!     Jack Dongarra, Argonne National Laboratory.
!     Iain Duff, AERE Harwell.
!     Jeremy Du Croz, Numerical Algorithms Group Ltd.
!     Sven Hammarling, Numerical Algorithms Group Ltd.
!
!  =====================================================================
!
!     .. External Functions ..
      LOGICAL LSAME
      EXTERNAL LSAME
!     ..
!     .. External Subroutines ..
      EXTERNAL XERBLA
!     ..
!     .. Local Scalars ..
      DOUBLE PRECISION TEMP
      INTEGER I,INFO,J,K,NROWA
      LOGICAL LSIDE,NOUNIT,UPPER
!     ..
!     .. Parameters ..
      DOUBLE PRECISION ONE,ZERO
      PARAMETER (ONE=1.0D+0,ZERO=0.0D+0)
!     ..
!
!     Test the input parameters.
!
      LSIDE = LSAME(SIDE,'L')
      IF (LSIDE) THEN
          NROWA = M
      ELSE
          NROWA = N
      END IF
      NOUNIT = LSAME(DIAG,'N')
      UPPER = LSAME(UPLO,'U')
!
      INFO = 0
      IF ((.NOT.LSIDE) .AND. (.NOT.LSAME(SIDE,'R'))) THEN
          INFO = 1
      ELSE IF ((.NOT.UPPER) .AND. (.NOT.LSAME(UPLO,'L'))) THEN
          INFO = 2
      ELSE IF ((.NOT.LSAME(TRANSA,'N')) .AND. &
               (.NOT.LSAME(TRANSA,'T')) .AND. &
               (.NOT.LSAME(TRANSA,'C'))) THEN
          INFO = 3
      ELSE IF ((.NOT.LSAME(DIAG,'U')) .AND. (.NOT.LSAME(DIAG,'N'))) THEN
          INFO = 4
      ELSE IF (M.LT.0) THEN
          INFO = 5
      ELSE IF (N.LT.0) THEN
          INFO = 6
      ELSE IF (LDA.LT.MAX(1,NROWA)) THEN
          INFO = 9
      ELSE IF (LDB.LT.MAX(1,M)) THEN
          INFO = 11
      END IF
      IF (INFO.NE.0) THEN
          CALL XERBLA('DTRSM ',INFO)
          RETURN
      END IF
!
!     Quick return if possible.
!
      IF (M.EQ.0 .OR. N.EQ.0) RETURN
!
!     And when  alpha.eq.zero.
!
      IF (ALPHA.EQ.ZERO) THEN
          DO J = 1,N
              DO I = 1,M
                  B(I,J) = ZERO
              END DO
          END DO
          RETURN
      END IF
!
!     Start the operations.
!
      IF (LSIDE) THEN
          IF (LSAME(TRANSA,'N')) THEN
!
!           Form  B := alpha*inv( A )*B.
!
              IF (UPPER) THEN
                  DO 60 J = 1,N
                      IF (ALPHA.NE.ONE) THEN
                          DO 30 I = 1,M
                              B(I,J) = ALPHA*B(I,J)
   30                     CONTINUE
                      END IF
                      DO 50 K = M,1,-1
                          IF (B(K,J).NE.ZERO) THEN
                              IF (NOUNIT) B(K,J) = B(K,J)/A(K,K)
                              DO 40 I = 1,K - 1
                                  B(I,J) = B(I,J) - B(K,J)*A(I,K)
   40                         CONTINUE
                          END IF
   50                 CONTINUE
   60             CONTINUE
              ELSE
                  DO 100 J = 1,N
                      IF (ALPHA.NE.ONE) THEN
                          DO 70 I = 1,M
                              B(I,J) = ALPHA*B(I,J)
   70                     CONTINUE
                      END IF
                      DO 90 K = 1,M
                          IF (B(K,J).NE.ZERO) THEN
                              IF (NOUNIT) B(K,J) = B(K,J)/A(K,K)
                              DO 80 I = K + 1,M
                                  B(I,J) = B(I,J) - B(K,J)*A(I,K)
   80                         CONTINUE
                          END IF
   90                 CONTINUE
  100             CONTINUE
              END IF
          ELSE
!
!           Form  B := alpha*inv( A**T )*B.
!
              IF (UPPER) THEN
                  DO 130 J = 1,N
                      DO 120 I = 1,M
                          TEMP = ALPHA*B(I,J)
                          DO 110 K = 1,I - 1
                              TEMP = TEMP - A(K,I)*B(K,J)
  110                     CONTINUE
                          IF (NOUNIT) TEMP = TEMP/A(I,I)
                          B(I,J) = TEMP
  120                 CONTINUE
  130             CONTINUE
              ELSE
                  DO 160 J = 1,N
                      DO 150 I = M,1,-1
                          TEMP = ALPHA*B(I,J)
                          DO 140 K = I + 1,M
                              TEMP = TEMP - A(K,I)*B(K,J)
  140                     CONTINUE
                          IF (NOUNIT) TEMP = TEMP/A(I,I)
                          B(I,J) = TEMP
  150                 CONTINUE
  160             CONTINUE
              END IF
          END IF
      ELSE
          IF (LSAME(TRANSA,'N')) THEN
!
!           Form  B := alpha*B*inv( A ).
!
              IF (UPPER) THEN
                  DO 210 J = 1,N
                      IF (ALPHA.NE.ONE) THEN
                          DO 170 I = 1,M
                              B(I,J) = ALPHA*B(I,J)
  170                     CONTINUE
                      END IF
                      DO 190 K = 1,J - 1
                          IF (A(K,J).NE.ZERO) THEN
                              DO 180 I = 1,M
                                  B(I,J) = B(I,J) - A(K,J)*B(I,K)
  180                         CONTINUE
                          END IF
  190                 CONTINUE
                      IF (NOUNIT) THEN
                          TEMP = ONE/A(J,J)
                          DO 200 I = 1,M
                              B(I,J) = TEMP*B(I,J)
  200                     CONTINUE
                      END IF
  210             CONTINUE
              ELSE
                  DO 260 J = N,1,-1
                      IF (ALPHA.NE.ONE) THEN
                          DO 220 I = 1,M
                              B(I,J) = ALPHA*B(I,J)
  220                     CONTINUE
                      END IF
                      DO 240 K = J + 1,N
                          IF (A(K,J).NE.ZERO) THEN
                              DO 230 I = 1,M
                                  B(I,J) = B(I,J) - A(K,J)*B(I,K)
  230                         CONTINUE
                          END IF
  240                 CONTINUE
                      IF (NOUNIT) THEN
                          TEMP = ONE/A(J,J)
                          DO 250 I = 1,M
                              B(I,J) = TEMP*B(I,J)
  250                     CONTINUE
                      END IF
  260             CONTINUE
              END IF
          ELSE
!
!           Form  B := alpha*B*inv( A**T ).
!
              IF (UPPER) THEN
                  DO 310 K = N,1,-1
                      IF (NOUNIT) THEN
                          TEMP = ONE/A(K,K)
                          DO 270 I = 1,M
                              B(I,K) = TEMP*B(I,K)
  270                     CONTINUE
                      END IF
                      DO 290 J = 1,K - 1
                          IF (A(J,K).NE.ZERO) THEN
                              TEMP = A(J,K)
                              DO 280 I = 1,M
                                  B(I,J) = B(I,J) - TEMP*B(I,K)
  280                         CONTINUE
                          END IF
  290                 CONTINUE
                      IF (ALPHA.NE.ONE) THEN
                          DO 300 I = 1,M
                              B(I,K) = ALPHA*B(I,K)
  300                     CONTINUE
                      END IF
  310             CONTINUE
              ELSE
                  DO 360 K = 1,N
                      IF (NOUNIT) THEN
                          TEMP = ONE/A(K,K)
                          DO 320 I = 1,M
                              B(I,K) = TEMP*B(I,K)
  320                     CONTINUE
                      END IF
                      DO 340 J = K + 1,N
                          IF (A(J,K).NE.ZERO) THEN
                              TEMP = A(J,K)
                              DO 330 I = 1,M
                                  B(I,J) = B(I,J) - TEMP*B(I,K)
  330                         CONTINUE
                          END IF
  340                 CONTINUE
                      IF (ALPHA.NE.ONE) THEN
                          DO 350 I = 1,M
                              B(I,K) = ALPHA*B(I,K)
  350                     CONTINUE
                      END IF
  360             CONTINUE
              END IF
          END IF
      END IF

      END SUBROUTINE DTRSM

