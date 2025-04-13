import logging
from pathlib import Path
import sys
from typing import Dict, List, Set, Tuple
from typ2anki.api import check_anki_running, get_deck_names
from typ2anki.cardscache import CardsCacheManager
from typ2anki.config import config
from typ2anki.parse import parse_cards, is_card_empty
from typ2anki.get_data import extract_ids_and_decks
from typ2anki.generator import GenerateCardProcess, generate_card_file, ensure_ankiconf_file, get_ankiconf_hash
from typ2anki.process import process_create_deck, process_image
from typ2anki.progressbar import FileProgressBar, ProgressBarManager
from typ2anki.utils import hash_string, print_header
import concurrent.futures
from functools import cache as functools_cache

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
    files_cards: Dict[str, List[Tuple[str,str,str]]] = {}

    card_ids: Set[str] = set()
    
    empty_cards_count = 0
    empty_cards_files: Dict[str,int] = {}
    parsing_errors = []
    # Parse all typ files
    for typ_file in typ_files_path.rglob("*.typ"):
        if typ_file.name == "ankiconf.typ":
            continue
        if typ_file.name.startswith("temporal-"):         
            continue

        file_cards_key = conf.path_relative_to_root(typ_file).as_posix()
        if conf.is_file_excluded(file_cards_key):
            continue

        cards = []
        def capture_cards(card):
            cards.append(card)

        parse_cards(typ_file, callback=capture_cards)

        ids, decks = extract_ids_and_decks(cards)
    
        files_cards[file_cards_key] = []

        for idx, card in enumerate(cards, start=1):
            card_id = ids.get(f"Card {idx}", "Unknown ID")
            deck_name = decks.get(f"Card {idx}", "Default")
            if card_id == "Unknown ID":
                continue
            
            if conf.is_deck_excluded(deck_name):
                continue

            if is_card_empty(card):
                if conf.dry_run:
                    print(f"Skipping empty card {deck_name}.{card_id}")
                empty_cards_count += 1
                empty_cards_files[file_cards_key] = empty_cards_files.get(file_cards_key, 0) + 1
                continue
            
            if conf.check_duplicates:
                if card_id in card_ids:
                    parsing_errors.append(f"Duplicate card id {card_id} in {deck_name}")
                    continue
                card_ids.add(card_id)
            
            cards_cache_manager.add_current_card_hash(deck_name, card_id, hash_string(card))

            files_cards[file_cards_key].append((deck_name, card_id, card))

        if len(files_cards[file_cards_key]) == 0:
            del files_cards[file_cards_key]
        

    if len(parsing_errors):
        print("Errors found:")
        for error in parsing_errors:
            print(f"  - {error}")
        return sys.exit(1)
    
    if len(files_cards) == 0:
        print("No cards found.")
        return
    
    if not conf.dry_run:
        if not check_anki_running():
            print_header(
                [
                    "Anki couldn't be detected.",
                    "Please make sure Anki is running and the AnkiConnect add-on is installed.",
                    "For more information about installing AnkiConnect, please see typ2anki's README"
                ],
            )
            return sys.exit(1)
            


    # Create progress bars
    progress_bars: Dict[str, FileProgressBar] = {}
    files_count = len(files_cards)
    longest_file_name = max(len(file_cards_key) for file_cards_key in files_cards) + 1
    
    if not conf.dry_run:        
        print("Processing, press 'q' to stop the process.")
        print(f"Legend: \033[32m✓Compiled\033[90m/\033[31m☓Errors\033[90m/\033[37m↷Cache Hits\033[90m/\033[94m∅Empty Cards\033[0m\n")
    
    for i,file_cards_key in enumerate(files_cards):
        progress_bars[file_cards_key] = FileProgressBar(len(files_cards[file_cards_key]), f"{file_cards_key.ljust(longest_file_name)}", position=files_count-i)
        if conf.dry_run:
            progress_bars[file_cards_key].enabled = False
        
    
    if not conf.dry_run:
        print("\n" * files_count, end="")
        for file_cards_key in progress_bars:
            progress_bars[file_cards_key].init()
    
    compiled_cards = 0
    cache_hits = 0
    def format_done_message(compiled, cache_hits, fails, empty):
        separator = "\033[90m/"
        green_compiled = f"\033[{"32m" if compiled > 0 else "90m"}✓{compiled}"
        red_fails = f"\033[{"31m" if fails > 0 else "90m"}☓{fails}"
        white_skipped = f"\033[{"37m" if cache_hits > 0 else "90m"}↷{cache_hits}"
        blue_empty = "" if empty == 0 else f"{separator}\033[94m∅{empty}" 
        reset = "\033[0m"
        
        # Concatenate the formatted segments
        return (green_compiled + 
            separator + 
            red_fails + 
            separator + 
            white_skipped + 
            blue_empty + 
            reset)
    # Generate cards and images
    for file_cards_key in files_cards:
        compiled_at_start = compiled_cards
        cache_hits_at_start = cache_hits
        cards = files_cards[file_cards_key]
        bar = progress_bars[file_cards_key]
        file_output_path = (Path(conf.path) / file_cards_key).resolve().parent

        existing_decks = get_deck_names()
        @functools_cache
        def get_real_deck_name(deck_name):
            s = "::" + deck_name
            for d in existing_decks:
                if d == deck_name or d.endswith(s):
                    return d
            return deck_name

        failed_cards = set()
        unique_decks = set()
        tasks: List[GenerateCardProcess] = []
        for deck_name, card_id, card in cards:
            unique_decks.add(deck_name)
            if not cards_cache_manager.card_needs_update(deck_name, card_id): 
                bar.next(f"Generating card for {deck_name}.{card_id}")
                cache_hits += 1
                continue
            g = generate_card_file(card, card_id, file_output_path)
            if conf.dry_run: continue
            if g is None:
                failed_cards.add(card_id)
                cards_cache_manager.remove_card_hash(deck_name, card_id)
                continue
            g.deck_name = deck_name
            g.real_deck_name = get_real_deck_name(deck_name)
            tasks.append(g)
        
        for deck_name in unique_decks:
            process_create_deck(get_real_deck_name(deck_name))

        def handle_task(t: GenerateCardProcess) -> tuple[bool,str,str]:
            nonlocal compiled_cards, failed_cards, cards_cache_manager, bar, file_output_path
            t.start()
            r = t.collect_integrated()
            if r:
                compiled_cards += 1
                process_image(t.real_deck_name, t.card_id, file_output_path)
            bar.next(f"Generated card for {t.deck_name}.{t.card_id}")
            t.clean()
            return (r, t.deck_name, t.card_id)

        if not conf.dry_run:
            with concurrent.futures.ThreadPoolExecutor(max_workers=conf.generation_concurrency) as executor:
                futures = [executor.submit(handle_task, t) for t in tasks]

                for future in concurrent.futures.as_completed(futures):
                    result = future.result()
                    if not result[0]:
                        failed_cards.add(result[2])
                        cards_cache_manager.remove_card_hash(result[1], result[2])

        d = compiled_cards - compiled_at_start
        bar.done(format_done_message(d, cache_hits - cache_hits_at_start, len(failed_cards), empty_cards_files.get(file_cards_key, 0)))
        
    if not conf.dry_run:
        ProgressBarManager.get_instance().finalize_output()
        cards_cache_manager.save_cache(output_path)
        if empty_cards_count > 0:
            print(f"Skipped {empty_cards_count} empty cards.")
        print(f"Compiled a total of {compiled_cards} cards.")


if __name__ == "__main__":
    main()
    config().destruct()
