use typst::layout::PagedDocument;

use crate::{card_wrapper::CardInfo, config, generator, typst_as_library};

pub fn compile_png_base64(typst_content: String) {
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
}

pub fn compile_cards(cards: &Vec<CardInfo>) {
    let base = generator::generate_card_file_content("ankiconf.typ".to_string(), "".to_string());
    let base_length = base.len();

    let cfg = config::get();
    todo!("Change how source is done, especially with the path it's at. See how Source::detached is done");
    let mut world = typst_as_library::TypstWrapperWorld::new(
        cfg.path.to_string_lossy().into_owned(),
        base.to_owned(),
    );

    let mut ra = base_length..base_length;

    for card in cards {
        world.source.edit(ra.clone(), &card.content);

        let last = world.source.text().len();
        ra = base_length..last;

        let document: PagedDocument = typst::compile(&world)
            .output
            .expect("Error compiling typst");

        let render = typst_render::render(document.pages.first().unwrap(), 2.0);
        let png = render.encode_png().expect("Failed to encode PNG");

        println!(
            "Compiled card ID {} to PNG ({} bytes)",
            card.card_id,
            png.len()
        );

        // std::fs::write(
        //     format!("/tmp/image_{}.png", card.card_id),
        //     png,
        // )
        // .expect("Failed to write /tmp/image.png");
    }
}
