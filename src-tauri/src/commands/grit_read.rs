//! Verified archive reads for the original timeline and overview.
//! Local rows stay authoritative until a day is archived or the local day is empty.

use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};
use work_review_core::analysis::hourly::{generate_fallback_summary, HourlyStats};
use work_review_core::config::WorkTimeSegment;
use work_review_core::database::{Activity, DailyStats, Database, HourlyAppBucket};
use work_review_core::error::AppError;

const MAX_ARCHIVE_RANGE_DAYS: i64 = 31;

#[derive(Debug)]
pub(crate) enum DayActivities {
    /// Caller should keep the existing SQL reader.
    Local,
    Archive(Vec<Activity>),
}

pub(crate) fn day_activities(
    data_dir: &Path,
    database: &Database,
    date: &str,
) -> Result<DayActivities, AppError> {
    let parsed = parse_day(date)?;
    let _ = parsed;
    let local = database
        .get_activities_in_range(Some(date), Some(date), 10_000)
        .map_err(|error| AppError::Unknown(error.to_string()))?;
    if local.len() >= 10_000 {
        return Err(AppError::Unknown(
            "该日活动超过读取上限，已拒绝不完整结果".to_string(),
        ));
    }
    let archived = archived_flag(data_dir, date)?;
    if archived {
        return Ok(DayActivities::Archive(load_verified(data_dir, date)?));
    }
    if !local.is_empty() {
        return Ok(DayActivities::Local);
    }
    match archive_dir(data_dir)? {
        Some(dir) if !dir.as_os_str().is_empty() => {
            let loaded = crate::grit_engine::load_archive(&dir, date).map_err(|error| {
                AppError::Unknown(format!("归档校验失败，已停止读取 {date}: {error}"))
            });
            match loaded {
                Ok(value) => Ok(DayActivities::Archive(activities_from_archive(value)?)),
                Err(error) => {
                    if archive_day_exists(&dir, date) {
                        Err(error)
                    } else {
                        Ok(DayActivities::Local)
                    }
                }
            }
        }
        _ => Ok(DayActivities::Local),
    }
}

pub(crate) fn ensure_archive_range(
    data_dir: &Path,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<(), AppError> {
    let days = (end - start).num_days();
    if days < MAX_ARCHIVE_RANGE_DAYS {
        return Ok(());
    }
    if range_has_archived_day(data_dir, start, end)? {
        return Err(AppError::Config(format!(
            "含归档日的读取区间最多 {MAX_ARCHIVE_RANGE_DAYS} 天"
        )));
    }
    Ok(())
}

fn ephemeral_database(activities: &[Activity]) -> Result<Database, AppError> {
    let database = Database::open_in_memory()?;
    let mut rows: Vec<&Activity> = activities.iter().collect();
    rows.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then(left.id.cmp(&right.id))
    });
    for activity in rows {
        database.insert_activity_snapshot(activity)?;
    }
    Ok(database)
}

pub(crate) fn timeline_page(
    date: &str,
    activities: Vec<Activity>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Activity>, AppError> {
    let database = ephemeral_database(&activities)?;
    database.get_timeline(date, limit, offset)
}

pub(crate) fn daily_stats_from_activities(
    date: &str,
    activities: &[Activity],
    segments: &[WorkTimeSegment],
) -> Result<DailyStats, AppError> {
    let _ = parse_day(date)?;
    let database = ephemeral_database(activities)?;
    database.get_daily_stats_with_segments(date, segments)
}

/// Deterministic hourly drawer text from the original hourly activity query.
/// Screenshot paths are kept only when they resolve to a regular file.
pub(crate) fn hourly_summaries_from_activities(
    date: &str,
    activities: &[Activity],
    data_dir: &Path,
) -> Result<Vec<serde_json::Value>, AppError> {
    let _ = parse_day(date)?;
    let database = ephemeral_database(activities)?;
    let mut summaries = Vec::new();
    for hour in 0..24 {
        let bucket = database.get_hourly_activities(date, hour)?;
        if bucket.is_empty() {
            continue;
        }
        let stats = HourlyStats::from_activities(bucket);
        let screenshots = stats
            .representative_screenshots
            .iter()
            .filter_map(|path| verified_capture_file(data_dir, path))
            .collect::<Vec<_>>();
        summaries.push(serde_json::json!({
            "hour": hour,
            "summary": generate_fallback_summary(&stats),
            "main_apps": stats.get_main_apps().join(", "),
            "activity_count": stats.activity_count,
            "total_duration": stats.total_duration,
            "representative_screenshots": screenshots,
        }));
    }
    Ok(summaries)
}

pub(crate) fn hourly_apps_from_activities(
    date: &str,
    activities: &[Activity],
) -> Result<Vec<HourlyAppBucket>, AppError> {
    let database = ephemeral_database(activities)?;
    database.get_hourly_app_breakdown(date)
}

pub(crate) fn add_hourly_app_buckets(target: &mut Vec<HourlyAppBucket>, extra: &[HourlyAppBucket]) {
    if target.len() < 24 {
        for hour in target.len()..24 {
            target.push(HourlyAppBucket {
                hour: hour as i32,
                total_duration: 0,
                apps: Vec::new(),
            });
        }
    }
    for bucket in extra {
        if !(0..24).contains(&bucket.hour) {
            continue;
        }
        let dest = &mut target[bucket.hour as usize];
        dest.total_duration += bucket.total_duration;
        for app in &bucket.apps {
            if let Some(existing) = dest
                .apps
                .iter_mut()
                .find(|item| item.app_name == app.app_name && item.category == app.category)
            {
                existing.duration += app.duration;
                if existing.screenshot_url.as_deref().unwrap_or("").is_empty() {
                    existing.screenshot_url.clone_from(&app.screenshot_url);
                }
            } else {
                dest.apps.push(app.clone());
            }
        }
    }
}

fn verified_capture_file(data_dir: &Path, raw: &str) -> Option<String> {
    let resolved = resolve_capture_path(data_dir, raw).ok()?;
    let meta = std::fs::symlink_metadata(&resolved).ok()?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return None;
    }
    Some(resolved.to_string_lossy().to_string())
}

pub(crate) fn resolve_capture_path(data_dir: &Path, raw: &str) -> Result<PathBuf, AppError> {
    let path = Path::new(raw);
    if !path.is_absolute() {
        super::shared::validate_relative_path(raw)?;
        return Ok(data_dir.join(path));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::Unknown("非法路径：不允许包含 ..".to_string()));
    }
    let root = archive_dir(data_dir)?
        .ok_or_else(|| AppError::Unknown("绝对截图路径不在已验证归档中".to_string()))?;
    let archive_root = root.join("grit-work");
    if !path.starts_with(&archive_root) {
        return Err(AppError::Unknown(
            "绝对截图路径不在已验证归档中".to_string(),
        ));
    }
    let meta = std::fs::symlink_metadata(path)
        .map_err(|error| AppError::Unknown(format!("无法读取归档截图: {error}")))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(AppError::Unknown("归档截图必须是普通文件".to_string()));
    }
    Ok(path.to_path_buf())
}

fn load_verified(data_dir: &Path, date: &str) -> Result<Vec<Activity>, AppError> {
    let dir = archive_dir(data_dir)?
        .ok_or_else(|| AppError::Unknown(format!("日期 {date} 已归档，但没有配置归档目录")))?;
    let value = crate::grit_engine::load_archive(&dir, date)
        .map_err(|error| AppError::Unknown(format!("归档校验失败，已停止读取 {date}: {error}")))?;
    activities_from_archive(value)
}

fn activities_from_archive(value: Value) -> Result<Vec<Activity>, AppError> {
    serde_json::from_value(value)
        .map_err(|error| AppError::Unknown(format!("归档活动无法还原为时间线条目: {error}")))
}

fn archived_flag(data_dir: &Path, date: &str) -> Result<bool, AppError> {
    let Some(connection) = open_grit_db(data_dir)? else {
        return Ok(false);
    };
    let value: Option<i64> = connection
        .query_row("SELECT archived FROM days WHERE date=?", [date], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| AppError::Unknown(format!("读取归档标记失败: {error}")))?;
    Ok(value.unwrap_or(0) != 0)
}

fn range_has_archived_day(
    data_dir: &Path,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<bool, AppError> {
    let Some(connection) = open_grit_db(data_dir)? else {
        return Ok(false);
    };
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM days WHERE archived=1 AND date>=?1 AND date<=?2",
            [
                start.format("%Y-%m-%d").to_string(),
                end.format("%Y-%m-%d").to_string(),
            ],
            |row| row.get(0),
        )
        .map_err(|error| AppError::Unknown(format!("读取归档区间失败: {error}")))?;
    Ok(count > 0)
}

fn archive_dir(data_dir: &Path) -> Result<Option<PathBuf>, AppError> {
    let Some(connection) = open_grit_db(data_dir)? else {
        return Ok(None);
    };
    let raw: Option<String> = connection
        .query_row("SELECT value FROM kv WHERE key='settings'", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| AppError::Unknown(format!("读取归档设置失败: {error}")))?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let settings: crate::grit::Settings = serde_json::from_str(&raw)
        .map_err(|error| AppError::Unknown(format!("归档设置无法解析，已停止读取: {error}")))?;
    if settings.archive_dir.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(settings.archive_dir)))
    }
}

fn open_grit_db(data_dir: &Path) -> Result<Option<Connection>, AppError> {
    let path = data_dir.join("grit.db");
    if !path.is_file() {
        return Ok(None);
    }
    Connection::open(&path)
        .map(Some)
        .map_err(|error| AppError::Unknown(format!("打开 Grit 记录库失败: {error}")))
}

fn archive_day_exists(archive_dir: &Path, date: &str) -> bool {
    archive_dir.join("grit-work").join(date).is_dir()
}

fn parse_day(date: &str) -> Result<NaiveDate, AppError> {
    if date.len() != 10 {
        return Err(AppError::Config("日期必须是 YYYY-MM-DD".to_string()));
    }
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|error| AppError::Config(format!("日期无效: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};
    use serde_json::json;
    use work_review_core::config::WorkTimeSegment;

    fn stamp() -> i64 {
        Local
            .with_ymd_and_hms(2020, 1, 15, 10, 0, 0)
            .unwrap()
            .timestamp()
    }

    fn activity(app: &str, path: &str) -> Activity {
        Activity {
            id: Some(1),
            timestamp: stamp(),
            app_name: app.into(),
            window_title: "note".into(),
            screenshot_path: path.into(),
            ocr_text: Some("hello".into()),
            category: "development".into(),
            duration: 120,
            browser_url: None,
            executable_path: Some("/usr/bin/app".into()),
            semantic_category: None,
            semantic_confidence: None,
            screenshot_url: Some("https://shots.example/a.png".into()),
        }
    }

    fn segment() -> WorkTimeSegment {
        WorkTimeSegment {
            start_hour: 9,
            start_minute: 0,
            end_hour: 18,
            end_minute: 0,
        }
    }

    #[test]
    fn archived_day_matches_local_duration_and_drops_duplicate_rows() {
        let root = std::env::temp_dir().join(format!("grit-read-{}", uuid::Uuid::new_v4()));
        let data = root.join("data");
        let archive = root.join("archive");
        std::fs::create_dir_all(data.join("shots")).unwrap();
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(data.join("shots/a.png"), b"png").unwrap();
        let database = Database::new(&data.join("app.db")).unwrap();
        let mut row = activity("Editor", "shots/a.png");
        database.insert_activity(&row).unwrap();
        let local_stats = database
            .get_daily_stats_with_segments("2020-01-15", &[segment()])
            .unwrap();
        row.id = Some(
            database
                .get_activities_in_range(Some("2020-01-15"), Some("2020-01-15"), 10)
                .unwrap()[0]
                .id
                .unwrap(),
        );
        let mut archived_row = row.clone();
        archived_row.screenshot_path = data.join("shots/a.png").display().to_string();
        crate::grit_engine::archive_day(
            &data,
            &archive,
            "2020-01-15",
            &serde_json::to_value(vec![archived_row.clone()]).unwrap(),
        )
        .unwrap();
        database.delete_activities_by_date("2020-01-15").unwrap();
        let connection = Connection::open(data.join("grit.db")).unwrap();
        connection
            .execute_batch("CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS days(date TEXT PRIMARY KEY,dismissed INTEGER NOT NULL DEFAULT 0,archived INTEGER NOT NULL DEFAULT 0);")
            .unwrap();
        let settings = crate::grit::Settings {
            archive_dir: archive.display().to_string(),
            ..crate::grit::Settings::default()
        };
        connection
            .execute(
                "INSERT INTO kv(key,value) VALUES('settings',?1)",
                [serde_json::to_string(&settings).unwrap()],
            )
            .unwrap();
        connection
            .execute("INSERT INTO days(date,archived) VALUES('2020-01-15',1)", [])
            .unwrap();
        let mut leftover = activity("Other", "shots/a.png");
        leftover.id = Some(99);
        database.insert_activity(&leftover).unwrap();
        match day_activities(&data, &database, "2020-01-15").unwrap() {
            DayActivities::Archive(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].app_name, "Editor");
                assert_eq!(rows[0].ocr_text.as_deref(), Some("hello"));
                let stats = daily_stats_from_activities("2020-01-15", &rows, &[segment()]).unwrap();
                assert_eq!(stats.total_duration, local_stats.total_duration);
                assert_eq!(stats.total_duration, 120);
            }
            DayActivities::Local => panic!("archived day must not stay on local rows"),
        }
        let image = crate::grit_engine::load_archive(&archive, "2020-01-15").unwrap()[0]
            ["screenshot_path"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(resolve_capture_path(&data, &image).is_ok());
        assert!(resolve_capture_path(&data, "/etc/passwd").is_err());
        std::fs::write(
            archive.join("grit-work/2020-01-15/activities.json"),
            b"corrupt",
        )
        .unwrap();
        assert!(day_activities(&data, &database, "2020-01-15")
            .unwrap_err()
            .to_string()
            .contains("归档校验失败"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn empty_local_day_reads_archive_and_long_range_fails() {
        let root = std::env::temp_dir().join(format!("grit-read-empty-{}", uuid::Uuid::new_v4()));
        let data = root.join("data");
        let archive = root.join("archive");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(data.join("a.png"), b"png").unwrap();
        let row = activity("Editor", &data.join("a.png").display().to_string());
        crate::grit_engine::archive_day(&data, &archive, "2020-01-15", &json!([row])).unwrap();
        let connection = Connection::open(data.join("grit.db")).unwrap();
        connection
            .execute_batch("CREATE TABLE kv(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE days(date TEXT PRIMARY KEY,dismissed INTEGER NOT NULL DEFAULT 0,archived INTEGER NOT NULL DEFAULT 0);")
            .unwrap();
        let settings = crate::grit::Settings {
            archive_dir: archive.display().to_string(),
            ..Default::default()
        };
        connection
            .execute(
                "INSERT INTO kv(key,value) VALUES('settings',?1)",
                [serde_json::to_string(&settings).unwrap()],
            )
            .unwrap();
        let database = Database::new(&data.join("app.db")).unwrap();
        match day_activities(&data, &database, "2020-01-15").unwrap() {
            DayActivities::Archive(rows) => assert_eq!(rows[0].app_name, "Editor"),
            DayActivities::Local => panic!("empty local day should read the archive"),
        }
        connection
            .execute("INSERT INTO days(date,archived) VALUES('2020-01-15',1)", [])
            .unwrap();
        assert!(ensure_archive_range(
            &data,
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2020, 1, 31).unwrap()
        )
        .is_ok());
        assert!(ensure_archive_range(
            &data,
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2020, 2, 1).unwrap()
        )
        .is_err());
        let error = ensure_archive_range(
            &data,
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2020, 3, 1).unwrap(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("31"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn archived_hourly_summary_and_domains_use_verified_evidence() {
        let root = std::env::temp_dir().join(format!("grit-read-hour-{}", uuid::Uuid::new_v4()));
        let data = root.join("data");
        let archive = root.join("archive");
        std::fs::create_dir_all(data.join("shots")).unwrap();
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(data.join("shots/live.png"), b"png").unwrap();
        let mut row = activity(
            "Firefox",
            &data.join("shots/live.png").display().to_string(),
        );
        row.browser_url = Some("https://docs.example.com:443/guide?q=1".into());
        row.semantic_category = Some("docs".into());
        row.executable_path = Some("/usr/bin/firefox".into());
        row.window_title = "Guide - docs.example.com".into();
        let connection = Connection::open(data.join("grit.db")).unwrap();
        connection
            .execute_batch("CREATE TABLE kv(key TEXT PRIMARY KEY,value TEXT NOT NULL);")
            .unwrap();
        let settings = crate::grit::Settings {
            archive_dir: archive.display().to_string(),
            ..Default::default()
        };
        connection
            .execute(
                "INSERT INTO kv(key,value) VALUES('settings',?1)",
                [serde_json::to_string(&settings).unwrap()],
            )
            .unwrap();
        let file_db = Database::new(&data.join("app.db")).unwrap();
        file_db.insert_activity(&row).unwrap();
        let mut editor = activity("Editor", &data.join("shots/live.png").display().to_string());
        editor.window_title = "notes.rs".into();
        editor.timestamp = stamp() + 30;
        editor.id = Some(2);
        editor.executable_path = Some("/usr/bin/editor".into());
        file_db.insert_activity(&editor).unwrap();
        let stored = file_db
            .get_activities_in_range(Some("2020-01-15"), Some("2020-01-15"), 10)
            .unwrap();
        crate::grit_engine::archive_day(
            &data,
            &archive,
            "2020-01-15",
            &serde_json::to_value(&stored).unwrap(),
        )
        .unwrap();
        let loaded = activities_from_archive(
            crate::grit_engine::load_archive(&archive, "2020-01-15").unwrap(),
        )
        .unwrap();
        let sql_stats = file_db
            .get_daily_stats_with_segments("2020-01-15", &[segment()])
            .unwrap();
        let archived_stats =
            daily_stats_from_activities("2020-01-15", &loaded, &[segment()]).unwrap();
        let mut sql_stats = sql_stats;
        let mut archived_stats = archived_stats;
        sql_stats
            .app_usage
            .sort_by(|left, right| left.app_name.cmp(&right.app_name));
        archived_stats
            .app_usage
            .sort_by(|left, right| left.app_name.cmp(&right.app_name));
        assert_eq!(
            serde_json::to_value(&sql_stats).unwrap(),
            serde_json::to_value(&archived_stats).unwrap()
        );
        assert!(archived_stats
            .domain_usage
            .iter()
            .any(|domain| domain.domain == "docs.example.com" && domain.duration == 120));
        assert!(archived_stats.browser_usage.iter().any(|browser| {
            browser.browser_name.to_lowercase().contains("firefox")
                && browser.executable_path.is_some()
                && browser
                    .domains
                    .iter()
                    .any(|domain| domain.domain == "docs.example.com")
        }));
        assert!(archived_stats.app_usage.iter().any(|app| {
            app.app_name == "Editor"
                && app.executable_path.as_deref() == Some("/usr/bin/editor")
                && app.screenshot_url.is_some()
        }));
        let sql_timeline = file_db.get_timeline("2020-01-15", None, None).unwrap();
        let archived_timeline = timeline_page("2020-01-15", loaded.clone(), None, None).unwrap();
        let project = |rows: &[Activity]| {
            rows.iter()
                .map(|row| {
                    (
                        row.app_name.clone(),
                        row.duration,
                        row.browser_url.clone(),
                        row.executable_path.clone(),
                        row.screenshot_url.clone(),
                        row.window_title.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(project(&sql_timeline), project(&archived_timeline));
        assert_eq!(
            serde_json::to_value(file_db.get_hourly_app_breakdown("2020-01-15").unwrap()).unwrap(),
            serde_json::to_value(hourly_apps_from_activities("2020-01-15", &loaded).unwrap())
                .unwrap()
        );
        let archived_path = loaded
            .iter()
            .find(|row| row.app_name == "Firefox")
            .unwrap()
            .screenshot_path
            .clone();
        let mut stale = activity("Editor", "shots/missing.png");
        stale.timestamp += 60;
        stale.id = Some(9);
        let mut with_stale = loaded.clone();
        with_stale.push(stale);
        let summaries = hourly_summaries_from_activities("2020-01-15", &with_stale, &data).unwrap();
        assert!(!summaries.is_empty());
        let shots = &summaries[0]["representative_screenshots"];
        assert!(shots.as_array().unwrap().contains(&json!(archived_path)));
        assert!(shots
            .as_array()
            .unwrap()
            .iter()
            .all(|path| path.as_str().unwrap() != "shots/missing.png"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn grouped_timeline_keeps_original_evidence_ids() {
        let mut older = activity("Editor", "shots/a.png");
        older.id = Some(123);
        older.window_title = "same-note".into();
        let mut newer = older.clone();
        newer.id = Some(907);
        newer.timestamp += 15;
        let database = Database::open_in_memory().unwrap();
        database.insert_activity_snapshot(&older).unwrap();
        database.insert_activity_snapshot(&newer).unwrap();
        let sql = database.get_timeline("2020-01-15", None, None).unwrap();
        let archived = timeline_page("2020-01-15", vec![older, newer], None, None).unwrap();
        assert_eq!(sql.len(), 1);
        assert_eq!(sql[0].id, Some(907));
        assert_eq!(archived[0].id, Some(907));
        assert_eq!(sql[0].duration, archived[0].duration);
    }
}
