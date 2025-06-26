from dataclasses import dataclass
import os
import re
from pathlib import Path
import subprocess
import sys
from typing import List, Literal

from .card_wrapper import CardInfo, CardGenerationProcess
from .config import config
from .progressbar import ProgressBarManager
from .utils import hash_string


def ensure_ankiconf_file(directory):
    ankiconf_path = Path(directory) / "ankiconf.typ"
    if not ankiconf_path.exists():
        default_content = """
#let conf(
  doc,
) = {
  doc
}
"""
        if config().dry_run:
            print(f"Creating ankiconf file at {ankiconf_path}")
            return

        with open(ankiconf_path, "w") as file:
            file.write(default_content)


def get_all_imports(typ_content: str) -> List[str]:
    pattern = r'^#import\s*"([^"]+)"\s*'

    r: List[str] = []
    imports = re.findall(pattern, typ_content, re.MULTILINE) or []
    for import_path in imports:
        if os.path.isabs(import_path):
            import_path = os.path.relpath(import_path, "/")
        import_path = os.path.join(config().path, import_path)
        if os.path.exists(import_path):
            r.append(import_path)
            with open(import_path, "r") as file:
                imported_content = file.read()
            imports += get_all_imports(imported_content)

    return sorted(set(r))


def get_ankiconf_hash(directory, filename="ankiconf.typ"):
    ankiconf_path = Path(directory) / filename
    if not ankiconf_path.exists():
        raise Exception(f"Ankiconf file not found at {ankiconf_path}")
    ankiconf = ankiconf_path.read_text()
    try:
        imports = get_all_imports(ankiconf)

        if config().dry_run:
            print(f"Reading imports in ankiconf: {imports}")

        for import_path in imports:
            assert os.path.exists(
                import_path
            ), f"Import path {import_path} does not exist: This shouldn't happen"
            ankiconf += "\n" + Path(import_path).read_text()
    except Exception as e:
        print(f"Error reading imports in ankiconf: {e}")

    return hash_string(ankiconf)


# Returns if the card was generated successfully
def generate_compilation_task(
    card_info: CardInfo, output_path
) -> Literal[True] | None:
    card_id = card_info.card_id
    temp_file = Path(output_path) / f"temporal-{card_id}.typ"
    output_file = (
        Path(output_path) / f"typ-{card_id}-{{p}}.{config().output_type}"
    )

    if config().dry_run:
        print(f"Generating card file for card {card_id} at {output_file}")
        return None

    ankiconf_relative_path = os.path.relpath(
        Path(config().path) / "ankiconf.typ",
        output_path,
    )

    card_type = (
        "custom-card" if "custom-card" in card_info.card_content else "card"
    )

    max_width = config().max_card_width
    display_with_width = ""

    if max_width == "auto":
        display_with_width = """
#let display_with_width(body) = {
  body
}
"""
    else:
        display_with_width = f"""
#let display_with_width(body) = {{
  layout(size => {{
    let (width,) = measure(body)
    if width > {max_width} {{
      width = {max_width}
    }} else {{
      width = auto
    }}
    context[
      #block(width: width,body)
    ]
  }})
}}
"""
    page_configuration = """
#set page(
  width: auto,
  height: auto,
  margin: 3pt,
  fill: rgb("#00000000"),
)"""
    if config().output_type == "html":
        page_configuration = ""

    template = f"""
#import "{ankiconf_relative_path}": *
#show: doc => conf(doc)

{page_configuration}

{display_with_width}

#let {card_type}(
  id: "",
  q: "",
  a: "",
  ..args
) = {{
  let args = arguments(..args, type: "basic")
  if args.at("type") == "basic" {{
    context[
      #display_with_width(q) \\
      #pagebreak()
      #display_with_width(a)
    ]
  }}
}}
{card_info.card_content}
"""

    try:
        with open(temp_file, "w") as file:
            file.write(template)
        ppi = config().card_ppi(card_info)
        if ppi > 0:
            ppi = ["--ppi", str(ppi)]
        else:
            ppi = []

        card_info.generation_process = CardGenerationProcess(
            card=card_info,
            parameters=[
                "typst",
                *(config().typst_global_flags),
                "c",
                *ppi,
                *(config().typst_compile_flags),
                str(temp_file),
                str(output_file),
            ],
            temp_file=temp_file,
        )
        return True
    except Exception:
        if os.path.exists(temp_file):
            os.remove(temp_file)
        return None
