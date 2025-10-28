#!/bin/bash
#SBATCH -p first
ulimit -s unlimited
source ~/.bashrc_CoLM202X_intel
#make clean
make &> logmake
wait
echo "make finish"
