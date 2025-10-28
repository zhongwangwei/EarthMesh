#!/bin/bash
#
# Script to switch between Intel and GNU Fortran compilers
# Usage: ./switch_compiler.sh [intel|gnu]
#

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 [intel|gnu]"
    echo ""
    echo "Current compiler configuration:"
    if [ -f Makeoptions ]; then
        if grep -q "FF = ifort" Makeoptions; then
            echo "  Intel Fortran (ifort)"
        elif grep -q "FF = gfortran" Makeoptions; then
            echo "  GNU Fortran (gfortran)"
        else
            echo "  Unknown"
        fi
    else
        echo "  No Makeoptions file found"
    fi
    exit 1
fi

COMPILER=$1

case $COMPILER in
    intel)
        echo "Switching to Intel Fortran compiler..."
        if [ -f Makeoptions.intel.bak ]; then
            cp Makeoptions.intel.bak Makeoptions
            echo "Success! Using Intel Fortran (ifort)"
        elif [ -f Makeoptions ] && grep -q "FF = ifort" Makeoptions; then
            echo "Already using Intel Fortran (ifort)"
        else
            echo "Error: Intel configuration not found"
            echo "Please ensure Makeoptions.intel.bak exists"
            exit 1
        fi
        ;;
    gnu)
        echo "Switching to GNU Fortran compiler..."
        if [ -f Makeoptions.gnu ]; then
            # Backup current Makeoptions if it's Intel
            if [ -f Makeoptions ] && grep -q "FF = ifort" Makeoptions; then
                cp Makeoptions Makeoptions.intel.bak
                echo "Backed up Intel configuration to Makeoptions.intel.bak"
            fi
            cp Makeoptions.gnu Makeoptions
            echo "Success! Using GNU Fortran (gfortran)"
        else
            echo "Error: Makeoptions.gnu not found"
            exit 1
        fi
        ;;
    *)
        echo "Error: Unknown compiler '$COMPILER'"
        echo "Usage: $0 [intel|gnu]"
        exit 1
        ;;
esac

echo ""
echo "Don't forget to run 'make clean' before recompiling!"
