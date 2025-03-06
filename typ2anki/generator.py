import os
from pathlib import Path
import subprocess
import sys

from typ2anki.config import config
from typ2anki.progressbar import ProgressBarManager
from typ2anki.utils import hash_string

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

# Returns if the card was generated successfully
def generate_card_file(card, card_id, output_path) -> bool:
    temp_file = Path(output_path) / "temporal.typ"
    output_file = Path(output_path) / f"typ-{card_id}-{{p}}.{config().output_type}"

    if config().dry_run:
        print(f"Generating card file for card {card_id} at {output_file}")
        return

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
        result = subprocess.run(
            ["typst", *(config().typst_global_flags), "c", *(config().typst_compile_flags), str(temp_file), str(output_file)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )
        if result.returncode != 0:
            msg = f"Error generating card {card_id}"
            if result.stdout:
                msg += f"\n{result.stdout.decode(errors='replace')}"
            if result.stderr:
                msg += f"\n{result.stderr.decode(errors='replace')}"
            ProgressBarManager.get_instance().log_message(msg)
        return result.returncode == 0
    finally:
        if os.path.exists(temp_file):
            os.remove(temp_file)
