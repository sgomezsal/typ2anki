use std::collections::{HashMap, HashSet};

use crate::{
    card_wrapper::{CardInfo, TypFileStats},
    output::OutputMessage,
};

mod anki_api;
mod card_wrapper;
mod cards_cache;
mod config;
mod output;
mod parse_file;
mod utils;

fn main() {
    let output = output::OutputManager::new();
    let mut cfg = config::get();
    if cfg.dry_run {
        output.send(OutputMessage::DbgShowConfig(cfg.clone()));
    }
    parse_file::check_ankiconf_exists();
    let ankiconf_hash = parse_file::get_ankiconf_hash();
    let mut cards_cache_manager = cards_cache::CardsCacheManager::init(ankiconf_hash, &output);

    // find all *.typ files inside of cfg.path, including nested
    let typ_files = walkdir::WalkDir::new(&cfg.path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("typ"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .filter(|p| {
            let s = p.file_name().unwrap_or_default().to_string_lossy();
            !(s == "ankiconf.typ" || s.starts_with("temporal-"))
        })
        .collect::<Vec<std::path::PathBuf>>();

    let mut i = 0;

    let mut cards: Vec<CardInfo> = Vec::new();
    let mut files: HashMap<String, TypFileStats> = HashMap::new();
    for filepath in &typ_files {
        let mut file = TypFileStats::new(filepath.clone());

        if let Ok(content) = std::fs::read_to_string(filepath) {
            let parsed = parse_file::parse_cards_string(&content);
            file.total_cards = parsed.len();
            for card_str in parsed.into_iter() {
                if parse_file::is_card_empty(&card_str) {
                    file.empty_cards += 1;
                    continue;
                }

                match CardInfo::from_string(i, &card_str, filepath.clone()) {
                    Ok(card_info) => {
                        cards_cache_manager.add_card_hash(
                            &card_info.deck_name,
                            &card_info.card_id,
                            &card_info.content_hash,
                        );
                        cards.push(card_info);
                        i += 1;
                    }
                    Err(_) => {
                        output.send(OutputMessage::ParsingError(format!(
                            "Warning: Failed to parse card in file {:?}",
                            filepath.to_string_lossy()
                        )));
                    }
                }
            }
        } else {
            output.send(OutputMessage::ParsingError(format!(
                "Warning: Failed to read file {:?}",
                filepath.to_string_lossy()
            )));
        }
        files.insert(filepath.to_string_lossy().to_string(), file);
    }

    if cfg.dry_run {
        output.send(OutputMessage::DbgFoundTypstFiles(files));
    }

    if cfg.check_duplicates {
        let mut seen_ids = HashSet::new();
        let mut e = false;
        for card in &cards {
            if seen_ids.contains(&card.card_id) {
                output.send(OutputMessage::ParsingError(format!(
                    "Warning: Duplicate card ID found: {}",
                    card.card_id
                )));
                e = true;
            } else {
                seen_ids.insert(card.card_id.clone());
            }
        }
        if e && !cfg.dry_run {
            output.send(OutputMessage::ParsingError(
                "Error: Duplicate card IDs found, aborting.".to_string(),
            ));
            return output.fail();
        }
    }

    cards_cache_manager.detect_configuration_change(&output);

    for card in &mut cards {
        let old_hash = 
    }

    // let cards: Vec<CardInfo> = cards_strings
    //     .iter()
    //     .enumerate()
    //     .map(|(i, s)| {
    //         CardInfo::from_string(i.try_into().unwrap(), s, filepath.try_into().unwrap()).unwrap()
    //     })
    //     .collect();
    // println!("Found {} cards: {:#?}", cards.len(), cards);
}
