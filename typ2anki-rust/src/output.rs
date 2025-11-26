use crate::{
    card_wrapper::{CardInfo, CardModificationStatus, TFiles},
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

impl From<&CardInfo> for OutputCompiledCardInfo {
    // Create OutputCompiledCardInfo from CardInfo without error message
    fn from(card: &CardInfo) -> Self {
        Self::build(card, None)
    }
}

pub enum OutputMessage {
    ListTypstFiles(TFiles),
    DbgShowConfig(config::Config),
    DbgConfigChangeDetection {
        total_cards: usize,
        config_changes: usize,
    },
    DbgCreateDeck(String),
    DbgSavedCache,
    DbgCompilationDone {
        files: TFiles,
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
    TypstDownloadingPackage(String),
}

pub trait OutputManager: Send + Sync {
    fn send(&self, msg: OutputMessage);
    fn ask_yes_no(&self, question: &str) -> bool;
    fn fail(&self);
}
