import os
import sys
import pathlib
import shutil
from typing import Optional, Union
import requests
import platform
import subprocess
import tempfile

os.environ["PATH"] = (
    os.environ["PATH"] + ":" + os.path.expanduser("~/.cargo/bin")
)

ADDON_DIR = str(pathlib.Path(__file__).parent.resolve())
REPO = "sgomezsal/typ2anki"
EXECUTABLE_NAME = "typ2anki"
if os.name == "nt":
    EXECUTABLE_NAME += ".exe"


def get_github_latest_tag() -> Optional[str]:
    url = f"https://api.github.com/repos/{REPO}/releases/latest"
    try:
        response = requests.get(url)
        response.raise_for_status()
        data = response.json()
        if "tag_name" not in data:
            return None
        data = str(data["tag_name"])
        if data.startswith("v"):
            data = data[1:]
        return data
    except (requests.RequestException, KeyError):
        return None


def github_release_filename(version: str) -> Optional[str]:
    platform_map = {
        "linux": "x86_64-unknown-linux-musl",
        "darwin": "x86_64-apple-darwin",
        "win32": "x86_64-pc-windows-gnu",
    }
    platform_key = sys.platform
    if platform_key not in platform_map:
        return None
    platform_str = platform_map[platform_key]
    if platform.machine() == "aarch64":
        platform_str = platform_str.replace("x86_64", "aarch64")

    filename = f"typ2anki-v{version}-{platform_str}"
    if os.name == "nt":
        filename += ".zip"
    else:
        filename += ".tar.gz"
    return filename


def download_from_github(filename: str, version: str) -> Optional[str]:
    try:
        url = (
            f"https://github.com/{REPO}/releases/download/v{version}/{filename}"
        )
        resp = requests.get(url, stream=True, timeout=30)
        resp.raise_for_status()

        user_files_dir = os.path.join(ADDON_DIR, "user_files")
        os.makedirs(user_files_dir, exist_ok=True)

        with tempfile.TemporaryDirectory() as td:
            archive_path = os.path.join(td, filename)
            with open(archive_path, "wb") as f:
                for chunk in resp.iter_content(8192):
                    if chunk:
                        f.write(chunk)

            extract_dir = os.path.join(td, "extract")
            os.makedirs(extract_dir, exist_ok=True)
            try:
                shutil.unpack_archive(archive_path, extract_dir)
            except (shutil.ReadError, ValueError):
                print("Failed to unpack the downloaded archive.")
                return None

            target_name_variants = {EXECUTABLE_NAME}
            if os.name == "nt":
                target_name_variants.add(EXECUTABLE_NAME + ".exe")

            for root, _, files in os.walk(extract_dir):
                for name in files:
                    if name in target_name_variants:
                        src = os.path.join(root, name)
                        dst = os.path.join(user_files_dir, EXECUTABLE_NAME)
                        shutil.copy2(src, dst)
                        if os.name != "nt":
                            os.chmod(dst, os.stat(dst).st_mode | 0o111)
                        return dst
            print("Executable not found in the extracted archive.")
        return None
    except requests.RequestException as e:
        print(f"Failed to download the file from GitHub. {e}")
        return None
    except Exception as e:
        print(f"An error occurred during download or extraction. {e}")
        return None


def get_executable_path() -> Optional[str]:
    possible_paths = [
        os.path.join(ADDON_DIR, "user_files", EXECUTABLE_NAME),
    ]
    for path in possible_paths:
        if os.path.isfile(path) and os.access(path, os.X_OK):
            return path
    if shutil.which(EXECUTABLE_NAME):
        return shutil.which(EXECUTABLE_NAME)
    return None


def detect_cargo_installed() -> bool:
    return shutil.which("cargo") is not None


def detect_cargo_binstall_installed() -> bool:
    return shutil.which("cargo-binstall") is not None


# Returns (is_version_matching, actual_version or None)
def test_version(
    executable_path: str, excepted_version: str
) -> tuple[bool, Optional[str]]:
    try:
        result = subprocess.run(
            [executable_path, "--version"], capture_output=True, text=True
        )
        output = result.stdout.strip()
        output = output.split(" ")[-1]
        if excepted_version in output:
            return (True, output)
        return (False, output)
    except Exception as e:
        print(f"Error testing version ({executable_path}): {e}")
        return (False, None)


# Returns optionaly the path to the executable, or (path, error) if download failed
def check_backend(
    download: bool,
) -> Union[Optional[str], tuple[Optional[str], str]]:
    executable_path = get_executable_path()
    latest_version = get_github_latest_tag()
    is_update = False

    if latest_version is None and executable_path is None:
        print(
            "Could not fetch the latest version from GitHub, and no executable found."
        )
        return (
            None,
            "Could not fetch latest executable version from and no executable found.",
        )
    elif latest_version is None:
        print(
            "Could not fetch the latest version from GitHub, but executable found."
        )
        return executable_path

    if executable_path:
        is_latest_version, actual_version = test_version(
            executable_path, latest_version
        )
        if not download and not is_latest_version:
            return (
                executable_path,
                f"Version mismatch (latest: {latest_version}, actual: {actual_version}).",
            )
        if is_latest_version:
            return executable_path
        else:
            print(
                f"Executable found, but version mismatch with latest (latest: {latest_version}, actual: {actual_version})."
            )
            if input("Update? (y/n): ").lower() != "y":
                return executable_path
            is_update = True
    elif not download:
        return (
            None,
            "typ2anki executable not found - you need to install it",
        )

    cargo_installed = detect_cargo_installed()
    cargo_binstall_installed = (
        cargo_installed and detect_cargo_binstall_installed()
    )
    github_file = github_release_filename(latest_version)
    if github_file is None and not cargo_installed:
        return (
            None,
            "Can't install typ2anki: unsupported platform and cargo not found for install from source.",
        )
    # 1 = GitHub download, 2 = cargo-binstall, 3 = cargo
    chosen_download_method = 0
    if is_update and executable_path:
        if ADDON_DIR in executable_path and github_file:
            chosen_download_method = 1
        elif cargo_binstall_installed:
            chosen_download_method = 2
        elif cargo_installed:
            chosen_download_method = 3
        else:
            return (
                executable_path,
                "No suitable method found to update typ2anki.",
            )
    else:
        options = []
        if github_file:
            options.append(
                "1. Download prebuilt binary from GitHub (recommended)"
            )
        if cargo_binstall_installed:
            options.append("2. Install via cargo-binstall")
        if cargo_installed:
            options.append("3. Install via cargo")
        if not options:
            return (
                None,
                "No suitable method found to download or install typ2anki.",
            )
        print("Choose a method to download/install typ2anki:")
        for option in options:
            print(option)
        while True:
            choice = input(f"Enter the number (1-3): ").strip()
            if choice.isdigit():
                choice_num = int(choice)
                if 1 <= choice_num <= 3:
                    chosen_download_method = choice_num
                    break
            print("Invalid choice. Please try again.")

    if chosen_download_method == 1:
        if not github_file:
            return (
                executable_path,
                "No suitable GitHub release for this platform.",
            )
        print("Updating from github...")
        new_executable_path = download_from_github(github_file, latest_version)
        print("Updated.")
        return new_executable_path
    elif chosen_download_method == 2:
        if not cargo_binstall_installed:
            return (executable_path, "cargo-binstall not found.")
        print("Updating via cargo-binstall...")
        print("Run: cargo binstall -y typ2anki")
        try:
            subprocess.run(
                [
                    "cargo",
                    "binstall",
                    "-y",
                    "typ2anki",
                ],
                check=True,
            )
            return shutil.which(EXECUTABLE_NAME)
        except subprocess.CalledProcessError as e:
            print(f"Cargo binstall update failed: {e}")
            return (executable_path, "Cargo binstall failed.")
    elif chosen_download_method == 3:
        if not cargo_installed:
            return (executable_path, "cargo not found.")
        print("Updating via cargo...")
        print("Run: cargo install --force typ2anki")
        try:
            subprocess.run(
                [
                    "cargo",
                    "install",
                    "--force",
                    "typ2anki",
                ],
                check=True,
            )
            return shutil.which(EXECUTABLE_NAME)
        except subprocess.CalledProcessError as e:
            print(f"Cargo install update failed: {e}")
            return (executable_path, "Cargo install failed.")
    else:
        return (
            executable_path,
            "No suitable method found to download or update typ2anki.",
        )
    return None


if __name__ == "__main__":
    result = check_backend(download=True)
    if isinstance(result, tuple):
        path, error = result
        if path:
            print(f"Executable path: {path}")
            if not error:
                print(
                    "Update successful - restart the typ2anki addon to use the new version."
                )
        if error:
            print(f"Error: {error}")
    elif result:
        print(f"Executable path : {result}")
        print(
            "Update successful - restart the typ2anki addon to use the new version."
        )
    else:
        print("typ2anki executable not found or could not be installed.")
    input("Press Enter to exit...")
