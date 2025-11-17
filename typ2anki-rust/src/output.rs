use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    card_wrapper::{CardInfo, CardModificationStatus, TypFileStats},
    config,
};

pub struct OutputCompiledCardInfo {
    pub file: String,
    pub card_id: String,
    pub card_status: CardModificationStatus,
    pub error_message: Option<String>,
}

impl OutputCompiledCardInfo {
    pub fn build(card: &CardInfo, error_message: Option<String>) -> Self {
        OutputCompiledCardInfo {
            file: card.source_file.to_string_lossy().into_owned(),
            card_id: card.card_id.clone(),
            card_status: card.modification_status.clone(),
            error_message,
        }
    }
}

pub enum OutputMessage {
    ListTypstFiles(Arc<Mutex<HashMap<PathBuf, TypFileStats>>>),
    DbgShowConfig(config::Config),
    DbgConfigChangeDetection {
        total_cards: usize,
        config_changes: usize,
    },
    DbgCreateDeck(String),
    DbgSavedCache,
    DbgCompilationDone {
        files: Arc<Mutex<HashMap<PathBuf, TypFileStats>>>,
    },
    DbgDone,
    ParsingError(String),
    SkipCompileCard(OutputCompiledCardInfo),
    CompileError(OutputCompiledCardInfo),
    PushError(OutputCompiledCardInfo),
    CompiledCard(OutputCompiledCardInfo),
    PushedCard(OutputCompiledCardInfo),
    NoAnkiConnection,
    ErrorSavingCache(String),
}

pub trait OutputManager: Send + Sync {
    fn send(&self, msg: OutputMessage);
    fn ask_yes_no(&self, question: &str) -> bool;
    fn fail(&self);
}
