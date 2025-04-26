from dataclasses import dataclass
from enum import Enum
import os
from pathlib import Path
import subprocess
from typing import List

from typ2anki.progressbar import ProgressBarManager


class CardModificationStatus(Enum):
    UNKNOWN = 0
    NEW = 1
    MODIFIED = 2
    UNMODIFIED = 3


@dataclass
class CardGenerationProcess:
    card: "CardInfo"
    parameters: List[str]
    temp_file: Path

    process: subprocess.Popen | None = None

    def start(self):
        self.process = subprocess.Popen(
            self.parameters, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )

    def collect(self):
        assert (
            self.process is not None
        ), "Process was not started before collection"

        stdout, stderr = self.process.communicate()
        return (
            self.process.returncode,
            stdout.decode(errors="replace").strip(),
            stderr.decode(errors="replace").strip(),
        )

    def collect_integrated(self) -> bool:
        returncode, stdout, stderr = self.collect()
        if returncode != 0:
            msg = f"Error generating card {self.card.card_id}"
            if stdout:
                msg += f"\n{stdout}"
            if stderr:
                msg += f"\n{stderr}"
            ProgressBarManager.get_instance().log_message(msg)
        return returncode == 0

    def run_integrated(self):
        self.start()
        return self.collect_integrated()

    def clean(self):
        if os.path.exists(self.temp_file):
            os.remove(self.temp_file)
        self.card.generation_process = None


@dataclass
class CardInfo:
    internal_id: int
    file_name: str
    card_id: str
    deck_name: str
    content_hash: str
    modification_status: CardModificationStatus = CardModificationStatus.UNKNOWN
    should_push: bool = True
    # the deck name used for the final push; can be in a collection, which adds a prefix
    anki_deck_name: str = ""
    generation_process: "CardGenerationProcess | None" = None
    output_front_anki_name: str | None = None
    output_back_anki_name: str | None = None
    old_anki_id: str | None = None

    def set_modification_status(self, cache_manager):
        old_hash = cache_manager.cache.get(
            f"{self.deck_name}_{self.card_id}", None
        )
        if old_hash is None:
            self.modification_status = CardModificationStatus.NEW
            self.should_push = True
            return
        # The old hash ends with the old content_hash
        if old_hash.endswith(self.content_hash):
            self.modification_status = CardModificationStatus.UNMODIFIED
        else:
            self.modification_status = CardModificationStatus.MODIFIED
        self.should_push = cache_manager.card_needs_push(self)
