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

/// Returns the platform-appropriate default data directory for
/// Alchemist. All files (config and DB) live here by default.
///
/// Linux/macOS: ~/.config/alchemist/
///   Respects $XDG_CONFIG_HOME on Linux.
/// Windows:     %APPDATA%\Alchemist\
///   Falls back to the working directory if APPDATA is unset.
fn default_data_dir() -> PathBuf {
    // Linux and macOS: follow XDG / ~/.config
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("alchemist");
            }
        }
        if let Some(home) = env::var_os("HOME") {
            if !home.is_empty() {
                return PathBuf::from(home).join(".config").join("alchemist");
            }
        }
        PathBuf::from(".")
    }

    // Windows: %APPDATA%\Alchemist\
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            if !appdata.is_empty() {
                return PathBuf::from(appdata).join("Alchemist");
            }
        }
        PathBuf::from(".")
    }

    // Any other platform: working directory
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        PathBuf::from(".")
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(path) = env::var("ALCHEMIST_CONFIG_PATH").or_else(|_| env::var("ALCHEMIST_CONFIG")) {
        return PathBuf::from(path);
    }
    default_data_dir().join(DEFAULT_CONFIG_PATH)
}

pub fn db_path() -> PathBuf {
    if let Ok(path) = env::var("ALCHEMIST_DB_PATH") {
        return PathBuf::from(path);
    }
    if let Ok(data_dir) = env::var("ALCHEMIST_DATA_DIR") {
        return Path::new(&data_dir).join(DEFAULT_DB_PATH);
    }
    default_data_dir().join(DEFAULT_DB_PATH)
}

pub fn config_mutable() -> bool {
    match env::var("ALCHEMIST_CONFIG_MUTABLE") {
        Ok(value) => parse_bool_env(&value).unwrap_or(true),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_takes_priority_for_config() {
        // env vars always win regardless of platform
        // SAFETY: single-threaded test; no other threads read this env var concurrently.
        unsafe { std::env::set_var("ALCHEMIST_CONFIG_PATH", "/tmp/test-config.toml") };
        assert_eq!(config_path(), PathBuf::from("/tmp/test-config.toml"));
        unsafe { std::env::remove_var("ALCHEMIST_CONFIG_PATH") };
    }

    #[test]
    fn env_override_takes_priority_for_db() {
        // SAFETY: single-threaded test.
        unsafe { std::env::set_var("ALCHEMIST_DB_PATH", "/tmp/test.db") };
        assert_eq!(db_path(), PathBuf::from("/tmp/test.db"));
        unsafe { std::env::remove_var("ALCHEMIST_DB_PATH") };
    }

    #[test]
    fn data_dir_override_for_db() {
        // SAFETY: single-threaded test.
        unsafe { std::env::set_var("ALCHEMIST_DATA_DIR", "/tmp/data") };
        assert_eq!(db_path(), PathBuf::from("/tmp/data/alchemist.db"));
        unsafe { std::env::remove_var("ALCHEMIST_DATA_DIR") };
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn default_dir_respects_xdg_config_home() {
        // SAFETY: single-threaded test.
        unsafe { std::env::remove_var("ALCHEMIST_CONFIG_PATH") };
        unsafe { std::env::remove_var("ALCHEMIST_CONFIG") };
        unsafe { std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg") };
        let dir = default_data_dir();
        assert_eq!(dir, PathBuf::from("/tmp/xdg/alchemist"));
        unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn default_dir_falls_back_to_home_config() {
        // SAFETY: single-threaded test.
        unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
        // HOME is always set in a test environment
        let home = std::env::var("HOME").unwrap();
        let expected = PathBuf::from(&home).join(".config").join("alchemist");
        assert_eq!(default_data_dir(), expected);
    }
}
