//! Day archive for screenshots and activity JSON.
//! Stage under the archive root, read back every hash, then rename into place.

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

const ACTIVITIES_FILE: &str = "activities.json";
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourceEntry {
    source: String,
    archived: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ArchiveManifest {
    date: String,
    original_activities_sha256: String,
    files: BTreeMap<String, String>,
    sources: Vec<SourceEntry>,
}

/// Copy one past day's screenshots and rewritten activities into
/// `{archive_dir}/grit-work/{date}`. An existing matching archive is success.
pub fn archive_day(
    data_dir: &Path,
    archive_dir: &Path,
    date: &str,
    activities: &Value,
) -> Result<(), String> {
    let plan = prepare(data_dir, archive_dir, date, activities)?;
    if plan.day_dir.exists() {
        let manifest = verify_day(&plan.day_dir, date)?;
        if manifest.original_activities_sha256 != plan.original_hash {
            return Err("existing archive does not match original activities".into());
        }
        return Ok(());
    }
    for shot in &plan.shots {
        ensure_regular_file(&shot.source)?;
    }
    let guard = stage_archive(&plan)?;
    if plan.day_dir.exists() {
        drop(guard);
        let manifest = verify_day(&plan.day_dir, date)?;
        if manifest.original_activities_sha256 != plan.original_hash {
            return Err("existing archive does not match original activities".into());
        }
        return Ok(());
    }
    guard.disarm();
    if let Err(e) = fs::rename(&guard.path, &plan.day_dir) {
        guard.arm();
        return Err(format!("rename archive: {e}"));
    }
    let manifest = verify_day(&plan.day_dir, date)?;
    if manifest.original_activities_sha256 != plan.original_hash {
        return Err("archived activities hash mismatch after rename".into());
    }
    Ok(())
}

/// Verify the archive and return the rewritten activity array.
pub fn load_archive(archive_dir: &Path, date: &str) -> Result<Value, String> {
    parse_past_day(date)?;
    let archive_dir = real_dir(archive_dir)?;
    let day_dir = day_dir(&archive_dir, date);
    let manifest = verify_day(&day_dir, date)?;
    read_activities(&day_dir, &manifest)
}

/// Delete archived source images after the archive and original hash check out.
/// Returns how many files were removed. Missing files count as already cleaned.
pub fn clean_archived_day(
    data_dir: &Path,
    archive_dir: &Path,
    date: &str,
    activities: &Value,
) -> Result<usize, String> {
    let plan = prepare(data_dir, archive_dir, date, activities)?;
    let manifest = verify_day(&plan.day_dir, date)?;
    if manifest.original_activities_sha256 != plan.original_hash {
        return Err("activities do not match archived original".into());
    }
    let planned: BTreeMap<&str, &PlannedShot> = plan
        .shots
        .iter()
        .map(|shot| (shot.source_str.as_str(), shot))
        .collect();
    if manifest.sources.len() != planned.len() {
        return Err("archive source map does not match activities".into());
    }
    for entry in &manifest.sources {
        let shot = planned
            .get(entry.source.as_str())
            .ok_or_else(|| format!("archive source not in activities: {}", entry.source))?;
        if entry.archived != shot.archived_str {
            return Err(format!("archive source mismatch: {}", entry.source));
        }
        // A previous clean may already have removed the local file. Compare
        // bytes only when the source is still present; the manifest hash was
        // verified against the archived image.
        if let Some(present) = &shot.sha256 {
            if present != &entry.sha256 {
                return Err(format!(
                    "source image does not match archive: {}",
                    entry.source
                ));
            }
        }
    }
    let mut removed = 0;
    for shot in &plan.shots {
        let expected = manifest
            .sources
            .iter()
            .find(|entry| entry.source == shot.source_str)
            .map(|entry| entry.sha256.as_str())
            .ok_or_else(|| format!("missing source checksum: {}", shot.source_str))?;
        match fs::symlink_metadata(&shot.source) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("symlink prohibited: {}", shot.source.display()));
            }
            Ok(meta) if meta.is_file() => {
                let on_disk = sha256_file(&shot.source)?;
                if on_disk != expected {
                    return Err(format!(
                        "source image does not match archive: {}",
                        shot.source.display()
                    ));
                }
                fs::remove_file(&shot.source)
                    .map_err(|e| format!("remove {}: {e}", shot.source.display()))?;
                removed += 1;
            }
            Ok(_) => {
                return Err(format!("source is not a file: {}", shot.source.display()));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("stat {}: {e}", shot.source.display())),
        }
    }
    Ok(removed)
}

struct PlannedShot {
    source: PathBuf,
    source_str: String,
    archived_rel: PathBuf,
    archived_str: String,
    /// `None` when the local source is already gone. Never an empty stand-in hash.
    sha256: Option<String>,
}

struct ArchivePlan {
    day_dir: PathBuf,
    original_hash: String,
    rewritten: Value,
    shots: Vec<PlannedShot>,
}

struct StageDir {
    path: PathBuf,
    armed: std::cell::Cell<bool>,
}

impl StageDir {
    fn disarm(&self) {
        self.armed.set(false);
    }

    fn arm(&self) {
        self.armed.set(true);
    }
}

impl Drop for StageDir {
    fn drop(&mut self) {
        if self.armed.get() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn prepare(
    data_dir: &Path,
    archive_dir: &Path,
    date: &str,
    activities: &Value,
) -> Result<ArchivePlan, String> {
    parse_past_day(date)?;
    let data_dir = real_dir(data_dir)?;
    let archive_dir = real_dir(archive_dir)?;
    if overlaps(&data_dir, &archive_dir) {
        return Err("data_dir and archive_dir overlap".into());
    }
    let day_dir = day_dir(&archive_dir, date);
    if overlaps(&data_dir, &day_dir) {
        return Err("archive destination overlaps data_dir".into());
    }
    let original_hash = sha256_bytes(&serde_json::to_vec(activities).map_err(|e| e.to_string())?);
    let rows = activities
        .as_array()
        .ok_or("activities must be a JSON array")?;
    let mut rewritten_rows = Vec::with_capacity(rows.len());
    let mut shots = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        let mut obj = row
            .as_object()
            .cloned()
            .ok_or("each activity must be a JSON object")?;
        let raw = obj
            .get("screenshot_path")
            .ok_or("activity missing screenshot_path")?
            .as_str()
            .ok_or("screenshot_path must be a string")?
            .to_string();
        if !raw.is_empty() {
            let source = resolve_inside(&data_dir, Path::new(&raw))?;
            reject_symlink_chain(&source, true)?;
            if overlaps(&source, &archive_dir) || overlaps(&source, &day_dir) {
                return Err("screenshot path overlaps archive".into());
            }
            let source_str = path_str(&source)?;
            let index = if let Some(index) = seen.get(&source_str) {
                *index
            } else {
                let rel = source
                    .strip_prefix(&data_dir)
                    .map_err(|_| "screenshot escapes data_dir")?;
                let archived_rel = Path::new("shots").join(rel);
                let archived = day_dir.join(&archived_rel);
                let sha256 = hash_source_if_present(&source)?;
                let shot = PlannedShot {
                    source,
                    source_str: source_str.clone(),
                    archived_rel,
                    archived_str: path_str(&archived)?,
                    sha256,
                };
                let index = shots.len();
                shots.push(shot);
                seen.insert(source_str, index);
                index
            };
            obj.insert(
                "screenshot_path".into(),
                Value::String(shots[index].archived_str.clone()),
            );
        }
        rewritten_rows.push(Value::Object(obj));
    }
    Ok(ArchivePlan {
        day_dir,
        original_hash,
        rewritten: Value::Array(rewritten_rows),
        shots,
    })
}

fn hash_source_if_present(source: &Path) -> Result<Option<String>, String> {
    match fs::symlink_metadata(source) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("symlink prohibited: {}", source.display()))
        }
        Ok(meta) if meta.is_file() => Ok(Some(sha256_file(source)?)),
        Ok(_) => Err(format!(
            "screenshot is not a regular file: {}",
            source.display()
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("stat {}: {e}", source.display())),
    }
}

fn stage_archive(plan: &ArchivePlan) -> Result<StageDir, String> {
    for shot in &plan.shots {
        if shot.sha256.is_none() {
            return Err(format!(
                "missing source screenshot: {}",
                shot.source.display()
            ));
        }
    }
    let parent = plan
        .day_dir
        .parent()
        .ok_or("archive day path has no parent")?
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|e| format!("create archive root: {e}"))?;
    reject_symlink_chain(&parent, false)?;
    let staging = parent.join(format!(
        ".staging-{}-{}",
        plan.day_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("day"),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&staging).map_err(|e| format!("create staging: {e}"))?;
    let guard = StageDir {
        path: staging.clone(),
        armed: std::cell::Cell::new(true),
    };
    let mut files = BTreeMap::new();
    for shot in &plan.shots {
        let dest = staging.join(&shot.archived_rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create shot dir: {e}"))?;
        }
        fs::copy(&shot.source, &dest).map_err(|e| format!("copy screenshot: {e}"))?;
        let copied = sha256_file(&dest)?;
        let expected = shot
            .sha256
            .as_deref()
            .ok_or_else(|| format!("missing source screenshot: {}", shot.source.display()))?;
        if copied != expected {
            return Err(format!(
                "copied screenshot hash mismatch: {}",
                dest.display()
            ));
        }
        files.insert(rel_key(&shot.archived_rel), copied);
    }
    let activities_bytes = serde_json::to_vec_pretty(&plan.rewritten).map_err(|e| e.to_string())?;
    write_new(&staging.join(ACTIVITIES_FILE), &activities_bytes)?;
    files.insert(ACTIVITIES_FILE.into(), sha256_bytes(&activities_bytes));
    let mut sources = Vec::with_capacity(plan.shots.len());
    for shot in &plan.shots {
        let sha256 = shot
            .sha256
            .clone()
            .ok_or_else(|| format!("missing source screenshot: {}", shot.source.display()))?;
        sources.push(SourceEntry {
            source: shot.source_str.clone(),
            archived: shot.archived_str.clone(),
            sha256,
        });
    }
    let manifest = ArchiveManifest {
        date: plan
            .day_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string(),
        original_activities_sha256: plan.original_hash.clone(),
        files,
        sources,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    write_new(&staging.join(MANIFEST_FILE), &manifest_bytes)?;
    readback_stage(&staging, &manifest, &manifest_bytes, &plan.rewritten)?;
    Ok(guard)
}

fn readback_stage(
    staging: &Path,
    manifest: &ArchiveManifest,
    manifest_bytes: &[u8],
    rewritten: &Value,
) -> Result<(), String> {
    let reread =
        fs::read(staging.join(MANIFEST_FILE)).map_err(|e| format!("readback manifest: {e}"))?;
    if reread != manifest_bytes {
        return Err("manifest readback mismatch".into());
    }
    let parsed: ArchiveManifest = serde_json::from_slice(&reread).map_err(|e| e.to_string())?;
    if &parsed != manifest {
        return Err("manifest parse readback mismatch".into());
    }
    for (rel, expected) in &manifest.files {
        let path = staging.join(rel);
        let got = sha256_file(&path)?;
        if &got != expected {
            return Err(format!("readback hash mismatch: {rel}"));
        }
    }
    let activities =
        fs::read(staging.join(ACTIVITIES_FILE)).map_err(|e| format!("readback activities: {e}"))?;
    let value: Value = serde_json::from_slice(&activities).map_err(|e| e.to_string())?;
    if &value != rewritten {
        return Err("activities readback mismatch".into());
    }
    let on_disk = list_files(staging)?;
    let mut expected_names: BTreeSet<String> = manifest.files.keys().cloned().collect();
    expected_names.insert(MANIFEST_FILE.into());
    if on_disk != expected_names {
        return Err("staged files do not match manifest".into());
    }
    Ok(())
}

fn verify_day(day_dir: &Path, date: &str) -> Result<ArchiveManifest, String> {
    reject_symlink_chain(day_dir, false)?;
    let meta = fs::symlink_metadata(day_dir).map_err(|e| format!("open archive {date}: {e}"))?;
    if !meta.is_dir() {
        return Err("archive day is not a directory".into());
    }
    let manifest_bytes =
        fs::read(day_dir.join(MANIFEST_FILE)).map_err(|e| format!("read manifest: {e}"))?;
    let manifest: ArchiveManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("parse manifest: {e}"))?;
    if manifest.date != date {
        return Err("manifest date mismatch".into());
    }
    let on_disk = list_files(day_dir)?;
    let mut expected: BTreeSet<String> = manifest.files.keys().cloned().collect();
    expected.insert(MANIFEST_FILE.into());
    if on_disk != expected {
        return Err("archive contents do not match manifest".into());
    }
    for (rel, expected_hash) in &manifest.files {
        if rel == MANIFEST_FILE
            || rel
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
        {
            return Err(format!("illegal archived path: {rel}"));
        }
        let path = day_dir.join(rel);
        reject_symlink_chain(&path, false)?;
        if lexical_abs(&path)? != path_under(day_dir, rel)? {
            return Err(format!("archived path escapes day dir: {rel}"));
        }
        let got = sha256_file(&path)?;
        if &got != expected_hash {
            return Err(format!("archived file hash mismatch: {rel}"));
        }
    }
    let _ = read_activities(day_dir, &manifest)?;
    Ok(manifest)
}

fn read_activities(day_dir: &Path, manifest: &ArchiveManifest) -> Result<Value, String> {
    let expected = manifest
        .files
        .get(ACTIVITIES_FILE)
        .ok_or("manifest missing activities.json")?;
    let bytes =
        fs::read(day_dir.join(ACTIVITIES_FILE)).map_err(|e| format!("read activities: {e}"))?;
    if &sha256_bytes(&bytes) != expected {
        return Err("activities.json hash mismatch".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let rows = value
        .as_array()
        .ok_or("archived activities must be an array")?;
    let mut referenced = BTreeSet::new();
    for row in rows {
        let obj = row
            .as_object()
            .ok_or("archived activity must be an object")?;
        let path = obj
            .get("screenshot_path")
            .and_then(Value::as_str)
            .ok_or("archived screenshot_path must be a string")?;
        if path.is_empty() {
            continue;
        }
        let entry = manifest
            .sources
            .iter()
            .find(|s| s.archived == path)
            .ok_or_else(|| format!("archived path missing from source map: {path}"))?;
        let archived = PathBuf::from(&entry.archived);
        if lexical_abs(&archived)? != archived || !archived.starts_with(day_dir) {
            return Err("archived screenshot escapes archive day".into());
        }
        let rel = archived
            .strip_prefix(day_dir)
            .map_err(|_| "archived screenshot escapes archive day")?;
        let key = rel_key(rel);
        let hashed = manifest
            .files
            .get(&key)
            .ok_or_else(|| format!("no checksum for {key}"))?;
        if hashed != &entry.sha256 || sha256_file(&archived)? != entry.sha256 {
            return Err(format!("checksum mismatch for {key}"));
        }
        referenced.insert(entry.source.clone());
    }
    if referenced.len() != manifest.sources.len() {
        return Err("source map does not match archived activities".into());
    }
    Ok(value)
}

fn parse_past_day(date: &str) -> Result<NaiveDate, String> {
    if date.len() != 10 {
        return Err("date must be YYYY-MM-DD".into());
    }
    let bytes = date.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return Err("date must be YYYY-MM-DD".into());
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return Err("date must be YYYY-MM-DD".into());
    }
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| "invalid date")?;
    if parsed.format("%Y-%m-%d").to_string() != date {
        return Err("invalid date".into());
    }
    if parsed >= Local::now().date_naive() {
        return Err("date must be a past day".into());
    }
    Ok(parsed)
}

fn day_dir(archive_dir: &Path, date: &str) -> PathBuf {
    archive_dir.join("grit-work").join(date)
}

fn real_dir(path: &Path) -> Result<PathBuf, String> {
    let abs = lexical_abs(path)?;
    reject_symlink_chain(&abs, false)?;
    let meta = fs::symlink_metadata(&abs).map_err(|e| format!("{}: {e}", abs.display()))?;
    if !meta.is_dir() {
        return Err(format!("not a directory: {}", abs.display()));
    }
    Ok(abs)
}

fn resolve_inside(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let joined = if path.is_absolute() {
        lexical_abs(path)?
    } else {
        lexical_abs(&root.join(path))?
    };
    if joined == *root || !joined.starts_with(root) {
        return Err(format!("path escapes data_dir: {}", path.display()));
    }
    Ok(joined)
}

fn overlaps(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

fn lexical_abs(path: &Path) -> Result<PathBuf, String> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut out = PathBuf::new();
    for component in abs.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err(format!("path escapes root: {}", path.display()));
                }
            }
            Component::Normal(part) => out.push(part),
        }
    }
    if !out.is_absolute() {
        return Err(format!("path is not absolute: {}", path.display()));
    }
    Ok(out)
}

fn reject_symlink_chain(path: &Path, allow_missing_tail: bool) -> Result<(), String> {
    let abs = lexical_abs(path)?;
    let mut cursor = PathBuf::new();
    let components: Vec<Component> = abs.components().collect();
    for (index, component) in components.iter().enumerate() {
        cursor.push(component);
        if matches!(component, Component::RootDir | Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&cursor) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("symlink prohibited: {}", cursor.display()));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if allow_missing_tail || index + 1 == components.len() {
                    return Ok(());
                }
                return Err(format!("missing path: {}", cursor.display()));
            }
            Err(e) => return Err(format!("{}: {e}", cursor.display())),
        }
    }
    Ok(())
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    reject_symlink_chain(path, false)?;
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(format!(
            "screenshot is not a regular file: {}",
            path.display()
        ));
    }
    Ok(())
}

fn path_under(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let mut out = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!("illegal relative path: {rel}"));
        }
        out.push(part);
    }
    Ok(out)
}

fn rel_key(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn path_str(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("non-utf8 path: {}", path.display()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    file.write_all(bytes)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

fn list_files(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    walk_files(root, root, &mut out)?;
    Ok(out)
}

fn walk_files(root: &Path, dir: &Path, out: &mut BTreeSet<String>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("read dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("symlink prohibited: {}", path.display()));
        }
        if meta.is_dir() {
            walk_files(root, &path, out)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| format!("path escapes archive: {}", path.display()))?;
            out.insert(rel_key(rel));
        } else {
            return Err(format!("unsupported archive entry: {}", path.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scratch() -> PathBuf {
        let path = std::env::temp_dir().join(format!("grit-engine-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(path.join("data")).unwrap();
        fs::create_dir_all(path.join("archive")).unwrap();
        path
    }

    const DAY: &str = "2020-01-15";

    #[test]
    fn roundtrip_then_repeat_clean_is_idempotent() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::create_dir_all(data.join("shots")).unwrap();
        fs::write(data.join("shots/a.png"), b"png-a").unwrap();
        fs::write(data.join("shots/a.png.ocr.log"), b"sidecar").unwrap();
        let absolute = data.join("shots/b.png");
        fs::write(&absolute, b"png-b").unwrap();
        let activities = json!([
            {"id": 1, "screenshot_path": "shots/a.png", "ocr_text": "alpha", "app": "ed"},
            {"id": 2, "screenshot_path": absolute.to_str().unwrap(), "ocr_text": "beta"}
        ]);
        archive_day(&data, &archive, DAY, &activities).unwrap();
        archive_day(&data, &archive, DAY, &activities).unwrap();
        let loaded = load_archive(&archive, DAY).unwrap();
        assert_eq!(loaded[0]["ocr_text"], "alpha");
        assert_eq!(loaded[0]["app"], "ed");
        assert_eq!(loaded[1]["ocr_text"], "beta");
        let archived_a = loaded[0]["screenshot_path"].as_str().unwrap();
        assert!(archived_a.ends_with("/shots/shots/a.png") || archived_a.ends_with("/shots/a.png"));
        assert_eq!(fs::read(archived_a).unwrap(), b"png-a");
        assert!(Path::new(archived_a).starts_with(archive.join("grit-work").join(DAY)));
        assert_eq!(fs::read(data.join("shots/a.png")).unwrap(), b"png-a");
        assert_eq!(
            fs::read(data.join("shots/a.png.ocr.log")).unwrap(),
            b"sidecar"
        );
        assert_eq!(
            clean_archived_day(&data, &archive, DAY, &activities).unwrap(),
            2
        );
        assert!(!data.join("shots/a.png").exists());
        assert!(!absolute.exists());
        assert_eq!(
            fs::read(data.join("shots/a.png.ocr.log")).unwrap(),
            b"sidecar"
        );
        assert_eq!(fs::read(archived_a).unwrap(), b"png-a");
        archive_day(&data, &archive, DAY, &activities).unwrap();
        assert_eq!(
            clean_archived_day(&data, &archive, DAY, &activities).unwrap(),
            0
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn corruption_blocks_clean_and_preserves_source() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::write(data.join("pic.png"), b"good-bytes").unwrap();
        let activities = json!([{"screenshot_path": "pic.png", "ocr_text": "keep"}]);
        archive_day(&data, &archive, DAY, &activities).unwrap();
        let archived = load_archive(&archive, DAY).unwrap();
        let image = archived[0]["screenshot_path"].as_str().unwrap();
        fs::write(image, b"tampered!!").unwrap();
        let err = clean_archived_day(&data, &archive, DAY, &activities).unwrap_err();
        assert!(err.contains("hash") || err.contains("checksum") || err.contains("match"));
        assert_eq!(fs::read(data.join("pic.png")).unwrap(), b"good-bytes");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn traversal_and_missing_source_are_rejected() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::write(root.join("outside.png"), b"secret").unwrap();
        let escape = json!([{"screenshot_path": "../outside.png", "ocr_text": "no"}]);
        assert!(archive_day(&data, &archive, DAY, &escape).is_err());
        assert_eq!(fs::read(root.join("outside.png")).unwrap(), b"secret");
        assert!(!archive.join("grit-work").join(DAY).exists());
        let missing = json!([{"screenshot_path": "gone.png", "ocr_text": "x"}]);
        assert!(archive_day(&data, &archive, DAY, &missing).is_err());
        assert!(!archive.join("grit-work").join(DAY).exists());
        assert!(archive_day(&data, &archive, "2020-1-15", &missing).is_err());
        assert!(archive_day(&data, &data, DAY, &missing).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
