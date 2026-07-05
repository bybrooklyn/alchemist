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

/// Async wrapper around [`device_id_for`]. Resolution does two blocking
/// syscalls (`canonicalize` + `metadata`); callers on the async runtime —
/// notably `Db::enqueue_job`, which the library scanner drives in a tight
/// per-file loop — must use this variant so the worker is never parked on
/// a slow network mount.
pub async fn device_id_for_async(path: &Path) -> Option<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || device_id_for(&path))
        .await
        .ok()
        .flatten()
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
    //
    // `std::fs::canonicalize` returns verbatim paths on Windows — `\\?\C:\...`
    // for lettered volumes and `\\?\UNC\server\share\...` for UNC shares — so
    // we must match on the parsed `Prefix` component rather than the raw string.
    // Matching on the prefix transparently handles both the verbatim and the
    // plain forms (`Disk`/`VerbatimDisk`, `UNC`/`VerbatimUNC`).
    use std::path::{Component, Prefix};

    let prefix = match path.components().next()? {
        Component::Prefix(prefix) => prefix.kind(),
        _ => return None,
    };

    match prefix {
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => {
            let drive = char::from(drive).to_ascii_uppercase();
            Some(format!("vol:{drive}"))
        }
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
            // Group by server+share.
            let server = server.to_str()?;
            let share = share.to_str()?;
            Some(format!("unc:{server}\\{share}"))
        }
        // DeviceNS / Verbatim (non-disk, non-UNC) prefixes have no meaningful
        // spindle grouping — treat as unknown.
        Prefix::DeviceNS(_) | Prefix::Verbatim(_) => None,
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
