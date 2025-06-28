#!/usr/bin/env bash
ADDON_DIR=$(dirname "$(realpath "$0")")

cd "$ADDON_DIR"

. ~/.profile

command=("$(which python3)" "-m" "typ2anki_cli.main" "${@:1}")

echo "Running: command=${command[@]}"
echo "---"

("${command[@]}") || true
(notify-send "typ2anki" "Compilation complete" --app-name "typ2anki") || true

cd -

echo "---"
echo "Press any key to exit..."
(read -n 1 -s) || bash -i
