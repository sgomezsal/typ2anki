import argparse
from dataclasses import dataclass
import json
from pathlib import Path
from typing import List, Literal
from fnmatch import fnmatch
import tempfile
import zipfile
import shutil
import html

from typ2anki.utils import hash_string

@dataclass
class Config:
    # User controlled options
    check_duplicates: bool
    exclude_decks: List[str]
    asked_path: str
    dry_run: bool = False
    max_card_width: str = "auto"
    check_checksums: bool = True

    # Processed options
    path: str = None
    __is_zip: bool = False

    # Internal options
    config_hash: str = None
    output_type: Literal['png','svg','html'] = "png"
    typst_global_flags: List[str] = None
    typst_compile_flags: List[str] = None
    style_image_front: str = None
    style_image_back: str = None

    def __post_init__(self):
        self.__set_real_path()
        self.config_hash = hash_string(json.dumps({
            "output_type": self.output_type,
            "style_image_front": self.style_image_front,
            "style_image_back": self.style_image_back,
            "exclude_decks": sorted(self.exclude_decks),
            "max_card_width": self.max_card_width
        },sort_keys=True))
        self.typst_global_flags = ["--color","always"]
        self.typst_compile_flags = ["--root",self.path]

        if self.output_type == "html":
            self.typst_compile_flags += ["--features","html"]

    def is_deck_excluded(self, deck_name: str) -> bool:
        return any(fnmatch(deck_name,excluded_deck) for excluded_deck in self.exclude_decks)

    def __set_real_path(self):
        path = Path(self.asked_path).resolve()
        if path.is_file() and path.suffix == ".zip":
            self.__is_zip = True
            tmpdirname = tempfile.mkdtemp()
            print(f"Extracting {path} to {tmpdirname}")
            with zipfile.ZipFile(path, 'r') as zip_ref:
                zip_ref.extractall(tmpdirname)
            self.path = tmpdirname
            return
        
        if not path.is_dir():
            raise ValueError(f"{path} is not a valid directory.")
        self.path = self.asked_path

    def path_relative_to_root(self,p: Path) -> Path:
        return p.relative_to(self.path)

    def get_card_side_html(self, path: str, loc: Literal['front','back']) -> str:
        attrs = {}
        if loc == "front" and self.style_image_front:
            attrs["style"] = self.style_image_front
        elif loc == "back" and self.style_image_back:
            attrs["style"] = self.style_image_back
        
        attr_str = " ".join(f'{key}="{html.escape(value,quote=True)}"' for key, value in attrs.items())
        return f'<img src="{html.escape(path,quote=True)}"{f" {attr_str}" if attr_str else ""}>'

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