use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

use clap::parser::ValueSource;
use clap::{ArgAction, CommandFactory, FromArgMatches, Parser};
use glob::Pattern;
use once_cell::sync::OnceCell;
use serde_json::{json, Value};
use toml::Value as TomlValue;

use html_escape::encode_double_quoted_attribute;

use crate::card_wrapper::CardInfo;
use crate::utils;
use std::sync::{Arc, RwLock};

pub const DEFAULT_CONFIG_FILENAME: &str = "typ2anki.toml";

#[derive(Parser, Debug)]
#[command(about = "Typ2Anki config parser", version)]
struct Cli {
    /// Specify the path to the config file. Set to empty string to disable config file.
    #[arg(long = "config-file", default_value = DEFAULT_CONFIG_FILENAME)]
    config_file: String,

    /// Enable duplicate checking
    #[arg(long = "check-duplicates")]
    check_duplicates: bool,

    /// Specify decks to exclude. Use multiple -e options. Glob patterns supported.
    #[arg(short = 'e', long = "exclude-decks", action = clap::ArgAction::Append)]
    exclude_decks: Vec<String>,

    /// Specify files to exclude. Use multiple --exclude-files options. Glob patterns supported.
    #[arg(long = "exclude-files", action = clap::ArgAction::Append)]
    exclude_files: Vec<String>,

    /// Specify how many cards at a time can be generated. Needs duplicate checking enabled.
    #[arg(long = "generation-concurrency", default_value = "")]
    generation_concurrency: String,

    /// Max card width, 'auto' or a value
    #[arg(long = "max-card-width", default_value = "auto")]
    max_card_width: String,

    /// Force reupload of all images
    #[arg(long = "no-cache")]
    no_cache: bool,

    /// Whether to recompile cards if the config has changed. Accepts 'y' or 'n', or '_' to ask.
    #[arg(long = "recompile-on-config-change", default_value = "_")]
    recompile_on_config_change: String,

    /// Run without making changes
    #[arg(long = "dry-run")]
    dry_run: bool,

    /// Hidden: print config
    #[arg(long = "print-config", hide = true)]
    print_config: bool,

    /// Path to Typst documents folder or zip (positional, allow spaces)
    #[arg(value_parser, required=true, num_args = 0..)]
    path: Vec<String>,

    #[arg(short = 'i', hide = true,action = ArgAction::SetTrue)]
    keep_terminal_open: bool,
}

fn load_toml_config(path: &Path) -> Option<TomlValue> {
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(path) {
        Ok(s) => match s.parse::<TomlValue>() {
            Ok(v) => Some(v),
            Err(e) => panic!("Error parsing TOML {}: {}", path.display(), e),
        },
        Err(e) => panic!("Error reading config file {}: {}", path.display(), e),
    }
}

fn get_real_path_simple(p: &str) -> String {
    match fs::canonicalize(p) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => p.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    // User controlled options
    pub check_duplicates: bool,
    pub exclude_decks: Vec<Pattern>,
    pub exclude_decks_string: Vec<String>,
    pub exclude_files: Vec<Pattern>,
    pub asked_path: String,
    pub path: PathBuf,
    pub recompile_on_config_change: Arc<RwLock<Option<bool>>>,

    // Processed options / defaults
    pub dry_run: bool,
    pub max_card_width: String,
    pub skip_cache: bool,
    pub generation_concurrency: usize,
    pub keep_terminal_open: bool,

    // Internal options
    pub is_zip: bool,
    pub config_hash: Option<String>,
    pub output_type: String,
    pub typst_input: Vec<(String, String)>,
}

impl Config {
    pub fn is_deck_excluded(&self, deck_name: &str) -> bool {
        self.exclude_decks.iter().any(|p| p.matches(deck_name))
    }

    pub fn is_file_excluded(&self, file_name: &str) -> bool {
        self.exclude_files.iter().any(|p| p.matches(file_name))
    }

    pub fn template_front(&self, _card_info: &CardInfo, front_image_path: &str) -> String {
        format!(
            r#"<img src="{}">"#,
            encode_double_quoted_attribute(front_image_path)
        )
    }

    pub fn template_back(&self, _card_info: &CardInfo, back_image_path: &str) -> String {
        format!(
            r#"<img src="{}">"#,
            encode_double_quoted_attribute(back_image_path)
        )
    }

    pub fn destruct(&self) {
        // Be careful not to panic in this function, as it is called during unwinding.
        if self.dry_run {
            println!("Destroying config (dry run)");
        }
        if self.is_zip && self.asked_path != self.path.to_string_lossy() {
            if let Err(e) = fs::remove_dir_all(&self.path) {
                eprintln!(
                    "Warning: Failed to remove temporary extracted zip directory {}: {}",
                    self.path.display(),
                    e
                );
            }
        }
    }

    pub fn compute_hash(&mut self) {
        let relevant_config = json!({
            "output_type": self.output_type,
            "max_card_width": self.max_card_width,
            "exclude_decks": self.exclude_decks_string.clone().sort(),
        });
        let relevant_config = utils::json_sorted_keys(&relevant_config);
        let s = serde_json::to_string(&relevant_config).unwrap();
        self.config_hash = Some(utils::hash_string(&s));
    }
}

// RAII guard to ensure Config::destruct() is called when run() exits or unwinds.
// We call destruct() inside catch_unwind to avoid panics during unwinding.
pub struct ConfigGuard;

impl Drop for ConfigGuard {
    fn drop(&mut self) {
        let _ = std::panic::catch_unwind(|| {
            let cfg = get();
            cfg.destruct();
        });
    }
}

fn parse_generation_concurrency(s: &str) -> usize {
    if s.is_empty() {
        1
    } else if s == "max" {
        num_cpus::get()
    } else {
        s.parse::<usize>().unwrap_or(1).max(1)
    }
}

pub fn parse_config() -> Config {
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap();

    let asked_path = if cli.path.is_empty() {
        ".".to_string()
    } else {
        cli.path.join(" ")
    };

    let mut check_duplicates = cli.check_duplicates;
    let mut exclude_decks = cli.exclude_decks.clone();
    let mut exclude_files = cli.exclude_files.clone();
    let mut dry_run = cli.dry_run;
    let mut max_card_width = cli.max_card_width.clone();
    let mut skip_cache = cli.no_cache;
    let mut generation_concurrency = parse_generation_concurrency(&cli.generation_concurrency);
    let mut recompile_on_config_change = cli.recompile_on_config_change.clone();

    #[derive(Debug)]
    enum ConfigSource {
        Cli,
        File,
        Default,
    }

    let mut source_map: HashMap<&str, ConfigSource> = HashMap::new();
    let c = &Cli::command();
    c.get_arguments().for_each(|arg| {
        let name = arg.get_id().as_str();
        match matches.value_source(name) {
            Some(ValueSource::CommandLine) | Some(ValueSource::EnvVariable) => {
                source_map.insert(name, ConfigSource::Cli);
            }
            Some(ValueSource::DefaultValue) => {
                source_map.insert(name, ConfigSource::Default);
            }
            None => {
                // This branch seems to match for exclude_files and exclude_decks when not provided.
                source_map.insert(name, ConfigSource::Default);
            }
            _ => {
                eprintln!("Unknown value source for arg {}", name);
            }
        }
    });

    let mut path = get_real_path_simple(&asked_path);
    let is_zip = if path.to_lowercase().ends_with(".zip") {
        true
    } else {
        false
    };

    if is_zip {
        let dir = tempdir()
            .expect("Failed to create temporary directory for zip extraction")
            .path()
            .to_path_buf();
        utils::unzip_file_to_dir(&Path::new(&path), &dir).expect("Failed to extract zip file");
        path = dir.to_string_lossy().to_string();
    }

    if !cli.config_file.is_empty() {
        let config_file_path = Path::new(&path).join(&cli.config_file);
        if let Some(table) = load_toml_config(&config_file_path) {
            if let Some(&ConfigSource::Default) = source_map.get("check_duplicates") {
                if let Some(v) = table.get("check_duplicates") {
                    if let Some(b) = v.as_bool() {
                        check_duplicates = b;
                        source_map.insert("check_duplicates", ConfigSource::File);
                    }
                }
            }
            if let Some(&ConfigSource::Default) = source_map.get("exclude_decks") {
                if let Some(v) = table.get("exclude_decks").and_then(|x| x.as_array()) {
                    exclude_decks = v
                        .iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect();
                    source_map.insert("exclude_decks", ConfigSource::File);
                }
            }
            if let Some(&ConfigSource::Default) = source_map.get("exclude_files") {
                if let Some(v) = table.get("exclude_files").and_then(|x| x.as_array()) {
                    exclude_files = v
                        .iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect();
                    source_map.insert("exclude_files", ConfigSource::File);
                }
            }

            if let Some(&ConfigSource::Default) = source_map.get("dry_run") {
                if let Some(v) = table.get("dry_run").and_then(|x| x.as_bool()) {
                    dry_run = v;
                    source_map.insert("dry_run", ConfigSource::File);
                }
            }

            if let Some(&ConfigSource::Default) = source_map.get("max_card_width") {
                if let Some(v) = table.get("max_card_width").and_then(|x| x.as_str()) {
                    max_card_width = v.to_string();
                    source_map.insert("max_card_width", ConfigSource::File);
                }
            }

            if let Some(&ConfigSource::Default) = source_map.get("no_cache") {
                if let Some(v) = table.get("check_checksums").and_then(|x| x.as_bool()) {
                    skip_cache = v;
                    source_map.insert("no_cache", ConfigSource::File);
                }
            }
            if let Some(&ConfigSource::Default) = source_map.get("generation_concurrency") {
                if let Some(v) = table.get("generation_concurrency").and_then(|x| {
                    Some(parse_generation_concurrency(
                        x.as_str()
                            .unwrap_or(x.as_integer().unwrap_or(1).to_string().as_str()),
                    ))
                }) {
                    generation_concurrency = v;
                    source_map.insert("generation_concurrency", ConfigSource::File);
                }
            }

            if let Some(&ConfigSource::Default) = source_map.get("recompile_on_config_change") {
                if let Some(v) = table
                    .get("recompile_on_config_change")
                    .and_then(|x| x.as_str())
                {
                    recompile_on_config_change = v.to_string();
                    source_map.insert("recompile_on_config_change", ConfigSource::File);
                }
            }
        }
    }
    // println!("Config sources: {:#?}", source_map);

    let mut typst_input: Vec<(String, String)> = Vec::new();
    typst_input.push(("typ2anki_compile".to_string(), "1".to_string()));

    if max_card_width != "auto" {
        typst_input.push(("max_card_width".to_string(), max_card_width.clone()));
    }

    if !check_duplicates && generation_concurrency > 1 {
        eprintln!("WARNING: Concurrent generation can't be enabled without duplicate checking. Disabling concurrent generation.");
        generation_concurrency = 1;
    } else if generation_concurrency > num_cpus::get() {
        eprintln!("WARNING: Requested generation concurrency ({}) exceeds number of CPU cores ({}). It is inefficient. Reducing to {}. You can set generation-concurrency to 'max' so that it always takes the amount of logical threads on a given machine.", generation_concurrency, num_cpus::get(), num_cpus::get());
        generation_concurrency = num_cpus::get();
    }

    if cli.print_config {
        let c = Cli::command();
        let mut options: Vec<serde_json::Value> = Vec::new();
        let hidden_args: Vec<String> = (vec![
            "config_file",
            "path",
            "print_config",
            "version",
            "keep_terminal_open",
        ])
        .iter()
        .map(|s| s.to_string())
        .collect();
        c.get_arguments().for_each(|arg| {
            let id = arg.get_id().as_str();
            if hidden_args.iter().any(|s| s == id) {
                return;
            }
            let source = match source_map.get(id).unwrap() {
                ConfigSource::Default => 0,
                ConfigSource::Cli => 1,
                ConfigSource::File => 2,
            };
            let cli_name = format!(
                "--{}",
                arg.get_long()
                    .map(|s| s.to_string())
                    .or(arg.get_short().map(|c| c.to_string()))
                    .unwrap()
            );
            let help = arg.get_help().unwrap().to_string();
            let value: Value = match id {
                "check_duplicates" => json!(check_duplicates),
                "exclude_decks" => json!(exclude_decks),
                "exclude_files" => json!(exclude_files),
                "dry_run" => json!(dry_run),
                "max_card_width" => json!(max_card_width),
                "no_cache" => json!(skip_cache),
                "generation_concurrency" => json!(generation_concurrency),
                "recompile_on_config_change" => json!(recompile_on_config_change),
                _ => json!(null),
            };
            let t = match arg.get_action() {
                ArgAction::SetTrue => "store_true".to_string(),
                ArgAction::Append => "append".to_string(),
                ArgAction::Set => "str".to_string(),
                other => format!("{:?}", other),
            };
            options.push(json!({
                "id": id,
                "source": source,
                "cli_name": cli_name,
                "help": help,
                "type": t,
                "value":value,
            }))
        });
        let output = json!({ "options": options });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        std::process::exit(0);
    }

    let mut cfg = Config {
        check_duplicates,
        exclude_decks: exclude_decks
            .iter()
            .map(|s| Pattern::new(s).unwrap_or_default())
            .collect(),
        exclude_files: exclude_files
            .iter()
            .map(|s| Pattern::new(s).unwrap_or_default())
            .collect(),
        exclude_decks_string: exclude_decks,
        asked_path: asked_path.clone(),
        path: PathBuf::from(path),
        recompile_on_config_change: Arc::new(
            match recompile_on_config_change.to_ascii_lowercase().as_str() {
                "y" | "yes" => Some(true),
                "n" | "no" => Some(false),
                "_" => None,
                _ => None,
            }
            .into(),
        ),
        dry_run,
        max_card_width,
        skip_cache,
        generation_concurrency,
        is_zip,
        config_hash: None,
        output_type: "png".to_string(),
        typst_input,
        keep_terminal_open: cli.keep_terminal_open,
    };
    cfg.compute_hash();

    cfg
}

static CACHED_CONFIG: OnceCell<Config> = OnceCell::new();

pub fn get() -> &'static Config {
    CACHED_CONFIG.get_or_init(|| parse_config())
}
