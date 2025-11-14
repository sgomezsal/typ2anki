use md5;
use serde_json::Value;
use std::collections::BTreeMap;

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
