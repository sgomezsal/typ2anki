#!/usr/bin/env bash

# Upload the resulting file to https://ankiweb.net/shared/info/1677591717

rm -rf typ2anki.ankiaddon.temp
rm -f typ2anki.ankiaddon

ADDON_DIR=$(dirname "$(realpath "$0")")/ankiaddon
mkdir typ2anki.ankiaddon.temp

cp "$ADDON_DIR"/*.py typ2anki.ankiaddon.temp

cd typ2anki.ankiaddon.temp

#echo '{"package": "typ2anki","name": "typ2anki","mod": $(date +%s)}` >manifest.json
echo "{\"package\": \"typ2anki\", \"name\": \"typ2anki\", \"mod\": $(date +%s)}" >manifest.json

zip -r ../typ2anki.ankiaddon *

rm -rf ../typ2anki.ankiaddon.temp
