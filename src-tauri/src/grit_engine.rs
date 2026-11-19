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
        // Retry after a crash: flush the published tree before the caller deletes sources.
        sync_archive(&plan.day_dir)?;
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
        sync_archive(&plan.day_dir)?;
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
    sync_archive(&plan.day_dir)?;
    Ok(())
}

/// Delete one verified archive day. The day is renamed to `{date}.expired` first.
/// A retry finishes that tombstone only when every remaining file still matches
/// the manifest. Unknown files are left in place.
pub fn expire_archive(archive_dir: &Path, date: &str) -> Result<(), String> {
    parse_past_day(date)?;
    let archive_dir = real_dir(archive_dir)?;
    let root = archive_dir.join("grit-work");
    if !path_exists_dir(&root)? {
        return Ok(());
    }
    let root = real_dir(&root)?;
    let day_dir = root.join(date);
    let tombstone = root.join(format!("{date}.expired"));
    if lexical_abs(&day_dir)? != day_dir || lexical_abs(&tombstone)? != tombstone {
        return Err("archive day path is not canonical".into());
    }
    if !day_dir.starts_with(&root) || !tombstone.starts_with(&root) || day_dir == tombstone {
        return Err("archive day escapes archive root".into());
    }
    let day_exists = path_exists_dir(&day_dir)?;
    let tomb_exists = path_exists_dir(&tombstone)?;
    if day_exists && tomb_exists {
        return Err("archive expire is ambiguous: day and tombstone both exist".into());
    }
    if day_exists {
        verify_day(&day_dir, date)?;
        fs::rename(&day_dir, &tombstone)
            .map_err(|e| format!("rename archive to tombstone: {e}"))?;
        // Activities store original absolute day paths; tombstone cleanup
        // verifies the same manifest hashes without reinterpreting those paths.
    } else if !tomb_exists {
        return Ok(());
    }
    cleanup_tombstone(&tombstone, date)
}

fn path_exists_dir(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("symlink prohibited: {}", path.display()))
        }
        Ok(meta) if meta.is_dir() => Ok(true),
        Ok(_) => Err(format!(
            "archive day is not a directory: {}",
            path.display()
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("stat {}: {e}", path.display())),
    }
}

fn cleanup_tombstone(dir: &Path, date: &str) -> Result<(), String> {
    reject_symlink_chain(dir, false)?;
    let manifest_path = dir.join(MANIFEST_FILE);
    match fs::symlink_metadata(&manifest_path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("symlink prohibited: {}", manifest_path.display()))
        }
        Ok(meta) if meta.is_file() => cleanup_manifest_tombstone(dir, date, &manifest_path),
        Ok(_) => Err("tombstone manifest is not a file".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => cleanup_empty_tombstone(dir),
        Err(error) => Err(format!("stat tombstone manifest: {error}")),
    }
}

/// Payload files go first. `manifest.json` stays until they are gone so a crash can resume.
fn cleanup_manifest_tombstone(dir: &Path, date: &str, manifest_path: &Path) -> Result<(), String> {
    let manifest_bytes =
        fs::read(manifest_path).map_err(|e| format!("read tombstone manifest: {e}"))?;
    let manifest: ArchiveManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("parse tombstone manifest: {e}"))?;
    if manifest.date != date {
        return Err("tombstone manifest date mismatch".into());
    }
    let on_disk = list_files(dir)?;
    let mut allowed: BTreeSet<String> = manifest.files.keys().cloned().collect();
    allowed.insert(MANIFEST_FILE.into());
    if !on_disk.is_subset(&allowed) {
        return Err("tombstone contains files that are not in the manifest".into());
    }
    let mut payload = Vec::new();
    for (rel, expected_hash) in &manifest.files {
        if rel == MANIFEST_FILE
            || rel
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(format!("illegal archived path: {rel}"));
        }
        let path = dir.join(rel);
        if lexical_abs(&path)? != path_under(dir, rel)? {
            return Err(format!("archived path escapes tombstone: {rel}"));
        }
        if !path.exists() {
            continue;
        }
        reject_symlink_chain(&path, false)?;
        if sha256_file(&path)? != *expected_hash {
            return Err(format!("tombstone file hash mismatch: {rel}"));
        }
        payload.push(path);
    }
    for path in payload {
        fs::remove_file(&path)
            .map_err(|e| format!("remove archived file {}: {e}", path.display()))?;
    }
    fs::remove_file(manifest_path).map_err(|e| format!("remove tombstone manifest: {e}"))?;
    remove_empty_owned_dirs(dir)
}

/// Manifest already removed. Only empty directories may remain.
fn cleanup_empty_tombstone(dir: &Path) -> Result<(), String> {
    let on_disk = list_files(dir)?;
    if !on_disk.is_empty() {
        return Err("tombstone has files but no manifest".into());
    }
    remove_empty_owned_dirs(dir)
}

fn remove_empty_owned_dirs(root: &Path) -> Result<(), String> {
    let mut dirs = Vec::new();
    collect_real_dirs(root, &mut dirs)?;
    dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for dir in dirs {
        fs::remove_dir(&dir)
            .map_err(|e| format!("remove empty archive dir {}: {e}", dir.display()))?;
    }
    Ok(())
}

fn collect_real_dirs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    reject_symlink_chain(dir, false)?;
    out.push(dir.to_path_buf());
    for entry in fs::read_dir(dir).map_err(|e| format!("read dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("symlink prohibited: {}", path.display()));
        }
        if meta.is_dir() {
            collect_real_dirs(&path, out)?;
        }
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
    for entry in &manifest.sources {
        if let Some(shot) = planned.get(entry.source.as_str()) {
            if entry.archived != shot.archived_str {
                return Err(format!("archive source mismatch: {}", entry.source));
            }
            // The local interpretation cache may be newer than the sealed copy.
            // verify_day already hashed the archived file. Keep the local cache.
            if is_image_interpretation(
                &plan.data_dir,
                date,
                &entry.source,
                &plan.day_dir,
                &entry.archived,
            ) {
                continue;
            }
            // A previous clean may already have removed the local file. Compare
            // bytes only when the source is still present; the manifest hash was
            // verified against the archived copy.
            if let Some(present) = &shot.sha256 {
                if present != &entry.sha256 {
                    return Err(format!(
                        "source bytes do not match archive: {}",
                        entry.source
                    ));
                }
            }
            continue;
        }
        if is_image_interpretation(
            &plan.data_dir,
            date,
            &entry.source,
            &plan.day_dir,
            &entry.archived,
        ) {
            // The archived copy was hashed above. A newer local cache stays.
            continue;
        }
        if !is_daily_ocr(
            &plan.data_dir,
            date,
            &entry.source,
            &plan.day_dir,
            &entry.archived,
        ) {
            return Err(format!(
                "archive source is not a screenshot or daily companion: {}",
                entry.source
            ));
        }
    }
    for shot in &plan.shots {
        if manifest
            .sources
            .iter()
            .any(|entry| entry.source == shot.source_str)
        {
            continue;
        }
        // A companion that appeared after the archive was sealed is not a source.
        if is_daily_ocr(
            &plan.data_dir,
            date,
            &shot.source_str,
            &plan.day_dir,
            &shot.archived_str,
        ) || is_image_interpretation(
            &plan.data_dir,
            date,
            &shot.source_str,
            &plan.day_dir,
            &shot.archived_str,
        ) {
            continue;
        }
        return Err(format!(
            "local source missing from archive map: {}",
            shot.source_str
        ));
    }
    // Durability: the archive must be on disk before any local source is removed.
    sync_archive(&plan.day_dir)?;
    let mut removed = 0;
    for entry in &manifest.sources {
        let source = PathBuf::from(&entry.source);
        if is_image_interpretation(
            &plan.data_dir,
            date,
            &entry.source,
            &plan.day_dir,
            &entry.archived,
        ) {
            continue;
        }
        if resolve_inside(&plan.data_dir, &source)? != lexical_abs(&source)? {
            return Err(format!("source escapes data_dir: {}", entry.source));
        }
        match fs::symlink_metadata(&source) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("symlink prohibited: {}", source.display()));
            }
            Ok(meta) if meta.is_file() => {
                let on_disk = sha256_file(&source)?;
                if on_disk != entry.sha256 {
                    return Err(format!(
                        "source bytes do not match archive: {}",
                        source.display()
                    ));
                }
                fs::remove_file(&source)
                    .map_err(|e| format!("remove {}: {e}", source.display()))?;
                removed += 1;
            }
            Ok(_) => {
                return Err(format!("source is not a file: {}", source.display()));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("stat {}: {e}", source.display())),
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
    data_dir: PathBuf,
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
    // Optional raw daily OCR log. Missing is fine; a symlink is not.
    let ocr_rel = PathBuf::from("ocr_logs").join(format!("{date}.txt"));
    let ocr_source = data_dir.join(&ocr_rel);
    reject_symlink_chain(&ocr_source, true)?;
    if let Some(sha256) = hash_source_if_present(&ocr_source)? {
        if overlaps(&ocr_source, &archive_dir) || overlaps(&ocr_source, &day_dir) {
            return Err("ocr log overlaps archive".into());
        }
        let archived = day_dir.join(&ocr_rel);
        shots.push(PlannedShot {
            source: ocr_source.clone(),
            source_str: path_str(&ocr_source)?,
            archived_rel: ocr_rel,
            archived_str: path_str(&archived)?,
            sha256: Some(sha256),
        });
    }
    // Optional per-day interpretation cache. Other JSON files are not companions.
    let interpretation_rel = PathBuf::from("image-interpretations").join(format!("{date}.json"));
    let interpretation_source = data_dir.join(&interpretation_rel);
    reject_symlink_chain(&interpretation_source, true)?;
    if let Some(sha256) = hash_source_if_present(&interpretation_source)? {
        if overlaps(&interpretation_source, &archive_dir)
            || overlaps(&interpretation_source, &day_dir)
        {
            return Err("image interpretation overlaps archive".into());
        }
        let archived = day_dir.join(&interpretation_rel);
        shots.push(PlannedShot {
            source: interpretation_source.clone(),
            source_str: path_str(&interpretation_source)?,
            archived_rel: interpretation_rel,
            archived_str: path_str(&archived)?,
            sha256: Some(sha256),
        });
    }
    Ok(ArchivePlan {
        data_dir,
        day_dir,
        original_hash,
        rewritten: Value::Array(rewritten_rows),
        shots,
    })
}

fn is_daily_ocr(data_dir: &Path, date: &str, source: &str, day_dir: &Path, archived: &str) -> bool {
    same_companion(
        data_dir,
        day_dir,
        "ocr_logs",
        &format!("{date}.txt"),
        source,
        archived,
    )
}

fn is_image_interpretation(
    data_dir: &Path,
    date: &str,
    source: &str,
    day_dir: &Path,
    archived: &str,
) -> bool {
    same_companion(
        data_dir,
        day_dir,
        "image-interpretations",
        &format!("{date}.json"),
        source,
        archived,
    )
}

fn same_companion(
    data_dir: &Path,
    day_dir: &Path,
    directory: &str,
    file_name: &str,
    source: &str,
    archived: &str,
) -> bool {
    let expected_source = data_dir.join(directory).join(file_name);
    let expected_archived = day_dir.join(directory).join(file_name);
    Path::new(source) == expected_source && Path::new(archived) == expected_archived
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
        copy_durable(&shot.source, &dest)?;
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
    sync_tree(&staging)?;
    sync_dir(&parent)?;
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
    let ocr_rel = format!("ocr_logs/{}.txt", manifest.date);
    let interpretation_rel = format!("image-interpretations/{}.json", manifest.date);
    for entry in &manifest.sources {
        if referenced.contains(&entry.source) {
            continue;
        }
        let archived = PathBuf::from(&entry.archived);
        if lexical_abs(&archived)? != archived || !archived.starts_with(day_dir) {
            return Err("archived companion escapes archive day".into());
        }
        let rel = archived
            .strip_prefix(day_dir)
            .map_err(|_| "archived companion escapes archive day")?;
        let key = rel_key(rel);
        if key != ocr_rel && key != interpretation_rel {
            return Err("source map does not match archived activities".into());
        }
        let hashed = manifest
            .files
            .get(&key)
            .ok_or("no checksum for daily companion")?;
        if hashed != &entry.sha256 || sha256_file(&archived)? != entry.sha256 {
            return Err("checksum mismatch for daily companion".into());
        }
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
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))?;
    Ok(())
}

fn copy_durable(from: &Path, to: &Path) -> Result<(), String> {
    let bytes = fs::read(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    write_new(to, &bytes)
}

fn sync_file(path: &Path) -> Result<(), String> {
    // FlushFileBuffers on Windows requires a write-capable handle. Never ignore
    // flush errors: an archive must be durable before any local evidence is removed.
    let file = fs::OpenOptions::new()
        .read(true)
        .write(cfg!(windows))
        .open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))?;
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let dir = File::open(path).map_err(|e| format!("open dir {}: {e}", path.display()))?;
        dir.sync_all()
            .map_err(|e| format!("sync dir {}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn sync_tree(root: &Path) -> Result<(), String> {
    sync_dir(root)?;
    for entry in fs::read_dir(root).map_err(|e| format!("read dir {}: {e}", root.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("symlink prohibited: {}", path.display()));
        }
        if meta.is_dir() {
            sync_tree(&path)?;
        } else if meta.is_file() {
            sync_file(&path)?;
        }
    }
    Ok(())
}

fn sync_archive(day_dir: &Path) -> Result<(), String> {
    sync_tree(day_dir)?;
    if let Some(parent) = day_dir.parent() {
        sync_dir(parent)?;
    }
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
        assert!(
            Path::new(archived_a).ends_with("shots/shots/a.png")
                || Path::new(archived_a).ends_with("shots/a.png")
        );
        assert_eq!(fs::read(archived_a).unwrap(), b"png-a");
        assert!(Path::new(archived_a)
            .canonicalize()
            .unwrap()
            .starts_with(archive.join("grit-work").join(DAY).canonicalize().unwrap()));
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

    #[test]
    fn ocr_sidecar_roundtrip_and_corruption_preserves_source() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::write(data.join("pic.png"), b"img").unwrap();
        fs::create_dir_all(data.join("ocr_logs")).unwrap();
        let log = data.join("ocr_logs").join(format!("{DAY}.txt"));
        fs::write(&log, b"raw-ocr").unwrap();
        fs::write(data.join("pic.png.ocr.log"), b"leave-me").unwrap();
        let activities = json!([{"screenshot_path": "pic.png", "ocr_text": "from-db"}]);
        archive_day(&data, &archive, DAY, &activities).unwrap();
        let day = archive.join("grit-work").join(DAY);
        let archived_log = day.join("ocr_logs").join(format!("{DAY}.txt"));
        assert_eq!(fs::read(&archived_log).unwrap(), b"raw-ocr");
        assert_eq!(
            load_archive(&archive, DAY).unwrap()[0]["ocr_text"],
            "from-db"
        );
        assert_eq!(
            clean_archived_day(&data, &archive, DAY, &activities).unwrap(),
            2
        );
        assert!(!data.join("pic.png").exists());
        assert!(!log.exists());
        assert_eq!(fs::read(data.join("pic.png.ocr.log")).unwrap(), b"leave-me");
        assert_eq!(fs::read(&archived_log).unwrap(), b"raw-ocr");
        assert_eq!(
            clean_archived_day(&data, &archive, DAY, &activities).unwrap(),
            0
        );

        let root2 = scratch();
        let data2 = root2.join("data");
        let archive2 = root2.join("archive");
        fs::write(data2.join("pic.png"), b"img").unwrap();
        fs::create_dir_all(data2.join("ocr_logs")).unwrap();
        let log2 = data2.join("ocr_logs").join(format!("{DAY}.txt"));
        fs::write(&log2, b"raw-ocr").unwrap();
        archive_day(&data2, &archive2, DAY, &activities).unwrap();
        let archived2 = archive2
            .join("grit-work")
            .join(DAY)
            .join("ocr_logs")
            .join(format!("{DAY}.txt"));
        fs::write(&archived2, b"corrupt").unwrap();
        assert!(clean_archived_day(&data2, &archive2, DAY, &activities).is_err());
        assert_eq!(fs::read(&log2).unwrap(), b"raw-ocr");
        assert_eq!(fs::read(data2.join("pic.png")).unwrap(), b"img");

        let root3 = scratch();
        let data3 = root3.join("data");
        let archive3 = root3.join("archive");
        fs::write(data3.join("pic.png"), b"img").unwrap();
        archive_day(&data3, &archive3, DAY, &activities).unwrap();
        assert!(!archive3
            .join("grit-work")
            .join(DAY)
            .join("ocr_logs")
            .exists());
        assert_eq!(
            clean_archived_day(&data3, &archive3, DAY, &activities).unwrap(),
            1
        );

        #[cfg(unix)]
        {
            let root4 = scratch();
            let data4 = root4.join("data");
            let archive4 = root4.join("archive");
            fs::write(data4.join("pic.png"), b"img").unwrap();
            fs::create_dir_all(data4.join("ocr_logs")).unwrap();
            let real = data4.join("ocr_logs").join("real.txt");
            fs::write(&real, b"secret").unwrap();
            std::os::unix::fs::symlink(&real, data4.join("ocr_logs").join(format!("{DAY}.txt")))
                .unwrap();
            assert!(archive_day(&data4, &archive4, DAY, &activities).is_err());
            assert_eq!(fs::read(&real).unwrap(), b"secret");
            assert!(!archive4.join("grit-work").join(DAY).exists());
            let _ = fs::remove_dir_all(&root4);
        }
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&root2);
        let _ = fs::remove_dir_all(&root3);
    }

    #[test]
    fn existing_archive_retry_syncs_before_partial_sources_are_cleaned() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::create_dir_all(data.join("ocr_logs")).unwrap();
        fs::write(data.join("a.png"), b"aaa").unwrap();
        fs::write(data.join("b.png"), b"bbb").unwrap();
        fs::write(data.join("ocr_logs").join(format!("{DAY}.txt")), b"log").unwrap();
        let activities = json!([
            {"screenshot_path": "a.png", "ocr_text": "a"},
            {"screenshot_path": "b.png", "ocr_text": "b"}
        ]);
        archive_day(&data, &archive, DAY, &activities).unwrap();
        fs::remove_file(data.join("a.png")).unwrap();
        archive_day(&data, &archive, DAY, &activities).unwrap();
        assert_eq!(fs::read(data.join("b.png")).unwrap(), b"bbb");
        assert_eq!(
            fs::read(data.join("ocr_logs").join(format!("{DAY}.txt"))).unwrap(),
            b"log"
        );
        assert_eq!(
            clean_archived_day(&data, &archive, DAY, &activities).unwrap(),
            2
        );
        assert!(!data.join("b.png").exists());
        assert!(!data.join("ocr_logs").join(format!("{DAY}.txt")).exists());
        assert_eq!(
            fs::read(
                archive
                    .join("grit-work")
                    .join(DAY)
                    .join("shots")
                    .join("a.png")
            )
            .unwrap(),
            b"aaa"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn expire_removes_only_the_verified_day_and_retries_a_tombstone() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::write(data.join("a.png"), b"aaa").unwrap();
        let activities = json!([{"screenshot_path": "a.png", "ocr_text": "a"}]);
        archive_day(&data, &archive, DAY, &activities).unwrap();
        archive_day(&data, &archive, "2020-01-16", &activities).unwrap();
        let keep = archive.join("grit-work").join("2020-01-16");
        let day = archive.join("grit-work").join(DAY);
        fs::write(day.join("extra.bin"), b"keep-me").unwrap();
        assert!(expire_archive(&archive, DAY).is_err());
        assert!(day.join("extra.bin").is_file());
        assert!(day.join("activities.json").is_file());
        fs::remove_file(day.join("extra.bin")).unwrap();
        expire_archive(&archive, DAY).unwrap();
        expire_archive(&archive, DAY).unwrap();
        assert!(!day.exists());
        assert!(keep.join("activities.json").is_file());
        assert!(load_archive(&archive, "2020-01-16").is_ok());

        archive_day(&data, &archive, DAY, &activities).unwrap();
        let tomb = archive.join("grit-work").join(format!("{DAY}.expired"));
        fs::rename(&day, &tomb).unwrap();
        let shot = tomb.join("shots").join("a.png");
        if shot.is_file() {
            fs::remove_file(&shot).unwrap();
        }
        expire_archive(&archive, DAY).unwrap();
        assert!(!tomb.exists());
        assert!(keep.join("activities.json").is_file());

        archive_day(&data, &archive, DAY, &activities).unwrap();
        fs::rename(&day, &tomb).unwrap();
        fs::write(tomb.join("evil.bin"), b"no").unwrap();
        assert!(expire_archive(&archive, DAY).is_err());
        assert_eq!(fs::read(tomb.join("evil.bin")).unwrap(), b"no");
        assert!(tomb.join("activities.json").is_file());
        fs::create_dir(&day).unwrap();
        assert!(expire_archive(&archive, DAY)
            .unwrap_err()
            .contains("ambiguous"));
        assert!(tomb.join("evil.bin").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn expire_interruption_keeps_manifest_until_payload_is_gone() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::write(data.join("a.png"), b"aaa").unwrap();
        let activities = json!([{"screenshot_path": "a.png", "ocr_text": "a"}]);
        archive_day(&data, &archive, DAY, &activities).unwrap();
        let day = archive.join("grit-work").join(DAY);
        let tomb = archive.join("grit-work").join(format!("{DAY}.expired"));
        fs::rename(&day, &tomb).unwrap();
        for entry in fs::read_dir(&tomb).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() != MANIFEST_FILE {
                if path.is_dir() {
                    fs::remove_dir_all(&path).unwrap();
                } else {
                    fs::remove_file(&path).unwrap();
                }
            }
        }
        assert!(tomb.join(MANIFEST_FILE).is_file());
        expire_archive(&archive, DAY).unwrap();
        assert!(!tomb.exists());

        fs::create_dir_all(tomb.join("shots")).unwrap();
        expire_archive(&archive, DAY).unwrap();
        assert!(!tomb.exists());

        fs::create_dir_all(&tomb).unwrap();
        fs::write(tomb.join("orphan.bin"), b"left").unwrap();
        let error = expire_archive(&archive, DAY).unwrap_err();
        assert!(error.contains("no manifest"));
        assert_eq!(fs::read(tomb.join("orphan.bin")).unwrap(), b"left");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn image_interpretation_is_archived_and_the_local_cache_stays() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::write(data.join("pic.png"), b"img").unwrap();
        fs::create_dir_all(data.join("ocr_logs")).unwrap();
        fs::create_dir_all(data.join("image-interpretations")).unwrap();
        let log = data.join("ocr_logs").join(format!("{DAY}.txt"));
        fs::write(&log, b"raw-ocr").unwrap();
        let cache = data
            .join("image-interpretations")
            .join(format!("{DAY}.json"));
        fs::write(&cache, b"{\"images\":[]}").unwrap();
        let wrong = data.join("image-interpretations").join("2020-01-16.json");
        fs::write(&wrong, b"{\"other\":true}").unwrap();
        fs::write(data.join("image-interpretations").join("notes.json"), b"{}").unwrap();
        let activities = json!([{"screenshot_path": "pic.png", "ocr_text": "from-db"}]);
        archive_day(&data, &archive, DAY, &activities).unwrap();
        let day = archive.join("grit-work").join(DAY);
        let archived = day
            .join("image-interpretations")
            .join(format!("{DAY}.json"));
        assert_eq!(fs::read(&archived).unwrap(), b"{\"images\":[]}");
        assert!(!day
            .join("image-interpretations")
            .join("2020-01-16.json")
            .exists());
        assert!(!day
            .join("image-interpretations")
            .join("notes.json")
            .exists());
        assert!(load_archive(&archive, DAY).is_ok());
        fs::write(&cache, b"{\"images\":[\"newer\"]}").unwrap();
        assert_eq!(
            clean_archived_day(&data, &archive, DAY, &activities).unwrap(),
            2
        );
        assert!(!data.join("pic.png").exists());
        assert!(!log.exists());
        assert_eq!(fs::read(&cache).unwrap(), b"{\"images\":[\"newer\"]}");
        assert_eq!(fs::read(&wrong).unwrap(), b"{\"other\":true}");
        assert_eq!(fs::read(&archived).unwrap(), b"{\"images\":[]}");
        expire_archive(&archive, DAY).unwrap();
        assert!(!archived.exists());
        assert_eq!(fs::read(&cache).unwrap(), b"{\"images\":[\"newer\"]}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn tampered_archived_interpretation_blocks_cleanup_and_old_archives_load() {
        let root = scratch();
        let data = root.join("data");
        let archive = root.join("archive");
        fs::write(data.join("pic.png"), b"img").unwrap();
        fs::create_dir_all(data.join("image-interpretations")).unwrap();
        let cache = data
            .join("image-interpretations")
            .join(format!("{DAY}.json"));
        fs::write(&cache, b"{\"v\":1}").unwrap();
        let activities = json!([{"screenshot_path": "pic.png"}]);
        archive_day(&data, &archive, DAY, &activities).unwrap();
        let archived = archive
            .join("grit-work")
            .join(DAY)
            .join("image-interpretations")
            .join(format!("{DAY}.json"));
        fs::write(&archived, b"{\"v\":2}").unwrap();
        assert!(clean_archived_day(&data, &archive, DAY, &activities).is_err());
        assert_eq!(fs::read(data.join("pic.png")).unwrap(), b"img");
        assert_eq!(fs::read(&cache).unwrap(), b"{\"v\":1}");

        let root2 = scratch();
        let data2 = root2.join("data");
        let archive2 = root2.join("archive");
        fs::write(data2.join("pic.png"), b"img").unwrap();
        archive_day(&data2, &archive2, DAY, &activities).unwrap();
        assert!(load_archive(&archive2, DAY).is_ok());
        assert!(!archive2
            .join("grit-work")
            .join(DAY)
            .join("image-interpretations")
            .exists());
        fs::create_dir_all(data2.join("image-interpretations")).unwrap();
        let later = data2
            .join("image-interpretations")
            .join(format!("{DAY}.json"));
        fs::write(&later, b"{\"after\":true}").unwrap();
        assert_eq!(
            clean_archived_day(&data2, &archive2, DAY, &activities).unwrap(),
            1
        );
        assert!(!data2.join("pic.png").exists());
        assert_eq!(fs::read(&later).unwrap(), b"{\"after\":true}");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&root2);
    }
}
