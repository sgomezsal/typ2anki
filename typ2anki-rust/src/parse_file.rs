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

const DEFAULT_ANKICONF: &str = "#let conf(
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

pub static QUESTION_EMPTY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"q:\s*(\[\s*\]|"\s*")"#).unwrap());
pub static ANSWER_EMPTY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"a:\s*(\[\s*\]|"\s*")"#).unwrap());

pub static ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"id:\s*"([^"]*)""#).unwrap());
pub static DECK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"target-deck:\s*"([^"]+)""#).unwrap());
pub static QUESTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"q:\s*(\[(?:.|\n)*\]|"(?:.|\n)*")"#).unwrap());
pub static ANSWER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"a:\s*(\[(?:.|\n)*\]|"(?:.|\n)*")"#).unwrap());

pub fn is_card_empty(card_str: &str) -> bool {
    QUESTION_EMPTY_RE.is_match(card_str) && ANSWER_EMPTY_RE.is_match(card_str)
}

pub fn get_ankiconf_hash() -> String {
    let cfg = config::get();
    let ankiconf_path = cfg.path.join("ankiconf.typ");
    if !ankiconf_path.exists() {
        return String::new();
    }
    let mut content = std::fs::read_to_string(ankiconf_path).unwrap_or_default();
    let imports = utils::get_all_typst_imports(content.as_str());

    for import in imports {
        if let Ok(import_content) = std::fs::read_to_string(&import) {
            content.push('\n');
            content.push_str(&import_content);
        }
    }

    utils::hash_string(&content)
}

#[cfg(feature = "tree-sitter")]
mod parse_card_tree_sitter {
    use super::*;
    use crate::card_wrapper::BarebonesCardInfo;

    use std::sync::Mutex;

    use once_cell::sync::OnceCell;
    use tree_sitter::{Node, Parser};

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
        _no_prelude: bool,
    ) -> Vec<String> {
        let cfg = config::get();
        const CARD_FUNCTION_NAME: &str = "custom-card";

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
        let mut cards = Vec::new();

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
                    prelude_range: None,
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

        let mut push_hashtag = false;
        let mut prelude = String::new();
        let mut previous_was_card = false;
        for call_node in tree.root_node().children(&mut cursor) {
            if call_node.kind() == "call" {
                if !check_isnt_let(&call_node) {
                    continue;
                }
                if let Some(item) =
                    get_function_from_call_node(source, call_node, CARD_FUNCTION_NAME)
                {
                    push_hashtag = false;
                    previous_was_card = true;
                    match handle_card_node(item, &call_node) {
                        Err(e) => {
                            let id = get_tagged_argument_value(source, &call_node, "id")
                                .map(|s| s.trim_matches(VALUE_TRIM_CHARS).to_string())
                                .unwrap_or("unknown_id".to_string());
                            output.send(OutputMessage::ParsingError(if !cfg.dry_run {
                                format!(
                                    "Warning: Failed to parse {CARD_FUNCTION_NAME} (id: {id}): {}",
                                    e
                                )
                            } else {
                                format!(
                                    "Failed to parse {CARD_FUNCTION_NAME} (id: {id}): {e}\n{}",
                                    call_node.utf8_text(source).unwrap_or("unknown_content")
                                )
                            }));
                        }
                        Ok(mut c) => {
                            c.prelude_range = Some(0..prelude.len());
                            cards.push(c);
                            // println!("Card: {:?}", c);
                        }
                    }
                }
            } else {
                if call_node.kind() == "let" {
                    if let Some(func_name) = call_node
                        .children(&mut call_node.walk())
                        .find(|s| s.kind() == "call")
                        .map(|n| n.child_by_field_name("item"))
                        .flatten()
                        .filter(|n| n.kind() == "identifier" || n.kind() == "ident")
                        .map(|n| n.utf8_text(source).ok())
                        .flatten()
                    {
                        if func_name == CARD_FUNCTION_NAME {
                            push_hashtag = false;
                            continue;
                        }
                    }
                } else if call_node.kind() == "import" {
                    if let Some(p) = call_node
                        .child_by_field_name("import")
                        .map(|n| n.utf8_text(source).ok())
                        .flatten()
                        .map(|s| s.trim_matches(VALUE_TRIM_CHARS).to_string())
                    {
                        if p.ends_with("ankiconf.typ") {
                            push_hashtag = false;
                            continue;
                        }
                    }
                } else if call_node.kind() == "show" {
                    if let Some(p) = call_node
                        .child_by_field_name("value")
                        .map(|n| n.utf8_text(source).ok())
                        .flatten()
                        .map(|s| s.trim_matches(VALUE_TRIM_CHARS).to_string())
                    {
                        if p.contains("conf(doc)") {
                            push_hashtag = false;
                            continue;
                        }
                    }
                }

                if push_hashtag {
                    prelude.push_str("#");
                    push_hashtag = false;
                }

                match call_node.kind() {
                    "#" => {
                        push_hashtag = true;
                    }
                    "parbreak" => {
                        if !previous_was_card {
                            prelude.push_str("\n");
                        }
                    }
                    "end" => {
                        if !previous_was_card {
                            prelude.push_str("\n");
                        }
                    }
                    "comment" => {
                        previous_was_card = false;
                    }
                    _ => {
                        if let Some(s) = call_node.utf8_text(source).ok() {
                            prelude.push_str(s);
                        }
                        previous_was_card = false;
                    }
                }
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

        cards
            .into_iter()
            .map(|c| {
                let mut card_str = String::new();
                if let Some(prelude_range) = &c.prelude_range {
                    card_str.push_str(&prelude[prelude_range.start..prelude_range.end]);
                    card_str.push_str("\n");
                }
                card_str.push_str(&content[c.byte_range.0..c.byte_range.1]);
                card_str
            })
            .collect()
    }
}

#[cfg(not(feature = "tree-sitter"))]
mod parse_card_fallback {
    use super::*;

    use std::ops::Range;

    enum Parser {
        Scanning,
        InPrelude {
            prelude_start: usize
        },
        InCard {
            prelude: Option<Range<usize>>,
            card_start: usize,
            paren_depth: i32
        }
    }

    const CARD_TYPES: [&str; 2] = ["#card(", "#custom-card("];
    const PRELUDE_STARTS: [&str; 2] = ["START", "start"];

    /// Checks if the `content` string has a card identifier starting at byte index `i`
    /// If it does, this returns the byte index of the first character after the opening paren.
    fn parse_card_start(content: &str, i: usize) -> Option<usize> {
        for card in CARD_TYPES {
            if content[i..].starts_with(card) {
                return Some(i + card.len());
            }
        }
        None
    }

    /// Checks if the `content string has a line or block comment starting at byte index `i`
    /// If it does, this returns a range indicating the inside of the comment, and the byte index
    /// of the first character after the end of the comment (after the linefeed for line comments)
    fn parse_comment(content: &str, i: usize) -> Option<(Range<usize>, usize)> {
        let len = content.len();
        if content[i..].starts_with("//") { // Line comment
            // Get the index of the character after the comment's end
            let end = content[i..].find('\n').map(|end| end + i).unwrap_or(len);
            let next = content.ceil_char_boundary(end + 1); // `ceil_char_boundary` returns `len` if its argument overflows `len`

            Some((i+2..end, next))
        } else if content[i..].starts_with("/*") { // Block comment
            let end = content[i+2..].find("*/").map(|end| end + i+2).unwrap_or(len);
            let next = content.ceil_char_boundary(end + 2); // skip */
            Some(((i+2..end), next))
        } else { None }
    }

    pub fn parse_cards_string(
        content: &str,
        _: &Arc<impl OutputManager + 'static>,
        no_prelude: bool,
    ) -> Vec<String> {
        let mut results: Vec<String> = Vec::new();

        let mut state = Parser::Scanning;
        let mut i: usize = 0;
        let len = content.len();

        while i < len {
            match &mut state {
                Parser::Scanning => {
                    // Card start (no prelude)
                    if let Some(next) = parse_card_start(content, i) {
                        state = Parser::InCard { card_start: i, prelude: None, paren_depth: 1 };
                        i = next;
                    } else if let Some((comment_inside, next)) = parse_comment(content, i) { // Comment
                        // Check for prelude start:
                        let trimmed = content[comment_inside].trim_start();
                        if !no_prelude && PRELUDE_STARTS.iter().any(|start| trimmed.starts_with(start)) {
                            state = Parser::InPrelude { prelude_start: next };
                        }
                        i = next;
                    } 
                    else {
                        let ch = content[i..].chars().next().unwrap();
                        i += ch.len_utf8();
                    }
                }
                Parser::InPrelude { prelude_start } => {
                    // Card start (with prelude)
                    if let Some(next) = parse_card_start(content, i) {
                        state = Parser::InCard {
                            prelude: Some((*prelude_start)..i),
                            card_start: i,
                            paren_depth: 1
                        };
                        i = next;
                    } else { // We don't care about comments in prelude
                        let ch = content[i..].chars().next().unwrap();
                        i += ch.len_utf8();
                    }
                },
                Parser::InCard { prelude, card_start, paren_depth } => {
                    // Skip comments completely to avoid false parenthesises in comments
                    if let Some((_, next)) = parse_comment(content, i) {
                        i = next;
                        continue;
                    }

                    let ch = content[i..].chars().next().unwrap();
                    i += ch.len_utf8();

                    // Count parenthesis scope depth as we are reading the card
                    // Once we leave the last parenthesis, we're done reading the card
                    *paren_depth += match ch {
                        '(' => 1, ')' => -1, _ => 0
                    };

                    if *paren_depth == 0 {
                        let card_interval = (*card_start)..i;
                        let result = if let Some(prelude) = prelude {
                            format!("{}\n{}", &content[prelude.clone()], &content[card_interval])
                        } else {
                            format!("{}", &content[card_interval])
                        };

                        results.push(result);

                        state = Parser::Scanning;
                    }
                },
            }
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

    let start = std::time::Instant::now();
    let parsed = parse_cards_string(&content, &output, false);
    let _duration = start.elapsed();

    if parsed.is_empty() {
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
