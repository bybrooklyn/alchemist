//! Per-platform device identifier for a filesystem path.
//!
//! Used by PERF-2 (source-drive job grouping) to detect when two queued
//! jobs live on the same physical disk so the scheduler can avoid making
//! the same spindle seek against itself.
//!
//! `None` is returned for paths that cannot be resolved (missing, broken
//! symlinks, network errors). Null `source_device` is treated as "unknown"
//! upstream and never groups with another job.

use std::path::Path;

pub fn device_id_for(path: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(path).ok()?;
    platform_device_id(&canonical)
}

#[cfg(unix)]
fn platform_device_id(path: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).ok()?;
    Some(format!("dev:{}", meta.dev()))
}

#[cfg(windows)]
fn platform_device_id(path: &Path) -> Option<String> {
    // Drive-letter granularity. Two files on the same lettered volume share
    // the same physical spindle for the overwhelming majority of self-hosted
    // setups; mount-point bind volumes can be overridden via config.
    let s = path.to_str()?;
    let mut chars = s.chars();
    let drive = chars.next()?;
    if drive.is_ascii_alphabetic() && chars.next() == Some(':') {
        Some(format!("vol:{}", drive.to_ascii_uppercase()))
    } else if s.starts_with(r"\\") {
        // UNC path: \\server\share\... — group by server+share.
        let trimmed = &s[2..];
        let mut parts = trimmed.splitn(3, '\\');
        let server = parts.next()?;
        let share = parts.next()?;
        Some(format!("unc:{}\\{}", server, share))
    } else {
        None
    }
}

#[cfg(not(any(unix, windows)))]
fn platform_device_id(_path: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_existing_file_on_local_fs() {
        let tmp = std::env::temp_dir();
        let id = device_id_for(&tmp);
        assert!(id.is_some(), "device_id_for(temp_dir) returned None");
    }

    #[test]
    fn returns_none_for_missing_path() {
        let id = device_id_for(Path::new("/nonexistent/path/that/should/never/exist"));
        assert!(id.is_none());
    }

    #[test]
    fn same_directory_returns_same_id() {
        let tmp = std::env::temp_dir();
        let a = device_id_for(&tmp);
        let b = device_id_for(&tmp);
        assert_eq!(a, b);
    }
}
