use regex::Regex;

use crate::card_wrapper::CardInfo;

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
