use regex::Regex;
use std::{path::PathBuf, sync::LazyLock};

use crate::{config, utils};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardModificationStatus {
    Unknown,
    New,
    Updated,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct TypFileStats {
    pub filepath: PathBuf,
    pub total_cards: usize,
    pub new_cards: usize,
    pub updated_cards: usize,
    pub unchanged_cards: usize,
    pub error_cards: usize,
    pub empty_cards: usize,
}

impl TypFileStats {
    pub fn new(filepath: PathBuf) -> Self {
        Self {
            filepath,
            total_cards: 0,
            new_cards: 0,
            updated_cards: 0,
            unchanged_cards: 0,
            error_cards: 0,
            empty_cards: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardInfo {
    pub internal_id: i64,
    // The file name from which the card is compiled
    pub filepath: PathBuf,
    // The user defined unique card_id
    pub card_id: String,
    // The user defined deck_name
    pub deck_name: String,
    // The deck name in anki, with leading folder
    pub anki_deck_name: Option<String>,
    // The card's content
    pub content: String,
    // A hash of the card's content
    pub content_hash: String,
    // The card's noticed modification status
    pub modification_status: CardModificationStatus,
}

impl CardInfo {
    pub fn from_string(
        internal_id: i64,
        card_str: &str,
        filepath: PathBuf,
    ) -> Result<Self, String> {
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
            filepath: filepath,
            card_id,
            deck_name: target_deck,
            anki_deck_name: None,
            content: card_str.to_string(),
            content_hash: utils::hash_string(card_str),
            modification_status: CardModificationStatus::Unknown,
        })
    }
}
