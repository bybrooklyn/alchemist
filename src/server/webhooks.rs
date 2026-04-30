//! Sonarr/Radarr webhook ingress handlers.

use super::{AppState, api_error_response};
use crate::config::ArrPathTranslation;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(crate) struct ArrWebhookPayload {
    #[serde(rename = "eventType")]
    event_type: String,
    path: Option<String>,
    #[serde(rename = "relativePath")]
    relative_path: Option<String>,
    movie: Option<ArrEntity>,
    series: Option<ArrEntity>,
    #[serde(rename = "movieFile")]
    movie_file: Option<ArrFile>,
    #[serde(rename = "episodeFile")]
    episode_file: Option<ArrFile>,
    #[serde(rename = "importedMovieFiles")]
    imported_movie_files: Vec<ArrFile>,
    #[serde(rename = "importedEpisodeFiles")]
    imported_episode_files: Vec<ArrFile>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ArrEntity {
    path: Option<String>,
    #[serde(rename = "folderPath")]
    folder_path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ArrFile {
    path: Option<String>,
    #[serde(rename = "relativePath")]
    relative_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct ArrWebhookResponse {
    accepted: bool,
    enqueued: bool,
    message: String,
    resolved_path: Option<String>,
}

impl ArrWebhookPayload {
    fn is_download_event(&self) -> bool {
        self.event_type.trim().eq_ignore_ascii_case("download")
    }
}

fn first_non_empty(values: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn join_relative(base: Option<&str>, relative: Option<&str>) -> Option<String> {
    let base = base?.trim();
    let relative = relative?.trim();
    if base.is_empty() || relative.is_empty() {
        return None;
    }
    let joined = PathBuf::from(base).join(relative);
    Some(joined.to_string_lossy().to_string())
}

fn resolve_arr_payload_path(payload: &ArrWebhookPayload) -> Option<String> {
    let imported_primary = payload
        .imported_episode_files
        .iter()
        .chain(payload.imported_movie_files.iter())
        .find_map(|file| first_non_empty([file.path.clone()]));

    let direct_file_path = first_non_empty([
        payload
            .episode_file
            .as_ref()
            .and_then(|file| file.path.clone()),
        payload
            .movie_file
            .as_ref()
            .and_then(|file| file.path.clone()),
        payload.path.clone(),
    ]);

    let payload_relative = first_non_empty([payload.relative_path.clone()]);
    let file_relative = first_non_empty([
        payload
            .episode_file
            .as_ref()
            .and_then(|file| file.relative_path.clone()),
        payload
            .movie_file
            .as_ref()
            .and_then(|file| file.relative_path.clone()),
    ]);
    let base_paths = [
        payload
            .movie
            .as_ref()
            .and_then(|movie| movie.path.as_deref()),
        payload
            .movie
            .as_ref()
            .and_then(|movie| movie.folder_path.as_deref()),
        payload
            .series
            .as_ref()
            .and_then(|series| series.path.as_deref()),
        payload
            .series
            .as_ref()
            .and_then(|series| series.folder_path.as_deref()),
    ];
    let joined_relative = base_paths.into_iter().find_map(|base| {
        join_relative(base, file_relative.as_deref())
            .or_else(|| join_relative(base, payload_relative.as_deref()))
    });

    first_non_empty([imported_primary, direct_file_path, joined_relative])
}

fn matches_prefix(path: &str, prefix: &str) -> bool {
    if path == prefix {
        return true;
    }
    match path.strip_prefix(prefix) {
        Some(rest) => rest.starts_with('/') || rest.starts_with('\\'),
        None => false,
    }
}

fn apply_translation(path: &str, translation: &ArrPathTranslation) -> Option<String> {
    let from = translation.from.trim();
    let to = translation.to.trim();
    if from.is_empty() || to.is_empty() || !matches_prefix(path, from) {
        return None;
    }
    let suffix = path.strip_prefix(from).unwrap_or_default();
    Some(format!("{to}{suffix}"))
}

fn translate_arr_path(path: &str, translations: &[ArrPathTranslation]) -> String {
    let mut selected: Option<&ArrPathTranslation> = None;
    for translation in translations {
        if apply_translation(path, translation).is_some() {
            let is_better = selected
                .map(|current| translation.from.len() > current.from.len())
                .unwrap_or(true);
            if is_better {
                selected = Some(translation);
            }
        }
    }
    selected
        .and_then(|translation| apply_translation(path, translation))
        .unwrap_or_else(|| path.to_string())
}

pub(crate) async fn arr_webhook_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<ArrWebhookPayload>,
) -> Response {
    if !payload.is_download_event() {
        return (
            StatusCode::ACCEPTED,
            axum::Json(ArrWebhookResponse {
                accepted: false,
                enqueued: false,
                message: "Ignoring non-Download Arr webhook event.".to_string(),
                resolved_path: None,
            }),
        )
            .into_response();
    }

    let resolved = match resolve_arr_payload_path(&payload) {
        Some(path) => path,
        None => {
            return api_error_response(
                StatusCode::BAD_REQUEST,
                "ARR_WEBHOOK_PATH_MISSING",
                "No import file path was found in webhook payload",
            );
        }
    };

    let translated = {
        let config = state.config.read().await;
        translate_arr_path(&resolved, &config.system.arr_path_translations)
    };

    match super::jobs::enqueue_job_from_submitted_path(state.as_ref(), translated.trim()).await {
        Ok(enqueue_result) => axum::Json(ArrWebhookResponse {
            accepted: true,
            enqueued: enqueue_result.enqueued,
            message: enqueue_result.message,
            resolved_path: Some(translated),
        })
        .into_response(),
        Err((status, _code, msg)) => {
            let error_code = if status.is_server_error() {
                "ARR_WEBHOOK_ENQUEUE_FAILED"
            } else {
                "ARR_WEBHOOK_ENQUEUE_REJECTED"
            };
            api_error_response(status, error_code, msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_imported_paths_before_other_fields() {
        let payload = ArrWebhookPayload {
            event_type: "Download".to_string(),
            imported_episode_files: vec![ArrFile {
                path: Some("/tv/show/episode.mkv".to_string()),
                relative_path: None,
            }],
            episode_file: Some(ArrFile {
                path: Some("/tv/show/old.mkv".to_string()),
                relative_path: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            resolve_arr_payload_path(&payload).as_deref(),
            Some("/tv/show/episode.mkv")
        );
    }

    #[test]
    fn resolves_relative_file_path_with_series_base() {
        let payload = ArrWebhookPayload {
            event_type: "Download".to_string(),
            series: Some(ArrEntity {
                path: Some("/media/tv/show".to_string()),
                folder_path: None,
            }),
            episode_file: Some(ArrFile {
                path: None,
                relative_path: Some("Season 01/episode.mkv".to_string()),
            }),
            ..Default::default()
        };
        assert_eq!(
            resolve_arr_payload_path(&payload).as_deref(),
            Some("/media/tv/show/Season 01/episode.mkv")
        );
    }

    #[test]
    fn uses_longest_matching_translation_prefix() {
        let translated = translate_arr_path(
            "/data/media/tv/show.mkv",
            &[
                ArrPathTranslation {
                    from: "/data".to_string(),
                    to: "/mnt".to_string(),
                },
                ArrPathTranslation {
                    from: "/data/media".to_string(),
                    to: "/srv/library".to_string(),
                },
            ],
        );
        assert_eq!(translated, "/srv/library/tv/show.mkv");
    }
}
