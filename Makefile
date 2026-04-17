# Makefile for EarthMesh

include ./Makeoptions
 
# Suppress macOS deployment version warnings
export MACOSX_DEPLOYMENT_TARGET = 26.0

# Source directory
SRCDIR = src

# Executable name
EXECUTABLE = mkgrd.x

####################################################################
.DEFAULT :

# Source files
SRCS = \
    $(SRCDIR)/consts_coms.F90 \
    $(SRCDIR)/MOD_utilities.F90 \
    $(SRCDIR)/icosahedron.F90 \
    $(SRCDIR)/MOD_grid_initialization.F90 \
    $(SRCDIR)/MOD_mask_process.F90 \
    $(SRCDIR)/MOD_namelist.F90 \
    $(SRCDIR)/MOD_data_preprocess.F90 \
    $(SRCDIR)/MOD_Area_judge.F90 \
    $(SRCDIR)/MOD_grid_preprocess.F90 \
    $(SRCDIR)/MOD_GetContain.F90 \
    $(SRCDIR)/MOD_GetRef.F90 \
    $(SRCDIR)/MOD_refine.F90 \
    $(SRCDIR)/MOD_mask_postproc.F90 \
    $(SRCDIR)/mkgrd.F90

# Object files (in current directory)
OBJS = $(notdir $(SRCS:.F90=.o))

####################################################################

all: $(EXECUTABLE)

$(EXECUTABLE): $(OBJS)
	${FF} ${FOPTS} ${OBJS} -o $@ ${LDFLAGS}
	@echo 'EarthMesh has been compiled successfully!'
	@echo 'Executable: $(EXECUTABLE)'
	mv *.o *.mod bld/

# Pattern rule for compilation
%.o: $(SRCDIR)/%.F90
	${FF} -c ${FOPTS} $(INCLUDE_DIR) -o $@ $<

clean:
	rm -rf bld
	rm -f ${EXECUTABLE} *.txt
	@echo 'Clean complete!'

.PHONY: all clean
