//! Signed GRIT portal updates. Portal sessions never cross the IPC boundary.
use crate::AppState;
use reqwest::Url;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_updater::UpdaterExt;
use work_review_core::error::AppError;

const API: &str = "https://login.gritprojects.se/api/software.php";
const PORTAL: &str = "https://login.gritprojects.se/software/";
const DEFAULT_UPDATE_CHECK_INTERVAL_HOURS: u64 = 24;
static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
fn default_update_check_interval() -> u64 {
    DEFAULT_UPDATE_CHECK_INTERVAL_HOURS
}
fn failure(message: impl ToString) -> AppError {
    AppError::Unknown(message.to_string())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettings {
    pub auto_check: bool,
    #[serde(default)]
    pub include_prerelease: bool,
    pub last_check_time: u64,
    #[serde(default = "default_update_check_interval")]
    pub check_interval_hours: u64,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_check: false,
            include_prerelease: false,
            last_check_time: 0,
            check_interval_hours: 24,
        }
    }
}

fn update_settings_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("update_settings.json")
}

fn load_update_settings_from_dir(data_dir: &Path) -> Result<UpdateSettings, AppError> {
    let settings_path = update_settings_path(data_dir);

    if !settings_path.exists() {
        return Ok(UpdateSettings::default());
    }

    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| AppError::Unknown(format!("读取更新设置失败: {e}")))?;

    serde_json::from_str(&content).map_err(|e| AppError::Unknown(format!("解析更新设置失败: {e}")))
}

fn save_update_settings_to_dir(data_dir: &Path, settings: &UpdateSettings) -> Result<(), AppError> {
    let settings_path = update_settings_path(data_dir);
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Unknown(format!("序列化更新设置失败: {e}")))?;

    std::fs::write(&settings_path, content)
        .map_err(|e| AppError::Unknown(format!("保存更新设置失败: {e}")))?;

    Ok(())
}

fn should_check_for_updates(settings: &UpdateSettings) -> bool {
    if !settings.auto_check {
        return false;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let interval_hours = if settings.check_interval_hours > 0 {
        settings.check_interval_hours
    } else {
        DEFAULT_UPDATE_CHECK_INTERVAL_HOURS
    };
    let elapsed_hours = now.saturating_sub(settings.last_check_time) / 3600;

    elapsed_hours >= interval_hours
}

/// 获取更新检查设置
#[tauri::command]
pub async fn get_update_settings(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<UpdateSettings, AppError> {
    let data_dir = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        state.data_dir.clone()
    };

    load_update_settings_from_dir(&data_dir)
}

/// 保存更新检查设置
#[tauri::command]
pub async fn save_update_settings(
    settings: UpdateSettings,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    let data_dir = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        state.data_dir.clone()
    };

    save_update_settings_to_dir(&data_dir, &settings)
}

/// 判断当前是否应自动检查更新
#[tauri::command]
pub async fn should_check_updates(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<bool, AppError> {
    let data_dir = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        state.data_dir.clone()
    };
    let settings = load_update_settings_from_dir(&data_dir)?;

    Ok(should_check_for_updates(&settings))
}

/// 更新时间检查时间戳
#[tauri::command]
pub async fn update_last_check_time(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    let data_dir = {
        let state = state.lock().map_err(|e| AppError::Unknown(e.to_string()))?;
        state.data_dir.clone()
    };
    let mut settings = load_update_settings_from_dir(&data_dir)?;
    settings.last_check_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    save_update_settings_to_dir(&data_dir, &settings)
}

#[tauri::command]
pub async fn quit_app_for_update(app: AppHandle) -> Result<(), AppError> {
    app.exit(0);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubUpdateInfo {
    current_version: String,
    latest_version: String,
    available: bool,
    auto_update_ready: bool,
    prerelease: bool,
    release_url: String,
    body: Option<String>,
    source: Option<String>,
}
#[derive(Deserialize)]
struct Latest {
    id: String,
    version: String,
}
fn newer(current: &str, candidate: &str) -> bool {
    match (Version::parse(current), Version::parse(candidate)) {
        (Ok(a), Ok(b)) => b.pre.is_empty() && b.build.is_empty() && b > a,
        _ => false,
    }
}
fn client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(failure)
}
async fn latest() -> Result<String, AppError> {
    let response = client()?
        .get(API)
        .query(&[("action", "latest"), ("id", "grit-work")])
        .send()
        .await?
        .error_for_status()?
        .json::<Latest>()
        .await?;
    if response.id != "grit-work" || Version::parse(&response.version).is_err() {
        return Err(failure("Portalen returnerade en ogiltig programversion."));
    }
    Ok(response.version)
}
#[tauri::command]
pub async fn check_github_update() -> Result<GithubUpdateInfo, AppError> {
    let version = latest().await?;
    let portable = crate::data_dir::is_portable_mode()?;
    Ok(GithubUpdateInfo {
        available: newer(env!("CARGO_PKG_VERSION"), &version),
        current_version: env!("CARGO_PKG_VERSION").into(),
        latest_version: version,
        auto_update_ready: cfg!(all(target_os = "windows", target_arch = "x86_64")) && !portable,
        prerelease: false,
        release_url: PORTAL.into(),
        body: None,
        source: Some(
            if portable {
                "portable-release-api"
            } else {
                "GRIT-portalen"
            }
            .into(),
        ),
    })
}
fn status(app: &AppHandle, message: &str) {
    let _ = app.emit("update-status", serde_json::json!({"message": message}));
}
// The remote window has no Tauri capabilities. Only Rust reads HttpOnly cookies,
// and forwards them exclusively to the fixed HTTPS portal origin (no redirects).
async fn portal_cookie(app: &AppHandle) -> Result<String, AppError> {
    let window = if let Some(window) = app.get_webview_window("grit-update-login") {
        window
    } else {
        WebviewWindowBuilder::new(app, "grit-update-login", WebviewUrl::External(
            Url::parse("https://login.gritprojects.se/?redirect_uri=https%3A%2F%2Flogin.gritprojects.se%2Fsoftware%2F").map_err(failure)?))
            .title("Logga in för att uppdatera GRIT Work").inner_size(960.0, 740.0)
            .build().map_err(failure)?
    };
    window.show().map_err(failure)?;
    window.set_focus().map_err(failure)?;
    status(
        app,
        "Logga in i portalfönstret. Uppdateringen fortsätter efter inloggningen.",
    );
    for _ in 0..300 {
        if app.get_webview_window("grit-update-login").is_none() {
            return Err(failure(
                "Inloggningen avbröts. Ingen uppdatering installerades.",
            ));
        }
        let cookies = window
            .cookies_for_url(Url::parse(API).map_err(failure)?)
            .map_err(failure)?;
        let cookie = cookies
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; ");
        if !cookie.is_empty() {
            let response = client()?
                .get(API)
                .header(reqwest::header::COOKIE, &cookie)
                .send()
                .await?;
            if response.status().is_success() {
                window.close().map_err(failure)?;
                return Ok(cookie);
            }
            if response.status() != reqwest::StatusCode::UNAUTHORIZED {
                return Err(failure(
                    "Portalen är tillfälligt otillgänglig. Försök igen senare.",
                ));
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let _ = window.close();
    Err(failure("Inloggningen tog för lång tid. Försök igen."))
}
fn valid_download(url: &Url, version: &str) -> bool {
    if url.scheme() != "https"
        || url.host_str() != Some("login.gritprojects.se")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/api/software.php"
        || url.fragment().is_some()
    {
        return false;
    }
    let pairs = url.query_pairs().collect::<Vec<_>>();
    let value = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_ref())
    };
    pairs.len() == 4
        && value("action") == Some("download")
        && value("id") == Some("grit-work")
        && value("version") == Some(version)
        && value("file").is_some_and(|s| s.ends_with(".exe") && !s.contains(['/', '\\']))
}
#[tauri::command]
pub async fn download_and_install_github_update(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    expected_version: Option<String>,
) -> Result<(), AppError> {
    let _install = INSTALL_LOCK
        .try_lock()
        .map_err(|_| failure("En uppdatering pågår redan."))?;
    if !cfg!(all(target_os = "windows", target_arch = "x86_64"))
        || crate::data_dir::is_portable_mode()?
    {
        return Err(failure(
            "Denna utgåva uppdateras manuellt via GRIT-portalen.",
        ));
    }
    let expected =
        expected_version.ok_or_else(|| failure("Kontrollera och bekräfta versionen först."))?;
    if !newer(env!("CARGO_PKG_VERSION"), &expected) || latest().await? != expected {
        return Err(failure(
            "Versionen är inte längre aktuell. Sök efter uppdateringar igen.",
        ));
    }
    {
        let guard = state.lock().map_err(failure)?;
        if guard.is_recording && !guard.is_paused {
            return Err(failure(
                "Pausa insamlingen innan du installerar uppdateringen.",
            ));
        }
    }
    let cookie = portal_cookie(&app).await?;
    let mut endpoint = Url::parse(API).map_err(failure)?;
    endpoint.query_pairs_mut().extend_pairs([
        ("action", "updater"),
        ("id", "grit-work"),
        ("version", expected.as_str()),
        ("arch", "x64"),
    ]);
    // Read authenticated hash independently of the signature, binding the exact bytes
    // to the confirmed version (a valid signature alone can belong to an older build).
    let metadata: serde_json::Value = client()?
        .get(endpoint.clone())
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let digest = metadata["sha256"]
        .as_str()
        .ok_or_else(|| failure("Kontrollsumma saknas."))?;
    let size = metadata["size"]
        .as_u64()
        .ok_or_else(|| failure("Filstorlek saknas."))?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(failure)?
        .header("Cookie", cookie)
        .map_err(failure)?
        .timeout(Duration::from_secs(300))
        .configure_client(|builder| builder.redirect(reqwest_updater::redirect::Policy::none()))
        .build()
        .map_err(failure)?;
    let update = updater
        .check()
        .await
        .map_err(failure)?
        .ok_or_else(|| failure("Ingen installerbar uppdatering finns."))?;
    if update.version != expected || !valid_download(&update.download_url, &expected) {
        return Err(failure(
            "Uppdateringen stämmer inte med den bekräftade versionen.",
        ));
    }
    let mut downloaded = 0_u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let message = match total.filter(|t| *t > 0) {
                    Some(total) => format!(
                        "Hämtar uppdatering: {} %",
                        downloaded.saturating_mul(100) / total
                    ),
                    None => format!("Hämtar uppdatering: {} kB", downloaded / 1024),
                };
                status(&app, &message);
            },
            || {},
        )
        .await
        .map_err(failure)?;
    if bytes.len() as u64 != size || hex::encode(Sha256::digest(&bytes)) != digest {
        return Err(failure(
            "Installationsfilens kontrollsumma eller storlek stämmer inte.",
        ));
    }
    if latest().await? != expected {
        return Err(failure("Versionen ändrades under hämtningen. Sök igen."));
    }
    {
        let guard = state.lock().map_err(failure)?;
        if guard.is_recording && !guard.is_paused {
            return Err(failure("Pausa insamlingen och försök igen."));
        }
    }
    status(
        &app,
        "Signaturen är verifierad. Installerar uppdateringen …",
    );
    update.install(bytes).map_err(failure)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_never_downgrade_or_accept_prereleases() {
        assert!(newer("1.1.4-rc.4", "1.1.4"));
        assert!(newer("1.9.9", "1.10.0"));
        for candidate in ["1.1.4", "1.1.3", "1.2.0-rc.1", "bad", "1.1.4+other"] {
            assert!(!newer("1.1.4", candidate));
        }
    }
    #[test]
    fn download_is_bound_to_portal_and_version() {
        let good = "https://login.gritprojects.se/api/software.php?action=download&id=grit-work&version=1.2.0&file=setup.exe";
        assert!(valid_download(&Url::parse(good).unwrap(), "1.2.0"));
        assert!(!valid_download(&Url::parse(good).unwrap(), "1.3.0"));
        for bad in [
            good.replace("https:", "http:"),
            good.replace("login.gritprojects.se", "evil.example"),
            good.replace("grit-work", "other"),
            format!("{good}&version=1.3.0"),
            good.replace("setup.exe", "../setup.exe"),
        ] {
            assert!(!valid_download(&Url::parse(&bad).unwrap(), "1.2.0"));
        }
    }
}
