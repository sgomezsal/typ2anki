from dataclasses import dataclass
from typing import Dict
from typ2anki.api import (
    get_cards_cache_string,
    upload_media,
    CARDS_CACHE_FILENAME,
)
import json
from typ2anki.card_wrapper import CardInfo
from typ2anki.config import config
from typ2anki.utils import hash_string


class CardsCacheManager:
    current_ankiconf_hash = None
    current_config_hash = None
    static_hash = None
    current_card_hashes: Dict[str, str] = {}
    cache: Dict[str, str] = {}

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
        self.current_card_hashes[f"{deck_name}_{card_id}"] = (
            self.static_hash + card_hash
        )

    def remove_card_hash(self, card: CardInfo):
        id = f"{card.deck_name}_{card.card_id}"
        self.current_card_hashes.pop(id, None)
        self.cache.pop(id, None)

    def card_needs_push(self, card: CardInfo):
        if config().check_checksums == False:
            return True
        id = f"{card.deck_name}_{card.card_id}"
        return self.current_card_hashes.get(id, "notfound1") != self.cache.get(
            id, "notfound2"
        )

    def save_cache(self, output_path):
        temp_file = output_path / CARDS_CACHE_FILENAME
        new_cache = self.cache
        new_cache.update(self.current_card_hashes)

        with open(temp_file, "w") as file:
            file.write(json.dumps(new_cache))

        upload_media(temp_file)
        temp_file.unlink(missing_ok=True)
