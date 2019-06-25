#!/bin/bash

# Assembles the minimal BIOS. Requires this suite of tools:
# https://github.com/rednex/rgbds

set -o errexit -o nounset
MY_PATH="`dirname \"$0\"`"

cd $MY_PATH

rgbasm -o minimal-bios.o minimal-bios.s \
    && rgblink -o ../data/minimal-bios.bin minimal-bios.o \
    && truncate -s 256 ../data/minimal-bios.bin \
    && rm minimal-bios.o
