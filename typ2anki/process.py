from pathlib import Path

from .card_wrapper import CardInfo
from .config import config
from .api import get_deck_names, upload_media, create_deck, add_or_update_card


def process_create_deck(deck_name):
    if config().dry_run:
        print(f"Creating deck {deck_name}")
        return
    create_deck(deck_name)


def process_image(card: CardInfo, output_path):
    card_id = card.card_id
    front_image = Path(output_path) / f"typ-{card_id}-1.{config().output_type}"
    back_image = Path(output_path) / f"typ-{card_id}-2.{config().output_type}"

    if config().dry_run:
        print(f"Pushing image for deck {card.deck_name} with card id {card_id}")
        return

    if front_image.exists() and back_image.exists():
        try:
            card.output_front_anki_name = upload_media(front_image)
            card.output_back_anki_name = upload_media(back_image)
            add_or_update_card(card, [card_id])
        finally:
            front_image.unlink(missing_ok=True)
            back_image.unlink(missing_ok=True)
