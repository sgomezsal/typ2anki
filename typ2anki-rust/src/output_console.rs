use std::{
    collections::HashMap,
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{card_wrapper::TypFileStats, config, output::*, utils};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub struct OutputConsole {
    multi: Arc<MultiProgress>,
    bars: Arc<Mutex<HashMap<String, ProgressBar>>>,
    bars_visible: Arc<Mutex<bool>>,
}

const PROGRESS_BAR_LENGTH: u64 = 40;

impl OutputConsole {
    pub fn new() -> Self {
        Self {
            multi: Arc::new(MultiProgress::new()),
            bars: Arc::new(Mutex::new(HashMap::new())),
            bars_visible: Arc::new(Mutex::new(false)),
        }
    }

    fn create_progress_bars(&self, files: Arc<Mutex<HashMap<PathBuf, TypFileStats>>>) {
        let files = files.lock().unwrap();
        {
            let mut visible = self.bars_visible.lock().unwrap();
            *visible = true;
        }

        let mut bars = self.bars.lock().unwrap();

        let cfg = config::get();
        let filenames: Vec<String> = files
            .keys()
            .map(|path| {
                path.strip_prefix(&cfg.path)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        let longest_path = filenames.iter().map(|p| p.len()).max().unwrap_or(20) as u64;

        for ((path, stats), filename) in files.iter().zip(filenames.iter()) {
            println!("{}", stats.stats_colored());
            let pb = self.create_progress_bar(filename, longest_path + 1, stats.total_cards as u64);
            bars.insert(path.to_string_lossy().to_string(), pb);
        }
    }

    fn create_progress_bar(&self, file_name: &str, msg_length: u64, len: u64) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(len));
        let format = format!(
            "{{msg:{}}} [{{bar:{}.cyan/blue}}] {{pos}}/{{len}}",
            msg_length, PROGRESS_BAR_LENGTH
        );
        pb.set_style(
            ProgressStyle::with_template(format.as_str())
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message(file_name.to_string());
        pb
    }

    fn progress_on_bar(&self, file_name: &str, inc: u64) {
        let bars = self.bars.lock().unwrap();
        if let Some(pb) = bars.get(file_name) {
            pb.inc(inc);
        }
    }

    fn println(&self, s: String) {
        let visible = self.bars_visible.lock().unwrap();
        if *visible {
            let _ = self.multi.println(s);
        } else {
            println!("{}", s);
        }
    }

    fn print_separator(&self) {
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&w| w > 0)
            .unwrap_or(80);
        self.println("=".repeat(width));
    }

    fn finish_all_bars(&self, files: Arc<Mutex<HashMap<PathBuf, TypFileStats>>>) {
        let bars = self.bars.lock().unwrap();
        for pb in bars.values() {
            if !pb.is_finished() {
                pb.finish();
            }
        }
        {
            let mut visible = self.bars_visible.lock().unwrap();
            *visible = false;
        }
    }
}
impl OutputManager for OutputConsole {
    fn ask_yes_no(&self, _question: &str) -> bool {
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

    fn fail(&self) {
        std::process::exit(1);
    }

    fn send(&self, msg: OutputMessage) {
        let cfg = config::get();
        match msg {
            OutputMessage::ListTypstFiles(files) => {
                self.print_separator();
                self.create_progress_bars(files);
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
                file: relative_file,
                ..
            }) => {
                self.progress_on_bar(&relative_file, 1);
            }
            OutputMessage::CompiledCard(OutputCompiledCardInfo { .. }) => {}
            OutputMessage::PushedCard(OutputCompiledCardInfo {
                file: relative_file,
                ..
            }) => {
                self.progress_on_bar(&relative_file, 1);
            }
            OutputMessage::CompileError(OutputCompiledCardInfo {
                card_id,
                file: relative_file,
                card_status,
                error_message,
            }) => {
                self.println(format!(
                    "Error compiling card ID {} from file {} with status {:?}: {}",
                    card_id,
                    relative_file,
                    card_status,
                    error_message.unwrap_or("Unknown error".to_string())
                ));
                self.progress_on_bar(&relative_file, 1);
            }
            OutputMessage::PushError(OutputCompiledCardInfo {
                card_id,
                file: relative_file,
                card_status,
                error_message,
            }) => {
                self.println(format!(
                    "Error pushing card to anki: ID {} from file {} with status {:?}: {}",
                    card_id,
                    relative_file,
                    card_status,
                    error_message.unwrap_or("Unknown error".to_string())
                ));
                self.progress_on_bar(&relative_file, 1);
            }
            OutputMessage::DbgCompilationDone { files } => {
                self.finish_all_bars(files);
                self.print_separator();
            }
        }
    }
}
