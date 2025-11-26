use once_cell::sync::OnceCell;
use std::{
    ops::Range,
    sync::{Arc, Mutex},
};
use typst::{
    layout::PagedDocument,
    syntax::{FileId, Source, VirtualPath},
};

use crate::{
    anki_api,
    card_wrapper::{CardInfo, CardModificationStatus, TFiles},
    cards_cache::CardsCacheManager,
    config, generator,
    output::{OutputCompiledCardInfo, OutputManager, OutputMessage},
    typst_as_library::{self, DiagnosticFormat, DownloadLocks},
    utils,
};

// A cache_manager should be passed so that in the case of an error during
// compilation or upload, the card's hash can be removed from the cache.
pub fn compile_cards_concurrent(
    cards: &Vec<CardInfo>,
    output: Arc<impl OutputManager + 'static>,
    cache_manager: Arc<Mutex<CardsCacheManager>>,
    file_stats: TFiles,
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
            let output_clone = output.clone();
            let cache_manager_clone = cache_manager.clone();
            let file_stats_clone = file_stats.clone();
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

static TYPST_PACKAGE_DOWNLOAD_LOCK: OnceCell<DownloadLocks> = OnceCell::new();

pub fn compile_cards(
    cards: &Vec<CardInfo>,
    output: Arc<impl OutputManager + 'static>,
    cache_manager: Arc<Mutex<CardsCacheManager>>,
    file_stats: TFiles,
) {
    if cards.is_empty() {
        return;
    }
    let cfg = config::get();

    let uploader = anki_api::CardUploaderThread::new();

    let mut base_length: usize = 0;
    let mut current_file_path = String::new();

    let mut world = typst_as_library::TypstWrapperWorld::new_with_download_locks(
        cfg.path.to_string_lossy().into_owned(),
        "".to_string(),
        &cfg.typst_input,
        TYPST_PACKAGE_DOWNLOAD_LOCK
            .get_or_init(|| DownloadLocks::default())
            .clone(),
    );
    world.output_manager = Some(output.clone());

    let mut content_range: Range<usize> = 0..0;

    let card_error = |card: &CardInfo, m: OutputMessage| {
        let mut cache_manager = cache_manager.lock().unwrap();
        cache_manager.remove_card_hash(card.deck_name.as_str(), &card.card_id);

        {
            let mut file_stats = file_stats.write().unwrap();
            if let Some(stats) = file_stats.get_mut(&card.source_file) {
                match card.modification_status {
                    CardModificationStatus::New => stats.new_cards.1 += 1,
                    CardModificationStatus::Updated => stats.updated_cards.1 += 1,
                    CardModificationStatus::Unchanged => stats.unchanged_cards.1 += 1,
                    CardModificationStatus::Unknown => {}
                }
            }
        }

        output.send(m);
    };

    // Returns a Result with Option of front and back base64 strings
    let mut compile_card = |card: &CardInfo| -> Result<Option<(String, String)>, String> {
        if card.modification_status == CardModificationStatus::Unchanged {
            output.send(OutputMessage::SkipCompileCard(card.into()));
            return Ok(None);
        }
        if current_file_path != card.path_relative_to_root() {
            current_file_path = card.path_relative_to_root();
            let base = generator::generate_card_file_content(
                card.relative_ankiconf_path(),
                "".to_string(),
            );
            base_length = base.len();
            world.source = Source::new(
                FileId::new(None, VirtualPath::new(&current_file_path)),
                base,
            );
            content_range = base_length..base_length;
        }
        world.source.edit(content_range.clone(), &card.content);

        let last = world.source.text().len();
        content_range = base_length..last;

        let out = typst::compile(&world);
        let document: PagedDocument = out.output.map_err(|e| {
            typst_as_library::render_diagnostics(
                &world,
                e.as_slice(),
                out.warnings.as_slice(),
                DiagnosticFormat::Human,
            )
            .unwrap_or_else(|_| "Failed to render diagnostics.".to_string())
        })?;

        if document.pages.len() < 2 {
            return Err("Error: Compiled document has less than 2 pages.".to_string());
        }

        let render = typst_render::render(&document.pages[0], 2.0)
            .encode_png()
            .map_err(|_| "Error encoding front side PNG.")?;
        let front_b64 = utils::b64_encode(render);

        let render = typst_render::render(&document.pages[1], 2.0)
            .encode_png()
            .map_err(|_| "Error encoding back side PNG.")?;
        let back_b64 = utils::b64_encode(render);

        output.send(OutputMessage::CompiledCard(card.into()));

        Ok(Some((front_b64, back_b64)))
    };

    for card in cards {
        match compile_card(card) {
            Ok(Some((front_b64, back_b64))) => {
                if let Err(e) = uploader.upload_card(card, &front_b64, &back_b64) {
                    card_error(
                        card,
                        OutputMessage::PushError(OutputCompiledCardInfo::build(
                            card,
                            Some(format!("Error uploading card to Anki: {}", e)),
                        )),
                    );
                } else {
                    output.send(OutputMessage::PushedCard(card.into()));
                }
            }
            Ok(None) => {}
            Err(msg) => {
                card_error(
                    card,
                    OutputMessage::CompileError(OutputCompiledCardInfo::build(card, Some(msg))),
                );
            }
        }
    }
}
