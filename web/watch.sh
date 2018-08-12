#!/bin/bash

hash watchexec || (echo "installing watchexec..." && cargo install watchexec)

watchexec \
    -w ".." \
    -i "dist/*" \
    -i "src/*.d.ts" \
    -i "target/*" \
    "./build-all.rs"
