#!/bin/bash

echo "Compiling basic OpenGL 1.0 test..."
gcc -o gltest main.c -lX11 -lGL -lm
echo "Done. You can run the test using: ./gltest"