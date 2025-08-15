from dataclasses import dataclass
from typing import Dict, Tuple
from .api import (
    get_cards_cache_string,
    upload_media,
    CARDS_CACHE_FILENAME,
)
import json
from .card_wrapper import CardInfo
from .config import config
from .utils import hash_string

CACHE_HASH_PART_LENGTH = 34


def cache_concat_hashes_padding(hash1: str, hash2: str) -> str:
    return hash1[:CACHE_HASH_PART_LENGTH].ljust(
        CACHE_HASH_PART_LENGTH, "0"
    ) + hash2[:CACHE_HASH_PART_LENGTH].ljust(CACHE_HASH_PART_LENGTH, "0")


class CardsCacheManager:
    current_ankiconf_hash = None
    current_config_hash = None
    static_hash = None
    current_card_hashes: Dict[str, str] = {}
    cache: Dict[str, str] = {}
    ignore_config_change = False

    def __init__(self):
        self.load_cache()

    # Creating a static_hash to be used as a base for the file hashes: objective of this is to be able to use the same cache file for different configurations
    def add_static_hashes(self, ankiconf_hash, config_hash):
        self.current_ankiconf_hash = ankiconf_hash
        self.current_config_hash = config_hash
        self.static_hash = hash_string(f"{ankiconf_hash}{config_hash}")

    def load_cache(self):
        # If we are not checking checksums, clear the cache
        if not config().check_checksums:
            print("Not checking checksums, using an empty cache")
            self.cache = {}
            return
        s = get_cards_cache_string()
        if not s:
            self.cache = {}
        else:
            try:
                self.cache = json.loads(s)
            except Exception as e:
                print("Error loading cache")

        if self.cache is None:
            self.cache = {}

    def add_current_card_hash(self, deck_name, card_id, card_hash):
        assert (
            self.static_hash is not None
        ), "Static hash is not set. Call add_static_hashes first."
        self.current_card_hashes[f"{deck_name}_{card_id}"] = (
            cache_concat_hashes_padding(self.static_hash, card_hash)
        )

    # Returns: (changed_configs,total_cards)
    def has_config_changed(self) -> Tuple[int, int]:
        config_changes = 0
        total_cards = 0
        for key, cached_hash in self.cache.items():
            if key in self.current_card_hashes:
                total_cards += 1
                if (
                    cached_hash[:CACHE_HASH_PART_LENGTH]
                    != self.current_card_hashes[key][:CACHE_HASH_PART_LENGTH]
                ):
                    config_changes += 1
        return (config_changes, total_cards)

    def detect_config_change(self):
        if not config().check_checksums:
            return

        if config().recompile_on_config_change != "_":
            self.ignore_config_change = (
                config().recompile_on_config_change != "y"
            )
            return

        config_changes, total_cards = self.has_config_changed()
        if config().dry_run:
            print(
                f"Config changes detected: {config_changes}, Total cards already cached: {total_cards}"
            )

        if total_cards == 0:
            return

        if config_changes / total_cards > 0.5:
            try:
                inp = input(
                    "A configuration or ankiconf.typ change has been detected. Do you wish to recompile all cards with this new config? (Y/n): "
                ).lower()
            except EOFError:
                print("No input available. Assuming 'Y'.")
                inp = "y"

            if inp == "n" or inp == "no":
                self.ignore_config_change = True
            else:
                self.ignore_config_change = False

    def remove_card_hash(self, card: CardInfo):
        id = f"{card.deck_name}_{card.card_id}"
        self.current_card_hashes.pop(id, None)
        self.cache.pop(id, None)

    def card_needs_push(self, card: CardInfo):
        if config().check_checksums == False:
            return True
        id = f"{card.deck_name}_{card.card_id}"

        new_hash = self.current_card_hashes.get(id, "notfound")
        old_hash = self.cache.get(id, "notfound")

        if new_hash == "notfound" or old_hash == "notfound":
            return True

        if self.ignore_config_change:
            return (
                new_hash[-CACHE_HASH_PART_LENGTH:]
                != old_hash[-CACHE_HASH_PART_LENGTH:]
            )
        return new_hash != old_hash

    def save_cache(self, output_path):
        temp_file = output_path / CARDS_CACHE_FILENAME
        new_cache = self.cache
        new_cache.update(self.current_card_hashes)

        with open(temp_file, "w") as file:
            file.write(json.dumps(new_cache))

        upload_media(temp_file)
        temp_file.unlink(missing_ok=True)
