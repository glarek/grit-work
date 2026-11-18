//! Grit's durable report workflow. Only this module may write time to Spiris.
use crate::{grit_engine, AppState};
use chrono::{Local, NaiveDate, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::State;
use uuid::Uuid;

const API: &str = "https://publicapi.tid.vismaspcs.se/v1";
static BUSY: AtomicBool = AtomicBool::new(false);
static CANCEL: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();
struct Operation;
impl Operation {
    fn start() -> Result<Self, String> {
        BUSY.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| {
                "Ett Grit-jobb körs redan. Vänta eller avbryt rapportjobbet.".to_string()
            })?;
        Ok(Self)
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub archive_dir: String,
    pub provider: String,
    pub model: String,
    pub user_id: i64,
    pub article_id: i64,
    pub activity_id: i64,
    pub time_code_id: i64,
    pub billable: bool,
    pub timeout_seconds: u64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            archive_dir: String::new(),
            provider: "codex".into(),
            model: "gpt-6.2-sol".into(),
            user_id: 0,
            article_id: 0,
            activity_id: 0,
            time_code_id: 0,
            billable: false,
            timeout_seconds: 600,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub number: String,
    pub active: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub id: i64,
    pub name: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Catalog {
    projects: Vec<Project>,
    users: Vec<Choice>,
    articles: Vec<Choice>,
    activities: Vec<Choice>,
    time_codes: Vec<Choice>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub project_id: i64,
    pub activity_id: i64,
    pub article_id: i64,
    pub start: i64,
    pub end: i64,
    pub minutes: i64,
    pub description: String,
    pub evidence_ids: Vec<i64>,
    pub needs_review: bool,
    pub remote_id: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub date: String,
    pub revision: String,
    pub status: String,
    pub summary: String,
    pub rows: Vec<Row>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Credentials {
    application_id: String,
    secret: String,
    static_token: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureSettings {
    interval_seconds: u64,
    display_mode: work_review_core::config::ScreenshotDisplayMode,
    jpeg_quality: u8,
    max_image_width: u32,
    width_mode: work_review_core::config::ScreenshotWidthMode,
}
impl CaptureSettings {
    fn from_config(c: &work_review_core::config::AppConfig) -> Self {
        Self {
            interval_seconds: c.screenshot_interval,
            display_mode: c.storage.screenshot_display_mode,
            jpeg_quality: c.storage.jpeg_quality,
            max_image_width: c.storage.max_image_width,
            width_mode: c.storage.screenshot_width_mode,
        }
    }
    fn validate(&self) -> Result<(), String> {
        if !(30..=3600).contains(&self.interval_seconds) {
            return Err("Välj ett intervall mellan 30 och 3 600 sekunder.".into());
        }
        if !(30..=95).contains(&self.jpeg_quality) || !(800..=16384).contains(&self.max_image_width)
        {
            return Err("Bildkvalitet måste vara 30–95 och maxbredd 800–16 384 pixlar.".into());
        }
        Ok(())
    }
}
#[derive(Serialize)]
struct CaptureMonitor {
    name: String,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    scale_factor: f64,
}
#[tauri::command]
pub async fn grit_save_capture_settings(
    settings: CaptureSettings,
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let _op = Operation::start()?;
    settings.validate()?;
    let mut config = state.lock().map_err(error)?.config.clone();
    config.screenshot_interval = settings.interval_seconds;
    config.storage.screenshot_display_mode = settings.display_mode;
    config.storage.jpeg_quality = settings.jpeg_quality;
    config.storage.max_image_width = settings.max_image_width;
    config.storage.screenshot_width_mode = settings.width_mode;
    crate::commands::persist_app_config(config, app, state.inner()).map_err(error)
}

#[derive(Serialize)]
pub struct Status {
    capture: CaptureSettings,
    monitors: Vec<CaptureMonitor>,
    settings: Settings,
    local_dir: String,
    yesterday: String,
    pending_dates: Vec<String>,
    projects: Vec<Project>,
    users: Vec<Choice>,
    articles: Vec<Choice>,
    activities: Vec<Choice>,
    time_codes: Vec<Choice>,
    credential_saved: bool,
    report: Option<Report>,
    last_error: Option<String>,
}
fn db(dir: &Path) -> Result<Connection, String> {
    std::fs::create_dir_all(dir).map_err(error)?;
    let db = Connection::open(dir.join("grit.db")).map_err(error)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(error)?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS reports(date TEXT PRIMARY KEY,body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS days(date TEXT PRIMARY KEY,dismissed INTEGER NOT NULL DEFAULT 0,archived INTEGER NOT NULL DEFAULT 0);").map_err(error)?;
    Ok(db)
}
fn get<T: serde::de::DeserializeOwned>(db: &Connection, key: &str) -> Result<Option<T>, String> {
    let v: Option<String> = db
        .query_row("SELECT value FROM kv WHERE key=?", [key], |r| r.get(0))
        .optional()
        .map_err(error)?;
    v.map(|s| serde_json::from_str(&s).map_err(error))
        .transpose()
}
fn put(db: &Connection, key: &str, v: &impl Serialize) -> Result<(), String> {
    db.execute("INSERT INTO kv(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,serde_json::to_string(v).map_err(error)?]).map_err(error)?;
    Ok(())
}
fn invalidate_draft_revisions(db: &Connection) -> Result<(), String> {
    let mut q = db.prepare("SELECT body FROM reports").map_err(error)?;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    for body in rows {
        let mut r: Report = serde_json::from_str(&body).map_err(error)?;
        if r.status == "draft" {
            r.revision = Uuid::new_v4().to_string();
            save_report(db, &r)?;
        }
    }
    Ok(())
}
fn report(db: &Connection, date: &str) -> Result<Option<Report>, String> {
    let v: Option<String> = db
        .query_row("SELECT body FROM reports WHERE date=?", [date], |r| {
            r.get(0)
        })
        .optional()
        .map_err(error)?;
    v.map(|s| serde_json::from_str(&s).map_err(error))
        .transpose()
}
fn save_report(db: &Connection, r: &Report) -> Result<(), String> {
    db.execute("INSERT INTO reports(date,body) VALUES(?1,?2) ON CONFLICT(date) DO UPDATE SET body=excluded.body",params![r.date,serde_json::to_string(r).map_err(error)?]).map_err(error)?;
    Ok(())
}
fn local_dir(state: &State<'_, Arc<Mutex<AppState>>>) -> Result<PathBuf, String> {
    Ok(state.lock().map_err(error)?.data_dir.clone())
}
fn past_date(date: &str) -> Result<NaiveDate, String> {
    let d =
        NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| "Ogiltigt datum".to_string())?;
    if d.format("%Y-%m-%d").to_string() != date || d >= Local::now().date_naive() {
        return Err("Välj ett avslutat kalenderdygn.".into());
    }
    Ok(d)
}
fn keyring() -> Result<keyring::Entry, String> {
    keyring::Entry::new("se.grit.work", "spiris")
        .map_err(|_| "Systemets nyckellager är inte tillgängligt.".into())
}
fn credentials() -> Result<Credentials, String> {
    let s = keyring()?.get_password().map_err(|_| {
        "Spara Spiris-uppgifterna i inställningarna och lås upp systemets nyckellager.".to_string()
    })?;
    serde_json::from_str(&s).map_err(|_| "Ogiltiga uppgifter i nyckellagret.".into())
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("GritWork/0.1")
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(error)
}
async fn token(client: &reqwest::Client) -> Result<String, String> {
    let c = credentials()?;
    if !c.static_token.is_empty() {
        return Ok(c.static_token);
    }
    let r = client
        .post(format!("{API}/Auth/Token"))
        .basic_auth(c.application_id, Some(c.secret))
        .json(&json!({}))
        .send()
        .await
        .map_err(|_| "Spiris-inloggning: nätverksfel.".to_string())?;
    let status = r.status();
    if !status.is_success() {
        return Err(format!("Spiris-inloggning nekades (HTTP {}). Kontrollera API-uppgifter och tillåtna IP-adresser.",status.as_u16()));
    }
    let v: Value = r
        .json()
        .await
        .map_err(|_| "Ogiltigt svar från Spiris-inloggningen.".to_string())?;
    v["accessToken"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or("Spiris returnerade ingen token.".into())
}
async fn list(
    client: &reqwest::Client,
    token: &str,
    path: &str,
    filters: &[(&str, String)],
) -> Result<Vec<Value>, String> {
    let mut result = vec![];
    for page in 1..=1000 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let mut query = vec![("page", page.to_string()), ("pageSize", "100".into())];
        query.extend_from_slice(filters);
        let mut attempts = 0;
        let v: Value = loop {
            let r = client
                .get(format!("{API}/{path}"))
                .bearer_auth(token)
                .query(&query)
                .send()
                .await
                .map_err(|_| format!("Spiris: nätverksfel vid läsning av {path}."))?;
            if r.status().as_u16() == 429 && attempts < 3 {
                let wait = r
                    .headers()
                    .get("Retry-After")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(2)
                    .clamp(1, 60);
                tokio::time::sleep(Duration::from_secs(wait)).await;
                attempts += 1;
                continue;
            }
            if !r.status().is_success() {
                return Err(format!(
                    "Spiris {path}: HTTP {}. Kontrollera API-behörigheter.",
                    r.status().as_u16()
                ));
            }
            break r
                .json()
                .await
                .map_err(|_| format!("Spiris {path}: ogiltigt JSON-svar."))?;
        };
        let items = v["items"]
            .as_array()
            .ok_or_else(|| format!("Spiris {path}: listformat saknas."))?;
        result.extend(items.iter().cloned());
        let pages = v["totalPages"]
            .as_u64()
            .ok_or("Spiris saknar sidinformation.")?;
        if page >= pages {
            return Ok(result);
        }
    }
    Err("Spiris-listan överskrider säker sidgräns. Inget ofullständigt register används.".into())
}
fn choices(items: Vec<Value>) -> Vec<Choice> {
    items
        .into_iter()
        .filter(|v| v["isActive"].as_bool() != Some(false))
        .filter_map(|v| {
            Some(Choice {
                id: v["id"].as_i64()?,
                name: v["name"]
                    .as_str()
                    .or(v["title"].as_str())
                    .unwrap_or("Namnlös")
                    .to_string(),
            })
        })
        .collect()
}
async fn catalog(client: &reqwest::Client, t: &str) -> Result<Catalog, String> {
    let p = list(client, t, "Core/Projects", &[]).await?;
    let projects = p
        .into_iter()
        .map(|v| {
            Ok(Project {
                id: v["id"].as_i64().ok_or("Projekt-ID saknas")?,
                name: v["title"].as_str().unwrap_or("").into(),
                number: v["orderNumber"].as_str().unwrap_or("").into(),
                active: v["status"]["isCompletedStatus"].as_bool() == Some(false),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let users = list(client, t, "Admin/Users", &[])
        .await?
        .into_iter()
        .filter_map(|v| {
            Some(Choice {
                id: v["id"].as_i64()?,
                name: format!(
                    "{} {}",
                    v["firstName"].as_str().unwrap_or(""),
                    v["lastName"].as_str().unwrap_or("")
                )
                .trim()
                .to_string(),
            })
        })
        .collect();
    let activities = choices(list(client, t, "Admin/Activities", &[]).await?);
    let time_codes = choices(list(client, t, "Admin/TimeCodes", &[]).await?);
    let articles = choices(list(client, t, "Admin/Articles", &[("type", "time".into())]).await?);
    Ok(Catalog {
        projects,
        users,
        activities,
        time_codes,
        articles,
    })
}
fn local_activities(state: &Arc<Mutex<AppState>>, date: &str) -> Result<Value, String> {
    let s = state.lock().map_err(error)?;
    let a = s
        .database
        .get_activities_in_range(Some(date), Some(date), 10_000)
        .map_err(error)?;
    if a.len() >= 10_000 {
        return Err("Dygnet har fler poster än exportgränsen. Underlaget bevaras.".into());
    }
    serde_json::to_value(a).map_err(error)
}
fn evidence(state: &Arc<Mutex<AppState>>, dir: &Path, date: &str) -> Result<Value, String> {
    let database = db(dir)?;
    let archived: bool = database
        .query_row("SELECT archived FROM days WHERE date=?", [date], |r| {
            r.get(0)
        })
        .optional()
        .map_err(error)?
        .unwrap_or(false);
    if archived {
        let settings: Settings = get(&database, "settings")?.unwrap_or_default();
        return grit_engine::load_archive(Path::new(&settings.archive_dir), date);
    }
    let a = local_activities(state, date)?;
    if a.as_array().is_some_and(|a| !a.is_empty()) {
        return Ok(a);
    }
    let db = db(dir)?;
    let settings: Settings = get(&db, "settings")?.unwrap_or_default();
    if settings.archive_dir.is_empty() {
        return Err("Inget underlag finns för datumet.".into());
    }
    grit_engine::load_archive(Path::new(&settings.archive_dir), date)
}
fn collect_dates(state: &Arc<Mutex<AppState>>, dir: &Path) -> Result<Vec<String>, String> {
    let mut dates = state
        .lock()
        .map_err(error)?
        .database
        .grit_activity_dates()
        .map_err(error)?;
    let db = db(dir)?;
    let mut q = db.prepare("SELECT date FROM days").map_err(error)?;
    for d in q.query_map([], |r| r.get::<_, String>(0)).map_err(error)? {
        dates.push(d.map_err(error)?);
    }
    dates.retain(|d| past_date(d).is_ok());
    dates.sort();
    dates.dedup();
    dates.reverse();
    Ok(dates)
}
fn status(state: &Arc<Mutex<AppState>>, dir: &Path) -> Result<Status, String> {
    let db = db(dir)?;
    let settings = get(&db, "settings")?.unwrap_or_default();
    let cat: Catalog = get(&db, "catalog")?.unwrap_or_default();
    let yesterday = (Local::now().date_naive() - chrono::Days::new(1)).to_string();
    let mut pending_dates = vec![];
    for date in collect_dates(state, dir)? {
        let dismissed: bool = db
            .query_row("SELECT dismissed FROM days WHERE date=?", [&date], |r| {
                r.get(0)
            })
            .optional()
            .map_err(error)?
            .unwrap_or(false);
        if !dismissed && report(&db, &date)?.is_none() {
            pending_dates.push(date);
        }
    }
    let capture = CaptureSettings::from_config(&state.lock().map_err(error)?.config);
    let monitors = crate::APP_HANDLE
        .get()
        .and_then(|app| app.available_monitors().ok())
        .unwrap_or_default()
        .into_iter()
        .map(|m| CaptureMonitor {
            name: m.name().cloned().unwrap_or_else(|| "Skärm".into()),
            width: m.size().width,
            height: m.size().height,
            x: m.position().x,
            y: m.position().y,
            scale_factor: m.scale_factor(),
        })
        .collect();
    Ok(Status {
        capture,
        monitors,
        settings,
        local_dir: dir.display().to_string(),
        report: report(&db, &yesterday)?,
        yesterday,
        pending_dates,
        projects: cat.projects,
        users: cat.users,
        articles: cat.articles,
        activities: cat.activities,
        time_codes: cat.time_codes,
        credential_saved: credentials().is_ok(),
        last_error: get::<String>(&db, "capture_error")?.or(get(&db, "last_error")?),
    })
}
#[tauri::command]
pub async fn grit_status(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Status, String> {
    status(state.inner(), &local_dir(&state)?)
}
#[tauri::command]
pub async fn grit_save_settings(
    settings: Settings,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let _op = Operation::start()?;
    if !["codex", "grok", "agy"].contains(&settings.provider.as_str())
        || settings.model.is_empty()
        || settings.model.len() > 100
        || !settings
            .model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-._/".contains(c))
    {
        return Err("Ogiltig agent eller modell.".into());
    }
    if !(30..=3600).contains(&settings.timeout_seconds) {
        return Err("Tidsgränsen måste vara 30–3600 sekunder.".into());
    }
    let dir = local_dir(&state)?;
    if !settings.archive_dir.is_empty() {
        let p = Path::new(&settings.archive_dir);
        if !p.is_absolute() {
            return Err("Arkivet måste vara en absolut sökväg.".into());
        }
        std::fs::create_dir_all(p).map_err(error)?;
        let p = p.canonicalize().map_err(error)?;
        let d = dir.canonicalize().map_err(error)?;
        if p.starts_with(&d) || d.starts_with(&p) {
            return Err("Arkivet och lokal appdata måste vara separata kataloger.".into());
        }
    }
    let database = db(&dir)?;
    let previous: Settings = get(&database, "settings")?.unwrap_or_default();
    if previous.archive_dir != settings.archive_dir {
        let count: i64 = database
            .query_row("SELECT COUNT(*) FROM days WHERE archived=1", [], |r| {
                r.get(0)
            })
            .map_err(error)?;
        if count > 0 {
            return Err("Arkivet innehåller tidigare dygn. Flytt av ett befintligt arkiv behöver göras med verifierad migrering; destinationen har inte ändrats.".into());
        }
    }
    invalidate_draft_revisions(&database)?;
    put(&database, "settings", &settings)
}
#[tauri::command]
pub async fn grit_save_credentials(
    application_id: String,
    secret: String,
    static_token: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let _op = Operation::start()?;
    if static_token.trim().is_empty() && (application_id.trim().is_empty() || secret.is_empty()) {
        return Err("Ange applikations-ID och hemlighet, eller en statisk token.".into());
    }
    let c = Credentials {
        application_id: application_id.trim().into(),
        secret,
        static_token: static_token.trim().into(),
    };
    let serialized = serde_json::to_string(&c).map_err(error)?;
    let entry = keyring()?;
    entry
        .set_password(&serialized)
        .map_err(|_| "Kunde inte spara i systemets nyckellager.".to_string())?;
    if entry
        .get_password()
        .map_err(|_| "Kunde inte verifiera nyckellagret.".to_string())?
        != serialized
    {
        return Err("Nyckellagret kunde inte verifieras.".into());
    }
    // A different account must never inherit a previous account's project cache or approval.
    let db = db(&local_dir(&state)?)?;
    put(&db, "catalog", &Catalog::default())?;
    invalidate_draft_revisions(&db)?;
    Ok(())
}
#[tauri::command]
pub async fn grit_sync_spiris(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Status, String> {
    let _op = Operation::start()?;
    let dir = local_dir(&state)?;
    let c = client()?;
    let t = token(&c).await?;
    let cat = catalog(&c, &t).await?;
    put(&db(&dir)?, "catalog", &cat)?;
    status(state.inner(), &dir)
}
#[tauri::command]
pub async fn grit_get_report(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Option<Report>, String> {
    past_date(&date)?;
    report(&db(&local_dir(&state)?)?, &date)
}
#[tauri::command]
pub async fn grit_dismiss(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    past_date(&date)?;
    db(&local_dir(&state)?)?.execute("INSERT INTO days(date,dismissed) VALUES(?1,1) ON CONFLICT(date) DO UPDATE SET dismissed=1",[date]).map_err(error)?;
    Ok(())
}
#[tauri::command]
pub async fn grit_cancel() -> Result<(), String> {
    CANCEL
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .store(true, Ordering::SeqCst);
    Ok(())
}

fn validate_report(r: &Report, cat: &Catalog, ev: &Value, strict: bool) -> Result<(), String> {
    let d = past_date(&r.date)?;
    if r.rows.is_empty() || r.rows.len() > 500 || r.summary.len() > 20_000 {
        return Err("Rapporten måste innehålla 1–500 rader och en rimlig sammanfattning.".into());
    }
    let a = ev.as_array().ok_or("Underlaget är inte en lista.")?;
    let mut ids = HashSet::new();
    let mut intervals = vec![];
    for row in &r.rows {
        if row.id.is_empty()
            || row.id.len() > 80
            || !row
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
            || !ids.insert(&row.id)
        {
            return Err("Varje rad måste ha ett unikt giltigt ID.".into());
        }
        let start = Local
            .timestamp_opt(row.start, 0)
            .single()
            .ok_or("Ogiltig starttid")?;
        let end = Local
            .timestamp_opt(row.end, 0)
            .single()
            .ok_or("Ogiltig sluttid")?;
        if start.date_naive() != d
            || end.date_naive() != d
            || row.end <= row.start
            || row.minutes <= 0
            || row.minutes > 1500
            || row.minutes * 60 > row.end - row.start
        {
            return Err(
                "Tider måste vara positiva och rymmas inom samma kalenderdygn och intervall."
                    .into(),
            );
        }
        if row.description.trim().is_empty() || row.description.chars().count() > 2000 {
            return Err("Varje rad behöver en beskrivning på högst 2 000 tecken.".into());
        }
        if strict && row.needs_review {
            return Err("Granska och bekräfta alla osäkra rader innan överföring.".into());
        }
        if row.project_id == 0 && !strict && row.needs_review {
        } else if !cat
            .projects
            .iter()
            .any(|p| p.id == row.project_id && p.active)
        {
            return Err("Rapporten innehåller ett okänt eller avslutat projekt. Synkronisera och välj ett aktivt projekt.".into());
        }
        for (id, choices, label) in [
            (row.article_id, &cat.articles, "tidsartikel"),
            (row.activity_id, &cat.activities, "aktivitet"),
        ] {
            if strict && !choices.iter().any(|c| c.id == id) {
                return Err(format!("Välj en giltig {label} för varje rad."));
            }
        }
        if row.evidence_ids.is_empty() {
            return Err("Varje rad måste hänvisa till verkligt underlag.".into());
        }
        let mut covered = vec![];
        for id in &row.evidence_ids {
            let v = a
                .iter()
                .find(|v| v["id"].as_i64() == Some(*id))
                .ok_or("Rapporten hänvisar till ett underlag som inte finns.")?;
            let t = v["timestamp"].as_i64().ok_or("Underlagets tid saknas")?;
            let duration = v["duration"]
                .as_i64()
                .ok_or("Underlagets tidslängd saknas")?
                .max(0);
            let beginning = t.saturating_sub(duration);
            let lo = beginning.max(row.start);
            let hi = t.min(row.end);
            if hi > lo {
                covered.push((lo, hi));
            }
        }
        covered.sort();
        let mut seconds = 0;
        let mut last = i64::MIN;
        for (lo, hi) in covered {
            seconds += (hi - lo.max(last)).max(0);
            last = last.max(hi);
        }
        if row.minutes * 60 > seconds {
            return Err(
                "Föreslagen tid överstiger det registrerade underlaget. Justera raden.".into(),
            );
        }
        intervals.push((row.start, row.end));
    }
    intervals.sort();
    if intervals.windows(2).any(|w| w[0].1 > w[1].0) {
        return Err("Rapportens tidsintervall överlappar.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn grit_generate(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Report, String> {
    let op = Arc::new(Operation::start()?);
    past_date(&date)?;
    let dir = local_dir(&state)?;
    let db = db(&dir)?;
    if report(&db, &date)?.is_some_and(|r| !["draft", "failed"].contains(&r.status.as_str())) {
        return Err("Rapporten är låst eller körs redan.".into());
    }
    let settings: Settings = get(&db, "settings")?.unwrap_or_default();
    let cat: Catalog = get(&db, "catalog")?.unwrap_or_default();
    if cat.projects.iter().all(|p| !p.active) {
        return Err("Anslut Spiris och synkronisera projekt först.".into());
    }
    let ev = evidence(state.inner(), &dir, &date)?;
    if ev.as_array().is_none_or(|a| a.is_empty()) {
        return Err("Inget underlag för datumet.".into());
    }
    let mut r = Report {
        date: date.clone(),
        revision: Uuid::new_v4().to_string(),
        status: "running".into(),
        summary: String::new(),
        rows: vec![],
        error: None,
    };
    let cancel = CANCEL
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone();
    cancel.store(false, Ordering::SeqCst);
    let project_data = json!({"projects":cat.projects,"activities":cat.activities,"articles":cat.articles,"default_activity_id":settings.activity_id,"default_article_id":settings.article_id});
    let mut ev_copy = ev.clone();
    let local_root = dir.canonicalize().map_err(error)?;
    let archive_root = Path::new(&settings.archive_dir).canonicalize().ok();
    for a in ev_copy.as_array_mut().ok_or("Ogiltigt underlag")? {
        if let Some(raw) = a["screenshot_path"].as_str().filter(|s| !s.is_empty()) {
            let p = Path::new(raw);
            let full = if p.is_absolute() {
                p.to_path_buf()
            } else {
                dir.join(p)
            };
            let actual = full
                .canonicalize()
                .map_err(|_| "En underlagsbild saknas.".to_string())?;
            if !actual.starts_with(&local_root)
                && !archive_root
                    .as_ref()
                    .is_some_and(|root| actual.starts_with(root))
            {
                return Err("Bildsökvägen ligger utanför underlaget.".into());
            }
            a["screenshot_path"] = json!(actual);
        }
    }
    let workspace = dir.join("jobs").join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&workspace).map_err(error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700))
            .map_err(error)?;
    }
    save_report(&db, &r)?;
    let worker_lease = op.clone();
    let workspace_copy = workspace.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _lease = worker_lease;
        crate::grit_cli::run_agent(
            &settings.provider,
            &settings.model,
            &workspace_copy,
            &ev_copy,
            &project_data,
            settings.timeout_seconds,
            cancel,
        )
    })
    .await
    .map_err(error)
    .and_then(|r| r);
    // Job scratch contains sensitive evidence; retain the report, not extra image copies.
    let _ = std::fs::remove_dir_all(workspace);
    let checked = result.and_then(|v| {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Output {
            summary: String,
            rows: Vec<Row>,
        }
        let o: Output = serde_json::from_value(v)
            .map_err(|e| format!("Agentens svar följer inte rapportschemat: {e}"))?;
        r.summary = o.summary;
        r.rows = o.rows;
        for row in &mut r.rows {
            row.remote_id = None;
            row.needs_review = true;
        }
        validate_report(&r, &cat, &ev, false)
    });
    match checked {
        Ok(()) => r.status = "draft".into(),
        Err(e) => {
            r.status = "failed".into();
            r.error = Some(e);
        }
    }
    save_report(&db, &r)?;
    Ok(r)
}
#[tauri::command]
pub async fn grit_save_draft(
    mut report: Report,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Report, String> {
    let _op = Operation::start()?;
    let dir = local_dir(&state)?;
    let db = db(&dir)?;
    let previous = self::report(&db, &report.date)?.ok_or("Rapporten finns inte.")?;
    if previous.status != "draft" || previous.revision != report.revision {
        return Err("Rapporten har ändrats eller är låst. Läs in den igen.".into());
    }
    if report.rows.iter().any(|r| r.remote_id.is_some()) {
        return Err("Utkast kan inte ange Spiris-kvitton.".into());
    }
    let cat = get(&db, "catalog")?.unwrap_or_default();
    let ev = evidence(state.inner(), &dir, &report.date)?;
    validate_report(&report, &cat, &ev, false)?;
    report.revision = Uuid::new_v4().to_string();
    report.status = "draft".into();
    report.error = None;
    save_report(&db, &report)?;
    Ok(report)
}
#[derive(Serialize)]
pub struct ArchiveResult {
    archived: usize,
    message: String,
}
fn archive_all(state: &Arc<Mutex<AppState>>, dir: &Path) -> Result<ArchiveResult, String> {
    let _op = Operation::start()?;
    let db = db(dir)?;
    let settings: Settings = get(&db, "settings")?.unwrap_or_default();
    if settings.archive_dir.is_empty() {
        return Err("Välj långtidslagring först. Lokalt underlag behålls.".into());
    }
    let archive = Path::new(&settings.archive_dir);
    let mut archived = 0;
    for date in collect_dates(state, dir)? {
        let ev = local_activities(state, &date)?;
        if ev.as_array().is_none_or(|a| a.is_empty()) {
            continue;
        }
        grit_engine::archive_day(dir, archive, &date, &ev)?;
        // Keep capture and manual database edits outside the final snapshot/delete window.
        let guard = state.lock().map_err(error)?;
        let current = serde_json::to_value(
            guard
                .database
                .get_activities_in_range(Some(&date), Some(&date), 10_000)
                .map_err(error)?,
        )
        .map_err(error)?;
        if current != ev {
            return Err("Underlaget ändrades under arkivering. Inget rensades.".into());
        }
        for a in ev.as_array().ok_or("Ogiltigt underlag")? {
            if let Some(p) = a["screenshot_path"].as_str().filter(|s| !s.is_empty()) {
                if guard
                    .database
                    .grit_screenshot_shared(p, &date)
                    .map_err(error)?
                {
                    return Err("En bild delas mellan dygn. Underlaget behålls.".into());
                }
            }
        }
        // Keep logs until images are safely removed; failures are recoverable from the verified archive.
        db.execute("INSERT INTO days(date,archived) VALUES(?1,1) ON CONFLICT(date) DO UPDATE SET archived=1",[&date]).map_err(error)?;
        grit_engine::clean_archived_day(dir, archive, &date, &ev)?;
        guard
            .database
            .delete_activities_by_date(&date)
            .map_err(error)?;
        db.execute("INSERT INTO days(date,archived) VALUES(?1,1) ON CONFLICT(date) DO UPDATE SET archived=1",[&date]).map_err(error)?;
        archived += 1;
    }
    put(&db, "last_error", &Option::<String>::None)?;
    Ok(ArchiveResult {
        archived,
        message: format!("{archived} dygn verifierade och rensade lokalt."),
    })
}
#[tauri::command]
pub async fn grit_archive(state: State<'_, Arc<Mutex<AppState>>>) -> Result<ArchiveResult, String> {
    let dir = local_dir(&state)?;
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || archive_all(&state, &dir))
        .await
        .map_err(error)?
}
async fn existing(
    c: &reqwest::Client,
    t: &str,
    date: &str,
    user: i64,
) -> Result<Vec<Value>, String> {
    list(
        c,
        t,
        "Core/TimeReports",
        &[
            ("filter.from", format!("{date} 00:00:00")),
            ("filter.to", format!("{date} 23:59:59")),
            ("filter.userIds", user.to_string()),
        ],
    )
    .await
}
fn marker(db: &Connection, date: &str, row: &Row) -> Result<String, String> {
    let install: String = match get(db, "installation")? {
        Some(i) => i,
        None => {
            let id = Uuid::new_v4().to_string();
            put(db, "installation", &id)?;
            id
        }
    };
    Ok(format!("grit-work:{install}:{date}:{}", row.id))
}
fn settings_match(db: &Connection, date: &str, s: &Settings) -> Result<(), String> {
    let key = format!("submission:{date}");
    let previous: Option<Settings> = get(db, &key)?;
    if let Some(p) = previous {
        if p.user_id != s.user_id || p.billable != s.billable || p.time_code_id != s.time_code_id {
            return Err(
                "Rapportens överföringsinställningar har ändrats. Återställ dem före avstämning."
                    .into(),
            );
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn grit_submit(
    date: String,
    revision: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Report, String> {
    let _op = Operation::start()?;
    past_date(&date)?;
    let dir = local_dir(&state)?;
    let db = db(&dir)?;
    let mut r = report(&db, &date)?.ok_or("Rapporten finns inte.")?;
    if r.status != "draft" || r.revision != revision {
        return Err("Rapporten är ändrad, redan skickad eller behöver avstämning.".into());
    }
    let s: Settings = get(&db, "settings")?.unwrap_or_default();
    let c = client()?;
    let t = token(&c).await?;
    let previous_catalog: Catalog = get(&db, "catalog")?.unwrap_or_default();
    let cat = catalog(&c, &t).await?;
    put(&db, "catalog", &cat)?;
    if r.rows.iter().any(|row| {
        let before = previous_catalog.projects.iter().find(|p| p.id == row.project_id);
        let after = cat.projects.iter().find(|p| p.id == row.project_id);
        !matches!((before, after), (Some(a), Some(b)) if a.name == b.name && a.number == b.number && b.active)
    }) {
        invalidate_draft_revisions(&db)?;
        return Err("Projektregistret har ändrats. Läs in och granska rapporten igen.".into());
    }
    if !cat.users.iter().any(|v| v.id == s.user_id)
        || !cat.time_codes.iter().any(|v| v.id == s.time_code_id)
    {
        return Err("Välj giltig användare och tidskod i inställningarna.".into());
    }
    let ev = evidence(state.inner(), &dir, &date)?;
    validate_report(&r, &cat, &ev, true)?;
    let project_ids: HashSet<i64> = r.rows.iter().map(|row| row.project_id).collect();
    for id in project_ids {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let response = c
            .get(format!("{API}/Core/Projects/{id}"))
            .bearer_auth(&t)
            .send()
            .await
            .map_err(|_| "Kunde inte kontrollera projektbehörighet.".to_string())?;
        if !response.status().is_success() {
            return Err(format!("Projekt {id} kunde inte verifieras i Spiris."));
        }
        let project: Value = response
            .json()
            .await
            .map_err(|_| "Ogiltigt projektsvar.".to_string())?;
        if project["status"]["isCompletedStatus"].as_bool() != Some(false) {
            return Err("Projektet har avslutats. Uppdatera rapporten.".into());
        }
        let member = project["allIsMember"].as_bool() == Some(true)
            || project["members"]
                .as_array()
                .is_some_and(|members| members.iter().any(|m| m["id"].as_i64() == Some(s.user_id)));
        if !member {
            return Err(format!("Vald användare är inte medlem i projekt {id}."));
        }
    }
    let remote = existing(&c, &t, &date, s.user_id).await?;
    // Conservatively refuse a second report for a day with pre-existing entries.
    // This catches manually reported time too; do not infer non-overlap from missing clock fields.
    if !remote.is_empty() {
        return Err("Det finns redan tid i Spiris för användaren och datumet. Stäm av den innan du skickar från Grit Work.".into());
    }
    put(&db, &format!("submission:{date}"), &s)?;
    r.status = "uploading".into();
    r.error = None;
    save_report(&db, &r)?;
    for i in 0..r.rows.len() {
        let row = &r.rows[i];
        let mark = marker(&db, &date, row)?;
        let hours = row.minutes as f64 / 60.0;
        let payload = json!({"date":format!("{date} 00:00:00"),"hours":hours,"invoiceableHours":if s.billable {hours}else{0.0},"comment":row.description,"internalComment":mark,"userId":s.user_id,"projectId":row.project_id,"activityId":row.activity_id,"timeCodeId":s.time_code_id,"timeArticleId":row.article_id});
        // Persist intent before POST. A crash after this point is an ambiguous submission.
        put(&db, &format!("attempt:{date}:{}", row.id), &payload)?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let response = c
            .post(format!("{API}/Core/TimeReports"))
            .bearer_auth(&t)
            .json(&payload)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                match response
                    .json::<Value>()
                    .await
                    .ok()
                    .and_then(|v| v["id"].as_i64())
                {
                    Some(id) if id > 0 => {
                        r.rows[i].remote_id = Some(id);
                        save_report(&db, &r)?;
                    }
                    _ => {
                        r.status = "unknown".into();
                        r.error=Some("Spiris tog emot anropet men kvittot kunde inte läsas. Kontrollera i Spiris; inget automatiskt nytt försök görs.".into());
                        save_report(&db, &r)?;
                        return Ok(r);
                    }
                }
            }
            Ok(response) => {
                let code = response.status().as_u16();
                r.status = if code >= 500 || code == 408 {
                    "unknown"
                } else if i > 0 {
                    "partial"
                } else {
                    "draft"
                }
                .into();
                r.error = Some(format!(
                    "Spiris svarade HTTP {code}. {}",
                    if r.status == "draft" {
                        "Kontrollera uppgifter och behörigheter innan nytt försök."
                    } else {
                        "Kontrollera resultatet i Spiris före fortsatt arbete."
                    }
                ));
                save_report(&db, &r)?;
                return Ok(r);
            }
            Err(_) => {
                r.status = "unknown".into();
                r.error = Some(
                    "Nätverksavbrott: utfallet är okänt. Ingen rad skickas om automatiskt.".into(),
                );
                save_report(&db, &r)?;
                return Ok(r);
            }
        }
    }
    r.status = "uploaded".into();
    save_report(&db, &r)?;
    Ok(r)
}
#[tauri::command]
pub async fn grit_reconcile(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Report, String> {
    let _op = Operation::start()?;
    past_date(&date)?;
    let db = db(&local_dir(&state)?)?;
    let mut r = report(&db, &date)?.ok_or("Rapport saknas")?;
    if !["unknown", "partial", "uploading"].contains(&r.status.as_str()) {
        return Err("Rapporten behöver ingen avstämning.".into());
    }
    let s: Settings =
        get(&db, &format!("submission:{date}"))?.ok_or("Överföringsinställningar saknas")?;
    settings_match(&db, &date, &s)?;
    let c = client()?;
    let t = token(&c).await?;
    let remote = existing(&c, &t, &date, s.user_id).await?;
    for row in &mut r.rows {
        row.remote_id = None;
        let mark = marker(&db, &date, row)?;
        let matches: Vec<_> = remote
            .iter()
            .filter(|v| v["internalComment"].as_str() == Some(mark.as_str()))
            .collect();
        if matches.len() > 1 {
            return Err("Flera Spiris-rader har samma referens. Kontrollera manuellt.".into());
        }
        if let Some(v) = matches.first() {
            if v["project"]["id"].as_i64() != Some(row.project_id)
                || (v["hours"].as_f64().unwrap_or(-1.0) - row.minutes as f64 / 60.0).abs() > 0.001
            {
                return Err("Spiris-raden har ändrats. Manuell avstämning krävs.".into());
            }
            row.remote_id = v["id"].as_i64();
        }
    }
    if r.rows.iter().all(|row| row.remote_id.is_some()) {
        r.status = "uploaded".into();
        r.error = None;
    } else {
        r.status = "unknown".into();
        r.error=Some("Alla rader kan inte verifieras ännu. Ingen ny överföring görs; kontrollera Spiris manuellt.".into());
    }
    save_report(&db, &r)?;
    Ok(r)
}
/// Recover durable jobs without ever assuming an interrupted POST failed.
pub fn recover(dir: &Path) -> Result<(), String> {
    let db = db(dir)?;
    let mut q = db.prepare("SELECT body FROM reports").map_err(error)?;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    for body in rows {
        let mut r: Report = serde_json::from_str(&body).map_err(error)?;
        if r.status == "running" {
            r.status = "failed".into();
            r.error =
                Some("Rapportjobbet avbröts när appen stängdes. Du kan starta om det.".into());
            save_report(&db, &r)?;
        } else if r.status == "uploading" {
            r.status = "unknown".into();
            r.error = Some("Appen stängdes under överföring. Kontrollera i Spiris.".into());
            save_report(&db, &r)?;
        }
    }
    Ok(())
}
pub async fn archive_task(state: Arc<Mutex<AppState>>) {
    // Run after startup and once per hour; only completed days are eligible.
    loop {
        let dir = match state.lock() {
            Ok(s) => s.data_dir.clone(),
            Err(_) => return,
        };
        let state_copy = state.clone();
        let dir_copy = dir.clone();
        let result = tokio::task::spawn_blocking(move || archive_all(&state_copy, &dir_copy)).await;
        if let Ok(Err(e)) = result {
            if !BUSY.load(Ordering::SeqCst) {
                if let Ok(db) = db(&dir) {
                    let _ = put(&db, "last_error", &Some(e));
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

pub fn capture_failed(dir: &Path, message: &str) {
    if let Ok(db) = db(dir) {
        let _ = put(
            &db,
            "capture_error",
            &Some(format!("Skärminsamlingen är pausad: {message}")),
        );
    }
}

pub fn capture_resumed(dir: &Path) {
    if let Ok(db) = db(dir) {
        let _ = db.execute("DELETE FROM kv WHERE key='capture_error'", []);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_settings_validate_and_survive_config_normalization() {
        let mut config = work_review_core::config::AppConfig::default();
        for interval in [30, 60, 300, 1800, 3600] {
            config.screenshot_interval = interval;
            config.normalize();
            let settings = CaptureSettings::from_config(&config);
            assert_eq!(settings.interval_seconds, interval);
            assert!(settings.validate().is_ok());
        }
        for interval in [0, 29, 3601, u64::MAX] {
            let mut settings = CaptureSettings::from_config(&config);
            settings.interval_seconds = interval;
            assert!(settings.validate().is_err());
        }
    }
    fn fixture() -> (Report, Catalog, Value) {
        let date = "2026-01-12";
        let start = Local
            .with_ymd_and_hms(2026, 1, 12, 9, 0, 0)
            .unwrap()
            .timestamp();
        let row = Row {
            id: "row-1".into(),
            project_id: 4,
            activity_id: 2,
            article_id: 3,
            start,
            end: start + 3600,
            minutes: 60,
            description: "Implementerade projektfunktion".into(),
            evidence_ids: vec![7],
            needs_review: false,
            remote_id: None,
        };
        let r = Report {
            date: date.into(),
            revision: "revision".into(),
            status: "draft".into(),
            summary: "Arbete".into(),
            rows: vec![row],
            error: None,
        };
        let c = Catalog {
            projects: vec![Project {
                id: 4,
                name: "Testprojekt".into(),
                number: "P4".into(),
                active: true,
            }],
            activities: vec![Choice {
                id: 2,
                name: "Utveckling".into(),
            }],
            articles: vec![Choice {
                id: 3,
                name: "Timme".into(),
            }],
            ..Catalog::default()
        };
        (
            r,
            c,
            json!([{"id":7,"timestamp":start+3600,"duration":3600}]),
        )
    }
    #[test]
    fn valid_evidence_uses_end_timestamp() {
        let (r, c, e) = fixture();
        assert!(validate_report(&r, &c, &e, true).is_ok());
    }
    #[test]
    fn rejects_hallucinated_project_and_evidence() {
        let (mut r, c, e) = fixture();
        r.rows[0].project_id = 999;
        assert!(validate_report(&r, &c, &e, true).is_err());
        r.rows[0].project_id = 4;
        r.rows[0].evidence_ids = vec![999];
        assert!(validate_report(&r, &c, &e, true).is_err());
    }
    #[test]
    fn uncertainty_blocks_submission() {
        let (mut r, c, e) = fixture();
        r.rows[0].needs_review = true;
        assert!(validate_report(&r, &c, &e, false).is_ok());
        assert!(validate_report(&r, &c, &e, true).is_err());
    }
    #[test]
    fn duplicate_evidence_cannot_inflate_time() {
        let (mut r, c, mut e) = fixture();
        e[0]["duration"] = json!(1800);
        r.rows[0].evidence_ids = vec![7, 7];
        assert!(validate_report(&r, &c, &e, true).is_err());
    }
    #[test]
    fn overlapping_rows_are_rejected() {
        let (mut r, c, e) = fixture();
        let mut second = r.rows[0].clone();
        second.id = "row-2".into();
        r.rows.push(second);
        assert!(validate_report(&r, &c, &e, true).is_err());
    }
    #[test]
    fn closed_projects_fail_even_for_old_drafts() {
        let (r, mut c, e) = fixture();
        c.projects[0].active = false;
        assert!(validate_report(&r, &c, &e, true).is_err());
    }
    #[test]
    fn interrupted_upload_is_never_retryable_draft() {
        let dir = std::env::temp_dir().join(format!("grit-recovery-{}", Uuid::new_v4()));
        let (mut r, _, _) = fixture();
        r.status = "uploading".into();
        save_report(&db(&dir).unwrap(), &r).unwrap();
        recover(&dir).unwrap();
        assert_eq!(
            report(&db(&dir).unwrap(), &r.date).unwrap().unwrap().status,
            "unknown"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn dates_reject_path_traversal_and_unfinished_days() {
        assert!(past_date("../../etc/passwd").is_err());
        assert!(past_date(&Local::now().date_naive().to_string()).is_err());
    }
}
