import pprint
import logging
from pathlib import Path
import sys
from typing import Dict, List, Set, Tuple
from .api import check_anki_running, get_deck_names
from .card_wrapper import CardInfo, CardModificationStatus
from .cardscache import CardsCacheManager
from .config import config
from .parse import parse_cards, is_card_empty
from .get_data import extract_ids_and_decks
from .generator import (
    generate_compilation_task,
    ensure_ankiconf_file,
    get_ankiconf_hash,
)
from .process import process_create_deck, process_image
from .progressbar import FileProgressBar, ProgressBarManager
from .utils import (
    hash_string,
    print_header,
)
import concurrent.futures
from functools import cache as functools_cache

logging.basicConfig(
    level=logging.INFO, format="%(asctime)s - %(levelname)s - %(message)s"
)


def main():
    conf = config()

    typ_files_path = Path(conf.path).resolve()
    if not typ_files_path.is_dir():
        logging.error(f"{typ_files_path} is not a valid directory.")
        return

    ensure_ankiconf_file(typ_files_path)

    cards_cache_manager = CardsCacheManager()
    cards_cache_manager.add_static_hashes(
        get_ankiconf_hash(typ_files_path), conf.config_hash
    )

    output_path = typ_files_path

    # Dictionary, key is filename, value is list of card internal ids
    cards_per_file: Dict[str, List[int]] = {}
    unique_card_ids: Set[str] = set()
    cards: List[CardInfo] = []

    empty_cards_count = 0
    empty_cards_filenames: Dict[str, int] = {}
    parsing_errors = []

    existing_anki_decks = get_deck_names()

    @functools_cache
    def get_anki_deck_name(deck_name):
        s = "::" + deck_name
        for d in existing_anki_decks:
            if d == deck_name or d.endswith(s):
                return d
        return deck_name

    # Parse all typ files
    for typ_file in typ_files_path.rglob("*.typ"):
        if typ_file.name == "ankiconf.typ":
            continue
        if typ_file.name.startswith("temporal-"):
            continue

        cards_per_file_key = conf.path_relative_to_root(typ_file).as_posix()
        if conf.is_file_excluded(cards_per_file_key):
            continue

        file_card_ids = []

        def capture_cards(card):
            file_card_ids.append(card)

        parse_cards(typ_file, callback=capture_cards)

        ids, decks = extract_ids_and_decks(file_card_ids)

        cards_per_file[cards_per_file_key] = []

        for idx, card in enumerate(file_card_ids, start=1):
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
                empty_cards_filenames[cards_per_file_key] = (
                    empty_cards_filenames.get(cards_per_file_key, 0) + 1
                )
                continue

            if conf.check_duplicates:
                if card_id in unique_card_ids:
                    parsing_errors.append(
                        f"Duplicate card id {card_id} in {deck_name}"
                    )
                    continue
                unique_card_ids.add(card_id)

            card_hash = hash_string(card)
            cards_cache_manager.add_current_card_hash(
                deck_name, card_id, card_hash
            )

            internal_id = len(cards)
            c = CardInfo(
                internal_id=internal_id,
                card_id=card_id,
                deck_name=deck_name,
                file_name=cards_per_file_key,
                modification_status=CardModificationStatus.UNKNOWN,
                content_hash=card_hash,
                anki_deck_name=get_anki_deck_name(deck_name),
                card_content=card,
            )
            cards.append(c)

            cards_per_file[cards_per_file_key].append(internal_id)

        if len(cards_per_file[cards_per_file_key]) == 0:
            del cards_per_file[cards_per_file_key]

    cards_cache_manager.detect_config_change()

    for card in cards:
        card.set_modification_status(cards_cache_manager)

    if len(parsing_errors):
        print("Errors found:")
        for error in parsing_errors:
            print(f"  - {error}")
        return sys.exit(1)

    if len(cards_per_file) == 0:
        print("No cards found.")
        return

    if not conf.dry_run:
        if not check_anki_running():
            print_header(
                [
                    "Anki couldn't be detected.",
                    "Please make sure Anki is running and the AnkiConnect add-on is installed.",
                    "For more information about installing AnkiConnect, please see typ2anki's README",
                ],
            )
            return sys.exit(1)

    # Create progress bars
    progress_bars: Dict[str, FileProgressBar] = {}
    files_count = len(cards_per_file)
    longest_file_name_length = (
        max(len(file_cards_key) for file_cards_key in cards_per_file) + 1
    )

    if not conf.dry_run:
        print("Processing, press 'q' to stop the process.")
        SEP = f"\033[90m/"
        print(
            f"Legend: \033[32m+New{SEP}\033[32m↑Updated{SEP}\033[31m☓Errors{SEP}\033[37m↷Cache Hits{SEP}\033[94m∅Empty Cards\033[0m\n"
        )

    for i, cards_per_file_key in enumerate(cards_per_file):
        progress_bars[cards_per_file_key] = FileProgressBar(
            len(cards_per_file[cards_per_file_key]),
            f"{cards_per_file_key.ljust(longest_file_name_length)}",
            position=files_count - i,
        )
        if conf.dry_run:
            progress_bars[cards_per_file_key].enabled = False

    if not conf.dry_run:
        ProgressBarManager.get_instance().init()

    compiled_cards = 0
    cache_hits = 0

    def format_done_message(
        compiled_new, compiled_updated, cache_hits, fails, empty
    ):
        separator = "\033[90m/"
        green_compiled = (
            f"\033[{"32m" if compiled_new > 0 else "90m"}+{compiled_new}"
        )
        green_compiled2 = f"\033[{"32m" if compiled_updated > 0 else "90m"}↑{compiled_updated}"
        red_fails = f"\033[{"31m" if fails > 0 else "90m"}☓{fails}"
        white_skipped = (
            f"\033[{"37m" if cache_hits > 0 else "90m"}↷{cache_hits}"
        )
        blue_empty = "" if empty == 0 else f"{separator}\033[94m∅{empty}"
        reset = "\033[0m"

        # Concatenate the formatted segments
        return (
            green_compiled
            + separator
            + green_compiled2
            + separator
            + red_fails
            + separator
            + white_skipped
            + blue_empty
            + reset
        )

    # Generate compilation tasks
    for cards_per_file_key in cards_per_file:
        file_card_ids = cards_per_file[cards_per_file_key]
        bar = progress_bars[cards_per_file_key]
        file_output_path = (
            (Path(conf.path) / cards_per_file_key).resolve().parent
        )

        stats = {
            "compiled_new": 0,
            "compiled_updated": 0,
            "cache_hits": 0,
            "failed": 0,
            "empty": empty_cards_filenames.get(cards_per_file_key, 0),
        }

        failed_cards = set()
        unique_decks = set()
        for internal_id in file_card_ids:
            card = cards[internal_id]
            unique_decks.add(card.deck_name)
            if not card.should_push:
                bar.next(f"Generating card for {card.deck_name}.{card.card_id}")
                stats["cache_hits"] += 1
                continue
            g = generate_compilation_task(
                card,
                file_output_path,
            )
            if conf.dry_run:
                continue
            if g is None:
                failed_cards.add(card.card_id)
                cards_cache_manager.remove_card_hash(card)
                continue

        for deck_name in unique_decks:
            process_create_deck(get_anki_deck_name(deck_name))

        def handle_task(card: CardInfo) -> tuple[bool, int]:
            nonlocal stats, bar, file_output_path
            assert (
                card.generation_process is not None
            ), "Card generation process is None"
            card.generation_process.start()
            r = card.generation_process.collect_integrated()
            if r:
                process_image(
                    card,
                    file_output_path,
                )
                if card.old_anki_id is None:
                    stats["compiled_new"] += 1
                else:
                    stats["compiled_updated"] += 1
            bar.next(f"Generated card for {card.deck_name}.{card.card_id}")
            card.generation_process.clean()
            return (r, card.internal_id)

        if not conf.dry_run:
            with concurrent.futures.ThreadPoolExecutor(
                max_workers=conf.generation_concurrency
            ) as executor:
                futures = []
                for internal_id in file_card_ids:
                    card = cards[internal_id]
                    if not card.should_push or card.generation_process is None:
                        continue
                    futures.append(executor.submit(handle_task, card))

                for future in concurrent.futures.as_completed(futures):
                    result = future.result()
                    if not result[0]:
                        card = cards[result[1]]
                        failed_cards.add(card.card_id)
                        cards_cache_manager.remove_card_hash(card)

        bar.done(
            format_done_message(
                stats["compiled_new"],
                stats["compiled_updated"],
                stats["cache_hits"],
                len(failed_cards),
                stats["empty"],
            )
        )
        compiled_cards += stats["compiled_new"] + stats["compiled_updated"]
        cache_hits += stats["cache_hits"]

    if not conf.dry_run:
        ProgressBarManager.get_instance().finalize_output()
        cards_cache_manager.save_cache(output_path)
        if empty_cards_count > 0:
            print(f"Skipped {empty_cards_count} empty cards.")
        print(f"Compiled a total of {compiled_cards} cards.")


if __name__ == "__main__":
    main()
    config().destruct()
