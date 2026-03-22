use std::env;
use std::path::{Path, PathBuf};

const APP_HOME_DIR: &str = ".alchemist";
const DEFAULT_CONFIG_PATH: &str = "config.toml";
const DEFAULT_DB_PATH: &str = "alchemist.db";

fn parse_bool_env(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn default_home_root_for(home: Option<&Path>) -> Option<PathBuf> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        home.filter(|path| !path.as_os_str().is_empty())
            .map(|path| path.join(APP_HOME_DIR))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = home;
        None
    }
}

fn default_home_root() -> Option<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from);
    default_home_root_for(home.as_deref())
}

fn default_config_path() -> PathBuf {
    default_home_root()
        .map(|root| root.join(DEFAULT_CONFIG_PATH))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH))
}

fn default_db_path() -> PathBuf {
    default_home_root()
        .map(|root| root.join(DEFAULT_DB_PATH))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DB_PATH))
}

pub fn config_path() -> PathBuf {
    env::var("ALCHEMIST_CONFIG_PATH")
        .or_else(|_| env::var("ALCHEMIST_CONFIG"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_config_path())
}

pub fn db_path() -> PathBuf {
    if let Ok(path) = env::var("ALCHEMIST_DB_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(data_dir) = env::var("ALCHEMIST_DATA_DIR") {
        return Path::new(&data_dir).join(DEFAULT_DB_PATH);
    }

    default_db_path()
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

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn default_home_root_uses_alchemist_directory() {
        let home = Path::new("/Users/tester");
        assert_eq!(
            default_home_root_for(Some(home)),
            Some(home.join(".alchemist"))
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn default_paths_live_under_alchemist() {
        let root = default_home_root_for(Some(Path::new("/Users/tester")))
            .expect("expected home root on unix-like target");
        assert_eq!(
            root.join(DEFAULT_CONFIG_PATH),
            PathBuf::from("/Users/tester/.alchemist/config.toml")
        );
        assert_eq!(
            root.join(DEFAULT_DB_PATH),
            PathBuf::from("/Users/tester/.alchemist/alchemist.db")
        );
    }
}
