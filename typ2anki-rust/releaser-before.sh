#!/usr/bin/env bash

cd ..
./bundle-ankiaddon.sh || touch ./typ2anki.ankiaddon
cd -
mv ../typ2anki.ankiaddon ./
