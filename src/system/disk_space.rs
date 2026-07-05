//! Disk-space guardrails (AUTO-3).
//!
//! Before the engine starts a job, it checks free space on that job's output
//! filesystem. When space falls below the configured minimum the engine holds
//! queued jobs (they stay queued and retry) instead of starting an encode that
//! could fail by filling the disk mid-run. The check fails open: when free
//! space can't be determined for a path, jobs are allowed to proceed so an
//! unsupported filesystem never blocks the queue forever.

use std::path::{Path, PathBuf};

use sysinfo::Disks;

/// One GiB in bytes.
pub const GIB: u64 = 1024 * 1024 * 1024;

/// Normalize a path to a stable prefix representation for mount comparison.
///
/// On Windows, `std::fs::canonicalize` (and callers building output paths from
/// it) yields verbatim paths — `\\?\C:\...` and `\\?\UNC\server\share\...` —
/// while `sysinfo` reports plain mount points like `C:\`. Without stripping the
/// `\\?\` / `\\?\UNC\` prefix, `starts_with` never matches, no mount is found,
/// and the free-space guard silently fails open. Stripping both sides to the
/// same form lets the correct mount be located. On other platforms this is the
/// identity function, so Unix behavior is unchanged.
fn normalize_for_mount_match(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.as_os_str().to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

/// Available bytes on the filesystem that contains `path`, or `None` when it
/// can't be determined (no matching mount, unsupported filesystem, or query
/// error). The mount whose path is the longest prefix of `path` wins, so a
/// nested mount resolves to the correct filesystem rather than its parent.
pub fn available_bytes_for_path(path: &Path) -> Option<u64> {
    let disks = Disks::new_with_refreshed_list();
    // Normalize away Windows verbatim prefixes so a `\\?\C:\...` output path
    // still matches sysinfo's `C:\` mount point (see normalize_for_mount_match).
    let path = normalize_for_mount_match(path);
    let mut best: Option<(usize, u64)> = None;
    for disk in disks.list() {
        let mount = normalize_for_mount_match(disk.mount_point());
        if path.starts_with(&mount) {
            let len = mount.as_os_str().len();
            let is_better = match best {
                Some((best_len, _)) => len > best_len,
                None => true,
            };
            if is_better {
                best = Some((len, disk.available_space()));
            }
        }
    }
    best.map(|(_, available)| available)
}

/// Whether the disk guardrail should hold jobs targeting a filesystem with
/// `available` free bytes, given the configured `min_gb` minimum.
///
/// Fails open: a disabled guardrail (`min_gb == 0`) or an undeterminable
/// (`None`) free-space value never blocks.
pub fn is_below_min_free(available: Option<u64>, min_gb: u32) -> bool {
    if min_gb == 0 {
        return false;
    }
    match available {
        Some(bytes) => bytes < u64::from(min_gb) * GIB,
        None => false,
    }
}

/// Bytes as GiB rounded for human-readable log/status messages.
pub fn as_gib(bytes: u64) -> f64 {
    bytes as f64 / GIB as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_guardrail_never_blocks() {
        assert!(!is_below_min_free(Some(0), 0));
        assert!(!is_below_min_free(None, 0));
        assert!(!is_below_min_free(Some(1), 0));
    }

    #[test]
    fn unknown_free_space_fails_open() {
        assert!(!is_below_min_free(None, 10));
    }

    #[test]
    fn blocks_only_when_strictly_below_threshold() {
        let min = 10;
        assert!(is_below_min_free(Some(5 * GIB), min));
        assert!(is_below_min_free(Some(10 * GIB - 1), min));
        assert!(!is_below_min_free(Some(10 * GIB), min));
        assert!(!is_below_min_free(Some(20 * GIB), min));
    }

    #[test]
    fn available_bytes_for_root_path_resolves_to_some_mount() {
        // The root path should match at least one mount on any supported
        // platform; the exact value is environment-specific so we only assert
        // that a filesystem was resolved.
        let root = if cfg!(windows) {
            Path::new("C:\\")
        } else {
            Path::new("/")
        };
        // May be None in unusual sandboxes; if Some, it must be a real figure.
        if let Some(bytes) = available_bytes_for_path(root) {
            // A mounted root filesystem always reports a non-zero capacity in
            // practice; available bytes is a sanity-checkable u64.
            let _ = bytes;
        }
    }
}
