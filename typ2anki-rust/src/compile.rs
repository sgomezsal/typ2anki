use typst::layout::PagedDocument;

use crate::{config, typst_as_library};

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
    std::fs::write("/tmp/image1.png", png).expect("Failed to write /tmp/image1.png");

    world.source.edit(25..25, "\nfeur coubeh");

    let document: PagedDocument = typst::compile(&world)
        .output
        .expect("Error compiling typst");

    let render = typst_render::render(document.pages.first().unwrap(), 2.0);
    let png = render.encode_png().expect("Failed to encode PNG");
    std::fs::write("/tmp/image2.png", png).expect("Failed to write /tmp/image2.png");
}
