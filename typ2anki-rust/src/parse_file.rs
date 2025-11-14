use std::sync::LazyLock;

use regex::Regex;

use crate::{config, utils};

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

pub fn parse_cards_string(content: &str) -> Vec<String> {
    let mut results: Vec<String> = Vec::new();
    let card_types = ["#card(", "#custom-card("];

    let mut inside_card = false;
    let mut balance: i32 = 0;
    let mut current_card = String::new();
    let mut i: usize = 0;
    let len = content.len();

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
        }

        if inside_card {
            // get next char safely and advance by its byte length
            let ch = content[i..].chars().next().unwrap();
            current_card.push(ch);
            if ch == '(' {
                balance += 1;
            } else if ch == ')' {
                balance -= 1;
            }
            i += ch.len_utf8();

            if balance == 0 {
                results.push(current_card.trim().to_string());
                inside_card = false;
                current_card.clear();
            }
            continue;
        }

        // not inside a card and no start matched: advance one char
        let ch = content[i..].chars().next().unwrap();
        i += ch.len_utf8();
    }

    results
}
