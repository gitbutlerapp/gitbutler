use std::path::PathBuf;

const IS_DEV: bool = cfg!(debug_assertions);

pub fn refname() -> String {
    match IS_DEV {
        true => "gitbutler-dev".to_string(),
        false => "gitbutler".to_string(),
    }
}

pub fn dir() -> PathBuf {
    match IS_DEV {
        true => PathBuf::from("gb-dev"),
        false => PathBuf::from("gb"),
    }
}
