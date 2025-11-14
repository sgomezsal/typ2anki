use crate::card_wrapper::CardInfo;

mod anki_api;
mod card_wrapper;
mod cards_cache;
mod config;
mod parse_file;
mod utils;

fn main() {
    let cfg = config::get();
    let filepath = "/home/gm/repos/typ2anki-fork/examples/main.typ";
    let content = std::fs::read_to_string(filepath).expect("Failed to read file");
    let cards_strings = parse_file::parse_cards_string(&content);

    let cards: Vec<CardInfo> = cards_strings
        .iter()
        .enumerate()
        .map(|(i, s)| CardInfo::from_string(i.try_into().unwrap(), s, "main.typ").unwrap())
        .collect();
    println!("Found {} cards: {:#?}", cards.len(), cards);
}
