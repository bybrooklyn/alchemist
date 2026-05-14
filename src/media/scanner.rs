use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error};
use walkdir::WalkDir;

use crate::media::pipeline::DiscoveredMedia;

pub struct Scanner {
    pub extensions: Vec<String>,
}

/// PERF-3 aggressive directory pruning. When enabled and `last_scanned_at`
/// is set for the watch root containing a directory, the walker will skip
/// descending into directories whose mtime hasn't advanced past the
/// last successful scan. This is unsafe on filesystems that do not
/// propagate child mtimes to their parent (NFS, SMB, some CoW snapshot
/// volumes), so it must be opted into per ScannerConfig.
///
/// Top-level entries of a watch root are never pruned regardless of
/// mtime so that newly added direct children are still discovered.
#[derive(Debug, Clone, Default)]
pub struct PruneOptions {
    pub enabled: bool,
    pub last_scanned_by_root: HashMap<PathBuf, i64>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner {
    pub fn new() -> Self {
        Self {
            extensions: vec![
                "mp4".to_string(),
                "mkv".to_string(),
                "mov".to_string(),
                "avi".to_string(),
                "m4v".to_string(),
            ],
        }
    }

    pub fn scan(&self, directories: Vec<PathBuf>) -> Vec<DiscoveredMedia> {
        let entries = directories.into_iter().map(|dir| (dir, true)).collect();
        self.scan_with_recursion(entries)
    }

    pub fn scan_with_recursion(&self, directories: Vec<(PathBuf, bool)>) -> Vec<DiscoveredMedia> {
        self.scan_with_options(directories, &PruneOptions::default())
    }

    pub fn scan_with_options(
        &self,
        directories: Vec<(PathBuf, bool)>,
        prune: &PruneOptions,
    ) -> Vec<DiscoveredMedia> {
        let files = Arc::new(Mutex::new(Vec::new()));
        let source_roots: Arc<Vec<PathBuf>> = Arc::new(
            directories
                .iter()
                .map(|(dir, _)| dir.clone())
                .collect::<Vec<_>>(),
        );

        directories.into_par_iter().for_each(|(dir, recursive)| {
            debug!("Scanning directory: {:?} (recursive: {})", dir, recursive);
            let mut local_files = Vec::new();
            let source_roots = source_roots.clone();
            let walker_base = if recursive {
                WalkDir::new(&dir)
            } else {
                WalkDir::new(&dir).max_depth(1)
            };

            let root_for_filter = dir.clone();
            let prune_enabled = prune.enabled;
            let last_scanned = prune
                .last_scanned_by_root
                .get(&dir)
                .copied()
                .filter(|_| prune_enabled);

            // Aggressive pruning: skip recursing into directories whose mtime
            // hasn't advanced past `last_scanned`. The root itself and its
            // direct children are never pruned so new top-level entries are
            // still picked up.
            let walker = walker_base.into_iter().filter_entry(move |entry| {
                if !prune_enabled || last_scanned.is_none() {
                    return true;
                }
                if !entry.file_type().is_dir() {
                    return true;
                }
                if entry.path() == root_for_filter {
                    return true;
                }
                if entry.depth() <= 1 {
                    return true;
                }
                let cutoff = match last_scanned {
                    Some(c) => c,
                    None => return true,
                };
                let dir_mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                match dir_mtime {
                    Some(m) if m <= cutoff => {
                        debug!(
                            "Pruning subtree {:?} (dir_mtime {} <= last_scanned {})",
                            entry.path(),
                            m,
                            cutoff,
                        );
                        false
                    }
                    _ => true,
                }
            });

            for entry in walker.filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                        if self.extensions.contains(&ext.to_lowercase()) {
                            debug!("Found media file: {:?}", entry.path());
                            let mtime = entry
                                .metadata()
                                .map(|m| m.modified().unwrap_or(SystemTime::UNIX_EPOCH))
                                .unwrap_or(SystemTime::UNIX_EPOCH);
                            local_files.push(DiscoveredMedia {
                                path: entry.path().to_path_buf(),
                                mtime,
                                source_root: resolve_source_root(
                                    entry.path(),
                                    source_roots.as_ref(),
                                ),
                            });
                        }
                    }
                }
            }
            match files.lock() {
                Ok(mut guard) => guard.extend(local_files),
                Err(e) => error!("Failed to lock scan results: {}", e),
            }
        });

        let mut final_files = match files.lock() {
            Ok(guard) => guard.clone(),
            Err(e) => {
                error!("Failed to lock scan results for finalize: {}", e);
                Vec::new()
            }
        };
        // Deterministic ordering
        final_files.sort_by(|a, b| a.path.cmp(&b.path));

        final_files
    }
}

fn resolve_source_root(path: &Path, source_roots: &[PathBuf]) -> Option<PathBuf> {
    source_roots
        .iter()
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_source_root_prefers_longest_matching_root() {
        let roots = vec![PathBuf::from("/media"), PathBuf::from("/media/movies")];
        let resolved = resolve_source_root(Path::new("/media/movies/action/example.mkv"), &roots);
        assert_eq!(resolved, Some(PathBuf::from("/media/movies")));
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("alchemist_scan_{label}_{}", rand::random::<u64>()));
        let _ = fs::create_dir_all(&p);
        p
    }

    /// Default mode (no pruning) walks the whole tree and finds the file
    /// even when the parent directory's mtime is stale. This is the core
    /// safety property of the PERF-3 safe-incremental design.
    #[test]
    fn safe_default_walks_stale_parent_mtime_dirs() -> anyhow::Result<()> {
        let root = unique_temp_dir("safedefault");
        let subdir = root.join("season");
        fs::create_dir_all(&subdir)?;
        let file = subdir.join("ep01.mkv");
        fs::write(&file, b"data")?;

        // Backdate the subdir so dir_mtime is older than any future last_scanned.
        filetime_set_old(&subdir);

        let scanner = Scanner::new();
        let found = scanner.scan_with_recursion(vec![(root.clone(), true)]);
        assert_eq!(
            found.len(),
            1,
            "safe mode should find file regardless of parent mtime"
        );
        assert_eq!(found[0].path, file);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    /// Aggressive pruning when enabled skips stale directories.
    #[test]
    fn aggressive_pruning_skips_stale_dirs_when_enabled() -> anyhow::Result<()> {
        let root = unique_temp_dir("aggressive");
        let subdir = root.join("library");
        fs::create_dir_all(&subdir)?;
        let file = subdir.join("show.mkv");
        fs::write(&file, b"data")?;

        // Pretend we already scanned and the dir mtime is *older* than that.
        filetime_set_old(&subdir);
        let last_scanned_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let scanner = Scanner::new();
        let mut prune = PruneOptions {
            enabled: true,
            last_scanned_by_root: HashMap::new(),
        };
        prune
            .last_scanned_by_root
            .insert(root.clone(), last_scanned_at);

        // depth=2 dir mtime <= last_scanned → pruned.
        let nested = subdir.join("season01");
        fs::create_dir_all(&nested)?;
        let nested_file = nested.join("ep.mkv");
        fs::write(&nested_file, b"x")?;
        filetime_set_old(&nested);

        let found = scanner.scan_with_options(vec![(root.clone(), true)], &prune);
        // Direct children of `root` are not pruned (depth<=1), so `show.mkv`
        // and any season dirs are still visited; but recursion into
        // season01 (depth>1, stale) is pruned, missing nested_file.
        assert!(
            found.iter().any(|m| m.path == file),
            "expected the depth-1 file to be discovered: {:?}",
            found
        );
        assert!(
            !found.iter().any(|m| m.path == nested_file),
            "aggressive pruning should have skipped season01 nested file: {:?}",
            found
        );

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    /// Same fixture as above but with pruning OFF must still find the
    /// nested file (proves pruning is gated on the flag, not always-on).
    #[test]
    fn aggressive_pruning_off_finds_nested_files() -> anyhow::Result<()> {
        let root = unique_temp_dir("aggressive_off");
        let subdir = root.join("library");
        fs::create_dir_all(&subdir)?;
        let nested = subdir.join("season01");
        fs::create_dir_all(&nested)?;
        let nested_file = nested.join("ep.mkv");
        fs::write(&nested_file, b"x")?;
        filetime_set_old(&nested);

        let scanner = Scanner::new();
        let prune = PruneOptions {
            enabled: false,
            last_scanned_by_root: HashMap::new(),
        };
        let found = scanner.scan_with_options(vec![(root.clone(), true)], &prune);
        assert!(
            found.iter().any(|m| m.path == nested_file),
            "with pruning off, nested file should be discovered: {:?}",
            found
        );

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    /// Helper: bash dir mtime backwards so it appears older than any
    /// "last_scanned_at" we'll set in the test. Uses utimensat via
    /// std::fs::File::set_modified — falls back to no-op on systems
    /// where set_modified isn't available.
    fn filetime_set_old(p: &Path) {
        let very_old = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        if let Ok(file) = fs::File::open(p) {
            let _ = file.set_modified(very_old);
        }
    }
}
