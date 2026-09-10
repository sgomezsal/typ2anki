use crate::{card_wrapper::CardInfo, config};

pub fn generate_card_file_content(ankiconf_relative_path: String, card_content: String) -> String {
    let cfg = config::get();

    // display_with_width: different when max_card_width == "auto"
    let display_with_width = if cfg.max_card_width == "auto" {
        r#"#let display_with_width(body) = {
  body
}"#
        .to_string()
    } else {
        format!(
            r#"#let display_with_width(body) = {{
  layout(size => {{
    let (width,) = measure(body)
    if width > {max} {{
      width = {max}
    }} else {{
      width = auto
    }}
    context[
      #block(width: width,body)
    ]
  }})
}}"#,
            max = cfg.max_card_width
        )
    };

    // page_configuration (empty for html output_type)
    let page_configuration = if cfg.output_type == "html" {
        "".to_string()
    } else {
        format!(
            r#"#set page(
  width: auto,
  height: auto,
  margin: 3pt,
  fill: {},
)
"#,
            cfg.resolve_background_color(),
        )
        .to_string()
    };

    // Assemble template by concatenation to avoid format-brace escaping
    let mut template = String::new();
    template.push_str(&format!(
        "#import \"{}\": *\n#show: doc => conf(doc)\n\n",
        ankiconf_relative_path
    ));
    if !page_configuration.is_empty() {
        template.push_str(&page_configuration);
        template.push_str("\n\n");
    } else {
        template.push('\n');
    }

    if cfg.foreground_color.is_some() {
        template.push_str(&format!(
            "#set text(fill: {})\n//#show math.equation: set text(fill: {})\n",
            cfg.resolve_foreground_color(),
            cfg.resolve_foreground_color(),
        ));
    }

    template.push_str(&display_with_width);
    template.push_str("\n\n");

    let cardlet = r#"#let card(
      id: "",
      q: "",
      a: "",
      ..args
    ) = {
      let args = arguments(..args, type: "basic")
      if args.at("type") == "basic" {
        context[
          #display_with_width(q)
          #pagebreak()
          #display_with_width(a)
        ]
      }
    }
    #let custom-card = card
    "#
    .to_string();
    template.push_str(&cardlet);
    template.push_str("\n\n");

    template.push_str(&card_content);
    template
}

#[allow(dead_code)]
pub fn generate_card_file(card: &CardInfo) -> String {
    println!(
        "Generating card file for card ID {} at {}",
        card.card_id,
        card.relative_ankiconf_path()
    );

    generate_card_file_content(card.relative_ankiconf_path(), card.content.clone())
}
