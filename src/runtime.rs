use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_PATH: &str = "config.toml";
const DEFAULT_DB_PATH: &str = "alchemist.db";

fn parse_bool_env(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub fn config_path() -> PathBuf {
    env::var("ALCHEMIST_CONFIG_PATH")
        .or_else(|_| env::var("ALCHEMIST_CONFIG"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_CONFIG_PATH))
}

pub fn db_path() -> PathBuf {
    if let Ok(path) = env::var("ALCHEMIST_DB_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(data_dir) = env::var("ALCHEMIST_DATA_DIR") {
        return Path::new(&data_dir).join(DEFAULT_DB_PATH);
    }

    PathBuf::from(DEFAULT_DB_PATH)
}

pub fn config_mutable() -> bool {
    match env::var("ALCHEMIST_CONFIG_MUTABLE") {
        Ok(value) => parse_bool_env(&value).unwrap_or(true),
        Err(_) => true,
    }
}
