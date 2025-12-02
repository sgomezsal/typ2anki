use colored::*;
use regex::Regex;
use std::{ops::Range, path::PathBuf, sync::LazyLock};

use crate::{cards_cache, config, utils};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardModificationStatus {
    Unknown,
    New,
    Updated,
    Unchanged,
}

type CardCountPair = (usize, usize); // (total count, errors)

fn card_pair_status(
    symbol: &str,
    color: fn(s: &str) -> ColoredString,
    (total, errors): &CardCountPair,
) -> String {
    if *total == 0 {
        return format!("{}{:0>2}", symbol.bright_black(), "0".bright_black());
    }
    let symbol = color(symbol);
    if *errors == 0 {
        format!("{}{:0>2}", symbol, color(total.to_string().as_str()))
    } else {
        format!(
            "{}{:0>2}{}{:0>2}",
            symbol,
            (total - errors).to_string().red(),
            "/".bright_black(),
            color(total.to_string().as_str())
        )
    }
}

fn card_single_status(symbol: &str, color: fn(s: &str) -> ColoredString, count: usize) -> String {
    if count == 0 {
        return format!("{}{:0>2}", symbol.bright_black(), "0".bright_black());
    }
    let symbol = color(symbol);
    format!("{}{:0>2}", symbol, color(count.to_string().as_str()))
}

#[derive(Debug, Clone)]
pub struct TypFileStats {
    pub total_cards: usize,
    pub new_cards: CardCountPair,
    pub updated_cards: CardCountPair,
    pub unchanged_cards: CardCountPair,
    pub empty_cards: usize,
    pub skipped_cards: usize,
}
pub type TFiles =
    std::sync::Arc<std::sync::RwLock<std::collections::HashMap<PathBuf, TypFileStats>>>;

impl TypFileStats {
    pub fn new(_filepath: PathBuf) -> Self {
        Self {
            total_cards: 0,
            new_cards: (0, 0),
            updated_cards: (0, 0),
            unchanged_cards: (0, 0),
            empty_cards: 0,
            skipped_cards: 0,
        }
    }

    pub fn total_errors(&self) -> usize {
        self.new_cards.1 + self.updated_cards.1 + self.unchanged_cards.1
    }

    pub fn stats_colored(&self) -> String {
        let separator = "|".bright_black();
        format!(
            "{}{}{}{}{}{}{}{}",
            card_pair_status("+", |s| s.green(), &self.new_cards),
            separator,
            card_pair_status("↑", |s| s.green(), &self.updated_cards),
            separator,
            card_single_status("☓", |s| s.red(), self.total_errors()),
            separator,
            card_pair_status("↷", |s| s.white(), &self.unchanged_cards),
            if self.empty_cards > 0 {
                format!(
                    "{}{}",
                    separator,
                    card_single_status("∅", |s| s.blue(), self.empty_cards)
                )
            } else {
                "".to_string()
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct BarebonesCardInfo {
    // The user defined unique card_id
    pub card_id: String,
    // The user defined deck_name
    pub deck_name: String,
    // The card's question
    pub question: String,
    // The card's answer
    pub answer: String,
    // range of bytes in the source file
    pub byte_range: (usize, usize),
    pub prelude_range: Option<Range<usize>>,
}

#[derive(Debug, Clone)]
pub struct CardInfo {
    // The file name from which the card is compiled
    pub source_file: PathBuf,
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
        _internal_id: i64,
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
            source_file: filepath,
            card_id,
            deck_name: target_deck,
            anki_deck_name: None,
            content: card_str.to_string(),
            content_hash: utils::hash_string(card_str),
            modification_status: CardModificationStatus::Unknown,
        })
    }

    pub fn set_status(&mut self, cards_cache_manager: &cards_cache::CardsCacheManager) {
        let cfg = config::get();
        let key = cards_cache::card_key(&self.deck_name, &self.card_id);
        if let Some(old_hash) = cards_cache_manager.old_cache.get(&key) {
            if old_hash.ends_with(&self.content_hash) {
                if !old_hash.starts_with(&cards_cache_manager.static_hash)
                    && cfg.recompile_on_config_change.read().unwrap().unwrap()
                {
                    self.modification_status = CardModificationStatus::Updated;
                } else {
                    self.modification_status = CardModificationStatus::Unchanged;
                }
            } else {
                self.modification_status = CardModificationStatus::Updated;
            }
        } else {
            self.modification_status = CardModificationStatus::New;
        }
    }

    pub fn path_relative_to_root(&self) -> String {
        let cfg = config::get();

        // relative path from cfg.path to output_path
        let relative_path = pathdiff::diff_paths(&self.source_file, &cfg.path)
            .unwrap_or(self.source_file.clone())
            .to_string_lossy()
            .into_owned();

        relative_path
    }

    pub fn relative_ankiconf_path(&self) -> String {
        let cfg = config::get();
        let output_path = self.source_file.parent().unwrap_or(&cfg.path).to_path_buf();

        // relative path from output_path to cfg.path / ankiconf.typ
        let ankiconf_relative_path = {
            let ankiconf_path = cfg.path.join("ankiconf.typ");
            pathdiff::diff_paths(&ankiconf_path, &output_path).unwrap_or(ankiconf_path)
        }
        .to_string_lossy()
        .into_owned();

        ankiconf_relative_path
    }

    pub fn image_path(&self, page: usize) -> String {
        format!("typ-{}-{}.png", self.card_id, page)
    }
}
