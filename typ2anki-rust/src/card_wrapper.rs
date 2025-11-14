use regex::Regex;
use std::sync::LazyLock;

use crate::utils;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardModificationStatus {
    Unknown,
    New,
    Updated,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct CardInfo {
    internal_id: i64,
    // The file name from which the card is compiled
    file_name: String,
    // The user defined unique card_id
    card_id: String,
    // The user defined deck_name
    deck_name: String,
    // The deck name in anki, with leading folder
    anki_deck_name: Option<String>,
    // The card's content
    content: String,
    // A hash of the card's content
    content_hash: String,
    // The card's noticed modification status
    modification_status: CardModificationStatus,
}

impl CardInfo {
    pub fn from_string(internal_id: i64, card_str: &str, file_name: &str) -> Result<Self, String> {
        static ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"id:\s*"([^"]+)""#).unwrap());
        static DECK_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"target-deck:\s*"([^"]+)""#).unwrap());

        let card_id = ID_RE
            .captures(card_str)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()));
        if let None = card_id {
            return Err("Card ID not found".to_string());
        }
        let card_id = card_id.unwrap();

        let target_deck = DECK_RE
            .captures(card_str)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()));
        if let None = target_deck {
            return Err("Target deck not found".to_string());
        }
        let target_deck = target_deck.unwrap();

        Ok(Self {
            internal_id,
            file_name: file_name.to_string(),
            card_id,
            deck_name: target_deck,
            anki_deck_name: None,
            content: card_str.to_string(),
            content_hash: utils::hash_string(card_str),
            modification_status: CardModificationStatus::Unknown,
        })
    }
}
