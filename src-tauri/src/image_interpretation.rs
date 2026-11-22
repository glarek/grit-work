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
            Mode::Ocr => "windows-ocr:v3".into(),
            Mode::Ai => format!("ai:v2:{}:{}", self.provider, self.model),
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
            || self.regions.iter().any(|region| {
                region.width == 0
                    || region.height == 0
                    || region.x.checked_add(region.width).is_none()
                    || region.y.checked_add(region.height).is_none()
            })
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
/// How much text was observed. The character count is not an accuracy or quality score.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionVolume {
    #[default]
    Unmeasured,
    NoText,
    Sparse,
    TextDetected,
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
    /// Pixel size of the file that was interpreted. Absent on older cache rows.
    #[serde(default)]
    pub source_width: Option<u32>,
    #[serde(default)]
    pub source_height: Option<u32>,
    #[serde(default)]
    pub extraction: ExtractionVolume,
    /// Set for a finished read with no text or only a short sample. Not a technical failure.
    #[serde(default)]
    pub quality_warning: Option<String>,
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
    /// Completed rows with any text, summary, observation, or region text.
    pub with_content: usize,
    pub no_text: usize,
    pub sparse: usize,
    pub running: bool,
    pub last_error: Option<String>,
    pub mode: Mode,
    pub next_run_at: Option<i64>,
    pub requires_consent: bool,
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
    let mut day = day;
    for entry in &mut day.entries {
        entry.content.validate()?;
        // Volume is derived metadata. Recompute it with the current Unicode rules
        // without rewriting the stored evidence or changing failure/retry states.
        if entry.status == EntryStatus::Completed {
            apply_completed_quality(entry);
        }
    }
    Ok(Some(day))
}

/// Volume only. A short identifier is still content. Region text is counted only when
/// `text` is empty, because a normal OCR line already includes its words.
fn extraction_volume(content: &ImageContent) -> ExtractionVolume {
    // Measure visible content without mutating the original evidence. OCR and AI
    // can return only BOM, direction marks or terminal controls on an empty image.
    let visible_chars =
        |text: &str| {
            text.chars().filter(|ch| {
        !ch.is_whitespace() && !ch.is_control()
            && !matches!(ch, '\u{00ad}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}'
                | '\u{e0100}'..='\u{e01ef}')
    }).count()
        };
    let text_chars = visible_chars(&content.text);
    let region_chars = if text_chars == 0 {
        content
            .regions
            .iter()
            .map(|region| visible_chars(&region.text))
            .sum()
    } else {
        0
    };
    let summary_chars = visible_chars(&content.summary)
        + content
            .observations
            .iter()
            .map(|item| visible_chars(item))
            .sum::<usize>();
    let observed = text_chars + region_chars + summary_chars;
    if observed == 0 {
        ExtractionVolume::NoText
    } else if observed <= 40 {
        ExtractionVolume::Sparse
    } else {
        ExtractionVolume::TextDetected
    }
}

fn quality_warning_for(volume: ExtractionVolume) -> Option<String> {
    match volume {
        ExtractionVolume::NoText => {
            Some("Ingen text lästes. Bilden kan vara tom eller bara innehålla grafik.".into())
        }
        ExtractionVolume::Sparse => Some(
            "Bara en kort text lästes. Teckenlängd säger inte om tolkningen är användbar.".into(),
        ),
        ExtractionVolume::TextDetected | ExtractionVolume::Unmeasured => None,
    }
}

fn apply_completed_quality(entry: &mut ImageEntry) {
    if entry.status != EntryStatus::Completed {
        return;
    }
    entry.extraction = extraction_volume(&entry.content);
    entry.quality_warning = quality_warning_for(entry.extraction);
}

fn note_source_size(entry: &mut ImageEntry, path: &Path) {
    if entry.source_width.is_some() && entry.source_height.is_some() {
        return;
    }
    if let Ok((width, height)) = image::image_dimensions(path) {
        entry.source_width = Some(width);
        entry.source_height = Some(height);
    }
}

pub fn entry_has_content(entry: &ImageEntry) -> bool {
    entry.status == EntryStatus::Completed
        && matches!(
            entry.extraction,
            ExtractionVolume::Sparse | ExtractionVolume::TextDetected
        )
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
            source_width: cached.and_then(|e| e.source_width),
            source_height: cached.and_then(|e| e.source_height),
            extraction: ExtractionVolume::Unmeasured,
            quality_warning: None,
        };
        note_source_size(&mut entry, &source.path);
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
        apply_completed_quality(&mut entry);
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
                day.entries[index].image_sha256 = Some(hash);
                day.entries[index].source_stamp = Some(source_stamp);
                day.entries[index].content = content;
                day.entries[index].status = EntryStatus::Completed;
                day.entries[index].error = None;
                apply_completed_quality(&mut day.entries[index]);
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
        with_content: 0,
        no_text: 0,
        sparse: 0,
        running,
        last_error: None,
        mode: day
            .map(|d| d.configuration.mode.clone())
            .unwrap_or_default(),
        next_run_at: None,
        requires_consent: false,
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
            match entry.extraction {
                ExtractionVolume::NoText => result.no_text += 1,
                ExtractionVolume::Sparse => {
                    result.sparse += 1;
                    if entry_has_content(entry) {
                        result.with_content += 1;
                    }
                }
                ExtractionVolume::TextDetected if entry_has_content(entry) => {
                    result.with_content += 1;
                }
                _ => {}
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
    allow_incomplete: bool,
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
        let incomplete = entry.is_some_and(|e| {
            matches!(
                e.status,
                EntryStatus::Pending
                    | EntryStatus::Processing
                    | EntryStatus::Failed
                    | EntryStatus::AwaitingConsent
            )
        });
        if incomplete && !allow_incomplete {
            return Err("Bildtolkningen är inte färdig. Tolka bilderna eller välj Av under Insamling & lagring.".into());
        }
        let payload = entry.map(|e| {
            // Word boxes remain in the durable file for inspection. Sending the
            // same text again inside thousands of regions wastes report context.
            // Unfinished rows keep their status and quality labels, never cached text.
            let mut content = if incomplete {
                ImageContent::default()
            } else {
                e.content.clone()
            };
            content.regions.clear();
            json!({
                "image_id": e.image_id,
                "captured_at": e.captured_at,
                "method": e.method,
                "status": e.status,
                "content": content,
                "source_width": e.source_width,
                "source_height": e.source_height,
                "extraction": e.extraction,
                "quality_warning": e.quality_warning,
                "with_content": !incomplete && entry_has_content(e),
            })
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
        let out = report_evidence(&tmp.0, Path::new(""), DATE, &ev, &day, false).unwrap();
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
            report_evidence(&tmp.0, &archive.0, DATE, &archived_ev, &second, false)
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
        assert!(report_evidence(&tmp.0, Path::new(""), DATE, &ev, &day, false).is_err());
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
        let blank = run(&tmp.0, &ev, &settings, true, usize::MAX, |_, _| {
            Ok(ImageContent::default())
        });
        assert!(blank.entries.iter().all(|e| {
            e.status == EntryStatus::Completed
                && e.extraction == ExtractionVolume::NoText
                && e.error.is_none()
                && e.quality_warning.is_some()
                && !entry_has_content(e)
        }));
        assert!(report_evidence(&tmp.0, Path::new(""), DATE, &ev, &blank, false).is_ok());
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
        let out = report_evidence(&tmp.0, Path::new(""), DATE, &ev, &day, false).unwrap();
        assert_eq!(out[3]["image_interpretation"]["status"], "no_image");
        assert_eq!(out[3]["image_interpretation"]["content"]["text"], "");
        assert_eq!(out[0]["image_interpretation"]["extraction"], "sparse");
        assert_eq!(out[0]["image_interpretation"]["with_content"], true);
        assert!(out[0]["image_interpretation"]["quality_warning"].is_string());
    }
    #[test]
    fn incomplete_draft_keeps_completed_text_and_drops_stale_rows() {
        let tmp = Temp::new();
        let specs = [
            ("done.png", EntryStatus::Completed, "KEEP-ID", true),
            ("wait.png", EntryStatus::Pending, "STALE-PENDING", false),
            (
                "work.png",
                EntryStatus::Processing,
                "STALE-PROCESSING",
                false,
            ),
            ("bad.png", EntryStatus::Failed, "STALE-FAILED", false),
            (
                "ask.png",
                EntryStatus::AwaitingConsent,
                "STALE-CONSENT",
                false,
            ),
        ];
        let mut entries = Vec::new();
        let mut rows = Vec::new();
        for (index, (file, status, stale, keep)) in specs.iter().enumerate() {
            let raw = format!("screenshots/{file}");
            let (key, _) = source_key(&tmp.0, Path::new(""), DATE, &raw).unwrap();
            let image_id = format!("{:x}", Sha256::digest(key.as_bytes()));
            entries.push(ImageEntry {
                image_id,
                source_file: raw.clone(),
                captured_at: index as i64,
                evidence_ids: vec![index as i64 + 1],
                method: Mode::Ocr,
                status: status.clone(),
                fingerprint: "windows-ocr:v2".into(),
                image_sha256: None,
                source_stamp: None,
                processed_at: Some(1),
                content: ImageContent {
                    text: (*stale).into(),
                    regions: vec![crate::image_ocr::OcrRegion {
                        text: "REGION-LEAK".into(),
                        x: 1,
                        y: 2,
                        width: 3,
                        height: 4,
                    }],
                    ..ImageContent::default()
                },
                error: None,
                source_width: Some(10),
                source_height: Some(20),
                extraction: ExtractionVolume::TextDetected,
                quality_warning: Some("kort".into()),
            });
            rows.push(json!({
                "id": index + 1,
                "screenshot_path": raw,
                "ocr_text": "RAW-OCR-LEAK",
                "executable_path": "/secret/bin",
                "keep": keep,
            }));
            let _ = keep;
        }
        let day = DayFile {
            schema_version: 1,
            date: DATE.into(),
            updated_at: 1,
            configuration: ImageSettings {
                mode: Mode::Ocr,
                ..ImageSettings::default()
            },
            entries,
        };
        let ev = Value::Array(rows);
        assert!(report_evidence(&tmp.0, Path::new(""), DATE, &ev, &day, false).is_err());
        let out = report_evidence(&tmp.0, Path::new(""), DATE, &ev, &day, true).unwrap();
        let rendered = out.to_string();
        assert!(!rendered.contains("STALE-"));
        assert!(!rendered.contains("REGION-LEAK"));
        assert!(!rendered.contains("RAW-OCR-LEAK"));
        assert!(!rendered.contains("screenshot_path"));
        assert!(!rendered.contains("ocr_text"));
        assert!(!rendered.contains("executable_path"));
        assert!(!rendered.contains("/secret/bin"));
        assert_eq!(out[0]["image_interpretation"]["status"], "completed");
        assert_eq!(out[0]["image_interpretation"]["content"]["text"], "KEEP-ID");
        assert!(out[0]["image_interpretation"]["content"]["regions"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(out[0]["image_interpretation"]["with_content"], true);
        for index in 1..5 {
            let item = &out[index]["image_interpretation"];
            assert_eq!(item["content"], json!(ImageContent::default()));
            assert_eq!(item["with_content"], false);
            assert_eq!(item["extraction"], "text_detected");
            assert_eq!(item["quality_warning"], "kort");
            assert_eq!(item["source_width"], 10);
        }
        assert_eq!(out[1]["image_interpretation"]["status"], "pending");
        assert_eq!(out[2]["image_interpretation"]["status"], "processing");
        assert_eq!(out[3]["image_interpretation"]["status"], "failed");
        assert_eq!(out[4]["image_interpretation"]["status"], "awaiting_consent");
    }

    #[test]
    fn old_completed_empty_ocr_stays_history_and_is_not_usable() {
        let tmp = Temp::new();
        let legacy = json!({
            "schema_version": 1,
            "date": DATE,
            "updated_at": 1,
            "configuration": {"mode":"ocr","interval_seconds":300,"provider":"codex","model":"gpt-6.2-sol"},
            "entries": [{
                "image_id": "abc",
                "source_file": "screenshots/0.png",
                "captured_at": 1,
                "evidence_ids": [1],
                "method": "ocr",
                "status": "completed",
                "fingerprint": "windows-ocr:v1",
                "image_sha256": null,
                "source_stamp": null,
                "processed_at": 1,
                "content": {"text":"","summary":"","observations":[],"regions":[],"language":"sv-SE","confidence":null},
                "error": null
            }]
        });
        fs::create_dir_all(tmp.0.join("image-interpretations")).unwrap();
        fs::write(
            file_path(&tmp.0, DATE).unwrap(),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();
        let day = load(&tmp.0, Path::new(""), DATE).unwrap().unwrap();
        assert_eq!(day.entries[0].status, EntryStatus::Completed);
        assert_eq!(day.entries[0].content.language.as_deref(), Some("sv-SE"));
        assert_eq!(day.entries[0].extraction, ExtractionVolume::NoText);
        assert!(!entry_has_content(&day.entries[0]));
        assert!(day.entries[0].quality_warning.is_some());
        let summary = status(&tmp.0, DATE, Some(&day), false).unwrap();
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.with_content, 0);
        assert_eq!(summary.no_text, 1);
        let short = extraction_volume(&ImageContent {
            text: "Faktura".into(),
            regions: vec![crate::image_ocr::OcrRegion {
                text: "Faktura".into(),
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }],
            ..ImageContent::default()
        });
        assert_eq!(short, ExtractionVolume::Sparse);
        let diagram = extraction_volume(&ImageContent {
            summary: "Flöde utan bokstäver i bilden.".into(),
            ..ImageContent::default()
        });
        assert_eq!(diagram, ExtractionVolume::Sparse);
        let long = extraction_volume(&ImageContent {
            text: "A".repeat(41),
            ..ImageContent::default()
        });
        assert_eq!(long, ExtractionVolume::TextDetected);
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

#[cfg(test)]
mod text_robustness_tests {
    use super::*;

    fn corpus() -> Vec<Value> {
        serde_json::from_str(include_str!("../../tests/fixtures/ocr-text-corpus.json")).unwrap()
    }

    /// Read-only review of supplied day files. Keep private OCR outside fixtures,
    /// logs and provider prompts; print only counts and a source checksum.
    #[test]
    #[ignore = "Set GRIT_OCR_REVIEW_FILES to a platform-separated list of local day JSON files"]
    fn supplied_day_files_are_lossless_report_evidence() {
        let paths = std::env::var_os("GRIT_OCR_REVIEW_FILES").expect("Set GRIT_OCR_REVIEW_FILES");
        let mut summaries = Vec::new();
        for path in std::env::split_paths(&paths) {
            let date = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("Missing date");
            let root = path.parent().expect("Missing parent");
            file_path(root, date).expect("Invalid date");
            let before = fs::read(&path).expect("Cannot read supplied file");
            let original: DayFile = serde_json::from_slice(&before)
                .unwrap_or_else(|_| panic!("Day file has an invalid schema"));
            let loaded = read_file(&path, date)
                .expect("Production reader rejected day file")
                .unwrap();
            loaded
                .configuration
                .validate()
                .expect("Invalid day configuration");
            assert_eq!(original.entries.len(), loaded.entries.len());
            let encoded = serde_json::to_vec(&loaded).unwrap();
            let decoded: DayFile = serde_json::from_slice(&encoded).unwrap();
            assert!(decoded == loaded, "JSON round-trip changed day evidence");

            // The uploaded cache does not contain activity durations or pixels.
            // Zero-duration probe rows exercise evidence attachment and batching
            // without inventing work time or attempting to re-OCR absent images.
            let rows: Vec<Value> = loaded.entries.iter().enumerate().map(|(i, entry)| {
                json!({"id":i,"timestamp":entry.captured_at,"duration":0,"screenshot_path":entry.source_file})
            }).collect();
            let payload = report_evidence(root, Path::new(""), date, &json!(rows), &loaded, false)
                .expect("Production report evidence rejected day file");
            let payload_rows = payload.as_array().unwrap();
            for (i, entry) in loaded.entries.iter().enumerate() {
                assert!(
                    entry.content == original.entries[i].content,
                    "Reader changed OCR content"
                );
                assert!(
                    entry.status == original.entries[i].status,
                    "Reader changed retry status"
                );
                let report_content = &payload_rows[i]["image_interpretation"]["content"];
                assert!(
                    payload_rows[i]["image_interpretation"]["image_id"] == entry.image_id,
                    "Source image identity did not match"
                );
                assert!(
                    report_content["text"] == entry.content.text,
                    "Report lost OCR text"
                );
                assert!(
                    report_content["summary"] == entry.content.summary,
                    "Report lost summary"
                );
                assert!(
                    report_content["observations"] == json!(entry.content.observations),
                    "Report lost observations"
                );
                assert_eq!(report_content["regions"].as_array().unwrap().len(), 0);
            }
            let batches = crate::grit_cli::build_batches(payload_rows)
                .expect("Report batching rejected day file");
            let mut seen = std::collections::HashSet::new();
            for batch in &batches {
                assert!(
                    batch
                        .iter()
                        .map(|row| serde_json::to_vec(row).unwrap().len())
                        .sum::<usize>()
                        <= 120_000
                );
                for row in batch {
                    let i = row["id"].as_u64().unwrap() as usize;
                    assert!(seen.insert(i), "Report duplicated a probe row");
                    assert!(*row == payload_rows[i], "Batching changed report evidence");
                }
            }
            assert_eq!(seen.len(), loaded.entries.len());
            assert!(
                fs::read(&path).unwrap() == before,
                "Review must never rewrite supplied files"
            );
            let state = status(root, date, Some(&loaded), false).unwrap();
            let languages: std::collections::BTreeSet<_> = loaded
                .entries
                .iter()
                .filter_map(|e| e.content.language.clone())
                .collect();
            let summary = json!({
                "date":date,"bytes":before.len(),"sha256":format!("{:x}",Sha256::digest(&before)),
                "entries":loaded.entries.len(),"completed":state.completed,"skipped":state.skipped,
                "with_content":state.with_content,"failed":state.failed,"languages":languages,
                "text_characters":loaded.entries.iter().map(|e|e.content.text.chars().count()).sum::<usize>(),
                "regions":loaded.entries.iter().map(|e|e.content.regions.len()).sum::<usize>(),
                "report_batches":batches.len(),"source_unchanged":true,"round_trip_exact":true,
                "report_text_exact":true,"recognition_retested":false
            });
            println!("{summary}");
            summaries.push(summary);
        }
        assert!(!summaries.is_empty(), "No supplied files were reviewed");
        if let Some(path) = std::env::var_os("GRIT_OCR_REVIEW_REPORT") {
            fs::write(path, serde_json::to_vec_pretty(&summaries).unwrap()).unwrap();
        }
    }

    #[test]
    fn unicode_corpus_survives_processing_storage_archive_and_report_evidence() {
        let root = std::env::temp_dir().join(format!("grit-ocr-corpus-{}", uuid::Uuid::new_v4()));
        let archive = root.with_extension("archive");
        fs::create_dir_all(&archive).unwrap();
        fs::create_dir_all(root.join("screenshots")).unwrap();
        let date = "2026-11-18";
        let path = "screenshots/Årsrapport – Öresund ①.png";
        fs::write(root.join(path), b"synthetic image, no private data").unwrap();
        let text = corpus()
            .iter()
            .map(|case| case["text"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let content = ImageContent {
            text: text.clone(),
            summary: "ÅÄÖ – 🧑‍💻".into(),
            observations: vec![text.clone()],
            regions: vec![OcrRegion {
                text: text.clone(),
                x: 1,
                y: 2,
                width: 30,
                height: 40,
            }],
            language: Some("sv-SE".into()),
            confidence: None,
        };
        let activities = json!([{"id":1,"timestamp":1000,"duration":300,"app_name":"Editor","window_title":text,"screenshot_path":path,"ocr_text":"obsolete"}]);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        let day = process_day(
            &root,
            Path::new(""),
            date,
            &activities,
            &settings,
            true,
            false,
            1,
            &Arc::new(AtomicBool::new(false)),
            |_, _| Ok(content.clone()),
        )
        .unwrap();
        assert_eq!(day.entries[0].status, EntryStatus::Completed);
        let loaded = load(&root, Path::new(""), date).unwrap().unwrap();
        assert_eq!(loaded.entries[0].content, content);
        let evidence =
            report_evidence(&root, Path::new(""), date, &activities, &loaded, false).unwrap();
        assert_eq!(
            evidence[0]["image_interpretation"]["content"]["text"],
            content.text
        );
        assert!(evidence[0].get("ocr_text").is_none());
        assert_eq!(
            evidence[0]["image_interpretation"]["content"]["regions"],
            json!([])
        );
        crate::grit_engine::archive_day(&root, &archive, date, &activities).unwrap();
        crate::grit_engine::clean_archived_day(&root, &archive, date, &activities).unwrap();
        // Synthetic evidence is deleted only after archive verification by production code.
        fs::remove_file(file_path(&root, date).unwrap()).unwrap();
        let recovered = load(&root, &archive, date).unwrap().unwrap();
        assert_eq!(recovered.entries[0].content, content);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(archive).unwrap();
    }

    #[test]
    fn text_limits_count_unicode_scalars_without_slicing_utf8() {
        for token in ["å", "漢", "🧑", "\u{0000}"] {
            let mut content = ImageContent {
                text: token.repeat(60_000),
                ..ImageContent::default()
            };
            assert!(content.validate().is_ok());
            assert_eq!(
                serde_json::from_slice::<ImageContent>(&serde_json::to_vec(&content).unwrap())
                    .unwrap(),
                content
            );
            content.text.push('x');
            assert!(content.validate().is_err());
        }
        let mut content = ImageContent {
            summary: "ö".repeat(4000),
            ..ImageContent::default()
        };
        assert!(content.validate().is_ok());
        content.summary.push('ö');
        assert!(content.validate().is_err());
        for confidence in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.01] {
            assert!(ImageContent {
                confidence: Some(confidence),
                ..ImageContent::default()
            }
            .validate()
            .is_err());
        }
    }

    #[test]
    fn invisible_characters_are_not_reported_as_observed_text() {
        for text in [
            "",
            " \t\r\n\u{00a0}\u{202f}",
            "\u{feff}\u{200b}\u{200c}\u{200d}\u{2060}\u{202e}\u{202c}",
            "\0\u{0007}\u{001b}\u{007f}",
            "\u{fe0f}",
        ] {
            let content = ImageContent {
                text: text.into(),
                ..ImageContent::default()
            };
            assert_eq!(
                extraction_volume(&content),
                ExtractionVolume::NoText,
                "{text:?}"
            );
            assert_eq!(
                content.text, text,
                "Classification must not rewrite evidence"
            );
        }
        for text in ["Å", "±", "Ø12", "漢", "👷🏽‍♀️", "\u{feff}A-101"] {
            assert_eq!(
                extraction_volume(&ImageContent {
                    text: text.into(),
                    ..ImageContent::default()
                }),
                ExtractionVolume::Sparse
            );
        }
    }

    #[test]
    fn cached_invisible_only_text_is_reclassified_without_rewriting_evidence() {
        let root =
            std::env::temp_dir().join(format!("grit-ocr-reclassify-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("screenshots")).unwrap();
        fs::write(root.join("screenshots/image.png"), b"synthetic").unwrap();
        let activities = json!([{"id":1,"timestamp":1000,"duration":300,"screenshot_path":"screenshots/image.png"}]);
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        let mut day = process_day(
            &root,
            Path::new(""),
            "2026-11-18",
            &activities,
            &settings,
            true,
            false,
            1,
            &Arc::new(AtomicBool::new(false)),
            |_, _| {
                Ok(ImageContent {
                    text: "\u{feff}\u{200b}".into(),
                    ..ImageContent::default()
                })
            },
        )
        .unwrap();
        day.entries[0].extraction = ExtractionVolume::Sparse;
        persist(&root, &day).unwrap();
        let before = fs::read(file_path(&root, &day.date).unwrap()).unwrap();
        let loaded = load(&root, Path::new(""), &day.date).unwrap().unwrap();
        assert_eq!(loaded.entries[0].content, day.entries[0].content);
        assert_eq!(loaded.entries[0].extraction, ExtractionVolume::NoText);
        assert!(!entry_has_content(&loaded.entries[0]));
        assert_eq!(
            fs::read(file_path(&root, &day.date).unwrap()).unwrap(),
            before
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_regions_cannot_be_persisted_as_valid_content() {
        for (x, y, width, height) in [
            (0, 0, 0, 2),
            (0, 0, 2, 0),
            (u32::MAX, 0, 2, 2),
            (0, u32::MAX, 2, 2),
        ] {
            let content = ImageContent {
                regions: vec![OcrRegion {
                    text: "Å".into(),
                    x,
                    y,
                    width,
                    height,
                }],
                ..ImageContent::default()
            };
            assert!(
                content.validate().is_err(),
                "invalid region: {x},{y},{width},{height}"
            );
        }
    }

    #[test]
    fn invalid_json_and_overlong_content_fail_without_automatic_retry() {
        for text in [r#"{"text":"\ud800"}"#, "{\"text\":\"\0\"}", "{\"text\":"] {
            assert!(serde_json::from_str::<ImageContent>(text).is_err());
        }
        assert!(serde_json::from_slice::<ImageContent>(b"{\"text\":\"\xff\"}").is_err());
        let root = std::env::temp_dir().join(format!("grit-ocr-invalid-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("screenshots")).unwrap();
        fs::write(root.join("screenshots/image.png"), b"synthetic").unwrap();
        let settings = ImageSettings {
            mode: Mode::Ocr,
            ..ImageSettings::default()
        };
        let activities = json!([{"id":1,"timestamp":1000,"duration":300,"screenshot_path":"screenshots/image.png"}]);
        let cancel = Arc::new(AtomicBool::new(false));
        let day = process_day(
            &root,
            Path::new(""),
            "2026-11-18",
            &activities,
            &settings,
            true,
            false,
            1,
            &cancel,
            |_, _| {
                Ok(ImageContent {
                    text: "🧑".repeat(60_001),
                    ..ImageContent::default()
                })
            },
        )
        .unwrap();
        assert_eq!(day.entries[0].status, EntryStatus::Failed);
        assert_eq!(day.entries[0].content, ImageContent::default());
        process_day(
            &root,
            Path::new(""),
            "2026-11-18",
            &activities,
            &settings,
            true,
            false,
            1,
            &cancel,
            |_, _| panic!("Failure must require explicit retry"),
        )
        .unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
