use std::{ops::Range, sync::Arc};
use typst::{
    layout::PagedDocument,
    syntax::{FileId, Source, VirtualPath},
};

use crate::{
    anki_api,
    card_wrapper::{CardInfo, CardModificationStatus},
    config, generator,
    output::{OutputCompiledCardInfo, OutputManager, OutputMessage},
    typst_as_library::{self, DiagnosticFormat},
};

/* pub fn compile_png_base64(typst_content: String) {
    let cfg = config::get();
    let mut world = typst_as_library::TypstWrapperWorld::new(
        cfg.path.to_string_lossy().into_owned(),
        typst_content.to_owned(),
    );

    let document: PagedDocument = typst::compile(&world)
        .output
        .expect("Error compiling typst");

    let render = typst_render::render(document.pages.first().unwrap(), 2.0);
    let png = render.encode_png().expect("Failed to encode PNG");

    println!("Compiled card to PNG ({} bytes)", png.len());

    // std::fs::write("/tmp/image1.png", png).expect("Failed to write /tmp/image1.png");
} */

pub fn compile_cards_concurrent(cards: &Vec<CardInfo>, output: Arc<OutputManager>) {
    let cfg = config::get();
    if cfg.generation_concurrency <= 1 {
        compile_cards(cards, output);
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
            let handle = std::thread::spawn(move || {
                compile_cards(&batch, output_clone);
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
    output: Arc<OutputManager>,
    // file_stats: &HashMap<PathBuf, Mutex<TypFileStats>>,
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

            output.send(OutputMessage::CompileError(OutputCompiledCardInfo::build(
                card,
                Some(s),
            )));

            continue;
        }
        let document: PagedDocument = out.output.expect("Error compiling typst");

        if document.pages.len() < 2 {
            output.send(OutputMessage::CompileError(OutputCompiledCardInfo::build(
                card,
                Some("Error: Compiled document has less than 2 pages.".to_string()),
            )));
            continue;
        }

        let render = typst_render::render(&document.pages[0], 2.0);
        let front_b64 = base64::encode(render.encode_png().expect("Failed to encode PNG"));

        let render = typst_render::render(&document.pages[1], 2.0);
        let back_b64 = base64::encode(render.encode_png().expect("Failed to encode PNG"));

        output.send(OutputMessage::CompiledCard(OutputCompiledCardInfo::build(
            card, None,
        )));

        if let Err(e) = uploader.upload_card(card, &front_b64, &back_b64) {
            output.send(OutputMessage::PushError(OutputCompiledCardInfo::build(
                card,
                Some(format!("Error uploading card to Anki: {}", e)),
            )));
            continue;
        }
    }
}
