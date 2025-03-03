import os
from pathlib import Path

from typ2anki.config import config

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

def generate_card_file(card, card_id, output_path):
    temp_file = Path(output_path) / "temporal.typ"
    output_file = Path(output_path) / f"{card_id}-{{p}}.png"

    if config().dry_run:
        print(f"Generating card file for card {card_id} at {output_file}")
        return

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
        

    template = f"""
#import "ankiconf.typ": *
#show: doc => conf(doc)

#set page(
  width: auto,
  height: auto,
  margin: 3pt,
  fill: rgb("#00000000"),
)

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
        os.system(f"typst c {temp_file} {output_file}")
    finally:
        if os.path.exists(temp_file):
            os.remove(temp_file)

    return output_file
