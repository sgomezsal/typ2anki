use std::{
    collections::HashMap,
    ops::Range,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use typst::{
    layout::PagedDocument,
    syntax::{FileId, Source, VirtualPath},
};

use crate::{
    anki_api,
    card_wrapper::{CardInfo, CardModificationStatus, TypFileStats},
    cards_cache::CardsCacheManager,
    config, generator,
    output::{OutputCompiledCardInfo, OutputManager, OutputMessage},
    typst_as_library::{self, DiagnosticFormat},
    utils,
};

// A cache_manager should be passed so that in the case of an error during
// compilation or upload, the card's hash can be removed from the cache.
pub fn compile_cards_concurrent(
    cards: &Vec<CardInfo>,
    output: Arc<impl OutputManager + 'static>,
    cache_manager: Arc<Mutex<CardsCacheManager>>,
    file_stats: Arc<Mutex<HashMap<PathBuf, TypFileStats>>>,
) {
    let cfg = config::get();
    if cfg.generation_concurrency <= 1 {
        compile_cards(cards, output, cache_manager, file_stats);
        return;
    }

    let total = cards.len();
    if total > 0 {
        let n_batches = std::cmp::min(cfg.generation_concurrency, total);
        let chunk_size = (total + n_batches - 1) / n_batches;

        let mut handles = Vec::with_capacity(n_batches);
        for i in 0..n_batches {
            let start = i * chunk_size;
            let end = ((i + 1) * chunk_size).min(total);
            let batch = cards[start..end].to_vec();
            let output_clone = Arc::clone(&output);
            let cache_manager_clone = Arc::clone(&cache_manager);
            let file_stats_clone = Arc::clone(&file_stats);
            let handle = std::thread::spawn(move || {
                compile_cards(&batch, output_clone, cache_manager_clone, file_stats_clone);
            });
            handles.push(handle);
        }

        for h in handles {
            let _ = h.join();
        }
    }
}

pub fn compile_cards(
    cards: &Vec<CardInfo>,
    output: Arc<impl OutputManager>,
    cache_manager: Arc<Mutex<CardsCacheManager>>,
    file_stats: Arc<Mutex<HashMap<PathBuf, TypFileStats>>>,
) {
    if cards.is_empty() {
        return;
    }
    let cfg = config::get();

    let uploader = anki_api::CardUploaderThread::new();

    let mut base = String::new();
    let mut base_length: usize = 0;
    let mut current_file_path = String::new();

    let mut world = typst_as_library::TypstWrapperWorld::new(
        cfg.path.to_string_lossy().into_owned(),
        base.to_owned(),
        &cfg.typst_input,
    );

    let mut ra: Range<usize> = 0..0;

    let card_error = |card: &CardInfo, m: OutputMessage| {
        output.send(m);
        let mut cache_manager = cache_manager.lock().unwrap();
        cache_manager.remove_card_hash(card.deck_name.as_str(), &card.card_id);

        let mut file_stats = file_stats.lock().unwrap();
        if let Some(stats) = file_stats.get_mut(&card.source_file) {
            match card.modification_status {
                CardModificationStatus::New => stats.new_cards.1 += 1,
                CardModificationStatus::Updated => stats.updated_cards.1 += 1,
                CardModificationStatus::Unchanged => stats.unchanged_cards.1 += 1,
                CardModificationStatus::Unknown => {}
            }
        }
    };

    for card in cards {
        if card.modification_status == CardModificationStatus::Unchanged {
            output.send(OutputMessage::SkipCompileCard(
                OutputCompiledCardInfo::build(card, None),
            ));
            continue;
        }
        if current_file_path != card.path_relative_to_root() {
            current_file_path = card.path_relative_to_root();
            base = generator::generate_card_file_content(
                card.relative_ankiconf_path(),
                "".to_string(),
            );
            base_length = base.len();
            world.source = Source::new(
                FileId::new(None, VirtualPath::new(&current_file_path)),
                base,
            );
            ra = base_length..base_length;
            // println!("Switched to new source file: {}", current_file_path);
        }

        world.source.edit(ra.clone(), &card.content);

        let last = world.source.text().len();
        ra = base_length..last;

        let out = typst::compile(&world);
        if out.output.is_err() {
            let s = typst_as_library::render_diagnostics(
                &world,
                out.output.unwrap_err().as_slice(),
                out.warnings.as_slice(),
                DiagnosticFormat::Human,
            )
            .expect("Failed to print diagnostics");

            card_error(
                card,
                OutputMessage::CompileError(OutputCompiledCardInfo::build(card, Some(s))),
            );

            continue;
        }
        if out.output.is_err() {
            card_error(
                card,
                OutputMessage::CompileError(OutputCompiledCardInfo::build(
                    card,
                    Some("Error compiling typst.".to_string()),
                )),
            );
            continue;
        }

        let document: PagedDocument = out.output.unwrap();

        if document.pages.len() < 2 {
            card_error(
                card,
                OutputMessage::CompileError(OutputCompiledCardInfo::build(
                    card,
                    Some("Error: Compiled document has less than 2 pages.".to_string()),
                )),
            );
            continue;
        }

        let render = typst_render::render(&document.pages[0], 2.0);
        let input = render.encode_png();
        if input.is_err() {
            card_error(
                card,
                OutputMessage::CompileError(OutputCompiledCardInfo::build(
                    card,
                    Some("Error encoding front side PNG.".to_string()),
                )),
            );
            continue;
        }
        let front_b64 = utils::b64_encode(input.unwrap());

        let render = typst_render::render(&document.pages[1], 2.0);
        let input = render.encode_png();
        if input.is_err() {
            card_error(
                card,
                OutputMessage::CompileError(OutputCompiledCardInfo::build(
                    card,
                    Some("Error encoding back side PNG.".to_string()),
                )),
            );
            continue;
        }
        let back_b64 = utils::b64_encode(input.unwrap());

        output.send(OutputMessage::CompiledCard(OutputCompiledCardInfo::build(
            card, None,
        )));

        if let Err(e) = uploader.upload_card(card, &front_b64, &back_b64) {
            card_error(
                card,
                OutputMessage::PushError(OutputCompiledCardInfo::build(
                    card,
                    Some(format!("Error uploading card to Anki: {}", e)),
                )),
            );
            continue;
        } else {
            output.send(OutputMessage::PushedCard(OutputCompiledCardInfo::build(
                card, None,
            )));
        }
    }
}
