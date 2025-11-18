use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::{
    anki_api::get_anki_deck_name,
    card_wrapper::{CardInfo, CardModificationStatus, TypFileStats},
    output::{OutputManager, OutputMessage},
    output_console::OutputConsole,
};

mod anki_api;
mod card_wrapper;
mod cards_cache;
mod compile;
mod config;
mod generator;
mod output;
mod output_console;
mod parse_file;
mod typst_as_library;
mod utils;

fn main() {
    let output = OutputConsole::new();
    run(output);
}

fn run(output: impl OutputManager + 'static) {
    let output = Arc::new(output);

    let cfg = config::get();
    let _cfg_guard = config::ConfigGuard;

    if cfg.dry_run {
        output.send(OutputMessage::DbgShowConfig(cfg.clone()));
    }
    parse_file::check_ankiconf_exists();
    let ankiconf_hash = parse_file::get_ankiconf_hash();
    let mut cards_cache_manager =
        cards_cache::CardsCacheManager::init(ankiconf_hash, output.as_ref());

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
    let files: Arc<Mutex<HashMap<PathBuf, TypFileStats>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut deck_names: HashSet<String> = HashSet::new();

    let mut files_lock = files.lock().unwrap();

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
        files_lock.insert(filepath.clone(), file);
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
            let _ = anki_api::create_deck(&get_anki_deck_name(deck_name));
        }
    }

    // check for duplicate card IDs
    if cfg.check_duplicates {
        let mut seen_ids = HashSet::new();
        let mut e = false;
        for card in &cards {
            if seen_ids.contains(&card.card_id) {
                output.send(OutputMessage::ParsingError(format!(
                    "Warning: Duplicate card ID found: {} ({})",
                    card.card_id,
                    card.source_file.to_string_lossy()
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

    cards_cache_manager.detect_configuration_change(output.as_ref());

    // set status for each card & assign anki deck name
    for card in &mut cards {
        card.set_status(&cards_cache_manager);
        card.anki_deck_name = Some(anki_api::get_anki_deck_name(&card.deck_name));
    }

    // update files stats based on card statuses
    for card in &cards {
        if let Some(file_stats) = files_lock.get_mut(&card.source_file) {
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

    drop(files_lock);

    output.send(OutputMessage::ListTypstFiles(files.clone()));

    // Compile and upload cards concurrently
    let cards_cache_manager = Arc::new(Mutex::new(cards_cache_manager));

    let now = Instant::now();
    compile::compile_cards_concurrent(
        &cards,
        output.clone(),
        cards_cache_manager.clone(),
        files.clone(),
    );
    let elapsed = now.elapsed();

    let cards_cache_manager = match Arc::try_unwrap(cards_cache_manager) {
        Ok(mutex) => mutex.into_inner().unwrap(),
        Err(_) => panic!("Failed to unwrap Arc for CardsCacheManager"),
    };

    output.send(OutputMessage::DbgCompilationDone {
        files: files.clone(),
    });

    let compiled_count = cards
        .iter()
        .filter(|c| c.modification_status != CardModificationStatus::Unchanged)
        .count();

    println!(
        "Compiled {} cards in {:.2?} ({:.2} cards/sec)",
        compiled_count,
        elapsed,
        compiled_count as f64 / elapsed.as_secs_f64()
    );

    // At the end, save the cache
    if !cfg.dry_run {
        cards_cache_manager.save_cache(output.as_ref());
    }

    output.send(OutputMessage::DbgDone);

    if cfg.keep_terminal_open {
        println!("Press Enter to exit...");
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
    }
}
