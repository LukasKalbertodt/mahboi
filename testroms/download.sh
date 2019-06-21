#!/bin/bash

# Since the license situation regarding these ROMs is not clear, they are not
# stored in this repository. Instead, they are downloaded with this script.

set -o errexit -o nounset
MY_PATH="`dirname \"$0\"`"

cd $MY_PATH


declare -a files=(
    "cgb_sound.zip"
    "cpu_instrs.zip"
    "dmg_sound.zip"
    "halt_bug.zip"
    "instr_timing.zip"
    "interrupt_time.zip"
    "mem_timing-2.zip"
    "mem_timing.zip"
    "oam_bug.zip"
)

for file in "${files[@]}"
do
   echo "$file"
   wget -nv "http://gbdev.gg8.se/files/roms/blargg-gb-tests/$file"
   unzip -qo $file
   rm $file
done
