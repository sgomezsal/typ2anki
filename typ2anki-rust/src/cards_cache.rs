use std::collections::HashMap;

const CACHE_HASH_PART_LENGTH: usize = 34;

#[derive(Debug, Clone)]
pub struct CardsCacheManager {
    static_hash: String,
    old_cache: HashMap<String, String>,
    new_cache: HashMap<String, String>,
    ignore_config_changes: bool,
}

impl CardsCacheManager {
    pub fn 
}