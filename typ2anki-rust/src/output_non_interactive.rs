use crate::{output::*, utils};

pub struct OutputNonInteractive {}

impl OutputNonInteractive {
    pub fn new() -> Self {
        Self {}
    }

    fn print_separator(&self) {
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&w| w > 0)
            .unwrap_or(80);
        let s = "=".repeat(width);
        println!("{s}");
    }
}

impl OutputManager for OutputNonInteractive {
    fn send(&self, msg: OutputMessage) {
        match msg {
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
                utils::print_header(
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
            OutputMessage::ListTypstFiles(_) => {
                self.print_separator();
            }
            OutputMessage::CompileError(OutputCompiledCardInfo {
                card_id,
                file: relative_file,
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
                file: relative_file,
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
            OutputMessage::PushedCard(OutputCompiledCardInfo {
                file,
                card_id,
                card_status,
                ..
            }) => {
                println!(
                    "Compiled and pushed card ID {} from file {} ({})",
                    card_id, file, card_status
                );
            }
            OutputMessage::SkipCompileCard(OutputCompiledCardInfo { .. }) => {}
            OutputMessage::CompiledCard(OutputCompiledCardInfo { .. }) => {}
            OutputMessage::DbgCompilationDone { files } => {
                self.print_separator();
                for (file, stats) in files.read().unwrap().iter() {
                    println!("{}: {}", file.to_string_lossy(), stats.stats_colored());
                }
                self.print_separator();
            }
            OutputMessage::TypstDownloadingPackage(pkg) => {
                println!("Downloading Typst package: {}", pkg);
            }
            OutputMessage::Fail(reason) => {
                if let Some(r) = reason {
                    eprintln!("Fail reason: {}", r);
                }
                std::process::exit(1);
            }
            OutputMessage::DbgDone => {}
        }
    }

    fn ask_yes_no(&self, _: &str, default_answer: bool) -> bool {
        default_answer
    }

    fn fail(&self) {
        self.send(OutputMessage::Fail(None));
    }

    fn fail_with_reason(&self, reason: String) {
        self.send(OutputMessage::Fail(Some(reason)));
    }
}
