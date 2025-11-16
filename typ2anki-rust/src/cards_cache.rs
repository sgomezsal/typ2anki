use std::collections::HashMap;

use crate::output::{OutputManager, OutputMessage};
use crate::utils::{self, hash_string};
use crate::{anki_api, config};

const CACHE_HASH_PART_LENGTH: usize = 34;

#[derive(Debug, Clone)]
pub struct CardsCacheManager {
    pub static_hash: String,
    pub old_cache: HashMap<String, String>,
    pub new_cache: HashMap<String, String>,
}

pub fn card_key(deck_name: &str, card_id: &str) -> String {
    format!("{}_{}", deck_name, card_id)
}

fn cache_concat_hashes_padding(hash1: &str, hash2: &str) -> String {
    let mut out = String::new();
    out.push_str(hash1);
    out.push_str(&"0".repeat(CACHE_HASH_PART_LENGTH.saturating_sub(hash1.len())));
    out.push_str(&"0".repeat(CACHE_HASH_PART_LENGTH.saturating_sub(hash2.len())));
    out.push_str(hash2);
    out
}

impl CardsCacheManager {
    pub fn init(ankiconf_hash: String, _output: &OutputManager) -> Self {
        let cfg = config::get();
        let static_hash =
            hash_string(format!("{}{}", ankiconf_hash, cfg.config_hash.as_ref().unwrap()).as_str());
        let cache: HashMap<String, String>;

        if !cfg.check_checksums {
            cache = HashMap::new();
        } else {
            let s = anki_api::get_cards_cache_string().unwrap_or("{}".to_string());
            cache = serde_json::from_str(&s).unwrap_or(HashMap::new());
        }

        Self {
            static_hash,
            new_cache: HashMap::new(),
            old_cache: cache,
        }
    }

    pub fn add_card_hash(&mut self, deck_name: &str, card_id: &str, content_hash: &str) {
        self.new_cache.insert(
            card_key(deck_name, card_id),
            cache_concat_hashes_padding(&self.static_hash, content_hash),
        );
    }

    // Removes the new hash for a card (used when a card fails to compile/upload)
    pub fn remove_card_hash(&mut self, deck_name: &str, card_id: &str) {
        let key = card_key(deck_name, card_id);
        self.new_cache.remove(&key);
        self.old_cache.remove(&key);
    }

    pub fn detect_configuration_change(&mut self, output: &OutputManager) {
        let cfg = config::get();
        if !cfg.check_checksums {
            return;
        }

        let mut config_changes = 0;
        let mut total_cards = 0;
        for (k, v) in &self.old_cache {
            total_cards += 1;
            if let Some(new_v) = self.new_cache.get(k) {
                if v[..CACHE_HASH_PART_LENGTH] != new_v[..CACHE_HASH_PART_LENGTH] {
                    config_changes += 1;
                }
            }
        }

        if cfg.dry_run {
            output.send(OutputMessage::DbgConfigChangeDetection {
                total_cards,
                config_changes,
            });
        }

        if cfg.recompile_on_config_change.read().unwrap().is_none() {
            if total_cards > 0 && (config_changes as f64) / (total_cards as f64) >= 0.2 {
                if output.ask_yes_no("A configuration or ankiconf.typ change has been detected. Do you wish to recompile all cards with this new config? (Y/n)") {
                    *cfg.recompile_on_config_change.write().unwrap() = Some(true);
                } else {
                    *cfg.recompile_on_config_change.write().unwrap() = Some(false);
                }
            }
        }
    }

    pub fn save_cache(&self, output: &OutputManager) {
        let push: HashMap<String, String> = self
            .old_cache
            .clone()
            .into_iter()
            .chain(self.new_cache.clone().into_iter())
            .collect();
        let s = serde_json::to_string(&push).unwrap_or("{}".to_string());
        let payload = utils::b64_encode(s);
        if let Err(e) = anki_api::upload_file(anki_api::CARDS_CACHE_FILENAME.into(), &payload) {
            output.send(OutputMessage::ErrorSavingCache(e));
        } else {
            output.send(OutputMessage::DbgSavedCache);
        }
    }
}
