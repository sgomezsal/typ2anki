from enum import Enum
import sys
from aqt import mw
from aqt.utils import showInfo
import subprocess
import os
import pathlib
import json
from typing import TypedDict, List, Union
import shlex


import aqt.qt as qt

ADDON_DIR = str(pathlib.Path(__file__).parent.resolve())


class ConfigKeySource(Enum):
    DEFAULT = 0
    CLI = 1
    CONFIG_FILE = 2


def get_typ2anki_command(params: list[str]) -> list[str]:
    command = [sys.executable, "-m", "typ2anki_cli.main"] + params
    return command


def convert_to_user_cli_command(params: list[str]) -> str:
    command = (
        ["cd", shlex.quote(ADDON_DIR), "&&"]
        + [shlex.quote(arg) for arg in params]
        + ["&&", "cd", "-"]
    )
    return " ".join(command)


def call_typ2anki_cli(
    params: list[str], showUser: bool
) -> None | subprocess.CompletedProcess:
    command = get_typ2anki_command(params)
    if showUser:
        subprocess.run(command, shell=True, cwd=ADDON_DIR)
    else:
        result = subprocess.run(
            command, capture_output=True, text=True, cwd=ADDON_DIR
        )
        return result


def openFileChoser() -> None:
    onChosenFile("/home/gm/Downloads/anki-sup (89).zip")
    return
    dialog = qt.QDialog(mw)
    mw.objFileDialog = dialog  # type: ignore
    dialog.setWindowTitle("typ2anki: Choose File or Folder")

    layout = qt.QVBoxLayout(dialog)

    file_button = qt.QPushButton("Choose File")
    folder_button = qt.QPushButton("Choose Folder")

    def choose_file():
        file_path, _ = qt.QFileDialog.getOpenFileName(
            dialog, "Select a file", "", "Zip Files (*.zip)"
        )
        if file_path:
            onChosenFile(file_path)

    def choose_folder():
        folder_path = qt.QFileDialog.getExistingDirectory(
            dialog, "Select a folder"
        )
        if folder_path:
            onChosenFile(folder_path)

    file_button.clicked.connect(choose_file)
    folder_button.clicked.connect(choose_folder)

    layout.addWidget(file_button)
    layout.addWidget(folder_button)
    dialog.setLayout(layout)
    dialog.show()


def onChosenFile(file_path: str) -> None:
    try:
        mw.objFileDialog.close()  # type: ignore
    except Exception:
        pass
    # showInfo(f"You selected: {file_path}")
    r = call_typ2anki_cli(["--print-config", file_path], False)
    if r:
        if r.returncode == 0:

            class Option(TypedDict):
                id: str
                source: int
                cli_name: str
                help: str
                type: str
                value: Union[str, int, bool, List[str]]

            class Config(TypedDict):
                options: List[Option]

            try:
                config: Config = json.loads(r.stderr)
                showInfo(
                    str(
                        convert_to_user_cli_command(
                            get_typ2anki_command(["--dry-run", file_path])
                        )
                    )
                )
            except json.JSONDecodeError as e:
                showInfo(f"Failed to decode JSON: {e}\nRaw output: {r.stderr}")
        else:
            showInfo(f"Error loading configuration: {r.stdout}\n{r.stderr}")
    else:
        showInfo("No configuration found or an error occurred.")


action = qt.QAction("typ2anki", mw)
qt.qconnect(action.triggered, openFileChoser)
mw.form.menuTools.addAction(action)
