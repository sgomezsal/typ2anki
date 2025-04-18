from dataclasses import dataclass
import os
from pathlib import Path
import subprocess
import sys
from typing import List

from typ2anki.config import config
from typ2anki.progressbar import ProgressBarManager
from typ2anki.utils import PassedCardDataForCompilation, hash_string


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


def get_ankiconf_hash(directory):
    ankiconf_path = Path(directory) / "ankiconf.typ"
    if not ankiconf_path.exists():
        raise Exception(f"Ankiconf file not found at {ankiconf_path}")
    return hash_string(ankiconf_path.read_text())


@dataclass
class GenerateCardProcess:
    temp_file: Path
    card_id: str
    parameters: List[str]
    deck_name: str = None
    real_deck_name: str = None
    process: subprocess.Popen = None

    def start(self):
        self.process = subprocess.Popen(
            self.parameters, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )

    def collect(self):
        stdout, stderr = self.process.communicate()
        return (
            self.process.returncode,
            stdout.decode(errors="replace").strip(),
            stderr.decode(errors="replace").strip(),
        )

    def collect_integrated(self) -> bool:
        returncode, stdout, stderr = self.collect()
        if returncode != 0:
            msg = f"Error generating card {self.card_id}"
            if stdout:
                msg += f"\n{stdout}"
            if stderr:
                msg += f"\n{stderr}"
            ProgressBarManager.get_instance().log_message(msg)
        return returncode == 0

    def run_integrated(self):
        self.start()
        return self.collect_integrated()

    def clean(self):
        if os.path.exists(self.temp_file):
            os.remove(self.temp_file)


# Returns if the card was generated successfully
def generate_card_file(
    card, card_info: PassedCardDataForCompilation, output_path
) -> GenerateCardProcess | None:
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

    card_type = "custom-card" if "custom-card" in card else "card"

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
{card}
"""

    try:
        with open(temp_file, "w") as file:
            file.write(template)
        ppi = config().card_ppi(card_info)
        if ppi > 0:
            ppi = ["--ppi", str(ppi)]
        else:
            ppi = []
        return GenerateCardProcess(
            card_id=card_id,
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
    except Exception:
        if os.path.exists(temp_file):
            os.remove(temp_file)
        return None
