import logging
from pathlib import Path
import sys
from typing import Dict, List, Set
from typ2anki.cardscache import CardsCacheManager
from typ2anki.config import config
from typ2anki.parse import parse_cards, is_card_empty
from typ2anki.get_data import extract_ids_and_decks
from typ2anki.generator import generate_card_file, ensure_ankiconf_file, get_ankiconf_hash
from typ2anki.process import process_create_deck, process_image
from typ2anki.progressbar import FileProgressBar, ProgressBarManager
from typ2anki.utils import hash_string

logging.basicConfig(level=logging.INFO, format="%(asctime)s - %(levelname)s - %(message)s")

def main():
    conf = config()
    typ_files_path = Path(conf.path).resolve()
    if not typ_files_path.is_dir():
        logging.error(f"{typ_files_path} is not a valid directory.")
        return

    ensure_ankiconf_file(typ_files_path)

    cards_cache_manager = CardsCacheManager()
    cards_cache_manager.add_static_hashes(
        get_ankiconf_hash(typ_files_path),
        conf.config_hash
    )


    output_path = typ_files_path

    # List of (deck_name, card_id, card) tuples
    files_cards: Dict[str, List[(str,str,str)]] = {}

    card_ids: Set[str] = set()

    # Parse all typ files
    for typ_file in typ_files_path.rglob("*.typ"):
        if typ_file.name == "ankiconf.typ":
            continue
        cards = []
        def capture_cards(card):
            cards.append(card)

        parse_cards(typ_file, callback=capture_cards)

        ids, decks = extract_ids_and_decks(cards)
        
        file_cards_key = conf.path_relative_to_root(typ_file).as_posix()
        files_cards[file_cards_key] = []

        for idx, card in enumerate(cards, start=1):
            card_id = ids.get(f"Card {idx}", "Unknown ID")
            deck_name = decks.get(f"Card {idx}", "Default")
            if card_id == "Unknown ID":
                continue
            
            if conf.is_deck_excluded(deck_name):
                continue

            if is_card_empty(card):
                if config().dry_run:
                    print(f"Skipping empty card {deck_name}.{card_id}")
                continue
            
            if conf.check_duplicates:
                if card_id in card_ids:
                    s = f"Duplicate card id {card_id} in {deck_name}"
                    print(s)
                    raise Exception(s)
                card_ids.add(card_id)
            
            cards_cache_manager.add_current_card_hash(deck_name, card_id, hash_string(card))

            files_cards[file_cards_key].append((deck_name, card_id, card))

        if len(files_cards[file_cards_key]) == 0:
            del files_cards[file_cards_key]

    if len(files_cards) == 0:
        print("No cards found.")
        return

    # Create progress bars
    progress_bars: Dict[str, FileProgressBar] = {}
    files_count = len(files_cards)
    longest_file_name = max(len(file_cards_key) for file_cards_key in files_cards) + 1
    
    if not conf.dry_run:        
        print("Processing, press 'q' to stop the process.\n")
    
    for i,file_cards_key in enumerate(files_cards):
        progress_bars[file_cards_key] = FileProgressBar(len(files_cards[file_cards_key]), f"{file_cards_key.ljust(longest_file_name)}", position=files_count-i)
        if conf.dry_run:
            progress_bars[file_cards_key].enabled = False
        
    
    if not conf.dry_run:
        print("\n" * files_count, end="")
        for file_cards_key in progress_bars:
            progress_bars[file_cards_key].init()
    
    compiled_cards = 0
    # Generate cards and images
    for file_cards_key in files_cards:
        cards = files_cards[file_cards_key]
        bar = progress_bars[file_cards_key]
        file_output_path = (Path(conf.path) / file_cards_key).resolve().parent

        failed_cards = set()
        unique_decks = set()
        for deck_name, card_id, card in cards:
            unique_decks.add(deck_name)
            bar.next(f"Generating card for {deck_name}.{card_id}")
            if not cards_cache_manager.card_needs_update(deck_name, card_id): continue
            if not generate_card_file(card, card_id, file_output_path):
                failed_cards.add(card_id)
                cards_cache_manager.remove_card_hash(deck_name, card_id)
            else:
                compiled_cards += 1

        for deck_name in unique_decks:
            process_create_deck(deck_name)            

        bar.reset()

        for deck_name, card_id, card in cards:
            if card_id in failed_cards:
                bar.next(f"Skipping card {deck_name}.{card_id} as error occured")
                continue
            bar.next(f"Pushing card for {deck_name}.{card_id}")
            if not cards_cache_manager.card_needs_update(deck_name, card_id): continue
            process_image(deck_name, card_id, card, file_output_path)

        if len(failed_cards) > 0:
            bar.done(f"Done - with {len(failed_cards)} errors")
        else:
            bar.done("Done!")
        
    if not conf.dry_run:
        ProgressBarManager.get_instance().finalize_output()
        cards_cache_manager.save_cache(output_path)
        print(f"Compiled a total of {compiled_cards} cards.")


if __name__ == "__main__":
    main()
    config().destruct()
