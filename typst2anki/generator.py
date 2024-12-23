import os
from pathlib import Path

def generate_card_file(card, card_id, output_path):
    temp_file = "temporal.typ"
    output_file = Path(output_path) / f"{card_id}-{{p}}.png"

    template = f"""
#import "ankiconf.typ": *
#show: doc => conf(doc)

#let card(
  id,
  target_deck: "Default",
  front,
  back,
) = {{
  set page(
    width: auto,
    height: auto,
    margin: 3pt,
    fill: rgb("#00000000"),
  )
  context[
    #front \\
    #pagebreak()
    #back
  ]
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
