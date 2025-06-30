#!/usr/bin/env bash
ADDON_DIR=$(dirname "$(realpath "$0")")

cd "$ADDON_DIR" >/dev/null 2>&1

. ~/.profile

command=("$(which python3)" "-m" "typ2anki_cli.main" "${@:1}")

echo "Running: command=${command[@]}"
echo "---"

("${command[@]}") || true
(notify-send "typ2anki" "Compilation complete" --app-name "typ2anki") || true

cd - >/dev/null 2>&1

echo "---"
echo "Press any key to exit..."
(read -n 1 -s) || $(getent passwd "$USER" | cut -d: -f7) || bash -i
