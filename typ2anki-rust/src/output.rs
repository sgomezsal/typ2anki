use std::{collections::HashMap, path::PathBuf};

use crate::{
    card_wrapper::{CardInfo, CardModificationStatus, TypFileStats},
    config, utils,
};
use std::io::{self, Write};

pub struct OutputCompiledCardInfo {
    pub relative_file: String,
    pub card_id: String,
    pub card_status: CardModificationStatus,
    pub error_message: Option<String>,
}

impl OutputCompiledCardInfo {
    pub fn build(card: &CardInfo, error_message: Option<String>) -> Self {
        OutputCompiledCardInfo {
            relative_file: card.source_file.to_string_lossy().into_owned(),
            card_id: card.card_id.clone(),
            card_status: card.modification_status.clone(),
            error_message,
        }
    }
}

pub enum OutputMessage {
    DbgFoundTypstFiles(HashMap<PathBuf, TypFileStats>),
    DbgShowConfig(config::Config),
    DbgConfigChangeDetection {
        total_cards: usize,
        config_changes: usize,
    },
    DbgCreateDeck(String),
    DbgSavedCache,
    ParsingError(String),
    SkipCompileCard(OutputCompiledCardInfo),
    CompileError(OutputCompiledCardInfo),
    PushError(OutputCompiledCardInfo),
    CompiledCard(OutputCompiledCardInfo),
    PushedCard(OutputCompiledCardInfo),
    NoAnkiConnection,
    ErrorSavingCache(String),
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
            OutputMessage::DbgSavedCache => {
                println!("Cards cache saved successfully.");
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
            OutputMessage::ErrorSavingCache(e) => {
                eprintln!("Error saving cards cache: {}", e);
            }
            OutputMessage::SkipCompileCard(OutputCompiledCardInfo {
                card_id,
                relative_file,
                card_status,
                error_message: _,
            }) => {
                println!(
                    "Skipping compilation of card ID {} from file {} with status {:?}",
                    card_id, relative_file, card_status
                );
            }
            OutputMessage::CompiledCard(OutputCompiledCardInfo {
                card_id,
                relative_file,
                card_status,
                error_message: _,
            }) => {
                println!(
                    "Compiled card ID {} from file {} with status {:?}",
                    card_id, relative_file, card_status
                );
            }
            OutputMessage::PushedCard(OutputCompiledCardInfo {
                card_id,
                relative_file,
                card_status,
                error_message: _,
            }) => {
                println!(
                    "Pushed card ID {} from file {} with status {:?} to Anki",
                    card_id, relative_file, card_status
                );
            }
            OutputMessage::CompileError(OutputCompiledCardInfo {
                card_id,
                relative_file,
                card_status,
                error_message,
            }) => {
                println!(
                    "Error compiling card ID {} from file {} with status {:?}: {}",
                    card_id,
                    relative_file,
                    card_status,
                    error_message.unwrap_or("Unknown error".to_string())
                );
            }
            OutputMessage::PushError(OutputCompiledCardInfo {
                card_id,
                relative_file,
                card_status,
                error_message,
            }) => {
                println!(
                    "Error pushing card to anki: ID {} from file {} with status {:?}: {}",
                    card_id,
                    relative_file,
                    card_status,
                    error_message.unwrap_or("Unknown error".to_string())
                );
            }
        }
    }

    pub fn fail(&self) {
        std::process::exit(1);
    }
}
