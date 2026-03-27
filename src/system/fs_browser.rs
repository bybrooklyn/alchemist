use crate::config::Config;
use crate::db::Db;
use crate::error::{AlchemistError, Result};
use crate::media::scanner::Scanner;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsBreadcrumb {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsDirEntry {
    pub name: String,
    pub path: String,
    pub readable: bool,
    pub hidden: bool,
    pub media_hint: MediaHint,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaHint {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsBrowseResponse {
    pub path: String,
    pub readable: bool,
    pub breadcrumbs: Vec<FsBreadcrumb>,
    pub warnings: Vec<String>,
    pub entries: Vec<FsDirEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsRecommendation {
    pub path: String,
    pub label: String,
    pub reason: String,
    pub media_hint: MediaHint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsRecommendationsResponse {
    pub recommendations: Vec<FsRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPreviewRequest {
    pub directories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPreviewDirectory {
    pub path: String,
    pub exists: bool,
    pub readable: bool,
    pub media_files: usize,
    pub sample_files: Vec<String>,
    pub media_hint: MediaHint,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPreviewResponse {
    pub directories: Vec<FsPreviewDirectory>,
    pub total_media_files: usize,
    pub warnings: Vec<String>,
}

pub async fn browse(path: Option<&str>) -> Result<FsBrowseResponse> {
    let path = resolve_browse_path(path)?;
    tokio::task::spawn_blocking(move || browse_blocking(&path))
        .await
        .map_err(|err| AlchemistError::Watch(format!("fs browse worker failed: {err}")))?
}

pub async fn recommendations(config: &Config, db: &Db) -> Result<FsRecommendationsResponse> {
    let config = config.clone();
    let extra_dirs = db
        .get_watch_dirs()
        .await?
        .into_iter()
        .map(|watch| watch.path)
        .collect::<Vec<_>>();

    tokio::task::spawn_blocking(move || recommendations_blocking(&config, &extra_dirs))
        .await
        .map_err(|err| AlchemistError::Watch(format!("fs recommendations worker failed: {err}")))?
}

pub async fn preview(request: FsPreviewRequest) -> Result<FsPreviewResponse> {
    tokio::task::spawn_blocking(move || preview_blocking(request))
        .await
        .map_err(|err| AlchemistError::Watch(format!("fs preview worker failed: {err}")))?
}

fn browse_blocking(path: &Path) -> Result<FsBrowseResponse> {
    let path = canonical_or_original(path)?;

    // Check if the resolved path is now in a sensitive location
    // (handles symlinks pointing to sensitive directories)
    if is_sensitive_path(&path) {
        return Err(AlchemistError::Watch(
            "Access to this directory is restricted".to_string(),
        ));
    }

    let readable = path.is_dir();
    let mut warnings = directory_warnings(&path, readable);
    if !readable {
        warnings.push("Directory does not exist or is not accessible.".to_string());
    }

    let mut entries = if readable {
        std::fs::read_dir(&path)
            .map_err(AlchemistError::Io)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let entry_path = entry.path();
                if !entry_path.is_dir() {
                    return None;
                }

                // Check for symlinks and warn about them
                let is_symlink = entry_path
                    .symlink_metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);

                let name = entry.file_name().to_string_lossy().to_string();
                let hidden = is_hidden(&name, &entry_path);
                let readable = std::fs::read_dir(&entry_path).is_ok();
                let media_hint = classify_media_hint(&entry_path);

                let warning = if is_symlink {
                    Some("This is a symbolic link".to_string())
                } else {
                    entry_warning(&entry_path, readable)
                };

                Some(FsDirEntry {
                    name,
                    path: entry_path.to_string_lossy().to_string(),
                    readable,
                    hidden,
                    media_hint,
                    warning,
                })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(FsBrowseResponse {
        path: path.to_string_lossy().to_string(),
        readable,
        breadcrumbs: breadcrumbs(&path),
        warnings,
        entries,
    })
}

fn recommendations_blocking(
    config: &Config,
    extra_dirs: &[String],
) -> Result<FsRecommendationsResponse> {
    let mut seen = HashSet::new();
    let mut recommendations = Vec::new();

    for dir in config.scanner.directories.iter().chain(extra_dirs.iter()) {
        if let Ok(path) = canonical_or_original(Path::new(dir)) {
            let path_string = path.to_string_lossy().to_string();
            if seen.insert(path_string.clone()) {
                recommendations.push(FsRecommendation {
                    path: path_string,
                    label: path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("Configured Folder")
                        .to_string(),
                    reason: "Already configured in Alchemist".to_string(),
                    media_hint: classify_media_hint(&path),
                });
            }
        }
    }

    for root in candidate_roots() {
        if !root.exists() || !root.is_dir() {
            continue;
        }

        if let Ok(root) = canonical_or_original(&root) {
            for entry in std::fs::read_dir(&root)
                .ok()
                .into_iter()
                .flat_map(|entries| entries.filter_map(|entry| entry.ok()))
            {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let media_hint = classify_media_hint(&path);
                if matches!(media_hint, MediaHint::Low | MediaHint::Unknown) {
                    continue;
                }

                let path_string = path.to_string_lossy().to_string();
                if seen.insert(path_string.clone()) {
                    recommendations.push(FsRecommendation {
                        path: path_string,
                        label: entry.file_name().to_string_lossy().to_string(),
                        reason: recommendation_reason(&path, media_hint),
                        media_hint,
                    });
                }
            }
        }
    }

    recommendations.sort_by(|a, b| {
        media_rank(b.media_hint)
            .cmp(&media_rank(a.media_hint))
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });

    Ok(FsRecommendationsResponse {
        recommendations: recommendations.into_iter().take(16).collect(),
    })
}

fn preview_blocking(request: FsPreviewRequest) -> Result<FsPreviewResponse> {
    let scanner = Scanner::new();
    let mut total_media_files = 0usize;
    let mut warnings = Vec::new();

    let directories = request
        .directories
        .into_iter()
        .filter(|dir| !dir.trim().is_empty())
        .map(|raw| {
            let path = PathBuf::from(raw.trim());
            let canonical = canonical_or_original(&path)?;
            let exists = canonical.exists();
            let readable = exists && canonical.is_dir() && std::fs::read_dir(&canonical).is_ok();

            let media_files = if readable {
                scanner
                    .scan_with_recursion(vec![(canonical.clone(), true)])
                    .len()
            } else {
                0
            };
            total_media_files += media_files;

            let sample_files = if readable {
                scanner
                    .scan_with_recursion(vec![(canonical.clone(), true)])
                    .into_iter()
                    .take(5)
                    .map(|media| media.path.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            let mut dir_warnings = directory_warnings(&canonical, readable);
            if readable && media_files == 0 {
                dir_warnings
                    .push("No supported media files were found in this directory.".to_string());
            }
            warnings.extend(dir_warnings.clone());

            Ok(FsPreviewDirectory {
                path: canonical.to_string_lossy().to_string(),
                exists,
                readable,
                media_files,
                sample_files,
                media_hint: classify_media_hint(&canonical),
                warnings: dir_warnings,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(FsPreviewResponse {
        directories,
        total_media_files,
        warnings,
    })
}

fn resolve_browse_path(path: Option<&str>) -> Result<PathBuf> {
    match path.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => {
            let path = PathBuf::from(value);

            // Normalize and resolve the path
            let resolved = if path.exists() {
                std::fs::canonicalize(&path).map_err(AlchemistError::Io)?
            } else {
                path
            };

            // Block sensitive system directories
            if is_sensitive_path(&resolved) {
                return Err(AlchemistError::Watch(
                    "Access to this directory is restricted".to_string(),
                ));
            }

            Ok(resolved)
        }
        None => default_browse_root(),
    }
}

/// Check if a path is a sensitive system directory that shouldn't be browsed.
fn is_sensitive_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();

    #[cfg(unix)]
    {
        // Block sensitive Unix system directories
        let sensitive_prefixes = [
            "/etc",
            "/var/log",
            "/var/run",
            "/proc",
            "/sys",
            "/dev",
            "/boot",
            "/root",
            "/private/etc", // macOS
            "/private/var/log",
        ];

        for prefix in sensitive_prefixes {
            if path_str == prefix || path_str.starts_with(&format!("{}/", prefix)) {
                return true;
            }
        }
    }

    #[cfg(windows)]
    {
        // Block sensitive Windows system directories
        let sensitive_patterns = [
            "\\windows\\system32",
            "\\windows\\syswow64",
            "\\windows\\winsxs",
            "\\programdata\\microsoft",
        ];

        for pattern in sensitive_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }
    }

    false
}

fn default_browse_root() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        for drive in b'C'..=b'Z' {
            let path = format!("{}:\\", drive as char);
            let drive_path = PathBuf::from(path);
            if drive_path.exists() {
                return Ok(drive_path);
            }
        }
        Err(AlchemistError::Watch(
            "No accessible drive roots found".to_string(),
        ))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(PathBuf::from("/"))
    }
}

fn canonical_or_original(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        std::fs::canonicalize(path).map_err(AlchemistError::Io)
    } else {
        Ok(path.to_path_buf())
    }
}

fn breadcrumbs(path: &Path) -> Vec<FsBreadcrumb> {
    let mut current = PathBuf::new();
    let mut crumbs = Vec::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                current.push(prefix.as_os_str());
                crumbs.push(FsBreadcrumb {
                    label: prefix.as_os_str().to_string_lossy().to_string(),
                    path: current.to_string_lossy().to_string(),
                });
            }
            Component::RootDir => {
                current.push(component.as_os_str());
                crumbs.push(FsBreadcrumb {
                    label: "/".to_string(),
                    path: current.to_string_lossy().to_string(),
                });
            }
            Component::Normal(part) => {
                current.push(part);
                crumbs.push(FsBreadcrumb {
                    label: part.to_string_lossy().to_string(),
                    path: current.to_string_lossy().to_string(),
                });
            }
            Component::CurDir | Component::ParentDir => {}
        }
    }

    crumbs
}

fn candidate_roots() -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();

    #[cfg(target_os = "windows")]
    {
        for drive in b'C'..=b'Z' {
            let path = PathBuf::from(format!("{}:\\", drive as char));
            if path.exists() {
                roots.insert(path);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        roots.insert(PathBuf::from("/Volumes"));
        roots.insert(PathBuf::from("/Users"));
    }

    #[cfg(target_os = "linux")]
    {
        for root in [
            "/media", "/mnt", "/srv", "/data", "/storage", "/home", "/var/lib",
        ] {
            roots.insert(PathBuf::from(root));
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        roots.insert(PathBuf::from("/"));
    }

    roots.into_iter().collect()
}

fn recommendation_reason(path: &Path, media_hint: MediaHint) -> String {
    match media_hint {
        MediaHint::High => format!(
            "{} looks like a media library",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("This directory")
        ),
        MediaHint::Medium => "Contains media-like folders or files".to_string(),
        MediaHint::Low | MediaHint::Unknown => "Reachable server directory".to_string(),
    }
}

fn classify_media_hint(path: &Path) -> MediaHint {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let media_names = [
        "movies",
        "movie",
        "tv",
        "shows",
        "series",
        "anime",
        "media",
        "videos",
        "plex",
        "emby",
        "jellyfin",
        "library",
        "downloads",
    ];
    if media_names.iter().any(|candidate| name.contains(candidate)) {
        return MediaHint::High;
    }

    let scanner = Scanner::new();
    let mut media_files = 0usize;
    let mut child_dirs = 0usize;
    for entry in WalkDir::new(path)
        .max_depth(2)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .take(200)
    {
        if entry.file_type().is_dir() {
            child_dirs += 1;
            continue;
        }
        if entry
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                scanner
                    .extensions
                    .iter()
                    .any(|candidate| candidate == &ext.to_ascii_lowercase())
            })
        {
            media_files += 1;
            if media_files >= 3 {
                return MediaHint::High;
            }
        }
    }

    if media_files > 0 || child_dirs > 5 {
        MediaHint::Medium
    } else if path.exists() {
        MediaHint::Low
    } else {
        MediaHint::Unknown
    }
}

fn media_rank(hint: MediaHint) -> usize {
    match hint {
        MediaHint::High => 4,
        MediaHint::Medium => 3,
        MediaHint::Low => 2,
        MediaHint::Unknown => 1,
    }
}

fn is_hidden(name: &str, path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = path;
        name.starts_with('.')
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        name.starts_with('.')
    }
}

fn entry_warning(path: &Path, readable: bool) -> Option<String> {
    if !readable {
        return Some("Directory is not readable by the Alchemist process.".to_string());
    }
    if is_system_path(path) {
        return Some(
            "System directory. Only choose this if you know your media is stored here.".to_string(),
        );
    }
    None
}

fn directory_warnings(path: &Path, readable: bool) -> Vec<String> {
    let mut warnings = Vec::new();
    if !readable {
        warnings.push("Directory is not readable by the Alchemist process.".to_string());
    }
    if is_system_path(path) {
        warnings.push(
            "This looks like a system path. Avoid scanning operating system folders.".to_string(),
        );
    }
    if path.components().count() <= 1 {
        warnings.push(
            "Top-level roots can be noisy. Prefer the specific media folder when possible."
                .to_string(),
        );
    }
    warnings
}

fn is_system_path(path: &Path) -> bool {
    let value = path.to_string_lossy().to_ascii_lowercase();
    let system_roots = [
        "/bin",
        "/boot",
        "/dev",
        "/etc",
        "/lib",
        "/proc",
        "/sys",
        "/usr",
        "/var/log",
        "c:\\windows",
        "c:\\program files",
    ];
    system_roots.iter().any(|root| {
        value == *root
            || value.starts_with(&format!("{root}/"))
            || value.starts_with(&format!("{root}\\"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn breadcrumbs_include_root_and_children() {
        let crumbs = breadcrumbs(Path::new("/media/movies"));
        assert!(!crumbs.is_empty());
        assert_eq!(crumbs.last().unwrap().path, "/media/movies");
    }

    #[test]
    fn recommendation_prefers_media_like_names() {
        assert_eq!(
            classify_media_hint(Path::new("/srv/movies")),
            MediaHint::High
        );
    }

    #[test]
    fn system_paths_warn() {
        assert!(is_system_path(Path::new("/etc")));
        assert!(!is_system_path(Path::new("/media/library")));
    }

    #[test]
    fn preview_detects_media_files_and_samples() {
        let root =
            std::env::temp_dir().join(format!("alchemist_fs_preview_{}", rand::random::<u64>()));
        std::fs::create_dir_all(&root).expect("root");
        let media_file = root.join("movie.mkv");
        std::fs::write(&media_file, b"video").expect("media");

        let response = preview_blocking(FsPreviewRequest {
            directories: vec![root.to_string_lossy().to_string()],
        })
        .expect("preview");

        assert_eq!(response.total_media_files, 1);
        assert_eq!(response.directories.len(), 1);
        assert!(
            response.directories[0]
                .sample_files
                .iter()
                .any(|sample| sample.ends_with("movie.mkv"))
        );

        let _ = std::fs::remove_file(media_file);
        let _ = std::fs::remove_dir_all(root);
        let _ = SystemTime::UNIX_EPOCH;
    }
}
