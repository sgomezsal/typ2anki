import base64
from typing import List
import requests
from pathlib import Path
import hashlib

from typ2anki.config import config
from typ2anki.utils import PassedCardDataForCompilation

ANKI_CONNECT_URL = "http://localhost:8765"

CARDS_CACHE_FILENAME = "_typ-cards-cache.json"


def send_request(payload):
    response = requests.post(ANKI_CONNECT_URL, json=payload).json()
    if response.get("error"):
        raise Exception(f"Anki API Error: {response['error']}")
    return response.get("result")


def check_anki_running() -> bool:
    try:
        response = requests.get(ANKI_CONNECT_URL).json()
    except Exception as e:
        return False
    if not response.get("apiVersion"):
        return False
    return True


def upload_media(file_path):
    file_path = Path(file_path)
    with open(file_path, "rb") as file:
        encoded_data = base64.b64encode(file.read()).decode("utf-8")

    payload = {
        "action": "storeMediaFile",
        "version": 6,
        "params": {
            "filename": file_path.name,
            "data": encoded_data,
        },
    }
    send_request(payload)
    return file_path.name


def get_media_dir_path():
    payload = {
        "action": "getMediaDirPath",
        "version": 6,
    }
    return send_request(payload)


def get_cards_cache_string():
    try:
        payload = {
            "action": "retrieveMediaFile",
            "version": 6,
            "params": {"filename": CARDS_CACHE_FILENAME},
        }
        s = send_request(payload)
        return base64.b64decode(s).decode("utf-8")
    except Exception as e:
        return None


def create_deck(deck_name):
    payload = {
        "action": "createDeck",
        "version": 6,
        "params": {"deck": deck_name},
    }
    send_request(payload)


def get_deck_names() -> List[str]:
    payload = {"action": "deckNames", "version": 6}
    try:
        return send_request(payload)
    except Exception as e:
        print(f"Error getting deck names: {e}")
        return []


def find_note_id_by_tag(tag):
    payload = {
        "action": "findNotes",
        "version": 6,
        "params": {"query": f"tag:{tag}"},
    }
    return send_request(payload)


def update_note(
    note_id,
    card_info: PassedCardDataForCompilation,
    front_image,
    back_image,
    tags,
):
    payload = {
        "action": "updateNoteFields",
        "version": 6,
        "params": {
            "note": {
                "id": note_id,
                "fields": {
                    "Front": config().template_front(card_info, front_image),
                    "Back": config().template_back(card_info, back_image),
                },
                "tags": tags,
            }
        },
    }
    send_request(payload)


def add_or_update_card(
    send_to_deck_name,
    card_info: PassedCardDataForCompilation,
    front_image,
    back_image,
    tags,
):
    note_ids = find_note_id_by_tag(card_info.card_id)
    if note_ids:
        update_note(note_ids[0], card_info, front_image, back_image, tags)
    else:
        payload = {
            "action": "addNote",
            "version": 6,
            "params": {
                "note": {
                    "deckName": send_to_deck_name,
                    "modelName": "Basic",
                    "fields": {
                        "Front": config().template_front(
                            card_info, front_image
                        ),
                        "Back": config().template_back(card_info, back_image),
                    },
                    "tags": tags,
                }
            },
        }
        send_request(payload)
