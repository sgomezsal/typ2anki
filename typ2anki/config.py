import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import sys
from typing import Any, Dict, List, Literal, Optional
from fnmatch import fnmatch
import tempfile
import zipfile
import shutil
import html
try:
    import tomllib  # Python 3.11+
except ImportError:
    try:
        import tomli as tomllib  # Python <3.11
    except ImportError:
        print("TOML support requires 'tomli' package. Install with: pip install tomli")
from typ2anki.utils import hash_string
import pprint

DEFAULT_CONFIG_FILENAME = "typ2anki.toml"
def load_toml_config(config_path: Path) -> Dict[str, Any] | None:
    # Check if file exists
    if not config_path.exists():
        return None
    try:
        with open(config_path, "rb") as f:
            return tomllib.load(f)
    except Exception as e:
        raise Exception(f"Error loading config file {config_path}: {e}")

def get_real_path(asked_path) -> str:
    path = Path(asked_path).resolve()
    if path.is_file() and path.suffix == ".zip":
        tmpdirname = tempfile.mkdtemp()
        print(f"Extracting {path} to {tmpdirname}")
        with zipfile.ZipFile(path, 'r') as zip_ref:
            zip_ref.extractall(tmpdirname)
        return tmpdirname
    
    if not path.is_dir():
        raise ValueError(f"{path} is not a valid directory.")
    return path

@dataclass
class Config:
    # User controlled options
    check_duplicates: bool
    exclude_decks: List[str]
    exclude_files: List[str]
    asked_path: str
    dry_run: bool = False
    max_card_width: str = "auto"
    check_checksums: bool = True
    generation_concurrency: int = 1

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
        p = Path(self.asked_path).resolve()
        self.__is_zip = p.is_file() and p.suffix == ".zip"
        
        self.config_hash = hash_string(json.dumps({
            "output_type": self.output_type,
            "style_image_front": self.style_image_front,
            "style_image_back": self.style_image_back,
            "exclude_decks": sorted(self.exclude_decks),
            "max_card_width": self.max_card_width
        },sort_keys=True))
        
        self.typst_global_flags = ["--color","always"]
        self.typst_compile_flags = ["--root",self.path]

        if not self.check_duplicates and self.generation_concurrency > 1:
            print("WARNING: Concurrent generation can't be enabled without duplicate checking. Disabling concurrent generation.")
            self.generation_concurrency = 1  

        if self.output_type == "html":
            self.typst_compile_flags += ["--features","html"]
        
        if self.max_card_width != "auto":
            self.typst_compile_flags += ["--input",f"max_card_width={self.max_card_width}"]

        self.typst_compile_flags += ["--input","typ2anki_compile=1"]

    def is_deck_excluded(self, deck_name: str) -> bool:
        return any(fnmatch(deck_name,excluded_deck) for excluded_deck in self.exclude_decks)

    def is_file_excluded(self, file_name: str) -> bool:
        return any(fnmatch(file_name,excluded_file) for excluded_file in self.exclude_files)

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
    # Track which arguments were explicitly set
    explicitly_set = set()
    
    class TrackingAction(argparse.Action):
        def __call__(self, parser, namespace, values, option_string=None):
            explicitly_set.add(self.dest)
            setattr(namespace, self.dest, values)
    
    class HelpArgumentFormatter(argparse.ArgumentDefaultsHelpFormatter):
        def _get_help_string(self, action):
            config_key = ""
            r = super()._get_help_string(action)
            if not action.dest or action.dest in ["config_file","help"]: return r
            for a in action.option_strings:
                if a.startswith("--"):
                    config_key = action.dest
                    break

            if config_key != "": r += f" -- config key: {config_key}"
            return r

    parser = argparse.ArgumentParser(
        description="""Typ2Anki is a tool that converts Typst documents into Anki flashcards. Source: https://github.com/sgomezsal/typ2anki""",
        formatter_class=HelpArgumentFormatter
    )
    parser.add_argument(
        "--config-file",
        help=f"Specify the path to the config file. All paths are relative to the directory specified as the path argument of this command. If not specified, the default config file is used, which is found at '{DEFAULT_CONFIG_FILENAME}' in the path. Set to empty string to disable config file.",
        default=DEFAULT_CONFIG_FILENAME
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
        "--exclude-files", 
        action="append", 
        default=[],
        help="Specify files to exclude. Use multiple --exclude-files options to exclude multiple files. Glob patterns are supported. Paths are relative to the path argument."
    )

    parser.add_argument(
        "--generation-concurrency", 
        action=TrackingAction,
        type=int, 
        default=1,
        help="Specify how many cards at a time can be generated. Needs duplicate checking enabled."
    )

    parser.add_argument(
        "--max-card-width",
        action=TrackingAction,
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
    if args.check_duplicates:
        explicitly_set.add('check_duplicates')
    if args.no_cache:
        explicitly_set.add('no_cache')
    if args.dry_run:
        explicitly_set.add('dry_run')
    if args.exclude_decks:
        explicitly_set.add('exclude_decks')
    if args.exclude_files:
        explicitly_set.add('exclude_files')

    c = {
        "check_duplicates": args.check_duplicates,
        "exclude_decks": args.exclude_decks,
        "exclude_files": args.exclude_files,
        "asked_path": " ".join(args.path), # Join the path in case it contains spaces
        "dry_run": args.dry_run,
        "max_card_width": args.max_card_width,
        "check_checksums": not args.no_cache,
        "generation_concurrency": args.generation_concurrency
    }

    real_path = get_real_path(c["asked_path"])
    c["path"] = real_path
    # Load config 
    if args.config_file != "":        
        config_file = args.config_file
        config_file_path = (Path(real_path) / config_file).resolve()
        config_file_data = load_toml_config(config_file_path)
        if config_file_data is None and config_file != DEFAULT_CONFIG_FILENAME:
            raise FileNotFoundError(f"Config file {config_file_path} not found.")
        print(f"Using config from {config_file_path}")
        
        if config_file_data is not None:
            for k,v in config_file_data.items():
                if k not in explicitly_set and k in c:
                    c[k] = v

    c = Config(**c)
    if c.dry_run:
        print("Using config:")
        pprint.pprint(vars(c), indent=2, width=100)
    return c

cached_config = None
def config() -> Config:
    global cached_config
    if cached_config is None:
        cached_config = parse_config()
    return cached_config
