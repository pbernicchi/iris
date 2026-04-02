#!/bin/sh
# Build dktest for IRIX using MIPSPro (n32 ABI, MIPS3)
cc -n32 -mips3 -O2 -o dktest dktest.c
echo "Built dktest (n32/mips3)"
