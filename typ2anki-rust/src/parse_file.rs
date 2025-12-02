use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use regex::Regex;

use crate::{
    card_wrapper::{CardInfo, TypFileStats},
    cards_cache::CardsCacheManager,
    config,
    output::{OutputManager, OutputMessage},
    utils,
};

const DEFAULT_ANKICONF: &'static str = "#let conf(
  doc,
) = {
  doc
}";

pub fn check_ankiconf_exists() {
    let cfg = config::get();
    let ankiconf_path = cfg.path.join("ankiconf.typ");
    if !ankiconf_path.exists() {
        std::fs::write(&ankiconf_path, DEFAULT_ANKICONF).expect("Failed to create ankiconf.typ");
    }
}

pub fn is_card_empty(card_str: &str) -> bool {
    static Q_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"q:\s*(\[\s*\]|"\s*")"#).unwrap());
    static A_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"a:\s*(\[\s*\]|"\s*")"#).unwrap());

    Q_RE.is_match(card_str) && A_RE.is_match(card_str)
}

pub fn get_ankiconf_hash() -> String {
    let cfg = config::get();
    let ankiconf_path = cfg.path.join("ankiconf.typ");
    if !ankiconf_path.exists() {
        return String::new();
    }
    let mut content = std::fs::read_to_string(ankiconf_path).unwrap_or(String::new());
    let imports = utils::get_all_typst_imports(content.as_str());

    for import in imports {
        if let Ok(import_content) = std::fs::read_to_string(&import) {
            content.push_str("\n");
            content.push_str(&import_content);
        }
    }

    utils::hash_string(&content)
}

#[cfg(feature = "tree-sitter")]
mod parse_card_tree_sitter {
    use crate::card_wrapper::BarebonesCardInfo;

    use super::*;
    use std::sync::Mutex;

    use once_cell::sync::OnceCell;
    use tree_sitter::{Node, Parser};
    use tree_sitter_typst;

    static TS_PARSER: OnceCell<Mutex<Parser>> = OnceCell::new();
    static VALUE_TRIM_CHARS: &[char] = &['"', ' ', '\n', '\t', '\r', '[', ']', ':'];

    fn get_tagged_argument_value(source: &[u8], node: &Node, arg_name: &str) -> Option<String> {
        let mut cursor = node.walk();

        let arguments_node = node
            .child_by_field_name("arguments")
            .or_else(|| node.child_by_field_name("group"))
            .or_else(|| node.named_child(1));

        let arguments_node = arguments_node?;

        for child in arguments_node.named_children(&mut cursor) {
            if child.kind() == "tagged" {
                if let Some(field_node) = child.child_by_field_name("field") {
                    let field_name = field_node.utf8_text(source).ok()?;
                    if field_name == arg_name {
                        if let Some(value_node) = child.child(2).or_else(|| child.child(1)) {
                            return value_node.utf8_text(source).ok().map(|s| s.to_string());
                        }
                    }
                }
            }
        }

        None
    }

    fn get_function_from_call_node<'a>(
        source: &[u8],
        node: Node<'a>,
        function_name: &str,
    ) -> Option<Node<'a>> {
        if let Some(item) = node.child_by_field_name("item") {
            if item.kind() == "identifier" || item.kind() == "ident" {
                let name = item.utf8_text(source).unwrap();
                if name == function_name {
                    return Some(node);
                }
            }
        }
        None
    }

    pub fn parse_cards_string(
        content: &str,
        output: &Arc<impl OutputManager + 'static>,
    ) -> Vec<String> {
        let cfg = config::get();

        let mut ts_parser = TS_PARSER
            .get_or_init(|| {
                let mut parser = Parser::new();
                parser
                    .set_language(tree_sitter_typst::language())
                    .expect("Error loading typst grammar");
                Mutex::new(parser)
            })
            .lock()
            .unwrap();

        let tree = match ts_parser.parse(content, None) {
            Some(t) => t,
            None => return vec![],
        };
        let source = content.as_bytes();
        let mut cursor = tree.root_node().walk();
        let mut calls = Vec::new();
        let mut stack = vec![tree.root_node()];

        let handle_card_node =
            |func_call: Node, parent: &Node| -> Result<BarebonesCardInfo, &str> {
                macro_rules! ga {
                    ($tag:expr) => {
                        get_tagged_argument_value(source, parent, $tag)
                            .map(|s| s.trim_matches(VALUE_TRIM_CHARS).to_string())
                            .filter(|s| s != "")
                    };
                }

                let id = ga!("id").ok_or("Couldn't parse id")?;
                let target_deck = ga!("target-deck").ok_or("Couldn't parse target-deck")?;

                Ok(BarebonesCardInfo {
                    card_id: id,
                    deck_name: target_deck,
                    question: ga!("q").unwrap_or(String::new()),
                    answer: ga!("a").unwrap_or(String::new()),
                    byte_range: (func_call.start_byte(), func_call.end_byte()),
                })
            };

        // Checks that the call_node isn't being defined in a let statement
        let check_isnt_let = |call_node: &Node| {
            if let Some(parent) = call_node.parent() {
                if parent.kind() == "let" {
                    return false;
                }
            }
            true
        };

        while let Some(call_node) = stack.pop() {
            if call_node.kind() == "call" && check_isnt_let(&call_node) {
                if let Some(item) = get_function_from_call_node(source, call_node, "custom-card") {
                    match handle_card_node(item, &call_node) {
                        Err(e) => {
                            let id = get_tagged_argument_value(source, &call_node, "id")
                                .map(|s| s.trim_matches(VALUE_TRIM_CHARS).to_string())
                                .unwrap_or("unknown_id".to_string());
                            output.send(OutputMessage::ParsingError(if !cfg.dry_run {
                                format!("Warning: Failed to parse custom-card (id: {}): {}", id, e)
                            } else {
                                format!(
                                    "Failed to parse custom-card (id: {}): {}\n{}",
                                    id,
                                    e,
                                    call_node.utf8_text(source).unwrap_or("unknown_content")
                                )
                            }));
                        }
                        Ok(c) => {
                            calls.push(c.byte_range);
                            println!("Card: {:?}", c);
                        }
                    }
                }
            }

            for child in call_node.children(&mut cursor) {
                stack.push(child);
            }
        }

        // println!("Function calls found: {:?}", calls);

        // if !calls.is_empty() {
        //     let first_call = calls[0];
        //     println!(
        //         "First call from {} to {}: {}",
        //         first_call.0,
        //         first_call.1,
        //         &content[first_call.0..first_call.1]
        //     );
        // }

        vec![]
    }
}

#[cfg(not(feature = "tree-sitter"))]
mod parse_card_fallback {
    pub fn parse_cards_string(content: &str) -> Vec<String> {
        let mut results: Vec<String> = Vec::new();
        let card_types = ["#card(", "#custom-card("];

        let mut inside_card = false;
        let mut balance: i32 = 0;
        let mut current_card = String::new();
        let mut i: usize = 0;
        let len = content.len();

        let mut current_prelude = String::new();
        let mut prelude_started = false;

        while i < len {
            if !inside_card {
                if card_types.iter().any(|ct| content[i..].starts_with(ct)) {
                    inside_card = true;
                    for ct in &card_types {
                        if content[i..].starts_with(ct) {
                            balance = 1;
                            current_card.clear();
                            current_card.push_str(ct);
                            i += ct.len();
                            break;
                        }
                    }
                    continue;
                }

                if !prelude_started {
                    if content[i..].starts_with("//START") || content[i..].starts_with("//start") {
                        prelude_started = true;
                        i += "//START".len();
                        continue;
                    } else if content[i..].starts_with("// START")
                        || content[i..].starts_with("// start")
                    {
                        prelude_started = true;
                        i += "// START".len();
                        continue;
                    }
                }
            }

            if inside_card {
                let ch = content[i..].chars().next().unwrap();
                current_card.push(ch);
                if ch == '(' {
                    balance += 1;
                } else if ch == ')' {
                    balance -= 1;
                }
                i += ch.len_utf8();

                if balance == 0 {
                    results.push(format!(
                        "{}\n{}",
                        current_prelude.trim(),
                        current_card.trim()
                    ));
                    inside_card = false;
                    current_card.clear();
                }
                continue;
            }

            // Not inside a card and prelude only tracked after marker found
            let ch = content[i..].chars().next().unwrap();
            if prelude_started {
                if ch == '\n' && current_prelude.ends_with('\n') {
                    // skip duplicate new line``
                } else {
                    current_prelude.push(ch);
                }
            }
            i += ch.len_utf8();
        }

        results
    }
}

#[cfg(not(feature = "tree-sitter"))]
pub use parse_card_fallback::parse_cards_string;
#[cfg(feature = "tree-sitter")]
pub use parse_card_tree_sitter::parse_cards_string;

pub fn parse_cards_from_file_content(
    filepath: &PathBuf,
    content: String,
    cards_cache_manager: &mut CardsCacheManager,
    output: Arc<impl OutputManager + 'static>,
    i: &mut i64,
    deck_names: &mut HashSet<String>,
    cards: &mut Vec<CardInfo>,
) -> Result<TypFileStats, String> {
    let cfg = config::get();

    let mut file = TypFileStats::new(filepath.clone());
    let parsed = parse_cards_string(&content, &output);
    if parsed.len() == 0 {
        return Ok(file);
    }

    for card_str in parsed.into_iter() {
        if is_card_empty(&card_str) {
            file.empty_cards += 1;
            continue;
        }

        match CardInfo::from_string(*i, &card_str, filepath.clone()) {
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
                *i += 1;
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
    Ok(file)
}
