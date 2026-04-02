#!/bin/bash

echo "Compiling basic OpenGL 1.0 test..."
cc -o gltest main.c -mips3 -n32 -lX11 -lGL -lm
echo "Done. You can run the test using: ./gltest"