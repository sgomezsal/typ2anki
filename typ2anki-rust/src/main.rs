use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Instant,
};

use crate::{
    card_wrapper::{CardInfo, CardModificationStatus, TypFileStats},
    generator::generate_card_file_content,
    output::{OutputManager, OutputMessage},
};

mod anki_api;
mod card_wrapper;
mod cards_cache;
mod compile;
mod config;
mod generator;
mod output;
mod parse_file;
mod typst_as_library;
mod utils;

fn main() {
    let output = OutputManager::new();
    run(&output);
}

fn run(output: &OutputManager) {
    let cfg = config::get();
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
    let mut files: HashMap<PathBuf, TypFileStats> = HashMap::new();
    let mut deck_names: HashSet<String> = HashSet::new();

    // parse each typ file
    for filepath in &typ_files {
        if cfg.is_file_excluded(filepath.to_string_lossy().as_ref()) {
            continue;
        }
        let mut file = TypFileStats::new(filepath.clone());

        if let Ok(content) = std::fs::read_to_string(filepath) {
            let parsed = parse_file::parse_cards_string(&content);
            if parsed.len() == 0 {
                continue;
            }
            for card_str in parsed.into_iter() {
                if parse_file::is_card_empty(&card_str) {
                    file.empty_cards += 1;
                    continue;
                }

                match CardInfo::from_string(i, &card_str, filepath.clone()) {
                    Ok(card_info) => {
                        if cfg.is_deck_excluded(card_info.deck_name.as_str()) {
                            file.skipped_cards += 1;
                            continue;
                        }
                        cards_cache_manager.add_card_hash(
                            &card_info.deck_name,
                            &card_info.card_id,
                            &card_info.content_hash,
                        );
                        deck_names.insert(card_info.deck_name.clone());
                        cards.push(card_info);
                        i += 1;
                        file.total_cards += 1;
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
        if file.total_cards == 0 {
            continue;
        }
        files.insert(filepath.to_owned(), file);
    }

    if cards.len() == 0 {
        output.send(OutputMessage::ParsingError(
            "No cards found, aborting.".to_string(),
        ));
        return output.fail();
    }

    // check anki connection
    if !anki_api::check_anki_running() {
        output.send(OutputMessage::NoAnkiConnection);
        if !cfg.dry_run {
            return output.fail();
        }
    }

    // create decks in anki
    for deck_name in &deck_names {
        if cfg.dry_run {
            output.send(OutputMessage::DbgCreateDeck(deck_name.to_string()));
        } else {
            let _ = anki_api::create_deck(&deck_name.as_str());
        }
    }

    // check for duplicate card IDs
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

    // set status for each card & assign anki deck name
    for card in &mut cards {
        card.set_status(&cards_cache_manager);
        card.anki_deck_name = Some(anki_api::get_anki_deck_name(&card.deck_name));
    }

    // update files stats based on card statuses
    for card in &cards {
        if let Some(file_stats) = files.get_mut(&card.source_file) {
            match card.modification_status {
                CardModificationStatus::Unchanged => {
                    file_stats.unchanged_cards.0 += 1;
                }
                CardModificationStatus::Updated => {
                    file_stats.updated_cards.0 += 1;
                }
                CardModificationStatus::New => {
                    file_stats.new_cards.0 += 1;
                }
                CardModificationStatus::Unknown => {}
            }
        }
    }

    let now = Instant::now();
    for card in &cards {
        // let s = generator::generate_card_file(card);
        // compile::compile_png_base64(s);
    }
    compile::compile_cards(&cards);
    let elapsed = now.elapsed();
    println!(
        "Compiled {} cards in {:.2?} ({:.2} cards/sec)",
        cards.len(),
        elapsed,
        cards.len() as f64 / elapsed.as_secs_f64()
    );

    if cfg.dry_run {
        output.send(OutputMessage::DbgFoundTypstFiles(files.clone()));
    }

    // println!("Found {} cards: {:#?}", cards.len(), cards);
}
