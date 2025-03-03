import argparse
from dataclasses import dataclass
import json
from pathlib import Path
from typing import List
from fnmatch import fnmatch
import tempfile
import zipfile
import shutil

from typ2anki.api import hash_string

@dataclass
class Config:
    check_duplicates: bool
    exclude_decks: List[str]
    asked_path: str
    dry_run: bool = False
    max_card_width: str = "auto"

    check_checksums: bool = True
    # The real path to the Typst documents folder, is set in post init to support zip files
    path: str = None
    __is_zip: bool = False
    config_hash: str = None

    def __post_init__(self):
        self.__set_real_path()
        self.config_hash = hash_string(json.dumps({
            "exclude_decks": self.exclude_decks,
            "max_card_width": self.max_card_width
        }))

    def is_deck_excluded(self, deck_name: str) -> bool:
        return any(fnmatch(deck_name,excluded_deck) for excluded_deck in self.exclude_decks)

    def __set_real_path(self):
        path = Path(self.asked_path).resolve()
        if path.is_file() and path.suffix == ".zip":
            self.__is_zip = True
            tmpdirname = tempfile.TemporaryDirectory(
                delete=False
            ).name
            print(f"Extracting {path} to {tmpdirname}")
            with zipfile.ZipFile(path, 'r') as zip_ref:
                zip_ref.extractall(tmpdirname)
            self.path = tmpdirname
            return
        
        if not path.is_dir():
            raise ValueError(f"{path} is not a valid directory.")
        self.path = self.asked_path

    def destruct(self):
        if self.__is_zip and self.path:
            shutil.rmtree(self.path)
            print(f"Deleted temporary zip directory {self.path}")


def parse_config() -> Config:
    parser = argparse.ArgumentParser(
        description="""Typ2Anki is a tool that converts Typst documents into Anki flashcards. Source: https://github.com/sgomezsal/typ2anki""",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter
    )
    parser.add_argument(
        "--check-duplicates",  
        action="store_true",
        help="Enable duplicate checking"
    )
    parser.add_argument(
        "-e", "--exclude-decks", 
        action="append", 
        default=[],
        help="Specify decks to exclude. Use multiple -e options to exclude multiple decks. Glob patterns are supported."
    )
    parser.add_argument(
        "--max-card-width",
        type=str,
        default="auto",
        help="Specify the maximum width of the cards, in typst units. Use 'auto' to not limit the width."
    )
    parser.add_argument(
        "--no-cache",
        action="store_true",
        help="Force reupload of all images"
    )
    # parser.add_argument(
    #     "--anki-connect-url", "-u", 
    #     default="http://localhost:8765", 
    #     help="Specify the Anki Connect URL"
    # )

    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Run the script without making any changes, only showing what would be done"
    )

    parser.add_argument(
        "path", 
        default=".", 
        nargs="*", # Have to use * to allow for spaces in the path
        help="Specify the path to the Typst documents folder or a zip file containing Typst documents"
    )
    
    args = parser.parse_args()
    c = Config(
        check_duplicates=args.check_duplicates,
        exclude_decks=args.exclude_decks,
        asked_path=" ".join(args.path), # Join the path in case it contains spaces
        dry_run=args.dry_run,
        max_card_width=args.max_card_width
    )
    if args.no_cache:
        c.check_checksums = False

    return c

cached_config = None
def config() -> Config:
    global cached_config
    if cached_config is None:
        cached_config = parse_config()
    return cached_config