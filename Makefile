# Makefile for CLM surface data processing

include ./Makeoptions

# Suppress macOS deployment version warnings
export MACOSX_DEPLOYMENT_TARGET = 26.0


# \
# name of executable
EXECUTABLE = mkgrd.x

####################################################################
.DEFAULT :


OBJS = \
	consts_coms.o \
        MOD_file_preprocess.o\
      	icosahedron.o \
      	blas.o \
      	lapack.o\
        MOD_data_preprocess.o\
        MOD_Area_judge.o\
        MOD_grid_preprocess.o\
	MOD_GetContain.o\
      	MOD_GetRef.o\
	MOD_refine.o\
        MOD_mask_postproc.o\
	mkgrd.o\
#        MOD_Threshold_Read.o
#        MOD_Get_Distance.o
#        MOD_postprocess.o
#        MOD_refine_sjx.o
#        MOD_refine_lbx.o
#        MOD_refine_lbx_step2.o
#        MOD_Get_Contain_Patch.o
		
####################################################################

${EXECUTABLE} : ${OBJS}  
	${FF} ${FOPTS} ${OBJS} -o $@ ${LDFLAGS}
	@echo 'program for making unstructure mesh surface data have been compiled successfully !'

$(OBJS) : %.o : %.F90 
	${FF} -c ${FOPTS} $(INCLUDE_DIR) -o $@ $< 


clean :
	${RM} -f *.o *.mod ${EXECUTABLE}
