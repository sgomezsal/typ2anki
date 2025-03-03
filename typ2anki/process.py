from pathlib import Path

from typ2anki.config import config
from .api import upload_media, create_deck, add_or_update_card

def process_create_deck(deck_name):
    if config().dry_run:
        print(f"Creating deck {deck_name}")
        return
    create_deck(deck_name)

def process_image(deck_name,card_id,card,output_path):
    front_image = Path(output_path) / f"{card_id}-1.png"
    back_image = Path(output_path) / f"{card_id}-2.png"
    
    if config().dry_run:
        print(f"Pushing image for deck {deck_name} with card id {card_id}")
        return

    if front_image.exists() and back_image.exists():
        try:
            front_name = upload_media(front_image)
            back_name = upload_media(back_image)
            add_or_update_card(deck_name, front_name, back_name, [card_id])
            pass
        finally:
            front_image.unlink(missing_ok=True)
            back_image.unlink(missing_ok=True)