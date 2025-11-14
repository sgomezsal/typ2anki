#![allow(dead_code)]
use base64;
use core::panic;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::Duration;

// Assume CardInfo lives here; adjust path if needed.
use crate::card_wrapper::CardInfo;

const ANKI_CONNECT_URL: &str = "http://localhost:8765";
const CARDS_CACHE_FILENAME: &str = "_typ-cards-cache.json";

fn send_request(payload: Value) -> Result<Value, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("reqwest build error: {}", e))?;
    let resp = client
        .post(ANKI_CONNECT_URL)
        .json(&payload)
        .send()
        .map_err(|e| format!("request error: {}", e))?;
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
    upload_file(filename, encoded)
}

pub fn upload_file(filename: String, base64_data: String) -> Result<String, String> {
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

pub fn update_note(note_id: i64, card: &CardInfo, tags: Vec<String>) -> Result<(), String> {
    panic!("Not implemented yet");
    /* // TODO: use config templates to generate front/back content, e.g.
    // let front = config().template_front(card, card.output_front_anki_name.as_deref().unwrap_or(""));
    // let back = config().template_back(card, card.output_back_anki_name.as_deref().unwrap_or(""));
    // For now we put placeholders or use provided output names if present.
    let front = card
        .output_front_anki_name
        .as_ref()
        .map(|s| s.clone())
        .unwrap_or_else(|| "[FRONT]".to_string());
    let back = card
        .output_back_anki_name
        .as_ref()
        .map(|s| s.clone())
        .unwrap_or_else(|| "[BACK]".to_string());

    let payload = json!({
        "action": "updateNoteFields",
        "version": 6,
        "params": {
            "note": {
                "id": note_id,
                "fields": {
                    "Front": front,
                    "Back": back
                },
                "tags": tags
            }
        }
    });
    send_request(payload)?;
    Ok(()) */
}

const BASIC_MODEL_LOCALES: [&str; 3] = ["Basic", "Basique", "Grundlegend"];

pub fn get_basic_model_name() -> Result<(String, Vec<String>), String> {
    // Returns (model_name, model_field_names)
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
    // TODO: consider caching model name/fields if desirable
    Ok((model_name, fields))
}

pub fn add_or_update_card(card: &mut CardInfo, tags: Vec<String>) -> Result<(), String> {
    panic!("Not implemented yet");
    // ensure images/names exist; Python asserted on output names
    /* if card.output_back_anki_name.is_none() || card.output_front_anki_name.is_none() {
        return Err("Card images are not set".to_string());
    }

    let note_ids = find_note_id_by_tag(&card.card_id)?;
    if !note_ids.is_empty() {
        card.old_anki_id = Some(note_ids[0]);
        update_note(note_ids[0], card, tags)?;
        Ok(())
    } else {
        let (model_name, model_fields) = get_basic_model_name()?;
        // TODO: use config templates to create the field HTML, currently use the stored image names or placeholders
        let front_field = card
            .output_front_anki_name
            .clone()
            .unwrap_or_else(|| "[FRONT]".to_string());
        let back_field = card
            .output_back_anki_name
            .clone()
            .unwrap_or_else(|| "[BACK]".to_string());

        let payload = json!({
            "action": "addNote",
            "version": 6,
            "params": {
                "note": {
                    "deckName": card.anki_deck_name,
                    "modelName": model_name,
                    "fields": {
                        model_fields[0]: front_field,
                        model_fields[1]: back_field
                    },
                    "tags": tags
                }
            }
        });
        send_request(payload)?;
        Ok(())
    } */
}
