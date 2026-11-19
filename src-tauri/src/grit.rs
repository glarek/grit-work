//! Grit's durable report workflow. Only this module may write time to Spiris.
use crate::{
    grit_engine,
    image_interpretation::{
        self, DayFile, ImageContent, ImageSettings, InterpretationStatus, Mode,
    },
    AppState,
};
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
static IMAGE_DAY: Mutex<Option<String>> = Mutex::new(None);
struct ImageLease;
impl Drop for ImageLease {
    fn drop(&mut self) {
        if let Ok(mut day) = IMAGE_DAY.lock() {
            *day = None;
        }
    }
}
struct Operation;
impl Operation {
    fn start() -> Result<Self, String> {
        BUSY.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| {
                "Ett GRIT-jobb körs redan. Vänta eller avbryt rapportjobbet.".to_string()
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
    pub archive_retention_days: u32,
    pub ai_consent: bool,
    pub use_report_history: bool,
    pub image_interpretation: ImageSettings,
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
            archive_retention_days: 90,
            ai_consent: false,
            use_report_history: false,
            image_interpretation: ImageSettings::default(),
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
    #[serde(default)]
    pub time_code_id: i64,
    pub start: i64,
    pub end: i64,
    pub minutes: i64,
    #[serde(default)]
    pub submitted_minutes: i64,
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub manual_reason: String,
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
    v.map(|s| {
        let mut r: Report = serde_json::from_str(&s).map_err(error)?;
        if ["draft"].contains(&r.status.as_str()) {
            apply_rounding(&mut r)?;
        } else {
            // Older submissions retain their original amounts for reconciliation.
            for row in &mut r.rows {
                if row.submitted_minutes == 0 {
                    row.submitted_minutes = row.minutes;
                }
            }
        }
        Ok(r)
    })
    .transpose()
}
/// Round once per project/day; keep measured intervals intact. The final row
/// carries the rounding adjustment, also when a project has multiple activities.
fn apply_rounding(report: &mut Report) -> Result<(), String> {
    let mut totals = std::collections::BTreeMap::<i64, (i64, usize)>::new();
    for (index, row) in report.rows.iter_mut().enumerate() {
        if !(1..=1500).contains(&row.minutes) {
            return Err("Ogiltig tidslängd.".into());
        }
        row.submitted_minutes = row.minutes;
        let entry = totals.entry(row.project_id).or_default();
        entry.0 = entry
            .0
            .checked_add(row.minutes)
            .ok_or("Tiden är för stor.")?;
        entry.1 = index;
    }
    for (_, (minutes, index)) in totals {
        report.rows[index].submitted_minutes += (30 - minutes % 30) % 30;
    }
    Ok(())
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
    tokio::time::timeout(Duration::from_secs(60), async {
        let mut result = vec![];
        let mut expected_pages = None;
        for page in 1..=50 {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let mut query = vec![("page", page.to_string()), ("pageSize", "100".into())];
            query.extend_from_slice(filters);
            let value = reporting_get(client, token, path, &query)
                .await
                .map_err(|e| e.message)?;
            let items = value["items"]
                .as_array()
                .ok_or("Spiris-svaret saknar listformat.")?;
            let pages = value["totalPages"]
                .as_u64()
                .ok_or("Spiris-svaret saknar sidinformation.")?;
            if pages > 50
                || items.len() > 100
                || (pages == 0 && !items.is_empty())
                || (page < pages && items.is_empty())
                || expected_pages.is_some_and(|old| old != pages)
            {
                return Err(
                    "Spiris-listan ändrades eller överskrider säker gräns. Läs in den igen.".into(),
                );
            }
            expected_pages = Some(pages);
            result.extend(items.iter().cloned());
            if page >= pages {
                return Ok(result);
            }
        }
        Err("Spiris-listan överskrider säker sidgräns.".into())
    })
    .await
    .map_err(|_| "Spiris-läsningen tog för lång tid. Försök igen.".to_string())?
}

// A read check proves only that GET worked, never authorization to submit time.
#[derive(Debug, Serialize)]
pub struct PermissionCheck {
    resource: String,
    ok: bool,
    tested: bool,
    http_status: Option<u16>,
    count: Option<usize>,
    message: String,
}
#[derive(Serialize)]
pub struct ConnectionTest {
    checked_at: i64,
    authentication: PermissionCheck,
    reads: Vec<PermissionCheck>,
    submission_permission: &'static str,
}
#[derive(Debug, Serialize)]
pub struct ProjectReportingChoices {
    project_id: i64,
    member: bool,
    active: bool,
    activities: Vec<Choice>,
    time_codes: Vec<Choice>,
    articles: Vec<Choice>,
    restriction_source: &'static str,
}
#[derive(Debug, Serialize)]
pub struct ExistingReportRow {
    id: i64,
    date: String,
    project: Option<Choice>,
    activity: Option<Choice>,
    time_code: Option<Choice>,
    article: Option<Choice>,
    hours: f64,
    comment: String,
}
#[derive(Serialize)]
pub struct ReportingContext {
    date: String,
    user: Choice,
    checked_at: i64,
    projects: Vec<ProjectReportingChoices>,
    existing_rows: Vec<ExistingReportRow>,
    previous_rows: Vec<ExistingReportRow>,
}
struct ReportingReadError {
    status: Option<u16>,
    message: String,
}
impl ReportingReadError {
    fn new(status: Option<u16>, message: &str) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}
async fn reporting_get(
    client: &reqwest::Client,
    token: &str,
    path: &str,
    query: &[(&str, String)],
) -> Result<Value, ReportingReadError> {
    let mut response = client
        .get(format!("{API}/{path}"))
        .bearer_auth(token)
        .query(query)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| {
            ReportingReadError::new(
                None,
                "Spiris kunde inte nås. Kontrollera anslutningen och försök igen.",
            )
        })?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(ReportingReadError::new(
            Some(status),
            match status {
                401 => "Spiris nekade inloggningen. Kontrollera sparade API-uppgifter.",
                403 => {
                    "Spiris nekade läsningen. Kontrollera läsbehörighet och tillåtna IP-adresser."
                }
                429 => "Spiris begränsar anropen. Vänta och försök igen.",
                _ => "Spiris kunde inte läsa resursen. Försök igen.",
            },
        ));
    }
    const MAX_BYTES: usize = 2 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_BYTES as u64)
    {
        return Err(ReportingReadError::new(
            Some(status),
            "Spiris-svaret överskrider säker storleksgräns.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        ReportingReadError::new(Some(status), "Spiris-svaret kunde inte läsas färdigt.")
    })? {
        if chunk.len() > MAX_BYTES.saturating_sub(bytes.len()) {
            return Err(ReportingReadError::new(
                Some(status),
                "Spiris-svaret överskrider säker storleksgräns.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| ReportingReadError::new(Some(status), "Spiris returnerade ett ogiltigt svar."))
}
fn untested_permission(resource: &str, message: &str) -> PermissionCheck {
    PermissionCheck {
        resource: resource.into(),
        ok: false,
        tested: false,
        http_status: None,
        count: None,
        message: message.into(),
    }
}
fn permission_result(resource: &str, result: Result<Value, ReportingReadError>) -> PermissionCheck {
    match result {
        Ok(value) if value["items"].is_array() => PermissionCheck {
            resource: resource.into(),
            ok: true,
            tested: true,
            http_status: Some(200),
            count: value["totalItemCount"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok()),
            message: "Läsning verifierad. Detta bevisar inte rätt att skicka tid.".into(),
        },
        Ok(_) => PermissionCheck {
            resource: resource.into(),
            ok: false,
            tested: true,
            http_status: Some(200),
            count: None,
            message: "Spiris svarade men listformatet kunde inte verifieras.".into(),
        },
        Err(error) => PermissionCheck {
            resource: resource.into(),
            ok: false,
            tested: true,
            http_status: error.status,
            count: None,
            message: error.message,
        },
    }
}
#[tauri::command]
pub async fn grit_test_connection(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<ConnectionTest, String> {
    let database = db(&local_dir(&state)?)?;
    let settings: Settings = get(&database, "settings")?.unwrap_or_default();
    let resources = [
        "Core/Projects",
        "Admin/Users",
        "Admin/Activities",
        "Admin/TimeCodes",
        "Admin/Articles",
        "Core/TimeReports",
    ];
    let c = client()?;
    let token_result = tokio::time::timeout(Duration::from_secs(30), token(&c)).await;
    let t = match token_result {
        Ok(Ok(t)) => t,
        _ => return Ok(ConnectionTest {
            checked_at: Local::now().timestamp(),
            authentication: PermissionCheck { resource: "authentication".into(), ok: false, tested: true, http_status: None, count: None, message: "Spiris-inloggningen kunde inte verifieras. Kontrollera sparade uppgifter, nyckellager och anslutning.".into() },
            reads: resources.iter().map(|path| untested_permission(path, "Inte testad: inloggning krävs.")).collect(),
            submission_permission: "not_tested",
        }),
    };
    let mut reads = Vec::new();
    for path in resources {
        let mut query = vec![("page", "1".into()), ("pageSize", "1".into())];
        if path == "Admin/Articles" {
            query.push(("type", "time".into()));
        }
        if path == "Core/TimeReports" {
            if settings.user_id <= 0 {
                reads.push(untested_permission(
                    path,
                    "Välj Spiris-användare för att testa läsning av dennes tidrapporter.",
                ));
                continue;
            }
            let today = Local::now().date_naive();
            query.extend([
                ("filter.userIds", settings.user_id.to_string()),
                ("filter.from", format!("{today} 00:00:00")),
                ("filter.to", format!("{today} 23:59:59")),
            ]);
        }
        let result = reporting_get(&c, &t, path, &query).await;
        reads.push(permission_result(path, result));
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let authenticated = reads.iter().any(|check| check.ok);
    Ok(ConnectionTest {
        checked_at: Local::now().timestamp(),
        authentication: PermissionCheck {
            resource: "authentication".into(),
            ok: authenticated,
            tested: true,
            http_status: None,
            count: None,
            message: if authenticated {
                "Inloggningen verifierades genom ett lyckat läsanrop."
            } else {
                "Ingen läsning lyckades; anslutningen kunde inte verifieras."
            }
            .into(),
        },
        reads,
        submission_permission: "not_tested",
    })
}
fn project_reporting_choices(
    id: i64,
    value: &Value,
    user: i64,
    cat: &Catalog,
) -> Result<ProjectReportingChoices, String> {
    if id <= 0 || value["id"].as_i64() != Some(id) {
        return Err("Projektets identitet kunde inte verifieras i Spiris.".into());
    }
    let active = !value["status"]["isCompletedStatus"]
        .as_bool()
        .ok_or("Projektstatus saknas i Spiris-svaret.")?;
    let member = match value["allIsMember"].as_bool() {
        Some(true) => true,
        Some(false) => value["members"]
            .as_array()
            .ok_or("Projektets medlemslista saknas.")?
            .iter()
            .any(|m| m["id"].as_i64() == Some(user)),
        None => return Err("Projektets medlemsbehörighet kunde inte verifieras.".into()),
    };
    // Documented API and read-only live inspection: no project-specific option
    // restrictions are exposed. Activity preferences must not be called restrictions.
    let available = active && member;
    Ok(ProjectReportingChoices {
        project_id: id,
        member,
        active,
        activities: if available {
            cat.activities.clone()
        } else {
            vec![]
        },
        time_codes: if available {
            cat.time_codes.clone()
        } else {
            vec![]
        },
        articles: if available {
            cat.articles.clone()
        } else {
            vec![]
        },
        restriction_source: "active_registries_no_project_restrictions_exposed",
    })
}
async fn fresh_project_choices(
    c: &reqwest::Client,
    t: &str,
    id: i64,
    user: i64,
    cat: &Catalog,
) -> Result<ProjectReportingChoices, String> {
    let value = reporting_get(c, t, &format!("Core/Projects/{id}"), &[])
        .await
        .map_err(|e| e.message)?;
    project_reporting_choices(id, &value, user, cat)
}
fn validate_reporting_selection(
    row: &Row,
    project: &ProjectReportingChoices,
) -> Result<(), String> {
    if project.project_id != row.project_id || !project.active || !project.member {
        return Err("Projektet måste vara aktivt och vald användare måste vara medlem.".into());
    }
    for (id, choices, label) in [
        (row.activity_id, &project.activities, "aktivitet"),
        (row.time_code_id, &project.time_codes, "tidskod"),
        (row.article_id, &project.articles, "tidsartikel"),
    ] {
        if id <= 0 || !choices.iter().any(|choice| choice.id == id) {
            return Err(format!(
                "Välj en aktiv {label} för varje projektrad innan du skickar."
            ));
        }
    }
    Ok(())
}
fn checked_remote_rows(rows: Vec<Value>, date: &str, user: i64) -> Result<Vec<Value>, String> {
    let expected =
        NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| "Ogiltigt rapportdatum.")?;
    if user <= 0
        || rows.iter().any(|row| {
            row["user"]["id"].as_i64() != Some(user)
                || row["date"].as_str().and_then(style_row_date) != Some(expected)
        })
    {
        return Err(
            "Spiris-svaret stämmer inte med vald användare och dag. Ingen överföring kan göras."
                .into(),
        );
    }
    Ok(rows)
}
fn remote_choice(value: &Value) -> Option<Choice> {
    let id = value["id"].as_i64().filter(|id| *id > 0)?;
    Some(Choice {
        id,
        name: value["name"]
            .as_str()
            .unwrap_or("Namnlös")
            .chars()
            .take(500)
            .collect(),
    })
}
fn existing_report_row(value: &Value, date: &str) -> Result<ExistingReportRow, String> {
    let id = value["id"]
        .as_i64()
        .filter(|id| *id > 0)
        .ok_or("Spiris-raden saknar giltigt ID.")?;
    let hours = value["hours"]
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or("Spiris-raden saknar giltig tid.")?;
    Ok(ExistingReportRow {
        id,
        date: date.into(),
        project: remote_choice(&value["project"]),
        activity: remote_choice(&value["activity"]),
        time_code: remote_choice(&value["timeCode"]),
        article: remote_choice(&value["timeArticle"]),
        hours,
        comment: value["comment"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(2000)
            .collect(),
    })
}
fn prior_project_row(
    value: &Value,
    from: NaiveDate,
    before: NaiveDate,
    user: i64,
    project: i64,
) -> Result<ExistingReportRow, String> {
    let date = value["date"]
        .as_str()
        .and_then(style_row_date)
        .ok_or("Tidrapporten saknar giltigt datum.")?;
    if value["user"]["id"].as_i64() != Some(user)
        || value["project"]["id"].as_i64() != Some(project)
        || date < from
        || date >= before
    {
        return Err(
            "Spiris-historiken stämmer inte med vald användare, projekt och period.".into(),
        );
    }
    existing_report_row(value, &date.to_string())
}
fn limit_previous_rows(rows: &mut Vec<ExistingReportRow>) {
    rows.sort_by(|left, right| {
        right
            .date
            .cmp(&left.date)
            .then_with(|| right.id.cmp(&left.id))
    });
    let mut seen = HashSet::new();
    rows.retain(|row| seen.insert(row.id));
    rows.truncate(30);
}
async fn previous_reporting_rows(
    c: &reqwest::Client,
    t: &str,
    date: &str,
    user: i64,
    projects: &[i64],
) -> Result<Vec<ExistingReportRow>, String> {
    let before = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| "Ogiltigt datum.")?;
    let from = before
        .checked_sub_days(chrono::Days::new(90))
        .ok_or("Ogiltig historikperiod.")?;
    let to = before.pred_opt().ok_or("Ogiltig historikperiod.")?;
    let mut history = Vec::new();
    for &project in projects {
        let rows = list(
            c,
            t,
            "Core/TimeReports",
            &[
                ("filter.from", format!("{from} 00:00:00")),
                ("filter.to", format!("{to} 23:59:59")),
                ("filter.userIds", user.to_string()),
                ("filter.projectId", project.to_string()),
            ],
        )
        .await?;
        for value in rows {
            history.push(prior_project_row(&value, from, before, user, project)?);
        }
        limit_previous_rows(&mut history);
    }
    Ok(history)
}

#[tauri::command]
pub async fn grit_reporting_context(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<ReportingContext, String> {
    past_date(&date)?;
    let database = db(&local_dir(&state)?)?;
    let settings: Settings = get(&database, "settings")?.unwrap_or_default();
    let saved = report(&database, &date)?;
    let mut ids = saved
        .as_ref()
        .map(|report| {
            report
                .rows
                .iter()
                .map(|row| row.project_id)
                .filter(|id| *id > 0)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() > 100 {
        return Err(
            "Rapporten innehåller för många projekt för en säker behörighetskontroll.".into(),
        );
    }
    tokio::time::timeout(Duration::from_secs(90), async {
        let c = client()?;
        let t = token(&c).await?;
        let cat = catalog(&c, &t).await?;
        let user = cat
            .users
            .iter()
            .find(|user| user.id > 0 && user.id == settings.user_id)
            .cloned()
            .ok_or("Välj en giltig Spiris-användare innan överföring.")?;
        let remote = existing(&c, &t, &date, user.id).await?;
        let existing_rows = remote
            .iter()
            .map(|value| existing_report_row(value, &date))
            .collect::<Result<Vec<_>, _>>()?;
        let previous_rows = previous_reporting_rows(&c, &t, &date, user.id, &ids).await?;
        let mut projects = Vec::new();
        for id in ids {
            projects.push(fresh_project_choices(&c, &t, id, user.id, &cat).await?);
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        Ok(ReportingContext {
            date,
            user,
            checked_at: Local::now().timestamp(),
            projects,
            existing_rows,
            previous_rows,
        })
    })
    .await
    .map_err(|_| "Spiris-kontrollen tog för lång tid. Försök igen.".to_string())?
}

fn choices(items: Vec<Value>) -> Vec<Choice> {
    items
        .into_iter()
        .filter(|v| v["isActive"].as_bool() == Some(true))
        .filter_map(|v| {
            Some(Choice {
                id: v["id"].as_i64().filter(|id| *id > 0)?,
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
fn read_day_evidence(
    state: &Arc<Mutex<AppState>>,
    dir: &Path,
    date: &str,
) -> Result<(Value, bool), String> {
    let s = state.lock().map_err(error)?;
    let (rows, archived) =
        match crate::commands::grit_read::day_activities(dir, &s.database, date).map_err(error)? {
            crate::commands::grit_read::DayActivities::Local => (
                s.database
                    .get_activities_in_range(Some(date), Some(date), 10_000)
                    .map_err(error)?,
                false,
            ),
            crate::commands::grit_read::DayActivities::Archive(rows) => (rows, true),
        };
    let (ignored_apps, excluded_domains) = crate::commands::collect_privacy_filters(&s);
    let rows =
        crate::commands::filter_activities_by_privacy(rows, &ignored_apps, &excluded_domains);
    Ok((serde_json::to_value(rows).map_err(error)?, archived))
}

fn evidence(state: &Arc<Mutex<AppState>>, dir: &Path, date: &str) -> Result<Value, String> {
    read_day_evidence(state, dir, date).map(|(rows, _)| rows)
}
fn collect_dates(state: &Arc<Mutex<AppState>>, dir: &Path) -> Result<Vec<String>, String> {
    let mut dates = state
        .lock()
        .map_err(error)?
        .database
        .grit_activity_dates()
        .map_err(error)?;
    let db = db(dir)?;
    let mut q = db
        .prepare("SELECT date FROM days UNION SELECT date FROM reports")
        .map_err(error)?;
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
        if !dismissed && report(&db, &date)?.is_none_or(|r| r.status != "uploaded") {
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
    settings.image_interpretation.validate()?;
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
    if !(1..=3650).contains(&settings.archive_retention_days) {
        return Err("Välj en lagringstid mellan 1 och 3 650 dagar.".into());
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
    work_date(&date)?;
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
            (row.time_code_id, &cat.time_codes, "tidskod"),
        ] {
            if strict && (id <= 0 || !choices.iter().any(|c| c.id == id)) {
                return Err(format!("Välj en giltig {label} för varje rad."));
            }
        }
        if row.manual {
            if row.manual_reason.trim().is_empty()
                || row.manual_reason.chars().count() > 2000
                || !row.evidence_ids.is_empty()
            {
                return Err(
                    "Manuell tid behöver en förklaring och får inte ange automatiskt underlag."
                        .into(),
                );
            }
            intervals.push((row.start, row.end));
            continue;
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

fn work_date(date: &str) -> Result<(), String> {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(error)?;
    if parsed.to_string() != date || parsed > Local::now().date_naive() {
        return Err("Välj ett giltigt datum till och med idag.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn grit_day_context(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Value, String> {
    work_date(&date)?;
    let dir = local_dir(&state)?;
    let database = db(&dir)?;
    let (ev, archived) = read_day_evidence(state.inner(), &dir, &date)?;
    let activities = ev.as_array().ok_or("Ogiltigt underlag")?;
    let seconds: i64 = activities
        .iter()
        .map(|a| a["duration"].as_i64().unwrap_or(0).max(0))
        .sum();
    let saved_report = report(&database, &date)?;
    let catalog: Catalog = get(&database, "catalog")?.unwrap_or_default();
    let mut allocation = std::collections::BTreeMap::<i64, (i64, bool)>::new();
    if let Some(report) = &saved_report {
        for row in &report.rows {
            let entry = allocation.entry(row.project_id).or_default();
            entry.0 += row.minutes;
            entry.1 |= row.needs_review;
        }
    }
    let projects: Vec<Value> = allocation.into_iter().map(|(id,(minutes,needs_review))| {
        let project = catalog.projects.iter().find(|p|p.id==id);
        json!({"id":id,"name":project.map(|p|p.name.as_str()).unwrap_or("Ej fördelat projekt"),"minutes":minutes,"needs_review":needs_review || project.is_none()})
    }).collect();
    Ok(
        json!({"source":if archived {"archive"} else {"local"}, "activities":activities.len(), "minutes":seconds as f64 / 60.0, "report_status":saved_report.map(|r|r.status),"projects":projects}),
    )
}

fn report_csv_cell(value: &str) -> String {
    let risky =
        value.trim_start().starts_with(['=', '+', '-', '@']) || value.starts_with(['\t', '\r']);
    format!(
        "\"{}{}\"",
        if risky { "'" } else { "" },
        value.replace('"', "\"\"")
    )
}

#[tauri::command]
pub async fn grit_export_report(
    date: String,
    target_path: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    work_date(&date)?;
    let database = db(&local_dir(&state)?)?;
    let saved = report(&database, &date)?.ok_or("Ingen sparad tidrapport finns för datumet.")?;
    if ["running", "uploading"].contains(&saved.status.as_str()) {
        return Err("Vänta tills rapportjobbet är klart före export.".into());
    }
    let catalog: Catalog = get(&database, "catalog")?.unwrap_or_default();
    let target = Path::new(&target_path);
    if target.extension().and_then(|v| v.to_str()) != Some("csv") {
        return Err("Välj en fil med ändelsen .csv.".into());
    }
    let mut output =
        String::from("\u{feff}Datum;Projekt-ID;Projekt;Start;Slut;Registrerade minuter;Rapporterade minuter;Beskrivning;Status\r\n");
    for row in &saved.rows {
        let project = catalog
            .projects
            .iter()
            .find(|p| p.id == row.project_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Ej fördelat projekt".into());
        let time = |ts| {
            Local
                .timestamp_opt(ts, 0)
                .single()
                .map(|d| d.to_rfc3339())
                .unwrap_or_default()
        };
        let fields = [
            date.clone(),
            row.project_id.to_string(),
            project,
            time(row.start),
            time(row.end),
            row.minutes.to_string(),
            row.submitted_minutes.to_string(),
            row.description.clone(),
            saved.status.clone(),
        ];
        output.push_str(
            &fields
                .iter()
                .map(|v| report_csv_cell(v))
                .collect::<Vec<_>>()
                .join(";"),
        );
        output.push_str("\r\n");
    }
    std::fs::write(target, output).map_err(error)
}

#[tauri::command]
pub async fn grit_generate(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Report, String> {
    prepare_report(date, state.inner()).await
}
fn require_ai_consent(settings: &Settings) -> Result<(), String> {
    if !settings.ai_consent {
        return Err("Godkänn överföring av skärmbilder och loggar till vald AI-leverantör i inställningarna först.".into());
    }
    Ok(())
}

const STYLE_MAX_PAGES: u64 = 20;
const STYLE_PAGE_SIZE: usize = 100;
const STYLE_RESPONSE_BYTES: usize = 1024 * 1024;
const STYLE_MAX_EXAMPLES: usize = 12;
const STYLE_EXAMPLE_CHARS: usize = 600;
const STYLE_TOTAL_CHARS: usize = 6000;
const STYLE_HISTORY_ERROR: &str = "Kunde inte läsa din rapporthistorik från Spiris. Kontrollera anslutning och behörigheter och försök igen, eller stäng av rapporthistorik i inställningarna.";

#[derive(Clone, Copy)]
struct StyleHistoryScope {
    user: i64,
    from: NaiveDate,
    to: NaiveDate,
}
impl StyleHistoryScope {
    fn query(self, page: u64) -> Vec<(&'static str, String)> {
        vec![
            ("filter.from", format!("{} 00:00:00", self.from)),
            ("filter.to", format!("{} 23:59:59", self.to)),
            ("filter.userIds", self.user.to_string()),
            ("page", page.to_string()),
            ("pageSize", STYLE_PAGE_SIZE.to_string()),
        ]
    }
}
fn style_history_scope(
    settings: &Settings,
    cat: &Catalog,
    target: &str,
) -> Result<Option<StyleHistoryScope>, String> {
    // This gate precedes date/user validation, keyring access and all network work.
    if !settings.use_report_history {
        return Ok(None);
    }
    require_ai_consent(settings)?;
    if settings.user_id <= 0 || !cat.users.iter().any(|user| user.id == settings.user_id) {
        return Err("Välj en giltig Spiris-användare från det synkroniserade registret, eller stäng av rapporthistorik.".into());
    }
    let target = NaiveDate::parse_from_str(target, "%Y-%m-%d").map_err(|_| STYLE_HISTORY_ERROR)?;
    Ok(Some(StyleHistoryScope {
        user: settings.user_id,
        from: target
            .checked_sub_days(chrono::Days::new(90))
            .ok_or(STYLE_HISTORY_ERROR)?,
        to: target.pred_opt().ok_or(STYLE_HISTORY_ERROR)?,
    }))
}
fn style_row_date(value: &str) -> Option<NaiveDate> {
    if value.len() == 10 {
        let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
        return (date.to_string() == value).then_some(date);
    }
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.date_naive())
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|date| date.date())
                .ok()
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
                .map(|date| date.date())
                .ok()
        })
}
/// Project only validated comment text; never retain API rows or other fields.
fn style_comment(row: &Value, scope: StyleHistoryScope) -> Option<(NaiveDate, String)> {
    if row["user"]["id"].as_i64()? != scope.user {
        return None;
    }
    let date = style_row_date(row["date"].as_str()?)?;
    if date < scope.from || date > scope.to {
        return None;
    }
    let text = row["comment"]
        .as_str()?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let text = text
        .chars()
        .filter(|c| !c.is_control())
        .take(STYLE_EXAMPLE_CHARS)
        .collect::<String>();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some((date, text))
}
fn style_reference(mut comments: Vec<(NaiveDate, String)>, scope: StyleHistoryScope) -> Value {
    // Newest duplicate wins. Date and source identifiers never reach the agent.
    comments.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let mut seen = HashSet::new();
    comments.retain(|(_, text)| seen.insert(text.clone()));
    let mut words = comments
        .iter()
        .map(|(_, text)| text.split_whitespace().count())
        .collect::<Vec<_>>();
    words.sort_unstable();
    let median = if words.is_empty() {
        0
    } else {
        (words[(words.len() - 1) / 2] + words[words.len() / 2]) / 2
    };
    // Reserve room for the newest short and long examples, then fill by recency.
    let short = comments
        .iter()
        .position(|(_, text)| text.split_whitespace().count() <= median);
    let long = comments
        .iter()
        .position(|(_, text)| text.split_whitespace().count() > median);
    let order = short.into_iter().chain(long).chain(0..comments.len());
    let mut selected = Vec::new();
    let mut used = HashSet::new();
    let mut chars = 0;
    for index in order {
        if selected.len() >= STYLE_MAX_EXAMPLES {
            break;
        }
        if !used.insert(index) {
            continue;
        }
        let text = &comments[index].1;
        if chars + text.chars().count() > STYLE_TOTAL_CHARS {
            continue;
        }
        chars += text.chars().count();
        selected.push(index);
    }
    selected.sort_unstable();
    let examples = selected
        .iter()
        .map(|&index| comments[index].1.clone())
        .collect::<Vec<_>>();
    json!({"sample_count":examples.len(), "median_words":median, "examples":examples,
        "scope":{"source":"own_time_report_comments", "owner":"selected_user", "from":scope.from.to_string(),
            "to":scope.to.to_string(), "days":90, "usage":"style_only_not_evidence"}})
}
fn style_page(value: &Value, page: u64) -> Result<(&[Value], u64), String> {
    let items = value["items"].as_array().ok_or(STYLE_HISTORY_ERROR)?;
    let pages = value["totalPages"].as_u64().ok_or(STYLE_HISTORY_ERROR)?;
    if page == 0
        || pages > STYLE_MAX_PAGES
        || items.len() > STYLE_PAGE_SIZE
        || (pages == 0 && (!items.is_empty() || page != 1))
        || (pages > 0 && page > pages)
        || (page < pages && items.is_empty())
    {
        return Err(STYLE_HISTORY_ERROR.into());
    }
    Ok((items, pages))
}
async fn fetch_style_history(scope: StyleHistoryScope) -> Result<Value, String> {
    // One deadline covers authentication, every page, throttling and response bodies.
    tokio::time::timeout(Duration::from_secs(30), async {
        let c = client().map_err(|_| STYLE_HISTORY_ERROR)?;
        let t = token(&c).await.map_err(|_| STYLE_HISTORY_ERROR)?;
        let mut comments = Vec::new();
        let mut expected_pages = None;
        for page in 1..=STYLE_MAX_PAGES {
            if page > 1 {
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            let mut response = c
                .get(format!("{API}/Core/TimeReports"))
                .bearer_auth(&t)
                .query(&scope.query(page))
                .timeout(Duration::from_secs(10))
                .send()
                .await
                .map_err(|_| STYLE_HISTORY_ERROR)?;
            if !response.status().is_success()
                || response
                    .content_length()
                    .is_some_and(|n| n > STYLE_RESPONSE_BYTES as u64)
            {
                return Err(STYLE_HISTORY_ERROR.to_string());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| STYLE_HISTORY_ERROR)? {
                if chunk.len() > STYLE_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                    return Err(STYLE_HISTORY_ERROR.to_string());
                }
                bytes.extend_from_slice(&chunk);
            }
            let value: Value = serde_json::from_slice(&bytes).map_err(|_| STYLE_HISTORY_ERROR)?;
            let (items, pages) = style_page(&value, page)?;
            if expected_pages.is_some_and(|expected| expected != pages) {
                return Err(STYLE_HISTORY_ERROR.to_string());
            }
            expected_pages = Some(pages);
            comments.extend(items.iter().filter_map(|row| style_comment(row, scope)));
            if page >= pages {
                return Ok(style_reference(comments, scope));
            }
        }
        Err(STYLE_HISTORY_ERROR.to_string())
    })
    .await
    .map_err(|_| STYLE_HISTORY_ERROR.to_string())?
}
async fn writing_style_with<F, Fut>(
    settings: &Settings,
    cat: &Catalog,
    target: &str,
    fetch: F,
) -> Result<Option<Value>, String>
where
    F: FnOnce(StyleHistoryScope) -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    match style_history_scope(settings, cat, target)? {
        None => Ok(None),
        Some(scope) => fetch(scope).await.map(Some),
    }
}

async fn prepare_report(date: String, state: &Arc<Mutex<AppState>>) -> Result<Report, String> {
    let op = Arc::new(Operation::start()?);
    past_date(&date)?;
    let dir = state.lock().map_err(error)?.data_dir.clone();
    let db = db(&dir)?;
    if report(&db, &date)?.is_some_and(|r| !["draft", "failed"].contains(&r.status.as_str())) {
        return Err("Rapporten är låst eller körs redan.".into());
    }
    let settings: Settings = get(&db, "settings")?.unwrap_or_default();
    require_ai_consent(&settings)?;
    let cat: Catalog = get(&db, "catalog")?.unwrap_or_default();
    if cat.projects.iter().all(|p| !p.active) {
        return Err("Anslut Spiris och synkronisera projekt först.".into());
    }
    let ev = evidence(state, &dir, &date)?;
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
    let writing_style = writing_style_with(&settings, &cat, &date, fetch_style_history).await?;
    let mut project_data = json!({"projects":cat.projects,"activities":cat.activities,"articles":cat.articles,"default_activity_id":0,"default_article_id":0,"default_time_code_id":0});
    if let Some(writing_style) = writing_style {
        project_data["writing_style"] = writing_style;
    }
    let interpreted = interpret_day(
        &dir,
        &date,
        &ev,
        &settings,
        false,
        usize::MAX,
        cancel.clone(),
        op.clone(),
    )
    .await?;
    let ev_copy = image_interpretation::report_evidence(
        &dir,
        Path::new(&settings.archive_dir),
        &date,
        &ev,
        &interpreted,
    )?;
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
            // Reporting metadata is chosen explicitly during submission review.
            row.activity_id = 0;
            row.article_id = 0;
            row.time_code_id = 0;
            if row.manual || !row.manual_reason.is_empty() {
                return Err("Agenten får inte skapa manuell tid utan underlag.".into());
            }
            row.needs_review |= row.project_id == 0;
        }
        validate_report(&r, &cat, &ev, false)?;
        apply_rounding(&mut r)
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
pub async fn grit_create_manual_report(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Report, String> {
    let _op = Operation::start()?;
    past_date(&date)?;
    let database = db(&local_dir(&state)?)?;
    if report(&database, &date)?.is_some_and(|r| r.status != "failed") {
        return Err("Rapporten finns redan. Läs in den för att komplettera.".into());
    }
    let r = Report {
        date,
        revision: Uuid::new_v4().to_string(),
        status: "draft".into(),
        summary: "Manuellt kompletterad arbetsdag".into(),
        rows: vec![],
        error: None,
    };
    save_report(&database, &r)?;
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
    apply_rounding(&mut report)?;
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
        // Snapshot the reusable JSON companion without invoking OCR/AI during archival.
        let filtered = evidence(state, dir, &date)?;
        let idle_cancel = Arc::new(AtomicBool::new(false));
        interpret_sync(dir, &date, &filtered, &settings, false, 0, idle_cancel)?;
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
    expire_finished_evidence(&db, archive, settings.archive_retention_days)?;
    put(&db, "last_error", &Option::<String>::None)?;
    Ok(ArchiveResult {
        archived,
        message: format!("{archived} dygn verifierade och rensade lokalt."),
    })
}
fn retention_eligible(date: NaiveDate, today: NaiveDate, days: u32, finished: bool) -> bool {
    finished && days > 0 && (today - date).num_days() >= i64::from(days)
}
fn expire_finished_evidence(
    database: &Connection,
    archive: &Path,
    days: u32,
) -> Result<(), String> {
    let mut query = database
        .prepare("SELECT date,dismissed FROM days WHERE archived=1")
        .map_err(error)?;
    let candidates = query
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    for (date, dismissed) in candidates {
        let finished =
            dismissed || report(database, &date)?.is_some_and(|r| r.status == "uploaded");
        let day = past_date(&date)?;
        if !retention_eligible(day, Local::now().date_naive(), days, finished) {
            continue;
        }
        // The tombstone marker makes readers return an empty evidence view after
        // expiry, while all report bodies and submission receipts remain intact.
        crate::grit_engine::expire_archive(archive, &date)?;
        // Derived interpretations share evidence retention, not permanent report receipts.
        if let Some(local_dir) = database.path().and_then(|path| Path::new(path).parent()) {
            let interpretation = image_interpretation::file_path(local_dir, &date)?;
            match std::fs::remove_file(interpretation) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("Kunde inte rensa den utgångna tolkningsfilen.".into()),
            }
        }
        put(database, &format!("expired:{date}"), &true)?;
    }
    Ok(())
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
    let rows = list(
        c,
        t,
        "Core/TimeReports",
        &[
            ("filter.from", format!("{date} 00:00:00")),
            ("filter.to", format!("{date} 23:59:59")),
            ("filter.userIds", user.to_string()),
        ],
    )
    .await?;
    checked_remote_rows(rows, date, user)
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
fn submission_payload(date: &str, row: &Row, settings: &Settings, marker: &str) -> Value {
    let hours = row.submitted_minutes as f64 / 60.0;
    json!({"date":format!("{date} 00:00:00"),"hours":hours,"invoiceableHours":if settings.billable {hours}else{0.0},"comment":row.description,"internalComment":marker,"userId":settings.user_id,"projectId":row.project_id,"activityId":row.activity_id,"timeCodeId":row.time_code_id,"timeArticleId":row.article_id})
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
    if s.user_id <= 0 || !cat.users.iter().any(|v| v.id == s.user_id) {
        return Err("Välj en giltig användare i inställningarna.".into());
    }
    let ev = evidence(state.inner(), &dir, &date)?;
    validate_report(&r, &cat, &ev, true)?;
    let project_ids: HashSet<i64> = r.rows.iter().map(|row| row.project_id).collect();
    for id in project_ids {
        let project = fresh_project_choices(&c, &t, id, s.user_id, &cat).await?;
        for row in r.rows.iter().filter(|row| row.project_id == id) {
            validate_reporting_selection(row, &project)?;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let remote = existing(&c, &t, &date, s.user_id).await?;
    // Conservatively refuse a second report for a day with pre-existing entries.
    // This catches manually reported time too; do not infer non-overlap from missing clock fields.
    if !remote.is_empty() {
        return Err("Det finns redan tid i Spiris för användaren och datumet. Stäm av den innan du skickar från GRIT Work.".into());
    }
    put(&db, &format!("submission:{date}"), &s)?;
    r.status = "uploading".into();
    r.error = None;
    save_report(&db, &r)?;
    for i in 0..r.rows.len() {
        // Recheck immediately before each POST, not only when the modal opened.
        let fresh = async {
            let options = Catalog {
                activities: choices(list(&c, &t, "Admin/Activities", &[]).await?),
                time_codes: choices(list(&c, &t, "Admin/TimeCodes", &[]).await?),
                articles: choices(
                    list(&c, &t, "Admin/Articles", &[("type", "time".into())]).await?,
                ),
                ..Catalog::default()
            };
            let project =
                fresh_project_choices(&c, &t, r.rows[i].project_id, s.user_id, &options).await?;
            validate_reporting_selection(&r.rows[i], &project)
        }
        .await;
        if let Err(message) = fresh {
            r.status = if i == 0 { "draft" } else { "partial" }.into();
            r.error = Some(message);
            save_report(&db, &r)?;
            return Ok(r);
        }
        let row = &r.rows[i];
        let mark = marker(&db, &date, row)?;
        let payload = submission_payload(&date, row, &s, &mark);
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
fn receipt_choice(attempt: Option<&Value>, key: &str, fallback: i64) -> Result<i64, String> {
    let id = match attempt {
        Some(payload) => payload[key]
            .as_i64()
            .ok_or("Överföringskvittot saknar rapporteringsval. Kontrollera manuellt.")?,
        None => fallback,
    };
    if id <= 0 {
        return Err("Överföringsvalet kan inte verifieras. Kontrollera manuellt.".into());
    }
    Ok(id)
}
fn verify_remote_receipt(
    row: &Row,
    settings: &Settings,
    attempt: Option<&Value>,
    remote: &Value,
) -> Result<i64, String> {
    // Legacy rows deserialize time_code_id as 0. Their durable attempt or original
    // submission settings remain authoritative; current defaults are never used.
    let legacy_code = if row.time_code_id > 0 {
        row.time_code_id
    } else {
        settings.time_code_id
    };
    let activity = receipt_choice(attempt, "activityId", row.activity_id)?;
    let article = receipt_choice(attempt, "timeArticleId", row.article_id)?;
    let code = receipt_choice(attempt, "timeCodeId", legacy_code)?;
    if activity != row.activity_id
        || article != row.article_id
        || (row.time_code_id > 0 && code != row.time_code_id)
        || remote["project"]["id"].as_i64() != Some(row.project_id)
        || remote["activity"]["id"].as_i64() != Some(activity)
        || remote["timeArticle"]["id"].as_i64() != Some(article)
        || remote["timeCode"]["id"].as_i64() != Some(code)
        || (remote["hours"].as_f64().unwrap_or(-1.0) - row.submitted_minutes as f64 / 60.0).abs()
            > 0.001
    {
        return Err("Spiris-radens projekt, tid eller rapporteringsval har ändrats. Manuell avstämning krävs.".into());
    }
    remote["id"]
        .as_i64()
        .filter(|id| *id > 0)
        .ok_or("Spiris-kvittot saknar giltigt ID.".into())
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
            let attempt: Option<Value> = get(&db, &format!("attempt:{date}:{}", row.id))?;
            row.remote_id = Some(verify_remote_receipt(row, &s, attempt.as_ref(), v)?);
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
    // Only called after single-instance admission. Job folders contain disposable
    // copies; original local evidence/verified archives remain authoritative.
    let jobs = dir.join("jobs");
    if let Ok(meta) = std::fs::symlink_metadata(&jobs) {
        if meta.is_dir() && !meta.file_type().is_symlink() {
            for entry in std::fs::read_dir(&jobs).map_err(error)? {
                let entry = entry.map_err(error)?;
                if Uuid::parse_str(&entry.file_name().to_string_lossy()).is_ok()
                    && entry.file_type().map_err(error)?.is_dir()
                {
                    std::fs::remove_dir_all(entry.path()).map_err(error)?;
                }
            }
        }
    }
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
struct ImageScratch(PathBuf);
impl Drop for ImageScratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[allow(clippy::too_many_arguments)]
fn interpret_sync(
    dir: &Path,
    date: &str,
    ev: &Value,
    settings: &Settings,
    retry_failed: bool,
    max_images: usize,
    cancel: Arc<AtomicBool>,
) -> Result<DayFile, String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(settings.timeout_seconds);
    image_interpretation::process_day(
        dir,
        Path::new(&settings.archive_dir),
        date,
        ev,
        &settings.image_interpretation,
        settings.ai_consent,
        retry_failed,
        max_images,
        &cancel,
        |path, options| {
            if cancel.load(Ordering::SeqCst) {
                return Err("Bildtolkningen avbröts. Välj Försök igen.".into());
            }
            if deadline <= std::time::Instant::now() {
                cancel.store(true, Ordering::SeqCst);
                return Err("Bildtolkningen nådde tidsgränsen. Fortsätt med Tolka nu.".into());
            }
            match options.mode {
                Mode::Off => Ok(ImageContent::default()),
                Mode::Ocr => {
                    let recognized = crate::image_ocr::recognize(path)?;
                    Ok(ImageContent {
                        text: recognized.text,
                        regions: recognized.regions,
                        language: recognized.language,
                        ..ImageContent::default()
                    })
                }
                Mode::Ai => {
                    require_ai_consent(settings)?;
                    let work = dir.join("jobs").join(format!("image-{}", Uuid::new_v4()));
                    std::fs::create_dir_all(&work)
                        .map_err(|_| "Kunde inte skapa bildtolkningsjobb.")?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o700))
                            .map_err(|_| "Kunde inte skydda bildtolkningsjobbet.")?;
                    }
                    let _scratch = ImageScratch(work.clone());
                    let timeout = deadline
                        .saturating_duration_since(std::time::Instant::now())
                        .as_secs()
                        .clamp(1, 120);
                    let result = crate::grit_cli::interpret_image(
                        &options.provider,
                        &options.model,
                        &work,
                        path,
                        timeout,
                        cancel.clone(),
                    )?;
                    serde_json::from_value(result).map_err(|_| {
                        "AI-tolkningen följer inte det gemensamma bildformatet.".into()
                    })
                }
            }
        },
    )
}
#[allow(clippy::too_many_arguments)]
async fn interpret_day(
    dir: &Path,
    date: &str,
    ev: &Value,
    settings: &Settings,
    retry_failed: bool,
    max_images: usize,
    cancel: Arc<AtomicBool>,
    operation: Arc<Operation>,
) -> Result<DayFile, String> {
    let (dir, date, ev, settings) = (
        dir.to_path_buf(),
        date.to_string(),
        ev.clone(),
        settings.clone(),
    );
    *IMAGE_DAY.lock().map_err(error)? = Some(date.clone());
    let lease = Arc::new(ImageLease);
    tokio::task::spawn_blocking(move || {
        let _operation = operation;
        let _lease = lease;
        interpret_sync(
            &dir,
            &date,
            &ev,
            &settings,
            retry_failed,
            max_images,
            cancel,
        )
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub async fn grit_interpretation_status(
    date: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<InterpretationStatus, String> {
    work_date(&date)?;
    let dir = local_dir(&state)?;
    let settings: Settings = get(&db(&dir)?, "settings")?.unwrap_or_default();
    let day = image_interpretation::load(&dir, Path::new(&settings.archive_dir), &date)?;
    let day = day.filter(|day| day.configuration == settings.image_interpretation);
    let running = IMAGE_DAY.lock().map_err(error)?.is_some();
    image_interpretation::status(&dir, &date, day.as_ref(), running)
}
#[tauri::command]
pub async fn grit_interpret_images(
    date: String,
    retry_failed: bool,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<InterpretationStatus, String> {
    let operation = Arc::new(Operation::start()?);
    work_date(&date)?;
    let dir = local_dir(&state)?;
    let settings: Settings = get(&db(&dir)?, "settings")?.unwrap_or_default();
    let ev = evidence(state.inner(), &dir, &date)?;
    let cancel = CANCEL
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone();
    cancel.store(false, Ordering::SeqCst);
    let day = interpret_day(
        &dir,
        &date,
        &ev,
        &settings,
        retry_failed,
        usize::MAX,
        cancel,
        operation,
    )
    .await?;
    image_interpretation::status(&dir, &date, Some(&day), false)
}
/// Current-day interpretation is incremental; an error is persisted and needs an explicit retry.
pub async fn image_interpretation_task(state: Arc<Mutex<AppState>>) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let Ok(operation) = Operation::start() else {
            continue;
        };
        let operation = Arc::new(operation);
        let dir = match state.lock() {
            Ok(s) => s.data_dir.clone(),
            Err(_) => return,
        };
        let date = Local::now().date_naive().to_string();
        let prepared = (|| {
            let settings: Settings = get(&db(&dir)?, "settings")?.unwrap_or_default();
            let ev = evidence(&state, &dir, &date)?;
            Ok::<_, String>((settings, ev))
        })();
        let (settings, ev) = match prepared {
            Ok(v) => v,
            Err(e) => {
                if let Ok(database) = db(&dir) {
                    let _ = put(&database, "last_error", &Some(e));
                }
                continue;
            }
        };
        let cancel = CANCEL
            .get_or_init(|| Arc::new(AtomicBool::new(false)))
            .clone();
        cancel.store(false, Ordering::SeqCst);
        if let Err(e) =
            interpret_day(&dir, &date, &ev, &settings, false, 1, cancel, operation).await
        {
            if let Ok(database) = db(&dir) {
                let _ = put(&database, "last_error", &Some(e));
            }
        }
    }
}

/// Prepare completed days without uploading. A persisted report, including a
/// failure, stops retries; user-edited drafts are never overwritten by the worker.
fn next_background_date(
    state: &Arc<Mutex<AppState>>,
    dir: &Path,
) -> Result<Option<String>, String> {
    let database = db(dir)?;
    let settings: Settings = get(&database, "settings")?.unwrap_or_default();
    if !settings.ai_consent {
        return Ok(None);
    }
    let cat: Catalog = get(&database, "catalog")?.unwrap_or_default();
    if !cat.projects.iter().any(|p| p.active) {
        return Ok(None);
    }
    let mut dates = collect_dates(state, dir)?;
    dates.reverse();
    for date in dates {
        let dismissed: bool = database
            .query_row("SELECT dismissed FROM days WHERE date=?", [&date], |r| {
                r.get(0)
            })
            .optional()
            .map_err(error)?
            .unwrap_or(false);
        if !dismissed && report(&database, &date)?.is_none() {
            return Ok(Some(date));
        }
    }
    Ok(None)
}
pub async fn report_task(state: Arc<Mutex<AppState>>) {
    // Delay gives startup recovery and the UI time to settle.
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        if BUSY.load(Ordering::SeqCst) {
            continue;
        }
        let dir = match state.lock() {
            Ok(s) => s.data_dir.clone(),
            Err(_) => return,
        };
        let date = match next_background_date(&state, &dir) {
            Ok(Some(date)) => date,
            Ok(None) => continue,
            Err(e) => {
                if let Ok(db) = db(&dir) {
                    let _ = put(&db, "last_error", &Some(e));
                }
                continue;
            }
        };
        if let Err(e) = prepare_report(date.clone(), &state).await {
            // Busy races are retried next tick. Other preflight failures have a
            // durable failed queue entry so an offline archive cannot spin CLIs.
            if !BUSY.load(Ordering::SeqCst) {
                if let Ok(database) = db(&dir) {
                    if report(&database, &date).ok().flatten().is_none() {
                        let failed = Report {
                            date,
                            revision: Uuid::new_v4().to_string(),
                            status: "failed".into(),
                            summary: String::new(),
                            rows: vec![],
                            error: Some(e),
                        };
                        let _ = save_report(&database, &failed);
                    }
                }
            }
        }
    }
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
    fn retention_preserves_unfinished_reports_and_respects_exact_boundary() {
        let day = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert!(!retention_eligible(
            day,
            day + chrono::Days::new(89),
            90,
            true
        ));
        assert!(retention_eligible(
            day,
            day + chrono::Days::new(90),
            90,
            true
        ));
        assert!(!retention_eligible(
            day,
            day + chrono::Days::new(190),
            90,
            false
        ));
    }
    #[test]
    fn rounding_groups_project_day_and_preserves_raw_evidence() {
        let (mut r, _, _) = fixture();
        r.rows[0].minutes = 10;
        let mut second = r.rows[0].clone();
        second.id = "second".into();
        second.minutes = 20;
        r.rows.push(second);
        apply_rounding(&mut r).unwrap();
        assert_eq!(r.rows.iter().map(|r| r.submitted_minutes).sum::<i64>(), 30);
        r.rows[1].minutes = 30;
        apply_rounding(&mut r).unwrap();
        assert_eq!(r.rows.iter().map(|r| r.submitted_minutes).sum::<i64>(), 60);
        assert_eq!(r.rows[1].minutes, 30);
        r.rows[1].project_id = 9;
        apply_rounding(&mut r).unwrap();
        assert_eq!(r.rows[0].submitted_minutes, 30);
        assert_eq!(r.rows[1].submitted_minutes, 30);
        // Idempotent; forged submitted amounts never survive saving.
        r.rows[0].submitted_minutes = 9999;
        apply_rounding(&mut r).unwrap();
        assert_eq!(r.rows[0].submitted_minutes, 30);
    }
    #[test]
    fn manual_time_requires_explanation_and_still_rejects_overlap() {
        let (mut r, c, e) = fixture();
        r.rows[0].manual = true;
        r.rows[0].evidence_ids.clear();
        assert!(validate_report(&r, &c, &json!([]), true).is_err());
        r.rows[0].manual_reason = "Kundmöte utan dator".into();
        assert!(validate_report(&r, &c, &json!([]), true).is_ok());
        let mut duplicate = r.rows[0].clone();
        duplicate.id = "overlap".into();
        r.rows.push(duplicate);
        assert!(validate_report(&r, &c, &e, true).is_err());
    }
    #[test]
    fn old_settings_require_informed_consent_and_default_to_ninety_days() {
        let settings: Settings = serde_json::from_value(json!({"provider":"codex"})).unwrap();
        assert!(!settings.ai_consent);
        assert_eq!(settings.archive_retention_days, 90);
        assert!(require_ai_consent(&settings).is_err());
        assert!(require_ai_consent(&Settings {
            ai_consent: true,
            ..settings
        })
        .is_ok());
    }
    #[test]
    fn spreadsheet_export_quotes_values_and_neutralizes_formulas() {
        assert_eq!(report_csv_cell("=1+1"), "\"'=1+1\"");
        assert_eq!(report_csv_cell("a;\"b\""), "\"a;\"\"b\"\"\"");
        assert_eq!(report_csv_cell("Utveckling"), "\"Utveckling\"");
    }
    #[test]
    fn work_date_rejects_future_and_invalid_dates() {
        assert!(work_date(&Local::now().date_naive().to_string()).is_ok());
        assert!(work_date("2099-12-31").is_err());
        assert!(work_date("2026-99-99").is_err());
    }

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
            time_code_id: 5,
            start,
            end: start + 3600,
            minutes: 60,
            submitted_minutes: 60,
            manual: false,
            manual_reason: String::new(),
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
            time_codes: vec![Choice {
                id: 5,
                name: "Normal".into(),
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
    fn reporting_metadata_is_optional_for_drafts_but_required_for_submission() {
        let (mut r, c, e) = fixture();
        r.rows[0].activity_id = 0;
        r.rows[0].article_id = 0;
        r.rows[0].time_code_id = 0;
        assert!(validate_report(&r, &c, &e, false).is_ok());
        assert!(validate_report(&r, &c, &e, true).is_err());
        let mut legacy = serde_json::to_value(&r.rows[0]).unwrap();
        legacy.as_object_mut().unwrap().remove("time_code_id");
        assert_eq!(
            serde_json::from_value::<Row>(legacy).unwrap().time_code_id,
            0
        );
    }
    #[test]
    fn reporting_choices_use_actual_active_registries_and_project_membership() {
        let (r, mut c, _) = fixture();
        c.activities = choices(vec![
            json!({"id":2,"name":"Active","isActive":true}),
            json!({"id":8,"isActive":false}),
            json!({"id":9}),
            json!({"id":0,"isActive":true}),
        ]);
        assert_eq!(c.activities.len(), 1);
        let mut project = json!({"id":4,"status":{"isCompletedStatus":false},"allIsMember":false,"members":[{"id":7}]});
        let allowed = project_reporting_choices(4, &project, 7, &c).unwrap();
        assert_eq!(
            allowed.restriction_source,
            "active_registries_no_project_restrictions_exposed"
        );
        assert!(validate_reporting_selection(&r.rows[0], &allowed).is_ok());
        let denied = project_reporting_choices(4, &project, 8, &c).unwrap();
        assert!(!denied.member && denied.activities.is_empty());
        assert!(validate_reporting_selection(&r.rows[0], &denied).is_err());
        project["status"]["isCompletedStatus"] = json!(true);
        assert!(validate_reporting_selection(
            &r.rows[0],
            &project_reporting_choices(4, &project, 7, &c).unwrap()
        )
        .is_err());
        assert!(project_reporting_choices(5, &project, 7, &c).is_err());
        project.as_object_mut().unwrap().remove("allIsMember");
        assert!(project_reporting_choices(4, &project, 7, &c).is_err());
    }
    #[test]
    fn reporting_payload_and_receipts_use_row_choices_with_legacy_fallback() {
        let (mut r, _, _) = fixture();
        let settings = Settings {
            user_id: 7,
            time_code_id: 99,
            ..Settings::default()
        };
        let payload = submission_payload(&r.date, &r.rows[0], &settings, "synthetic-marker");
        assert_eq!(payload["timeCodeId"], 5);
        let mut remote = json!({"id":123,"project":{"id":4},"activity":{"id":2},"timeArticle":{"id":3},"timeCode":{"id":5},"hours":1.0});
        assert_eq!(
            verify_remote_receipt(&r.rows[0], &settings, Some(&payload), &remote).unwrap(),
            123
        );
        remote["timeCode"]["id"] = json!(99);
        assert!(verify_remote_receipt(&r.rows[0], &settings, Some(&payload), &remote).is_err());
        r.rows[0].time_code_id = 0;
        assert!(verify_remote_receipt(&r.rows[0], &settings, None, &remote).is_ok());
        // An old attempt is authoritative even when its row predates time_code_id.
        remote["timeCode"]["id"] = json!(5);
        assert!(verify_remote_receipt(&r.rows[0], &settings, Some(&payload), &remote).is_ok());
        remote["activity"]["id"] = json!(99);
        assert!(verify_remote_receipt(&r.rows[0], &settings, Some(&payload), &remote).is_err());
    }
    #[test]
    fn reporting_remote_projection_is_scoped_and_does_not_expose_internal_data() {
        let value = json!({"id":10,"date":"2026-01-12","user":{"id":7},"project":{"id":4,"name":"Project"},"activity":{"id":2,"name":"Work"},"timeCode":{"id":5,"name":"Normal"},"timeArticle":{"id":3,"name":"Hour"},"hours":1,"comment":"Public description","internalComment":"PRIVATE MARKER","cost":123});
        assert!(checked_remote_rows(vec![value.clone()], "2026-01-12", 7).is_ok());
        assert!(checked_remote_rows(vec![value.clone()], "2026-01-13", 7).is_err());
        assert!(checked_remote_rows(vec![value.clone()], "2026-01-12", 8).is_err());
        let row = existing_report_row(&value, "2026-01-12").unwrap();
        let output = serde_json::to_value(row).unwrap();
        assert_eq!(output["activity"]["name"], "Work");
        assert_eq!(output["time_code"]["id"], 5);
        assert_eq!(output["article"]["name"], "Hour");
        assert!(output.get("internalComment").is_none() && output.get("cost").is_none());
        assert!(!output.to_string().contains("PRIVATE MARKER"));
    }
    #[test]
    fn reporting_prior_rows_require_user_project_and_strict_ninety_day_scope() {
        let before = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let from = before - chrono::Days::new(90);
        let mut value =
            json!({"id":1,"user":{"id":7},"project":{"id":4},"date":"2026-01-01","hours":1});
        assert!(prior_project_row(&value, from, before, 7, 4).is_ok());
        assert!(prior_project_row(&value, from, before, 7, 5).is_err());
        assert!(prior_project_row(&value, from, before, 8, 4).is_err());
        value["date"] = json!("2025-12-31");
        assert!(prior_project_row(&value, from, before, 7, 4).is_err());
        value["date"] = json!("2026-04-01");
        assert!(prior_project_row(&value, from, before, 7, 4).is_err());
        let mut rows = Vec::new();
        for id in 1..=40 {
            value["id"] = json!(id);
            value["date"] = json!(format!("2026-03-{:02}", ((id - 1) % 28) + 1));
            rows.push(prior_project_row(&value, from, before, 7, 4).unwrap());
        }
        rows.push(prior_project_row(&value, from, before, 7, 4).unwrap());
        limit_previous_rows(&mut rows);
        assert_eq!(rows.len(), 30);
        assert_eq!(rows.iter().map(|r| r.id).collect::<HashSet<_>>().len(), 30);
        assert!(rows.windows(2).all(|pair| pair[0].date >= pair[1].date));
    }
    #[test]
    fn reporting_read_permissions_are_independent_and_do_not_prove_post_rights() {
        let denied = permission_result(
            "Core/Projects",
            Err(ReportingReadError::new(Some(403), "Denied")),
        );
        let allowed = permission_result(
            "Admin/Activities",
            Ok(json!({"items":[],"totalItemCount":0})),
        );
        assert!(!denied.ok && denied.tested && denied.http_status == Some(403));
        assert!(allowed.ok && allowed.tested);
        assert!(allowed.message.contains("inte rätt att skicka tid"));
        let pending = untested_permission("Core/TimeReports", "Select user");
        assert!(!pending.tested && !pending.ok);
        let result = ConnectionTest {
            checked_at: 0,
            authentication: allowed,
            reads: vec![denied, pending],
            submission_permission: "not_tested",
        };
        assert_eq!(
            serde_json::to_value(result).unwrap()["submission_permission"],
            "not_tested"
        );
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
    fn recovery_removes_only_owned_job_copies() {
        let dir = std::env::temp_dir().join(format!("grit-scratch-{}", Uuid::new_v4()));
        let job = dir.join("jobs").join(Uuid::new_v4().to_string());
        std::fs::create_dir_all(&job).unwrap();
        std::fs::write(job.join("image.jpg"), b"copy").unwrap();
        let unrelated = dir.join("jobs").join("notes");
        std::fs::create_dir_all(&unrelated).unwrap();
        let original = dir.join("original.jpg");
        std::fs::write(&original, b"original evidence").unwrap();
        recover(&dir).unwrap();
        assert!(!job.exists());
        assert!(unrelated.exists());
        assert_eq!(std::fs::read(original).unwrap(), b"original evidence");
        std::fs::remove_dir_all(dir).unwrap();
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

#[cfg(test)]
mod style_history_tests {
    use super::*;
    fn settings() -> Settings {
        Settings {
            user_id: 7,
            ai_consent: true,
            use_report_history: true,
            ..Settings::default()
        }
    }
    fn catalog_fixture() -> Catalog {
        Catalog {
            users: vec![Choice {
                id: 7,
                name: "Synthetic user".into(),
            }],
            ..Catalog::default()
        }
    }
    fn scope() -> StyleHistoryScope {
        style_history_scope(&settings(), &catalog_fixture(), "2026-04-01")
            .unwrap()
            .unwrap()
    }
    #[tokio::test]
    async fn style_history_optout_defaults_never_fetch_and_optin_requires_consent_and_owner() {
        for settings in [
            Settings::default(),
            serde_json::from_value::<Settings>(json!({"provider":"codex"})).unwrap(),
        ] {
            assert!(!settings.use_report_history);
            let result =
                writing_style_with(&settings, &Catalog::default(), "not-a-date", |_| async {
                    panic!("optout must not access credentials or network")
                })
                .await
                .unwrap();
            assert!(result.is_none());
        }
        let enabled = settings();
        assert!(style_history_scope(&enabled, &Catalog::default(), "2026-04-01").is_err());
        assert!(style_history_scope(
            &Settings {
                ai_consent: false,
                ..enabled.clone()
            },
            &catalog_fixture(),
            "2026-04-01"
        )
        .is_err());
        assert!(style_history_scope(
            &Settings {
                user_id: 0,
                ..enabled
            },
            &catalog_fixture(),
            "2026-04-01"
        )
        .is_err());
    }
    #[test]
    fn style_history_filters_cover_exact_prior_ninety_days() {
        let scope = scope();
        assert_eq!((scope.to - scope.from).num_days(), 89);
        assert_eq!(scope.from.to_string(), "2026-01-01");
        assert_eq!(scope.to.to_string(), "2026-03-31");
        assert_eq!(
            scope.query(2),
            vec![
                ("filter.from", "2026-01-01 00:00:00".into()),
                ("filter.to", "2026-03-31 23:59:59".into()),
                ("filter.userIds", "7".into()),
                ("page", "2".into()),
                ("pageSize", "100".into())
            ]
        );
        let leap = style_history_scope(&settings(), &catalog_fixture(), "2024-03-01")
            .unwrap()
            .unwrap();
        assert_eq!(leap.to.to_string(), "2024-02-29");
        assert_eq!((leap.to - leap.from).num_days(), 89);
    }
    #[test]
    fn style_history_rejects_wrong_owner_dates_missing_and_blank_comments() {
        let scope = scope();
        for row in [
            json!({"user":{"id":8},"date":"2026-03-01","comment":"foreign"}),
            json!({"userId":7,"date":"2026-03-01","comment":"unverified owner"}),
            json!({"user":{"id":7},"date":"2025-12-31","comment":"old"}),
            json!({"user":{"id":7},"date":"2026-04-01T00:00:00Z","comment":"target"}),
            json!({"user":{"id":7},"date":"2026-04-02","comment":"future"}),
            json!({"user":{"id":7},"date":"2026-03-01not-a-date","comment":"malformed"}),
            json!({"user":{"id":7},"date":"2026-03-01","comment":" \n\t "}),
            json!({"user":{"id":7},"date":"2026-03-01","internalComment":"not public comment"}),
        ] {
            assert!(style_comment(&row, scope).is_none());
        }
        for date in [
            "2026-01-01",
            "2026-03-31T23:59:59.123Z",
            "2026-03-31T23:59:59",
            "2026-03-31 23:59:59",
        ] {
            let row = json!({"user":{"id":7},"date":date,"comment":"  Clear\n task  ","internalComment":"never include","project":{"name":"never include"},"hours":8});
            assert_eq!(style_comment(&row, scope).unwrap().1, "Clear task");
        }
    }
    #[test]
    fn style_history_deduplicates_bounds_unicode_and_keeps_short_and_long_examples() {
        let scope = scope();
        let mut comments = Vec::new();
        for day in 1..=25 {
            let row = json!({"user":{"id":7},"date":format!("2026-03-{day:02}"),"comment":format!("{day} {}", "å".repeat(800))});
            comments.push(style_comment(&row, scope).unwrap());
        }
        comments.push((scope.to, "Short task".into()));
        comments.push((scope.from, "Short task".into()));
        comments.push((
            scope.to,
            "A longer representative description with several distinct words".into(),
        ));
        let output = style_reference(comments, scope);
        let examples = output["examples"].as_array().unwrap();
        assert!(examples.len() <= STYLE_MAX_EXAMPLES);
        assert_eq!(
            output["sample_count"].as_u64().unwrap() as usize,
            examples.len()
        );
        assert_eq!(examples.iter().filter(|v| **v == "Short task").count(), 1);
        assert!(examples
            .iter()
            .any(|v| v == "A longer representative description with several distinct words"));
        assert!(examples
            .iter()
            .all(|v| v.as_str().unwrap().chars().count() <= STYLE_EXAMPLE_CHARS));
        assert!(
            examples
                .iter()
                .map(|v| v.as_str().unwrap().chars().count())
                .sum::<usize>()
                <= STYLE_TOTAL_CHARS
        );
        let keys = output
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        assert_eq!(
            keys,
            HashSet::from(["sample_count", "median_words", "examples", "scope"])
        );
        assert_eq!(output["median_words"], 2);
        assert_eq!(output["scope"]["usage"], "style_only_not_evidence");
    }
    #[test]
    fn style_history_rejects_oversized_or_incomplete_page_metadata() {
        assert!(style_page(&json!({"items":[],"totalPages":0}), 1).is_ok());
        assert!(style_page(&json!({"items":[],"totalPages":21}), 1).is_err());
        assert!(style_page(&json!({"items":[],"totalPages":2}), 1).is_err());
        assert!(style_page(&json!({"items":[]}), 1).is_err());
        assert!(style_page(&json!({"items":vec![json!({});101],"totalPages":1}), 1).is_err());
        assert!(style_page(&json!({"items":[{}],"totalPages":0}), 1).is_err());
    }
    #[tokio::test]
    async fn style_history_empty_is_valid_but_fetch_failure_never_claims_a_style() {
        let value = writing_style_with(
            &settings(),
            &catalog_fixture(),
            "2026-04-01",
            |scope| async move { Ok(style_reference(vec![], scope)) },
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(value["sample_count"], 0);
        assert_eq!(value["median_words"], 0);
        assert_eq!(value["examples"], json!([]));
        let error = writing_style_with(&settings(), &catalog_fixture(), "2026-04-01", |_| async {
            Err(STYLE_HISTORY_ERROR.into())
        })
        .await
        .unwrap_err();
        assert!(error.contains("försök igen"));
        assert!(error.contains("stäng av rapporthistorik"));
    }
}
