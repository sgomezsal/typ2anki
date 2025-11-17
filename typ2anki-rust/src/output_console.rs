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
    files: Mutex<Option<Arc<Mutex<HashMap<PathBuf, TypFileStats>>>>>,
}

const PROGRESS_BAR_LENGTH: u64 = 40;

impl OutputConsole {
    pub fn new() -> Self {
        Self {
            multi: Arc::new(MultiProgress::new()),
            bars: Arc::new(Mutex::new(HashMap::new())),
            bars_visible: Arc::new(Mutex::new(false)),
            files: Mutex::new(None),
        }
    }

    fn create_progress_bars(&self, files: Arc<Mutex<HashMap<PathBuf, TypFileStats>>>) {
        let mut stored_files = self.files.lock().unwrap();
        *stored_files = Some(files.clone());

        let files = files.lock().unwrap();
        {
            let mut visible = self.bars_visible.lock().unwrap();
            *visible = true;
        }

        let mut bars = self.bars.lock().unwrap();

        let cfg = config::get();

        let files_sorted: Vec<(&PathBuf, &TypFileStats)> = {
            let mut v: Vec<(&PathBuf, &TypFileStats)> = files.iter().collect();
            v.sort_by_key(|(path, _)| path.to_string_lossy().to_string());
            v
        };

        let filenames: Vec<String> = files_sorted
            .iter()
            .map(|(path, _)| {
                path.strip_prefix(&cfg.path)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        let longest_path = filenames.iter().map(|p| p.len()).max().unwrap_or(20) as u64;
        let longest_count = files
            .values()
            .map(|stats| stats.total_cards)
            .max()
            .unwrap_or(0) as u64;
        let longest_count = longest_count.to_string().len() as u64;

        // Create a total progress bar
        {
            let total_cards: u64 = files.values().map(|s| s.total_cards as u64).sum();
            let longest_pos = total_cards.to_string().len() as u64;

            let pb = self.multi.add(ProgressBar::new(total_cards));
            let format = format!(
                "{{prefix}} [{{wide_bar:.red/green}}] {{pos:>{}}}/{{len:<{}}} {{per_sec:<2}} ETA: {{eta}}",
                longest_pos, longest_pos
            );
            pb.set_style(
                ProgressStyle::with_template(format.as_str())
                    .unwrap()
                    .progress_chars("##-"),
            );
            pb.set_prefix("All:");
            bars.insert("all".to_string(), pb);
        }

        for ((path, stats), filename) in files_sorted.iter().zip(filenames.iter()) {
            let pb = self.create_progress_bar(
                filename,
                longest_path + 1,
                longest_count,
                stats.total_cards as u64,
            );
            bars.insert(path.to_string_lossy().to_string(), pb);
        }
    }

    fn create_progress_bar(
        &self,
        file_name: &str,
        prefix_length: u64,
        longest_pos: u64,
        len: u64,
    ) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(len));
        let format = format!(
            "{{prefix:{}}} [{{bar:{}.cyan/blue}}] {{pos:>{}}}/{{len:<{}}} {{msg}}",
            prefix_length, PROGRESS_BAR_LENGTH, longest_pos, longest_pos
        );
        pb.set_style(
            ProgressStyle::with_template(format.as_str())
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_prefix(file_name.to_string());
        pb.set_message("");
        pb
    }

    fn progress_on_bar(&self, file_name: &str, inc: u64) {
        let bars = self.bars.lock().unwrap();
        if let Some(pb) = bars.get(file_name) {
            pb.inc(inc);
            if pb.position() >= pb.length().unwrap_or(0) {
                let stored_files = self.files.lock().unwrap();
                let stored_files = stored_files.as_ref().unwrap().lock().unwrap();
                let stats = stored_files
                    .iter()
                    .find(|(path, _)| path.to_string_lossy() == *file_name)
                    .map(|(_, stats)| stats)
                    .unwrap();
                pb.finish_with_message(stats.stats_colored());
            }
            bars.get("all").unwrap().inc(inc);
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
        let files = files.lock().unwrap();
        for (file, pb) in bars.iter() {
            if !pb.is_finished() {
                if file == "all" {
                    pb.finish();
                    continue;
                }
                let stats = files
                    .iter()
                    .find(|(path, _)| path.to_string_lossy() == *file)
                    .map(|(_, stats)| stats)
                    .unwrap();
                pb.finish_with_message(stats.stats_colored());
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
                self.println("".to_string());
                self.print_separator();
            }
            OutputMessage::DbgDone => {}
        }
    }
}
