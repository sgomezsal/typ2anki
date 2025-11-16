use base64::{engine::general_purpose::STANDARD, DecodeError, Engine as _};
use md5;
use regex::Regex;
use serde_json::Value;
use std::cmp::max;
use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, iter};

// Hashes the string as md5 hex digest
pub fn hash_string(input: &str) -> String {
    let digest = md5::compute(input);
    format!("{:x}", digest)
}

pub fn json_sorted_keys(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut btree = BTreeMap::new();
            for (k, v) in map {
                btree.insert(k.clone(), json_sorted_keys(v));
            }
            Value::Object(btree.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.iter().map(json_sorted_keys).collect()),
        _ => v.clone(),
    }
}

pub fn get_all_typst_imports(typst_content: &str) -> Vec<String> {
    let pattern = Regex::new(r#"(?m)^#import\s*"([^"]+)"\s*"#).unwrap();
    let mut r: Vec<String> = Vec::new();

    let mut imports: Vec<String> = pattern
        .captures_iter(typst_content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();

    let mut idx = 0;
    while idx < imports.len() {
        let mut import_path = imports[idx].clone();

        if Path::new(&import_path).is_absolute() {
            if let Ok(rel) = Path::new(&import_path).strip_prefix("/") {
                import_path = rel.to_string_lossy().into_owned();
            } else if import_path.starts_with('/') {
                import_path = import_path.trim_start_matches('/').to_string();
            }
        }

        let joined_path = {
            let base = crate::config::get().path.clone();
            Path::new(&base).join(&import_path)
        };

        if joined_path.exists() {
            let joined_str = joined_path.to_string_lossy().into_owned();
            if !r.contains(&joined_str) {
                r.push(joined_str.clone());
            }

            if let Ok(content) = fs::read_to_string(&joined_path) {
                for cap in pattern.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        imports.push(m.as_str().to_string());
                    }
                }
            }
        }

        idx += 1;
    }

    r.sort();
    r.dedup();
    r
}

pub fn print_header(lines: &[&str], width: usize, border_char: char) {
    let width = if width == 0 {
        let max_line_length = lines.iter().map(|line| line.len()).max().unwrap_or(0);
        max(max_line_length + 10, 80)
    } else {
        width
    };

    let border: String = iter::repeat(border_char).take(width).collect();
    println!("{}", border);
    for line in lines {
        let centered_line = format!("{:^width$}", line, width = width);
        println!("{}", centered_line);
    }
    println!("{}", border);
}

pub fn b64_encode<T: AsRef<[u8]>>(input: T) -> String {
    STANDARD.encode(input)
}

pub fn b64_decode(input: &str) -> Result<Vec<u8>, DecodeError> {
    STANDARD.decode(input)
}
