#!/usr/bin/env bash

p=$(pwd)
cd ..
./bundle-ankiaddon.sh || touch ./typ2anki.ankiaddon
cd "$p"
mv ../typ2anki.ankiaddon ./
