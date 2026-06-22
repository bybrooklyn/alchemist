use crate::config::{UpdateChannel, UpdatesConfig};
use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const UPDATE_MANIFEST_ASSET: &str = "alchemist-update-manifest.json";
const GITHUB_RELEASES_API: &str = "https://api.github.com/repos/bybrooklyn/alchemist/releases";
const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallType {
    Docker,
    Homebrew,
    Aur,
    Source,
    DirectBinary,
    WindowsExe,
    Unknown,
}

impl InstallType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Homebrew => "homebrew",
            Self::Aur => "aur",
            Self::Source => "source",
            Self::DirectBinary => "direct_binary",
            Self::WindowsExe => "windows_exe",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Verified,
    PublicKeyUnavailable,
    ManifestUnavailable,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    SelfUpdate,
    Guided,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateAssetStatus {
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub channel: UpdateChannel,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub checked_at: String,
    pub install_type: InstallType,
    pub can_self_update: bool,
    pub action: UpdateAction,
    pub guidance: Option<String>,
    pub guidance_command: Option<String>,
    pub verification_status: VerificationStatus,
    pub verification_error: Option<String>,
    pub asset: Option<UpdateAssetStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignedUpdateManifest {
    pub signed: UpdateManifestPayload,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateManifestPayload {
    pub schema_version: u32,
    pub channel: UpdateChannel,
    pub version: String,
    pub release_url: String,
    pub assets: Vec<UpdateManifestAsset>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateManifestAsset {
    pub os: String,
    pub arch: String,
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct StagedUpdate {
    pub archive_path: PathBuf,
    pub staged_binary_path: PathBuf,
    pub staging_dir: PathBuf,
    pub version: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

pub fn embedded_public_key_b64() -> Option<&'static str> {
    option_env!("ALCHEMIST_UPDATE_PUBLIC_KEY_B64").and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    })
}

pub async fn check_for_updates(config: &UpdatesConfig) -> Result<UpdateStatus> {
    let current_version = crate::version::current().to_string();
    let checked_at = chrono::Utc::now().to_rfc3339();
    let install_type = detect_install_type();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("alchemist/{}", crate::version::current()))
        .build()?;

    let Some(release) = fetch_latest_release_for_channel(&client, config.channel).await? else {
        return Ok(status_without_release(
            current_version,
            checked_at,
            config.channel,
            install_type,
        ));
    };

    let manifest_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == UPDATE_MANIFEST_ASSET);

    let Some(manifest_asset) = manifest_asset else {
        return Ok(status_from_release_without_manifest(
            current_version,
            checked_at,
            config.channel,
            install_type,
            release,
            VerificationStatus::ManifestUnavailable,
            Some(format!(
                "{UPDATE_MANIFEST_ASSET} was not published for this release"
            )),
        ));
    };

    let manifest_response = client
        .get(&manifest_asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?;
    let manifest = manifest_response
        .json::<SignedUpdateManifest>()
        .await
        .context("parse signed update manifest")?;

    let verification = match verify_manifest_with_embedded_key(&manifest) {
        Ok(()) => (VerificationStatus::Verified, None),
        Err(err) if embedded_public_key_b64().is_none() => (
            VerificationStatus::PublicKeyUnavailable,
            Some(err.to_string()),
        ),
        Err(err) => (VerificationStatus::Failed, Some(err.to_string())),
    };

    let asset = select_current_asset(&manifest.signed).map(UpdateAssetStatus::from);
    let latest_version = manifest.signed.version.clone();
    let release_url = manifest.signed.release_url.clone();
    let update_available =
        version_is_newer_for_channel(&latest_version, &current_version, config.channel);
    let can_self_update = update_available
        && verification.0 == VerificationStatus::Verified
        && install_type == InstallType::DirectBinary
        && asset.is_some()
        && direct_binary_updates_supported();
    let (action, guidance, guidance_command) =
        update_action_for_install_type(install_type, can_self_update);

    Ok(UpdateStatus {
        current_version,
        channel: config.channel,
        latest_version: Some(latest_version),
        update_available,
        release_url: Some(release_url),
        checked_at,
        install_type,
        can_self_update,
        action,
        guidance,
        guidance_command,
        verification_status: verification.0,
        verification_error: verification.1,
        asset,
    })
}

pub async fn stage_update_asset(asset: &UpdateAssetStatus, version: &str) -> Result<StagedUpdate> {
    // RAII cleanup for the staging directory. `armed` is flipped to `false`
    // just before we return a successful `StagedUpdate` so callers still get
    // ownership of the directory; on any error path the drop nukes it.
    struct StagingDirCleanup {
        path: PathBuf,
        armed: bool,
    }
    impl Drop for StagingDirCleanup {
        fn drop(&mut self) {
            if self.armed {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    let staging_dir = create_update_staging_dir()?;
    let mut staging_guard = StagingDirCleanup {
        path: staging_dir.clone(),
        armed: true,
    };
    let archive_path = staging_dir.join(&asset.filename);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(format!("alchemist/{}", crate::version::current()))
        .build()?;

    // Stream the response straight to disk so a 100+ MB archive never has to
    // sit fully in RAM, and hash on the fly to avoid a second pass.
    let mut response = client.get(&asset.url).send().await?.error_for_status()?;
    let mut file = tokio::fs::File::create(&archive_path).await?;
    let mut hasher = Sha256::new();
    while let Some(chunk) = response.chunk().await? {
        hasher.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    drop(file);

    let actual_sha = {
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write as _;
            let _ = write!(&mut hex, "{byte:02x}");
        }
        hex
    };
    if !actual_sha.eq_ignore_ascii_case(&asset.sha256) {
        return Err(anyhow!(
            "downloaded asset hash mismatch: expected {}, got {}",
            asset.sha256,
            actual_sha
        ));
    }

    // `tar -xzf` and the staged binary's `--version` probe are both blocking
    // operations driven by external processes. Push them onto the blocking
    // pool so the calling worker can keep servicing other async tasks while
    // they run.
    let staged_binary_path = {
        let archive = archive_path.clone();
        let dir = staging_dir.clone();
        tokio::task::spawn_blocking(move || extract_archive(&archive, &dir))
            .await
            .map_err(|err| anyhow!("extract worker failed: {err}"))??
    };
    {
        let staged = staged_binary_path.clone();
        let version_owned = version.to_string();
        tokio::task::spawn_blocking(move || verify_staged_binary_version(&staged, &version_owned))
            .await
            .map_err(|err| anyhow!("version probe worker failed: {err}"))??;
    }

    staging_guard.armed = false;
    Ok(StagedUpdate {
        archive_path,
        staged_binary_path,
        staging_dir,
        version: version.to_string(),
    })
}

/// Pre-flight checks before downloading and applying an update: confirm there is
/// enough free disk space for the archive + extracted binary + headroom, and
/// that the install directory is writable. Failing here is far better than
/// failing part-way through a download or after the old binary has been moved.
pub fn preflight_update_environment(asset_size: u64) -> Result<()> {
    // Room for the compressed archive, the extracted binary (~3x the archive),
    // and headroom, with a sane floor for tiny/unknown assets.
    const FLOOR_BYTES: u64 = 512 * 1024 * 1024;
    let required = asset_size.saturating_mul(4).max(FLOOR_BYTES);
    let temp = crate::runtime::temp_dir();
    if let Some(available) = crate::system::disk_space::available_bytes_for_path(&temp) {
        if available < required {
            return Err(anyhow!(
                "not enough free disk space to apply the update: {:.1} GiB available at {}, need ~{:.1} GiB",
                crate::system::disk_space::as_gib(available),
                temp.display(),
                crate::system::disk_space::as_gib(required),
            ));
        }
    }

    // For in-place binary updates, confirm the install directory is writable now
    // so we don't discover it only after staging.
    if direct_binary_updates_supported() {
        let exe = std::env::current_exe().context("resolve current executable")?;
        if let Some(dir) = exe.parent() {
            let probe = dir.join(format!(".alchemist-write-probe-{}", rand::random::<u64>()));
            match fs::File::create(&probe) {
                Ok(_) => {
                    let _ = fs::remove_file(&probe);
                }
                Err(err) => {
                    return Err(anyhow!(
                        "install directory {} is not writable for self-update: {err}",
                        dir.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn spawn_update_helper(staged: &StagedUpdate, backup_path: &Path) -> Result<PathBuf> {
    if !direct_binary_updates_supported() {
        return Err(anyhow!(
            "direct binary self-update is only supported on Linux and macOS"
        ));
    }

    let current_exe = std::env::current_exe().context("resolve current executable")?;
    let helper_path = staged.staging_dir.join("alchemist-update-helper.sh");
    let rollback_path = staged
        .staging_dir
        .join(format!("alchemist-{}.rollback", crate::version::current()));
    let log_path = staged.staging_dir.join("alchemist-update.log");
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();

    write_helper_script(&helper_path)?;

    let mut command = Command::new("/bin/sh");
    command
        .arg(&helper_path)
        .arg(std::process::id().to_string())
        .arg(&current_exe)
        .arg(&staged.staged_binary_path)
        .arg(&rollback_path)
        .arg(&log_path)
        .arg(backup_path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.spawn().context("spawn update helper")?;

    Ok(log_path)
}

pub fn detect_install_type() -> InstallType {
    if is_docker_environment() {
        return InstallType::Docker;
    }

    if cfg!(target_os = "windows") {
        return InstallType::WindowsExe;
    }

    let Ok(exe) = std::env::current_exe() else {
        return InstallType::Unknown;
    };
    detect_install_type_for_path(&exe)
}

pub fn detect_install_type_for_path(exe: &Path) -> InstallType {
    let normalized = exe
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if normalized.contains("/target/") {
        return InstallType::Source;
    }
    if normalized.contains("/homebrew/cellar/") || normalized.contains("/cellar/alchemist/") {
        return InstallType::Homebrew;
    }
    if cfg!(target_os = "linux")
        && (normalized == "/usr/bin/alchemist" || normalized.starts_with("/usr/lib/alchemist/"))
    {
        return InstallType::Aur;
    }
    if cfg!(any(target_os = "linux", target_os = "macos"))
        && exe.file_name().and_then(|name| name.to_str()) == Some("alchemist")
    {
        return InstallType::DirectBinary;
    }
    InstallType::Unknown
}

pub fn version_is_newer_for_channel(latest: &str, current: &str, channel: UpdateChannel) -> bool {
    if channel == UpdateChannel::Nightly {
        return latest.trim() != current.trim();
    }
    parse_version_key(latest) > parse_version_key(current)
}

pub fn verify_signed_manifest(manifest: &SignedUpdateManifest, public_key_b64: &str) -> Result<()> {
    if manifest.signed.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported update manifest schema {}",
            manifest.signed.schema_version
        ));
    }

    let public_key_bytes = general_purpose::STANDARD
        .decode(public_key_b64.trim())
        .context("decode update public key")?;
    let public_key_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| anyhow!("update public key must be 32 bytes"))?;
    let public_key =
        VerifyingKey::from_bytes(&public_key_array).context("load update public key")?;

    let signature_bytes = general_purpose::STANDARD
        .decode(manifest.signature.trim())
        .context("decode update manifest signature")?;
    let signature_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| anyhow!("update manifest signature must be 64 bytes"))?;
    let signature = Signature::from_bytes(&signature_array);
    let signed_payload = serde_json::to_vec(&manifest.signed)?;
    public_key
        .verify(&signed_payload, &signature)
        .context("verify update manifest signature")?;
    Ok(())
}

fn verify_manifest_with_embedded_key(manifest: &SignedUpdateManifest) -> Result<()> {
    let Some(public_key) = embedded_public_key_b64() else {
        return Err(anyhow!(
            "ALCHEMIST_UPDATE_PUBLIC_KEY_B64 was not embedded at build time"
        ));
    };
    verify_signed_manifest(manifest, public_key)
}

fn status_without_release(
    current_version: String,
    checked_at: String,
    channel: UpdateChannel,
    install_type: InstallType,
) -> UpdateStatus {
    let (action, guidance, guidance_command) = update_action_for_install_type(install_type, false);
    UpdateStatus {
        current_version,
        channel,
        latest_version: None,
        update_available: false,
        release_url: None,
        checked_at,
        install_type,
        can_self_update: false,
        action,
        guidance,
        guidance_command,
        verification_status: VerificationStatus::ManifestUnavailable,
        verification_error: Some(format!("No {channel} release was found")),
        asset: None,
    }
}

fn status_from_release_without_manifest(
    current_version: String,
    checked_at: String,
    channel: UpdateChannel,
    install_type: InstallType,
    release: GitHubRelease,
    verification_status: VerificationStatus,
    verification_error: Option<String>,
) -> UpdateStatus {
    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let update_available = version_is_newer_for_channel(&latest_version, &current_version, channel);
    let (action, guidance, guidance_command) = update_action_for_install_type(install_type, false);
    UpdateStatus {
        current_version,
        channel,
        latest_version: Some(latest_version),
        update_available,
        release_url: Some(release.html_url),
        checked_at,
        install_type,
        can_self_update: false,
        action,
        guidance,
        guidance_command,
        verification_status,
        verification_error,
        asset: None,
    }
}

async fn fetch_latest_release_for_channel(
    client: &reqwest::Client,
    channel: UpdateChannel,
) -> Result<Option<GitHubRelease>> {
    let releases = client
        .get(GITHUB_RELEASES_API)
        .query(&[("per_page", "50")])
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<GitHubRelease>>()
        .await?;
    Ok(releases
        .into_iter()
        .find(|release| release_matches_channel(release, channel)))
}

fn release_matches_channel(release: &GitHubRelease, channel: UpdateChannel) -> bool {
    if release.draft {
        return false;
    }
    let tag = release.tag_name.to_ascii_lowercase();
    match channel {
        UpdateChannel::Stable => !release.prerelease && !tag.contains("nightly"),
        UpdateChannel::Rc => release.prerelease && tag.contains("-rc"),
        UpdateChannel::Nightly => release.prerelease && tag.contains("nightly"),
    }
}

fn select_current_asset(manifest: &UpdateManifestPayload) -> Option<&UpdateManifestAsset> {
    let os = current_update_os();
    let arch = current_update_arch();
    manifest
        .assets
        .iter()
        .find(|asset| asset.os == os && asset.arch == arch)
}

fn current_update_os() -> &'static str {
    std::env::consts::OS
}

fn current_update_arch() -> &'static str {
    std::env::consts::ARCH
}

fn direct_binary_updates_supported() -> bool {
    cfg!(any(target_os = "linux", target_os = "macos"))
}

fn update_action_for_install_type(
    install_type: InstallType,
    can_self_update: bool,
) -> (UpdateAction, Option<String>, Option<String>) {
    if can_self_update {
        return (
            UpdateAction::SelfUpdate,
            Some("Alchemist can install this update after draining active jobs.".to_string()),
            None,
        );
    }

    match install_type {
        InstallType::Docker => (
            UpdateAction::Guided,
            Some("Update the container image and restart the service.".to_string()),
            Some("docker compose pull && docker compose up -d".to_string()),
        ),
        InstallType::Homebrew => (
            UpdateAction::Guided,
            Some("This binary is owned by Homebrew; update it through Homebrew.".to_string()),
            Some("brew update && brew upgrade alchemist".to_string()),
        ),
        InstallType::Aur => (
            UpdateAction::Guided,
            Some("This binary appears to be package-managed; update it through your AUR helper.".to_string()),
            Some("yay -Syu alchemist-bin".to_string()),
        ),
        InstallType::Source => (
            UpdateAction::Guided,
            Some("This looks like a source checkout; rebuild from the repository.".to_string()),
            Some("git pull && just build".to_string()),
        ),
        InstallType::WindowsExe => (
            UpdateAction::Guided,
            Some("Windows executable replacement is manual in this release.".to_string()),
            None,
        ),
        InstallType::DirectBinary => (
            UpdateAction::Unsupported,
            Some("Self-update is unavailable until the release manifest is verified for this platform.".to_string()),
            None,
        ),
        InstallType::Unknown => (
            UpdateAction::Unsupported,
            Some("Alchemist could not determine how this binary is managed.".to_string()),
            None,
        ),
    }
}

impl From<&UpdateManifestAsset> for UpdateAssetStatus {
    fn from(asset: &UpdateManifestAsset) -> Self {
        Self {
            filename: asset.filename.clone(),
            url: asset.url.clone(),
            sha256: asset.sha256.clone(),
            size: asset.size,
            os: asset.os.clone(),
            arch: asset.arch.clone(),
        }
    }
}

fn is_docker_environment() -> bool {
    Path::new("/.dockerenv").exists()
        || Path::new("/.containerenv").exists()
        || fs::read_to_string("/proc/1/cgroup")
            .map(|cgroup| {
                cgroup.contains("docker")
                    || cgroup.contains("containerd")
                    || cgroup.contains("kubepods")
            })
            .unwrap_or(false)
}

fn create_update_staging_dir() -> Result<PathBuf> {
    let mut staging_dir = crate::runtime::temp_dir();
    staging_dir.push(format!("update-{}", rand::random::<u64>()));
    fs::create_dir_all(&staging_dir)?;
    Ok(staging_dir)
}

fn extract_archive(archive_path: &Path, staging_dir: &Path) -> Result<PathBuf> {
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(staging_dir)
        .output()
        .context("extract update archive")?;
    if !output.status.success() {
        return Err(anyhow!(
            "extract update archive failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let staged_binary_path = staging_dir.join("alchemist");
    if !staged_binary_path.is_file() {
        return Err(anyhow!(
            "update archive did not contain an alchemist binary"
        ));
    }
    Ok(staged_binary_path)
}

fn verify_staged_binary_version(binary_path: &Path, version: &str) -> Result<()> {
    let output = Command::new(binary_path)
        .arg("--version")
        .output()
        .context("run staged binary version check")?;
    if !output.status.success() {
        return Err(anyhow!(
            "staged binary version check failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains(version) {
        return Err(anyhow!(
            "staged binary version mismatch: expected {version}, got {}",
            stdout.trim()
        ));
    }
    Ok(())
}

fn write_helper_script(path: &Path) -> Result<()> {
    let mut file = fs::File::create(path)?;
    // The `$4` (rollback) argument is retained for positional compatibility but
    // is no longer used: the swap now keeps a co-located rollback beside the
    // current binary so it is always on the same filesystem as the install path.
    file.write_all(
        br#"set -eu
parent_pid="$1"
current="$2"
staged="$3"
rollback="$4"
log="$5"
backup="$6"
shift 6

# Wait for the old daemon to exit so the binary file is no longer in use.
while kill -0 "$parent_pid" 2>/dev/null; do
  sleep 1
done

new="${current}.update-new.$$"
prev="${current}.update-previous"

{
  echo "Applying Alchemist update"
  echo "Backup snapshot: $backup"

  # Stage the new binary *beside* the current one so the swap is an atomic,
  # same-filesystem rename. The staged copy lives in a temp dir that may be on a
  # different filesystem, where mv would be a non-atomic copy+unlink.
  cp "$staged" "$new"
  chmod +x "$new"

  # Keep a co-located rollback copy of the current binary (same filesystem, so
  # restoring it later is also atomic). -p preserves mode/ownership where possible.
  if [ -f "$current" ]; then
    cp -p "$current" "$prev" 2>/dev/null || cp "$current" "$prev"
  fi

  # Atomic replace.
  mv -f "$new" "$current"
  chmod +x "$current"

  # Launch the new binary and confirm it survives startup for 15s.
  "$current" "$@" &
  child="$!"
  sleep 15
  if kill -0 "$child" 2>/dev/null; then
    echo "Update applied successfully; new binary is running"
    rm -f "$prev"
  else
    wait "$child" || true
    echo "Updated Alchemist exited during startup; rolling back to previous binary"
    mv -f "$current" "${current}.failed-update" 2>/dev/null || true
    if [ -f "$prev" ]; then
      mv -f "$prev" "$current"
      chmod +x "$current"
      "$current" "$@" &
      echo "Previous binary restored and relaunched"
    else
      echo "ERROR: no rollback copy available; manual recovery required (backup: $backup)"
    fi
  fi
} >> "$log" 2>&1
"#,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct VersionKey {
    major: u64,
    minor: u64,
    patch: u64,
    pre_rank: u64,
    pre_number: u64,
}

fn parse_version_key(value: &str) -> VersionKey {
    let sanitized = value.trim().trim_start_matches('v');
    let mut parts = sanitized.splitn(2, ['-', '+']);
    let core = parts.next().unwrap_or_default();
    let suffix = parts.next().unwrap_or_default().to_ascii_lowercase();
    let mut core_parts = core.split('.').filter_map(|part| part.parse::<u64>().ok());
    let major = core_parts.next().unwrap_or(0);
    let minor = core_parts.next().unwrap_or(0);
    let patch = core_parts.next().unwrap_or(0);

    let (pre_rank, pre_number) = if suffix.is_empty() {
        (3, 0)
    } else if suffix.contains("rc") {
        (2, first_number_in(&suffix).unwrap_or(0))
    } else if suffix.contains("nightly") {
        (1, 0)
    } else {
        (0, 0)
    };

    VersionKey {
        major,
        minor,
        patch,
        pre_rank,
        pre_number,
    }
}

fn first_number_in(value: &str) -> Option<u64> {
    value
        .split(|ch: char| !ch.is_ascii_digit())
        .find_map(|part| {
            if part.is_empty() {
                None
            } else {
                part.parse::<u64>().ok()
            }
        })
}

#[allow(dead_code)]
fn command_args_without_binary() -> Vec<OsString> {
    std::env::args_os().skip(1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn helper_script_is_valid_sh_and_does_atomic_rollback() {
        let dir = std::env::temp_dir().join(format!("alchemist-helper-{}", rand::random::<u64>()));
        if let Err(e) = fs::create_dir_all(&dir) {
            panic!("create temp dir: {e}");
        }
        let script = dir.join("helper.sh");
        if let Err(e) = write_helper_script(&script) {
            panic!("write helper script: {e}");
        }

        let Ok(body) = fs::read_to_string(&script) else {
            panic!("read helper script");
        };
        // Same-filesystem atomic swap + co-located rollback markers.
        assert!(body.contains("${current}.update-new."));
        assert!(body.contains("${current}.update-previous"));
        assert!(body.contains("mv -f \"$new\" \"$current\""));
        assert!(body.contains("rolling back to previous binary"));

        // The generated script must be valid POSIX sh.
        if let Ok(output) = Command::new("sh").arg("-n").arg(&script).output() {
            assert!(
                output.status.success(),
                "sh -n rejected helper script: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_compare_handles_rc_progression() {
        assert!(version_is_newer_for_channel(
            "0.3.2-rc.2",
            "0.3.2-rc.1",
            UpdateChannel::Rc
        ));
        assert!(version_is_newer_for_channel(
            "0.3.2",
            "0.3.2-rc.2",
            UpdateChannel::Stable
        ));
        assert!(!version_is_newer_for_channel(
            "0.3.1",
            "0.3.2-rc.2",
            UpdateChannel::Stable
        ));
    }

    #[test]
    fn nightly_compare_treats_different_build_as_update() {
        assert!(version_is_newer_for_channel(
            "0.3.2-nightly+abc1234",
            "0.3.2-nightly+def5678",
            UpdateChannel::Nightly
        ));
    }

    #[test]
    fn install_detection_respects_package_manager_paths() {
        assert_eq!(
            detect_install_type_for_path(Path::new(
                "/opt/homebrew/Cellar/alchemist/0.3.1/bin/alchemist"
            )),
            InstallType::Homebrew
        );
        if cfg!(target_os = "linux") {
            assert_eq!(
                detect_install_type_for_path(Path::new("/usr/bin/alchemist")),
                InstallType::Aur
            );
        }
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert_eq!(
                detect_install_type_for_path(Path::new("/usr/local/bin/alchemist")),
                InstallType::DirectBinary
            );
        }
    }

    #[test]
    fn signed_manifest_verifies_with_public_key() {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let signed = UpdateManifestPayload {
            schema_version: MANIFEST_SCHEMA_VERSION,
            channel: UpdateChannel::Stable,
            version: "1.2.3".to_string(),
            release_url: "https://example.invalid/release".to_string(),
            assets: vec![UpdateManifestAsset {
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                filename: "alchemist-linux-x86_64.tar.gz".to_string(),
                url: "https://example.invalid/alchemist-linux-x86_64.tar.gz".to_string(),
                sha256: "0".repeat(64),
                size: 1,
            }],
        };
        let payload = match serde_json::to_vec(&signed) {
            Ok(payload) => payload,
            Err(err) => panic!("serialize test manifest: {err}"),
        };
        let signature = signing_key.sign(&payload);
        let manifest = SignedUpdateManifest {
            signed,
            signature: general_purpose::STANDARD.encode(signature.to_bytes()),
        };
        let public_key = general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes());

        assert!(verify_signed_manifest(&manifest, &public_key).is_ok());
    }
}
