use std::{collections::HashMap, path::PathBuf};

use crate::{card_wrapper::TypFileStats, config, utils};
use std::io::{self, Write};

pub enum OutputMessage {
    DbgFoundTypstFiles(HashMap<PathBuf, TypFileStats>),
    DbgShowConfig(config::Config),
    DbgConfigChangeDetection {
        total_cards: usize,
        config_changes: usize,
    },
    DbgCreateDeck(String),
    ParsingError(String),
    NoAnkiConnection,
}

pub struct OutputManager {}

impl OutputManager {
    pub fn new() -> Self {
        OutputManager {}
    }

    pub fn send(&self, msg: OutputMessage) {
        self.print(msg);
    }

    pub fn ask_yes_no(&self, _question: &str) -> bool {
        {
            loop {
                print!("{} [Y/n]: ", _question);
                let _ = io::stdout().flush();
                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_err() {
                    return false;
                }
                match input.trim().to_lowercase().as_str() {
                    "y" | "yes" | "" => return true,
                    "n" | "no" => return false,
                    _ => println!("Please answer 'y' or 'n'."),
                }
            }
        }
    }

    fn print(&self, msg: OutputMessage) {
        match msg {
            OutputMessage::DbgFoundTypstFiles(files) => {
                println!("Found {} .typ files: {:?}", files.len(), files);
            }
            OutputMessage::DbgShowConfig(cfg) => {
                println!("Current Configuration: {:#?}", cfg);
            }
            OutputMessage::DbgConfigChangeDetection {
                total_cards,
                config_changes,
            } => {
                println!(
                    "Configuration Change Detection: {} cards checked, {} configuration changes detected.",
                    total_cards, config_changes
                );
            }
            OutputMessage::DbgCreateDeck(deck_name) => {
                println!("Creating deck: {}", deck_name);
            }
            OutputMessage::ParsingError(err) => {
                eprintln!("Parsing Error: {}", err);
            }
            OutputMessage::NoAnkiConnection => {
                utils::print_header (
                    &[
                        "Anki couldn't be detected.",
                        "Please make sure Anki is running and the AnkiConnect add-on is installed.",
                        "For more information about installing AnkiConnect, please see typ2anki's README",
                    ],
                    0,
                    '=',
                );
            }
        }
    }

    pub fn fail(&self) {
        std::process::exit(1);
    }
}
