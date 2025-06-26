import pathlib
import os
import sys
import subprocess

ADDON_DIR = str(pathlib.Path(__file__).parent.resolve())

os.chdir(ADDON_DIR)
process = subprocess.Popen(
    ["python", "-m", "typ2anki_cli.main"] + sys.argv[1:], cwd=ADDON_DIR
)
process.wait()
sys.exit(process.returncode)
