use std::{collections::HashMap, sync::RwLock};

lazy_static! {
    static ref MAPPING: RwLock<HashMap<String, git2::Oid>> = RwLock::new(HashMap::new());
}

pub fn set_hash_mapping(session_id: &str, hash: &git2::Oid) {
    MAPPING
        .write()
        .unwrap()
        .insert(session_id.to_string(), hash.clone());
}

pub fn get_hash_mapping(hash: &str) -> Option<git2::Oid> {
    MAPPING.read().unwrap().get(hash).cloned()
}
