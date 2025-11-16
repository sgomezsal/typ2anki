#![allow(dead_code)]
use base64;
use once_cell::sync::OnceCell;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

// Assume CardInfo lives here; adjust path if needed.
use crate::card_wrapper::CardInfo;
use crate::config;

const ANKI_CONNECT_URL: &str = "http://localhost:8765";
pub const CARDS_CACHE_FILENAME: &str = "_typ-cards-cache.json";

fn _handle_response(resp: reqwest::blocking::Response) -> Result<Value, String> {
    let v: Value = resp
        .json()
        .map_err(|e| format!("invalid json response: {}", e))?;
    if let Some(err) = v.get("error") {
        if !err.is_null() {
            return Err(format!("Anki API Error: {}", err));
        }
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

fn send_request(payload: Value) -> Result<Value, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("reqwest build error: {}", e))?;
    _handle_response(
        client
            .post(ANKI_CONNECT_URL)
            .json(&payload)
            .send()
            .map_err(|e| format!("request error: {:?}", e))?,
    )
}

pub fn check_anki_running() -> bool {
    let client = Client::builder().timeout(Duration::from_secs(3)).build();
    if client.is_err() {
        return false;
    }
    let client = client.unwrap();
    let resp = client.get(ANKI_CONNECT_URL).send();
    if resp.is_err() {
        return false;
    }
    let v: Result<Value, _> = resp.unwrap().json();
    if let Ok(json) = v {
        return json.get("apiVersion").is_some();
    }
    false
}

pub fn upload_media(file_path: &Path) -> Result<String, String> {
    let data = fs::read(file_path).map_err(|e| format!("read file error: {}", e))?;
    let encoded = base64::encode(&data);
    let filename = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "invalid filename".to_string())?
        .to_string();
    upload_file(filename, &encoded)
}

pub fn upload_file(filename: String, base64_data: &String) -> Result<String, String> {
    let payload = json!({
        "action": "storeMediaFile",
        "version": 6,
        "params": {
            "filename": filename,
            "data": base64_data
        }
    });
    send_request(payload)?;
    Ok(filename)
}

pub fn get_media_dir_path() -> Result<String, String> {
    let payload = json!({
        "action": "getMediaDirPath",
        "version": 6
    });
    let res = send_request(payload)?;
    res.as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "unexpected response".to_string())
}

pub fn get_cards_cache_string() -> Option<String> {
    let payload = json!({
        "action": "retrieveMediaFile",
        "version": 6,
        "params": { "filename": CARDS_CACHE_FILENAME }
    });
    match send_request(payload) {
        Ok(val) => {
            if let Some(s) = val.as_str() {
                match base64::decode(s) {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(s) => Some(s),
                        Err(_) => None,
                    },
                    Err(_) => None,
                }
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

pub fn create_deck(deck_name: &str) -> Result<(), String> {
    let payload = json!({
        "action": "createDeck",
        "version": 6,
        "params": { "deck": deck_name }
    });
    send_request(payload)?;
    Ok(())
}

pub fn get_deck_names() -> Vec<String> {
    let payload = json!({ "action": "deckNames", "version": 6 });
    match send_request(payload) {
        Ok(val) => val
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

static CACHED_DECK_NAMES: OnceCell<Vec<String>> = OnceCell::new();

static ANKI_DECK_MAP: OnceCell<Mutex<HashMap<String, String>>> = OnceCell::new();

pub fn get_anki_deck_name(typ_deck_name: &str) -> String {
    let map = ANKI_DECK_MAP.get_or_init(|| Mutex::new(HashMap::new()));

    // Check cache
    let guard = map.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(cached_name) = guard.get(typ_deck_name) {
        return cached_name.clone();
    }
    drop(guard);

    let cached = CACHED_DECK_NAMES.get_or_init(|| get_deck_names());
    let s = format!("::{}", typ_deck_name);
    let result = cached
        .iter()
        .find(|&name| name.ends_with(&s))
        .cloned()
        .unwrap_or_else(|| typ_deck_name.to_string());

    // Update cache
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(typ_deck_name.to_string(), result.clone());

    result
}

pub fn find_note_id_by_tag(tag: &str) -> Result<Vec<i64>, String> {
    let payload = json!({
        "action": "findNotes",
        "version": 6,
        "params": { "query": format!("tag:{}", tag) }
    });
    let res = send_request(payload)?;
    if let Some(arr) = res.as_array() {
        let mut out = Vec::new();
        for v in arr {
            if let Some(n) = v.as_i64() {
                out.push(n);
            }
        }
        Ok(out)
    } else {
        Ok(Vec::new())
    }
}

type ModelInfo = (String, (String, String));

static CACHED_BASICAL_MODEL_NAME: OnceCell<ModelInfo> = OnceCell::new();

const BASIC_MODEL_LOCALES: [&str; 3] = ["Basic", "Basique", "Grundlegend"];

fn _get_basic_model_name() -> Result<ModelInfo, String> {
    let payload = json!({ "action": "modelNames", "version": 6 });
    let models = send_request(payload)?;
    let model_list = models
        .as_array()
        .ok_or_else(|| "modelNames returned unexpected type".to_string())?;
    let mut basic_model_name: Option<String> = None;
    'outer: for locale in &BASIC_MODEL_LOCALES {
        for v in model_list {
            if let Some(s) = v.as_str() {
                if s == *locale {
                    basic_model_name = Some(s.to_string());
                    break 'outer;
                }
            }
        }
    }
    let model_name = basic_model_name.ok_or_else(|| "Basic model not found in Anki".to_string())?;
    let payload2 = json!({
        "version": 6,
        "action": "modelFieldNames",
        "params": { "modelName": model_name }
    });
    let fields_val = send_request(payload2)?;
    let fields = fields_val
        .as_array()
        .ok_or_else(|| "modelFieldNames returned unexpected type".to_string())?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err(format!(
            "Basic model should have 2 fields, but found {}",
            fields.len()
        ));
    }

    Ok((model_name, (fields[0].clone(), fields[1].clone())))
}

fn get_basic_model_name() -> &'static ModelInfo {
    CACHED_BASICAL_MODEL_NAME.get_or_init(|| {
        _get_basic_model_name().unwrap_or((
            "Basic".to_string(),
            ("Front".to_string(), "Back".to_string()),
        ))
    })
}

pub struct CardUploaderThread {
    client: Client,
}
impl CardUploaderThread {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build reqwest client");
        Self { client }
    }

    fn upload_file(&self, filename: String, base64_data: &String) -> Result<String, String> {
        let payload = json!({
            "action": "storeMediaFile",
            "version": 6,
            "params": {
                "filename": filename,
                "data": base64_data
            }
        });
        _handle_response(
            self.client
                .post(ANKI_CONNECT_URL)
                .json(&payload)
                .send()
                .map_err(|e| format!("request error: {}", e))?,
        )?;
        Ok(filename)
    }

    pub fn upload_card(
        &self,
        card: &CardInfo,
        front_data_base64: &String,
        back_data_base64: &String,
    ) -> Result<(), String> {
        let cfg = config::get();
        self.upload_file(card.image_path(1), front_data_base64)?;
        self.upload_file(card.image_path(2), back_data_base64)?;

        let note_ids = find_note_id_by_tag(&card.card_id)?;
        let tags = vec![card.card_id.clone()];

        let payload = if !note_ids.is_empty() {
            let note_id = note_ids[0];

            json!({
                "action": "updateNoteFields",
                "version": 6,
                "params": {
                    "note": {
                        "id": note_id,
                        "fields": {
                            "Front": cfg.template_front(card,card.image_path(1).as_str()),
                            "Back": cfg.template_back(card,card.image_path(2).as_str()),
                        },
                        "tags": tags
                    }
                }
            })
        } else {
            let (model_name, (model_field_front, model_field_back)) = get_basic_model_name();
            json!({
                "action": "addNote",
                "version": 6,
                "params": {
                    "note": {
                        "deckName": card.anki_deck_name,
                        "modelName": model_name,
                        "fields": {
                            model_field_front: cfg.template_front(card,card.image_path(1).as_str()),
                            model_field_back: cfg.template_back(card,card.image_path(2).as_str()),
                        },
                        "tags": tags
                    }
                }
            })
        };
        send_request(payload)?;
        Ok(())
    }
}
