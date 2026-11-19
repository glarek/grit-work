//! Durable, reusable image evidence. A missing interpretation never borrows another image.
use crate::image_ocr::OcrRegion;
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const SCHEMA_VERSION: u32 = 1;
const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Off,
    Ocr,
    Ai,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ImageSettings {
    pub mode: Mode,
    pub interval_seconds: u64,
    pub provider: String,
    pub model: String,
}
impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            mode: Mode::Off,
            interval_seconds: 300,
            provider: "codex".into(),
            model: "gpt-6.2-sol".into(),
        }
    }
}
impl ImageSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(30..=3600).contains(&self.interval_seconds) {
            return Err("Bildtolkningens intervall måste vara 30–3 600 sekunder.".into());
        }
        if !["codex", "grok", "agy"].contains(&self.provider.as_str())
            || self.model.trim().is_empty()
            || self.model.len() > 200
            || self.model.chars().any(char::is_control)
        {
            return Err("Välj en giltig CLI-leverantör och modell för bildtolkning.".into());
        }
        Ok(())
    }
    fn fingerprint(&self) -> String {
        match self.mode {
            Mode::Off => "off:v1".into(),
            Mode::Ocr => "windows-ocr:v1".into(),
            Mode::Ai => format!("ai:v1:{}:{}", self.provider, self.model),
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImageContent {
    pub text: String,
    pub summary: String,
    pub observations: Vec<String>,
    pub regions: Vec<OcrRegion>,
    pub language: Option<String>,
    pub confidence: Option<f64>,
}
impl ImageContent {
    pub fn validate(&self) -> Result<(), String> {
        let chars = self.text.chars().count()
            + self.summary.chars().count()
            + self
                .observations
                .iter()
                .map(|s| s.chars().count())
                .sum::<usize>()
            + self
                .regions
                .iter()
                .map(|r| r.text.chars().count())
                .sum::<usize>();
        if chars > 60_000
            || self.summary.chars().count() > 4000
            || self.observations.len() > 32
            || self.regions.len() > 2000
            || self.language.as_ref().is_some_and(|s| s.len() > 64)
            || self
                .confidence
                .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            return Err(
                "Bildtolkningen överskrider tillåten storlek eller har ogiltigt format.".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Disabled,
    SkippedInterval,
    Pending,
    Processing,
    Completed,
    Failed,
    AwaitingConsent,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceStamp {
    bytes: u64,
    modified_ns: u128,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImageEntry {
    pub image_id: String,
    pub source_file: String,
    pub captured_at: i64,
    pub evidence_ids: Vec<i64>,
    pub method: Mode,
    pub status: EntryStatus,
    pub fingerprint: String,
    pub image_sha256: Option<String>,
    pub source_stamp: Option<SourceStamp>,
    pub processed_at: Option<i64>,
    pub content: ImageContent,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DayFile {
    pub schema_version: u32,
    pub date: String,
    pub updated_at: i64,
    pub configuration: ImageSettings,
    pub entries: Vec<ImageEntry>,
}
#[derive(Clone, Debug, Serialize)]
pub struct InterpretationStatus {
    pub date: String,
    pub file: String,
    pub completed: usize,
    pub pending: usize,
    pub failed: usize,
    pub disabled: usize,
    pub skipped: usize,
    pub running: bool,
    pub last_error: Option<String>,
}
pub fn file_path(root: &Path, date: &str) -> Result<PathBuf, String> {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "Ogiltigt datum för bildtolkning.".to_string())?;
    if parsed.to_string() != date {
        return Err("Ange datum som YYYY-MM-DD.".into());
    }
    Ok(root
        .join("image-interpretations")
        .join(format!("{date}.json")))
}
fn read_file(path: &Path, date: &str) -> Result<Option<DayFile>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Tolkningsfilen kunde inte öppnas.".into()),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_FILE_BYTES {
        return Err("Tolkningsfilen är inte en giltig begränsad datafil.".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| "Kunde inte läsa tolkningsfilen.")?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Kunde inte läsa tolkningsfilen.")?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Tolkningsfilen är för stor.".into());
    }
    let day: DayFile = serde_json::from_slice(&bytes)
        .map_err(|_| "Tolkningsfilen är skadad eller har fel format.")?;
    if day.schema_version != SCHEMA_VERSION || day.date != date || day.entries.len() > MAX_ENTRIES {
        return Err("Tolkningsfilens version, datum eller antal bilder är ogiltigt.".into());
    }
    for entry in &day.entries {
        entry.content.validate()?;
    }
    Ok(Some(day))
}
pub fn load(root: &Path, archive: &Path, date: &str) -> Result<Option<DayFile>, String> {
    let local = file_path(root, date)?;
    if let Some(parent) = local.parent().filter(|p| p.exists()) {
        let root_real = root.canonicalize().map_err(|_| "Datamappen saknas.")?;
        if !parent
            .canonicalize()
            .map_err(|_| "Tolkningsmappen saknas.")?
            .starts_with(root_real)
        {
            return Err("Tolkningsmappen ligger utanför lokal data.".into());
        }
    }
    if let Some(day) = read_file(&local, date)? {
        return Ok(Some(day));
    }
    // Archive callers have verified the day before using its evidence. Verify again
    // before consuming a companion file; never trust an unmanifested sidecar.
    if !archive.as_os_str().is_empty() {
        let archived_root = archive.join("grit-work").join(date);
        let archived = file_path(&archived_root, date)?;
        if archived.exists() {
            crate::grit_engine::load_archive(archive, date)?;
            return read_file(&archived, date);
        }
    }
    Ok(None)
}
pub fn persist(root: &Path, day: &DayFile) -> Result<(), String> {
    let path = file_path(root, &day.date)?;
    let parent = path.parent().ok_or("Ogiltig tolkningsmapp.")?;
    fs::create_dir_all(parent).map_err(|_| "Kunde inte skapa tolkningsmappen.")?;
    let root_real = root.canonicalize().map_err(|_| "Datamappen saknas.")?;
    if !parent
        .canonicalize()
        .map_err(|_| "Tolkningsmappen saknas.")?
        .starts_with(&root_real)
    {
        return Err("Tolkningsmappen ligger utanför lokal data.".into());
    }
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Tolkningsfilen får inte vara en symbolisk länk.".into());
    }
    let bytes = serde_json::to_vec_pretty(day).map_err(|_| "Kunde inte skapa tolkningsfilen.")?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Dygnets tolkningsfil är för stor. Öka tolkningsintervallet.".into());
    }
    let temp = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.create_new(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .map_err(|_| "Kunde inte skapa säker tolkningsfil.")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Kunde inte spara tolkningsfilen.")?;
        drop(file);
        if fs::read(&temp).map_err(|_| "Kunde inte verifiera tolkningsfilen.")? != bytes {
            return Err("Tolkningsfilens återläsning stämmer inte.".to_string());
        }
        fs::rename(&temp, &path).map_err(|_| "Kunde inte publicera tolkningsfilen.")?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|d| d.sync_all())
            .map_err(|_| "Kunde inte synkronisera tolkningsmappen.")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn source_key(
    root: &Path,
    archive: &Path,
    date: &str,
    raw: &str,
) -> Result<(String, PathBuf), String> {
    let original = PathBuf::from(raw);
    let path = if original.is_absolute() {
        original.canonicalize().unwrap_or(original)
    } else {
        original
    };
    let local_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let root = local_root.as_path();
    let archive_base = archive
        .canonicalize()
        .unwrap_or_else(|_| archive.to_path_buf());
    let archive_shots = archive_base.join("grit-work").join(date).join("shots");
    let (relative, full) = if !path.is_absolute() {
        (path.clone(), root.join(&path))
    } else if let Ok(rel) = path.strip_prefix(root) {
        (rel.to_path_buf(), path.clone())
    } else if !archive.as_os_str().is_empty() {
        (
            path.strip_prefix(&archive_shots)
                .map_err(|_| "Bildsökvägen ligger utanför underlaget.")?
                .to_path_buf(),
            path.clone(),
        )
    } else {
        return Err("Bildsökvägen ligger utanför underlaget.".into());
    };
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Ogiltig sökväg till underlagsbild.".into());
    }
    let key = relative.to_string_lossy().replace('\\', "/");
    Ok((key, full))
}
fn stamp(path: &Path) -> Result<SourceStamp, String> {
    let m = fs::symlink_metadata(path).map_err(|_| "Underlagsbilden saknas.")?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > MAX_IMAGE_BYTES {
        return Err("Underlagsbilden är ogiltig eller för stor.".into());
    }
    Ok(SourceStamp {
        bytes: m.len(),
        modified_ns: m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    })
}
fn image_hash(path: &Path, root: &Path, archive: &Path) -> Result<String, String> {
    let actual = path.canonicalize().map_err(|_| "Underlagsbilden saknas.")?;
    let local = root.canonicalize().map_err(|_| "Datamappen saknas.")?;
    let remote = if archive.as_os_str().is_empty() {
        None
    } else {
        archive.canonicalize().ok()
    };
    if !actual.starts_with(&local) && !remote.is_some_and(|r| actual.starts_with(r)) {
        return Err("Underlagsbilden ligger utanför tillåten lagring.".into());
    }
    stamp(path)?;
    let mut file = File::open(actual)
        .map_err(|_| "Kunde inte öppna underlagsbilden.")?
        .take(MAX_IMAGE_BYTES + 1);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut size = 0;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|_| "Kunde inte läsa underlagsbilden.")?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > MAX_IMAGE_BYTES {
            return Err("Underlagsbilden är för stor.".into());
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
struct Source {
    key: String,
    path: PathBuf,
    timestamp: i64,
    ids: Vec<i64>,
}
fn capture_timestamp(path: &Path, date: &str) -> Option<i64> {
    let stem = path.file_stem()?.to_str()?;
    if stem.len() != 10 || stem.as_bytes().get(6) != Some(&b'_') {
        return None;
    }
    let date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let time = chrono::NaiveTime::parse_from_str(stem, "%H%M%S_%3f").ok()?;
    Local
        .from_local_datetime(&date.and_time(time))
        .single()
        .map(|t| t.timestamp())
}
fn sources(
    root: &Path,
    archive: &Path,
    date: &str,
    activities: &Value,
) -> Result<Vec<Source>, String> {
    let mut found: BTreeMap<String, Source> = BTreeMap::new();
    let rows = activities
        .as_array()
        .ok_or("Ogiltigt aktivitetsunderlag.")?;
    if rows.len() > MAX_ENTRIES {
        return Err("För många aktivitetsposter för ett dygn.".into());
    }
    for row in rows {
        let Some(raw) = row["screenshot_path"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        let (key, path) = source_key(root, archive, date, raw)?;
        let log_timestamp = row["timestamp"]
            .as_i64()
            .ok_or("Aktiviteten saknar tidsstämpel.")?;
        // Capture names carry the capture time, whereas the activity timestamp
        // is the interval end and may move when adjacent logs are merged.
        let timestamp = capture_timestamp(&path, date).unwrap_or(log_timestamp);
        let id = row["id"].as_i64().ok_or("Aktiviteten saknar ID.")?;
        let source = found.entry(key.clone()).or_insert_with(|| Source {
            key,
            path,
            timestamp,
            ids: vec![],
        });
        source.timestamp = source.timestamp.min(timestamp);
        source.ids.push(id);
    }
    let mut result: Vec<_> = found.into_values().collect();
    result.sort_by_key(|s| (s.timestamp, s.key.clone()));
    for source in &mut result {
        source.ids.sort_unstable();
        source.ids.dedup();
    }
    Ok(result)
}
fn plan(
    root: &Path,
    archive: &Path,
    date: &str,
    activities: &Value,
    settings: &ImageSettings,
    consent: bool,
    old: Option<DayFile>,
) -> Result<(DayFile, HashMap<String, PathBuf>), String> {
    settings.validate()?;
    let old: HashMap<_, _> = old
        .map(|d| {
            d.entries
                .into_iter()
                .map(|e| (e.image_id.clone(), e))
                .collect()
        })
        .unwrap_or_default();
    let mut entries = Vec::new();
    let mut paths = HashMap::new();
    let mut previous_selected: Option<i64> = None;
    let mut source_list = sources(root, archive, date, activities)?;
    // Merging activity logs can advance a row's timestamp. Keep the first
    // observed image time and order before applying the sampling interval.
    for source in &mut source_list {
        let id = format!("{:x}", Sha256::digest(source.key.as_bytes()));
        if let Some(cached) = old.get(&id) {
            source.timestamp = cached.captured_at;
        }
    }
    source_list.sort_by_key(|s| (s.timestamp, s.key.clone()));
    for source in source_list {
        let id = format!("{:x}", Sha256::digest(source.key.as_bytes()));
        let cached = old.get(&id);
        let timestamp = cached.map(|e| e.captured_at).unwrap_or(source.timestamp);
        let due = previous_selected.is_none_or(|previous| {
            timestamp.saturating_sub(previous) >= settings.interval_seconds as i64
        });
        if due {
            previous_selected = Some(timestamp);
        }
        let fingerprint = settings.fingerprint();
        let mut entry = ImageEntry {
            image_id: id.clone(),
            source_file: source.key,
            captured_at: timestamp,
            evidence_ids: source.ids,
            method: settings.mode.clone(),
            status: EntryStatus::Pending,
            fingerprint: fingerprint.clone(),
            image_sha256: None,
            source_stamp: None,
            processed_at: None,
            content: ImageContent::default(),
            error: None,
        };
        if settings.mode == Mode::Off {
            entry.status = EntryStatus::Disabled;
        } else if !due {
            entry.status = EntryStatus::SkippedInterval;
        } else if settings.mode == Mode::Ai && !consent {
            entry.status = EntryStatus::AwaitingConsent;
            entry.error = Some("AI-samtycke krävs innan bilder får skickas.".into());
        } else {
            match stamp(&source.path) {
                Ok(current_stamp) => {
                    if let Some(cached) = cached.filter(|c| c.fingerprint == fingerprint) {
                        let same = if cached.source_stamp.as_ref() == Some(&current_stamp) {
                            true
                        } else {
                            cached.image_sha256.as_ref().is_some_and(|h| {
                                image_hash(&source.path, root, archive).is_ok_and(|new| new == *h)
                            })
                        };
                        if same {
                            entry.status = cached.status.clone();
                            entry.content = cached.content.clone();
                            entry.image_sha256 = cached.image_sha256.clone();
                            entry.processed_at = cached.processed_at;
                            entry.error = cached.error.clone();
                        }
                    }
                    entry.source_stamp = Some(current_stamp);
                    if entry.status == EntryStatus::Processing {
                        entry.status = EntryStatus::Failed;
                        entry.error = Some("Bildtolkningen avbröts. Välj Försök igen.".into());
                        entry.content = ImageContent::default();
                    }
                }
                Err(error) => {
                    entry.status = EntryStatus::Failed;
                    entry.error = Some(error);
                }
            }
        }
        paths.insert(id, source.path);
        entries.push(entry);
    }
    Ok((
        DayFile {
            schema_version: SCHEMA_VERSION,
            date: date.into(),
            updated_at: Local::now().timestamp(),
            configuration: settings.clone(),
            entries,
        },
        paths,
    ))
}
/// One caller owns the application's operation lease while planning or mutating this file.
#[allow(clippy::too_many_arguments)]
pub fn process_day<F>(
    root: &Path,
    archive: &Path,
    date: &str,
    activities: &Value,
    settings: &ImageSettings,
    consent: bool,
    retry_failed: bool,
    max_images: usize,
    cancel: &Arc<AtomicBool>,
    mut interpret: F,
) -> Result<DayFile, String>
where
    F: FnMut(&Path, &ImageSettings) -> Result<ImageContent, String>,
{
    let old = load(root, archive, date)?;
    let (mut day, paths) = plan(
        root,
        archive,
        date,
        activities,
        settings,
        consent,
        old.clone(),
    )?;
    if old.as_ref().is_none_or(|previous| {
        previous.entries != day.entries || previous.configuration != day.configuration
    }) {
        persist(root, &day)?;
    } else if let Some(previous) = old {
        day.updated_at = previous.updated_at;
    }
    let mut attempted = 0;
    for index in 0..day.entries.len() {
        if attempted >= max_images || cancel.load(Ordering::SeqCst) {
            break;
        }
        if !matches!(day.entries[index].status, EntryStatus::Pending)
            && !(retry_failed && day.entries[index].status == EntryStatus::Failed)
        {
            continue;
        }
        attempted += 1;
        day.entries[index].status = EntryStatus::Processing;
        day.entries[index].content = ImageContent::default();
        day.entries[index].error = None;
        persist(root, &day)?;
        let path = paths
            .get(&day.entries[index].image_id)
            .ok_or("Bildreferensen saknas.")?;
        let result = (|| {
            let hash = image_hash(path, root, archive)?;
            let before = stamp(path)?;
            let content = interpret(path, settings)?;
            content.validate()?;
            if stamp(path)? != before || image_hash(path, root, archive)? != hash {
                return Err("Underlagsbilden ändrades under tolkningen.".into());
            }
            Ok((content, hash, before))
        })();
        match result {
            Ok((content, hash, source_stamp)) => {
                day.entries[index].content = content;
                day.entries[index].image_sha256 = Some(hash);
                day.entries[index].source_stamp = Some(source_stamp);
                day.entries[index].status = EntryStatus::Completed;
            }
            Err(error) => {
                day.entries[index].status = EntryStatus::Failed;
                day.entries[index].error = Some(error);
            }
        }
        day.entries[index].processed_at = Some(Local::now().timestamp());
        day.updated_at = Local::now().timestamp();
        persist(root, &day)?;
    }
    Ok(day)
}
pub fn status(
    root: &Path,
    date: &str,
    day: Option<&DayFile>,
    running: bool,
) -> Result<InterpretationStatus, String> {
    let mut result = InterpretationStatus {
        date: date.into(),
        file: file_path(root, date)?.display().to_string(),
        completed: 0,
        pending: 0,
        failed: 0,
        disabled: 0,
        skipped: 0,
        running,
        last_error: None,
    };
    if let Some(day) = day {
        for entry in &day.entries {
            match entry.status {
                EntryStatus::Completed => result.completed += 1,
                EntryStatus::Disabled => result.disabled += 1,
                EntryStatus::SkippedInterval => result.skipped += 1,
                EntryStatus::Failed => result.failed += 1,
                _ => result.pending += 1,
            }
            if result.last_error.is_none() {
                result.last_error = entry.error.clone();
            }
        }
    }
    Ok(result)
}
pub fn report_evidence(
    root: &Path,
    archive: &Path,
    date: &str,
    activities: &Value,
    day: &DayFile,
) -> Result<Value, String> {
    let mut output = activities.clone();
    for row in output.as_array_mut().ok_or("Ogiltigt underlag.")? {
        let raw = row["screenshot_path"].as_str().unwrap_or("");
        let entry = if raw.is_empty() {
            None
        } else {
            let (key, _) = source_key(root, archive, date, raw)?;
            let id = format!("{:x}", Sha256::digest(key.as_bytes()));
            day.entries.iter().find(|e| e.image_id == id)
        };
        if entry.is_some_and(|e| {
            matches!(
                e.status,
                EntryStatus::Pending
                    | EntryStatus::Processing
                    | EntryStatus::Failed
                    | EntryStatus::AwaitingConsent
            )
        }) {
            return Err("Bildtolkningen är inte färdig. Tolka bilderna eller välj Av under Insamling & lagring.".into());
        }
        let payload = entry.map(|e| {
            // Word boxes remain in the durable file for inspection. Sending the
            // same text again inside thousands of regions wastes report context.
            let mut content = e.content.clone();
            content.regions.clear();
            json!({"image_id":e.image_id,"captured_at":e.captured_at,"method":e.method,"status":e.status,"content":content})
        }).unwrap_or_else(|| json!({"method":day.configuration.mode,"status":"no_image","content":ImageContent::default()}));
        let obj = row.as_object_mut().ok_or("Ogiltig aktivitet.")?;
        obj.remove("screenshot_path");
        obj.remove("ocr_text");
        obj.remove("executable_path");
        obj.insert("image_interpretation".into(), payload);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("grit-image-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(p.join("screenshots")).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    const DATE: &str = "2026-11-18";
    fn logs(root: &Path) -> Value {
        for n in 0..3 {
            fs::write(
                root.join(format!("screenshots/{n}.png")),
                format!("synthetic image {n}"),
            )
            .unwrap();
        }
        json!((0..3).map(|n|json!({"id":n+1,"timestamp":1000+n*300,"duration":300,"app_name":"Editor","window_title":"Projekt ORBIT","screenshot_path":format!("screenshots/{n}.png"),"ocr_text":"OLD OCR MUST NOT LEAK"})).collect::<Vec<_>>())
    }
    fn content() -> ImageContent {
        ImageContent {
            text: "DRAWING-42".into(),
            ..ImageContent::default()
        }
    }
    fn run<F>(
        root: &Path,
        ev: &Value,
        settings: &ImageSettings,
        retry: bool,
        limit: usize,
        f: F,
    ) -> DayFile
    where
        F: FnMut(&Path, &ImageSettings) -> Result<ImageContent, String>,
    {
        process_day(
            root,
            Path::new(""),
            DATE,
            ev,
            settings,
            true,
            retry,
            limit,
            &Arc::new(AtomicBool::new(false)),
            f,
        )
        .unwrap()
    }
    #[test]
    fn off_writes_empty_content_and_never_opens_or_interprets_images() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        fs::remove_dir_all(tmp.0.join("screenshots")).unwrap();
        let day = run(
            &tmp.0,
            &ev,
            &ImageSettings::default(),
            false,
            usize::MAX,
            |_, _| panic!("Off cannot interpret"),
        );
        assert!(day
            .entries
            .iter()
            .all(|e| e.status == EntryStatus::Disabled && e.content == ImageContent::default()));
        let out = report_evidence(&tmp.0, Path::new(""), DATE, &ev, &day).unwrap();
        let text = out.to_string();
        assert!(!text.contains("OLD OCR"));
        assert!(!text.contains("screenshot_path"));
        assert!(text.contains("disabled"));
        assert_eq!(load(&tmp.0, Path::new(""), DATE).unwrap(), Some(day));
    }
    #[test]
    fn interval_selects_actual_images_and_keeps_skips_explicit() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            interval_seconds: 600,
            ..ImageSettings::default()
        };
        let mut calls = 0;
        let day = run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            calls += 1;
            Ok(content())
        });
        assert_eq!(calls, 2);
        assert_eq!(day.entries[1].status, EntryStatus::SkippedInterval);
        assert_eq!(day.entries[1].content, ImageContent::default());
        assert_eq!(day.entries[0].evidence_ids, vec![1]);
        assert_eq!(day.entries[2].evidence_ids, vec![3]);
    }
    #[test]
    fn merged_activity_does_not_reorder_cached_images_or_change_sampling() {
        let tmp = Temp::new();
        let mut ev = logs(&tmp.0);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            interval_seconds: 600,
            ..ImageSettings::default()
        };
        let first = run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            Ok(content())
        });
        ev[0]["timestamp"] = json!(3000);
        let second = run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            panic!("Cached images must not run again")
        });
        assert_eq!(first.entries, second.entries);
    }
    #[test]
    fn capture_filename_takes_precedence_over_merged_log_timestamp() {
        let timestamp =
            capture_timestamp(Path::new("screenshots/2026-11-18/093045_123.jpg"), DATE).unwrap();
        assert_eq!(
            Local
                .timestamp_opt(timestamp, 0)
                .unwrap()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            "2026-11-18 09:30:45"
        );
        assert!(capture_timestamp(Path::new("other.png"), DATE).is_none());
    }
    #[test]
    fn archived_images_reuse_interpretation_without_resending_or_ocr() {
        let tmp = Temp::new();
        let archive = Temp::new();
        let ev = logs(&tmp.0);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        let first = run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            Ok(content())
        });
        crate::grit_engine::archive_day(&tmp.0, &archive.0, DATE, &ev).unwrap();
        crate::grit_engine::clean_archived_day(&tmp.0, &archive.0, DATE, &ev).unwrap();
        // Also exercise recovery from the verified companion after local cache loss.
        fs::remove_file(file_path(&tmp.0, DATE).unwrap()).unwrap();
        let archived_ev = crate::grit_engine::load_archive(&archive.0, DATE).unwrap();
        let second = process_day(
            &tmp.0,
            &archive.0,
            DATE,
            &archived_ev,
            &settings,
            true,
            false,
            usize::MAX,
            &Arc::new(AtomicBool::new(false)),
            |_, _| panic!("Archive copy must reuse the interpreted content"),
        )
        .unwrap();
        assert_eq!(
            first
                .entries
                .iter()
                .map(|e| &e.image_id)
                .collect::<Vec<_>>(),
            second
                .entries
                .iter()
                .map(|e| &e.image_id)
                .collect::<Vec<_>>()
        );
        assert!(second
            .entries
            .iter()
            .all(|e| e.status == EntryStatus::Completed && e.content == content()));
        assert!(
            report_evidence(&tmp.0, &archive.0, DATE, &archived_ev, &second)
                .unwrap()
                .to_string()
                .contains("DRAWING-42")
        );
    }
    #[cfg(unix)]
    #[test]
    fn cache_directory_cannot_redirect_outside_local_data() {
        let root = Temp::new();
        let elsewhere = Temp::new();
        std::os::unix::fs::symlink(&elsewhere.0, root.0.join("image-interpretations")).unwrap();
        assert!(load(&root.0, Path::new(""), DATE).is_err());
    }
    #[test]
    fn completed_interpretations_are_reused_and_model_change_invalidates() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        let mut settings = ImageSettings {
            mode: Mode::Ai,
            ..ImageSettings::default()
        };
        let first = run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            Ok(content())
        });
        let cached = run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            panic!("cached image rerun")
        });
        assert_eq!(first.entries, cached.entries);
        settings.model = "new-model".into();
        let mut calls = 0;
        run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            calls += 1;
            Ok(content())
        });
        assert_eq!(calls, 3);
    }
    #[test]
    fn ai_without_consent_records_pending_empty_content_without_calls() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        let settings = ImageSettings {
            mode: Mode::Ai,
            ..ImageSettings::default()
        };
        let day = process_day(
            &tmp.0,
            Path::new(""),
            DATE,
            &ev,
            &settings,
            false,
            false,
            10,
            &Arc::new(AtomicBool::new(false)),
            |_, _| panic!("no consent"),
        )
        .unwrap();
        assert!(day
            .entries
            .iter()
            .all(|e| e.status == EntryStatus::AwaitingConsent
                && e.content == ImageContent::default()));
        assert!(report_evidence(&tmp.0, Path::new(""), DATE, &ev, &day).is_err());
    }
    #[test]
    fn failed_ocr_is_not_empty_success_and_needs_explicit_retry() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        let failed = run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            Err("OCR saknas".into())
        });
        assert!(failed
            .entries
            .iter()
            .all(|e| e.status == EntryStatus::Failed));
        run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            panic!("automatic retry")
        });
        let recovered = run(&tmp.0, &ev, &settings, true, usize::MAX, |_, _| {
            Ok(ImageContent::default())
        });
        assert!(recovered
            .entries
            .iter()
            .all(|e| e.status == EntryStatus::Completed && e.content.text.is_empty()));
    }
    #[test]
    fn max_images_resumes_without_repeating_completed_images() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        let first = run(&tmp.0, &ev, &settings, false, 1, |_, _| Ok(content()));
        assert_eq!(
            first
                .entries
                .iter()
                .filter(|e| e.status == EntryStatus::Completed)
                .count(),
            1
        );
        let mut calls = 0;
        let second = run(&tmp.0, &ev, &settings, false, 1, |_, _| {
            calls += 1;
            Ok(content())
        });
        assert_eq!(calls, 1);
        assert_eq!(
            second
                .entries
                .iter()
                .filter(|e| e.status == EntryStatus::Completed)
                .count(),
            2
        );
    }
    #[test]
    fn image_mutation_during_processing_rejects_content() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        let day = run(&tmp.0, &ev, &settings, false, 1, |path, _| {
            fs::write(path, "changed image data").unwrap();
            Ok(content())
        });
        assert_eq!(day.entries[0].status, EntryStatus::Failed);
        assert_eq!(day.entries[0].content, ImageContent::default());
    }
    #[test]
    fn changed_image_invalidates_cached_content() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            Ok(content())
        });
        fs::write(
            tmp.0.join("screenshots/1.png"),
            "replacement of different size",
        )
        .unwrap();
        let mut calls = 0;
        run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            calls += 1;
            Ok(content())
        });
        assert_eq!(calls, 1);
    }
    #[test]
    fn switching_off_clears_previous_ai_content() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        let ai = ImageSettings {
            mode: Mode::Ai,
            ..ImageSettings::default()
        };
        run(&tmp.0, &ev, &ai, false, usize::MAX, |_, _| Ok(content()));
        let off = run(
            &tmp.0,
            &ev,
            &ImageSettings::default(),
            false,
            usize::MAX,
            |_, _| panic!(),
        );
        assert!(off
            .entries
            .iter()
            .all(|e| e.content == ImageContent::default()));
        assert!(!fs::read_to_string(file_path(&tmp.0, DATE).unwrap())
            .unwrap()
            .contains("DRAWING-42"));
    }
    #[test]
    fn missing_image_does_not_borrow_another_image() {
        let tmp = Temp::new();
        let mut ev = logs(&tmp.0);
        ev.as_array_mut().unwrap().push(json!({"id":50,"timestamp":2300,"duration":300,"app_name":"Reader","screenshot_path":""}));
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        let day = run(&tmp.0, &ev, &settings, false, usize::MAX, |_, _| {
            Ok(content())
        });
        let out = report_evidence(&tmp.0, Path::new(""), DATE, &ev, &day).unwrap();
        assert_eq!(out[3]["image_interpretation"]["status"], "no_image");
        assert_eq!(out[3]["image_interpretation"]["content"]["text"], "");
    }
    #[test]
    fn corrupted_day_file_and_traversal_fail_closed() {
        let tmp = Temp::new();
        let ev = logs(&tmp.0);
        run(
            &tmp.0,
            &ev,
            &ImageSettings::default(),
            false,
            0,
            |_, _| panic!(),
        );
        fs::write(file_path(&tmp.0, DATE).unwrap(), "{").unwrap();
        assert!(load(&tmp.0, Path::new(""), DATE).is_err());
        assert!(file_path(&tmp.0, "../2026-11-18").is_err());
        assert!(source_key(&tmp.0, Path::new(""), DATE, "../secret.png").is_err());
    }
}
