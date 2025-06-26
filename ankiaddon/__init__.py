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


def resolve_key_source(i: int) -> str:
    if i == ConfigKeySource.DEFAULT.value:
        return "default"
    elif i == ConfigKeySource.CLI.value:
        return "cli"
    elif i == ConfigKeySource.CONFIG_FILE.value:
        return "config_file"
    else:
        raise ValueError(f"Unknown key source: {i}")


class Option(TypedDict):
    id: str
    source: int
    cli_name: str
    help: str
    type: str
    value: Union[str, int, bool, List[str]]


class Config(TypedDict):
    options: List[Option]
    options_clis: dict[str, str]


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
            try:
                config: Config = json.loads(r.stderr)
                config["options_clis"] = {}
                for option in config["options"]:
                    if option["cli_name"]:
                        config["options_clis"][option["id"]] = option[
                            "cli_name"
                        ]
                # showInfo(
                #     str(
                #         convert_to_user_cli_command(
                #             get_typ2anki_command(["--dry-run", file_path])
                #         )
                #     )
                # )
                show_config_dialog(config, file_path)
            except json.JSONDecodeError as e:
                showInfo(f"Failed to decode JSON: {e}\nRaw output: {r.stderr}")
        else:
            showInfo(f"Error loading configuration: {r.stdout}\n{r.stderr}")
    else:
        showInfo("No configuration found or an error occurred.")


def show_config_dialog(config: "Config", file_path: str):
    dialog = qt.QDialog(mw)
    dialog.setWindowTitle("typ2anki: Configuration Overrides")
    layout = qt.QFormLayout(dialog)  # Use QFormLayout for key-value pairs

    # Add a header row for the override checkbox
    header_layout = qt.QHBoxLayout()
    header_label = qt.QLabel("Override setting")
    header_layout.addWidget(header_label)
    header_layout.addStretch(1)
    layout.addRow("", header_layout)  # Empty label for the header row

    widgets = {}  # Store widgets for each option

    for option in config["options"]:
        option_id = option["id"]
        option_type = option["type"]
        option_value = option["value"]
        option_source = option["source"]

        # Create a checkbox for enabling/disabling override
        override_checkbox = qt.QCheckBox()
        override_checkbox.setChecked(False)  # Initially unchecked

        # Create an appropriate widget based on the option type
        if option_type == "str":
            input_widget = qt.QLineEdit(str(option_value))
        elif option_type == "int":
            input_widget = qt.QSpinBox()
            input_widget.setValue(int(option_value))  # type: ignore
        elif option_type == "store_true":
            input_widget = qt.QCheckBox()
            input_widget.setChecked(bool(option_value))
        elif option_type == "append":
            input_widget = qt.QLineEdit(
                ",".join(option_value)
                if isinstance(option_value, list)
                else str(option_value)
            )  # Join list with commas for display
        else:
            input_widget = qt.QLineEdit(
                str(option_value)
            )  # Default to text input

        input_widget.setEnabled(False)  # Initially disabled

        # Function to enable/disable the input widget based on checkbox state
        def toggle_widget(checkbox, widget):
            widget.setEnabled(checkbox.isChecked())

        override_checkbox.stateChanged.connect(
            lambda state, w=input_widget, c=override_checkbox: toggle_widget(
                c, w
            )
        )

        # Add widgets to the layout with the option ID as the label
        label = f"{option_id} ({resolve_key_source(option_source)})"
        hbox = qt.QHBoxLayout()
        hbox.addWidget(override_checkbox)
        hbox.addWidget(input_widget)

        layout.addRow(label, hbox)

        widgets[option_id] = {
            "checkbox": override_checkbox,
            "widget": input_widget,
            "type": option_type,
        }

    def on_ok():
        overrides = {}
        for option_id, widget_data in widgets.items():
            checkbox = widget_data["checkbox"]
            widget = widget_data["widget"]
            option_type = widget_data["type"]

            if checkbox.isChecked():
                if option_type == "str":
                    overrides[option_id] = widget.text()
                elif option_type == "int":
                    overrides[option_id] = widget.value()
                elif option_type == "bool":
                    overrides[option_id] = widget.isChecked()
                elif option_type == "list[str]":
                    overrides[option_id] = (
                        widget.text().split(",")
                        if isinstance(widget, qt.QLineEdit)
                        else widget.text()
                    )  # Split comma-separated string into list
                else:
                    overrides[option_id] = widget.text()

        params = ["--dry-run"]
        for option_id, value in overrides.items():
            params.append(f"{config['options_clis'].get(option_id)}")
            params.append(str(value))

        params.append(file_path)

        showInfo(str(convert_to_user_cli_command(get_typ2anki_command(params))))
        dialog.close()

    ok_button = qt.QPushButton("OK")
    ok_button.clicked.connect(on_ok)
    layout.addRow(ok_button)

    dialog.setLayout(layout)
    dialog.show()


action = qt.QAction("typ2anki", mw)
qt.qconnect(action.triggered, openFileChoser)
mw.form.menuTools.addAction(action)
