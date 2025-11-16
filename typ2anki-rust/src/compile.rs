use std::{ops::Range, sync::Arc};

use typst::{
    layout::PagedDocument,
    syntax::{FileId, Source, VirtualPath},
};

use crate::{
    card_wrapper::CardInfo,
    config, generator,
    output::{OutputManager, OutputMessage},
    typst_as_library,
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

pub fn compile_cards(cards: &Vec<CardInfo>, output: Arc<OutputManager>) {
    if cards.is_empty() {
        return;
    }

    let mut base = String::new();
    let mut base_length: usize = 0;
    let mut current_file_path = String::new();

    let cfg = config::get();
    // todo!("Change how source is done, especially with the path it's at. See how Source::detached is done");
    let mut world = typst_as_library::TypstWrapperWorld::new(
        cfg.path.to_string_lossy().into_owned(),
        base.to_owned(),
    );

    let mut ra: Range<usize> = 0..0;

    for card in cards {
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
            println!("Switched to new source file: {}", current_file_path);
        }

        world.source.edit(ra.clone(), &card.content);

        let last = world.source.text().len();
        ra = base_length..last;

        let document: PagedDocument = typst::compile(&world)
            .output
            .expect("Error compiling typst");

        let render = typst_render::render(document.pages.first().unwrap(), 2.0);
        let png = render.encode_png().expect("Failed to encode PNG");

        output.send(OutputMessage::CompiledCard {
            relative_file: current_file_path.clone(),
            card_id: format!("{} ; bytes: {}", card.card_id, png.len()),
            card_status: card.modification_status.clone(),
        });

        // std::fs::write(
        //     format!("/tmp/image_{}.png", card.card_id),
        //     png,
        // )
        // .expect("Failed to write /tmp/image.png");
    }
}
