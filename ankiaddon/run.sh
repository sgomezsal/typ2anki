#!/usr/bin/env bash
ADDON_DIR=$(dirname "$(realpath "$0")")

cd "$ADDON_DIR" >/dev/null 2>&1

if [ -f ~/.profile ]; then
    . ~/.profile
fi

best_python=""
best_version=""
pythons=("python3" "python" "python3.10" "python3.11" "python3.12" "python3.13" "/usr/bin/python3" "/usr/bin/python" "/usr/local/bin/python3" "/usr/local/bin/python")

for python in "${pythons[@]}"; do
    if [[ ! "$python" =~ ^/ ]]; then
        python_path=$(which "$python" 2>/dev/null) 2>/dev/null
        if [[ -z "$python_path" ]]; then
            continue
        fi
    else
        python_path="$python"
    fi

    if [[ -x "$python_path" ]]; then
        version=$("$python_path" -c "import sys; print('.'.join(map(str, sys.version_info[:3])))" 2>/dev/null)
        if [[ -n "$version" ]]; then
            if [[ -z "$best_version" || "$version" > "$best_version" ]]; then
                best_version="$version"
                best_python="$python_path"
            fi
        fi
    fi
done

if [[ -n "$best_python" ]]; then
    # echo "Using python: $best_python (version: $best_version)"
    :
else
    echo "No suitable python version found."
    exit 1
fi
 
if [[ "${1}" == "--back" ]]; then
    shift
    command=("$best_python" "-m" "typ2anki_cli.main" "${@}")
    ("${command[@]}")
elif  [[ "${1}" == "-i" ]]; then
    shift

    command=("$best_python" "-m" "typ2anki_cli.main" "${@}")
    echo "Running: command=${command[@]}"
    echo "---"

    ("${command[@]}") || true
    (notify-send "typ2anki" "Compilation complete" --app-name "typ2anki") || true
else
    command=("$best_python" "-m" "typ2anki_cli.main" "${@}")

    echo "Running: command=${command[@]}"
    echo "---"

    ("${command[@]}") || true
    (notify-send "typ2anki" "Compilation complete" --app-name "typ2anki") || true

    echo "---"
    echo "Press any key to exit..."
    (read -n 1 -s) || $(getent passwd "$USER" | cut -d: -f7) || bash -i
fi
