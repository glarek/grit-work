//! Local OCR through Windows.Media.Ocr. Linux builds expose the same
//! `recognize` signature and the pure tiling helpers used by tests.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[cfg(windows)]
const MAX_FILE_BYTES: u64 = 40 * 1024 * 1024;
#[cfg(windows)]
const MAX_PIXELS: u64 = 40_000_000;
#[cfg(windows)]
const MAX_EDGE: u32 = 20_000;
#[cfg(windows)]
const MAX_TILES: u32 = 36;
#[cfg(windows)]
const TILE_WAIT: std::time::Duration = std::time::Duration::from_secs(8);
#[cfg(windows)]
const TOTAL_WAIT: std::time::Duration = std::time::Duration::from_secs(45);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OcrRegion {
    pub text: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OcrOutput {
    pub text: String,
    pub language: Option<String>,
    pub regions: Vec<OcrRegion>,
}

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OcrTile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[cfg(any(windows, test))]
pub(crate) struct WordBox {
    text: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

pub fn recognize(path: &Path) -> Result<OcrOutput, String> {
    recognize_platform(path, None)
}

/// Use a specific packaged recognizer, with no fallback to a different language.
/// Also used by the Windows integration harness to exercise both dependencies.
#[allow(dead_code)]
pub fn recognize_in_language(path: &Path, language: &str) -> Result<OcrOutput, String> {
    if !["sv-SE", "en-US"].contains(&language) {
        return Err("Välj OCR-språket sv-SE eller en-US.".into());
    }
    recognize_platform(path, Some(language))
}

#[cfg(not(windows))]
fn recognize_platform(_path: &Path, _language: Option<&str>) -> Result<OcrOutput, String> {
    Err("OCR via Windows.Media.Ocr stöds inte på den här plattformen.".to_string())
}

#[cfg(any(windows, test))]
pub(crate) fn plan_tiles(
    width: u32,
    height: u32,
    max_dimension: u32,
    max_tiles: u32,
) -> Result<Vec<OcrTile>, String> {
    if width == 0 || height == 0 {
        return Err("Bilden har ingen läsbar storlek.".to_string());
    }
    if max_dimension == 0 {
        return Err("Windows OCR rapporterade ingen giltig bildgräns.".to_string());
    }
    if max_tiles == 0 {
        return Err("Bilden är för stor för OCR.".to_string());
    }

    // Native OCR can cut a drawing identifier at a tile boundary. Keep a
    // shared strip at native resolution and merge words by their source boxes.
    let overlap = 256.min(max_dimension / 4);
    let step = max_dimension - overlap;
    let columns = width.saturating_sub(max_dimension).div_ceil(step) + 1;
    let rows = height.saturating_sub(max_dimension).div_ceil(step) + 1;
    let tile_count = columns.saturating_mul(rows);
    if tile_count > max_tiles {
        return Err("Bilden är för stor för OCR utan att förminska texten.".to_string());
    }

    let mut tiles = Vec::with_capacity(tile_count as usize);
    for row in 0..rows {
        for column in 0..columns {
            let x = column.saturating_mul(step);
            let y = row.saturating_mul(step);
            let width = max_dimension.min(width.saturating_sub(x));
            let height = max_dimension.min(height.saturating_sub(y));
            if width == 0 || height == 0 {
                return Err("Bilden kunde inte delas upp för OCR.".to_string());
            }
            tiles.push(OcrTile {
                x,
                y,
                width,
                height,
            });
        }
    }
    Ok(tiles)
}

#[cfg(any(windows, test))]
pub(crate) fn region_in_source(
    word: WordBox,
    tile: OcrTile,
    image_width: u32,
    image_height: u32,
) -> Option<OcrRegion> {
    if word.text.trim().is_empty() || word.width <= 0.0 || word.height <= 0.0 {
        return None;
    }

    let tile_right = tile.x.saturating_add(tile.width);
    let tile_bottom = tile.y.saturating_add(tile.height);
    let left = clamp_coord(word.x, tile.width).saturating_add(tile.x);
    let top = clamp_coord(word.y, tile.height).saturating_add(tile.y);
    let right = clamp_coord(word.x + word.width, tile.width)
        .saturating_add(tile.x)
        .min(tile_right)
        .min(image_width);
    let bottom = clamp_coord(word.y + word.height, tile.height)
        .saturating_add(tile.y)
        .min(tile_bottom)
        .min(image_height);
    let width = right.saturating_sub(left);
    let height = bottom.saturating_sub(top);
    if width == 0 || height == 0 {
        return None;
    }

    Some(OcrRegion {
        text: word.text,
        x: left,
        y: top,
        width,
        height,
    })
}

#[cfg(any(windows, test))]
fn clamp_coord(value: f32, limit: u32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let rounded = value.ceil() as u64;
    u32::try_from(rounded).unwrap_or(u32::MAX).min(limit)
}

#[cfg(windows)]
fn validate_decoded_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("Bilden har ingen läsbar storlek.".to_string());
    }
    if width > MAX_EDGE || height > MAX_EDGE {
        return Err("Bilden är för stor för OCR.".to_string());
    }
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels > MAX_PIXELS {
        return Err("Bilden är för stor för OCR.".to_string());
    }
    Ok(())
}

#[cfg(windows)]
fn recognize_platform(path: &Path, requested_language: Option<&str>) -> Result<OcrOutput, String> {
    use std::time::Instant;

    use image::imageops::crop_imm;
    use image::ImageReader;
    use windows::Media::Ocr::OcrEngine;

    let metadata =
        std::fs::metadata(path).map_err(|_| "Bildfilen kunde inte öppnas.".to_string())?;
    if !metadata.is_file() {
        return Err("Bildfilen kunde inte öppnas.".to_string());
    }
    if metadata.len() == 0 {
        return Err("Bildfilen är tom eller skadad.".to_string());
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err("Bildfilen är för stor.".to_string());
    }

    let reader = ImageReader::open(path).map_err(|_| "Bildfilen kunde inte läsas.".to_string())?;
    let mut reader = reader;
    reader.limits(decode_limits());
    let dimensions = reader
        .into_dimensions()
        .map_err(|_| "Bildfilen är skadad eller har ett format som inte kan läsas.".to_string())?;
    validate_decoded_size(dimensions.0, dimensions.1)?;

    let reader = ImageReader::open(path).map_err(|_| "Bildfilen kunde inte läsas.".to_string())?;
    let mut reader = reader;
    reader.limits(decode_limits());
    let image = reader
        .decode()
        .map_err(|_| "Bildfilen är skadad eller har ett format som inte kan läsas.".to_string())?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    validate_decoded_size(width, height)?;

    // windows-core 0.62 does not call RoInitialize. Its factory only calls
    // CoIncrementMTAUsage after CO_E_NOTINITIALIZED, and that does not join this
    // spawn_blocking thread to an apartment.
    let _apartment = enter_mta()?;
    let max_dimension = OcrEngine::MaxImageDimension()
        .map_err(|_| "Windows OCR kunde inte läsa motorns bildgräns.".to_string())?;
    let tiles = plan_tiles(width, height, max_dimension, MAX_TILES)?;
    let (engine, language) = select_engine(requested_language)?;
    let single_tile = tiles.len() == 1;
    let mut native_text = String::new();
    let mut regions = Vec::new();
    let started = Instant::now();

    for (tile_index, tile) in tiles.into_iter().enumerate() {
        if started.elapsed() >= TOTAL_WAIT {
            return Err("OCR avbröts eftersom den sammanlagda tidsgränsen överskreds.".to_string());
        }
        let view = crop_imm(&rgba, tile.x, tile.y, tile.width, tile.height).to_image();
        let gray = grayscale_on_white(view);
        let bitmap = gray_bitmap(gray.as_raw(), tile.width, tile.height)?;
        let operation = engine
            .RecognizeAsync(&bitmap)
            .map_err(|_| "Windows OCR kunde inte starta tolkningen.".to_string())?;
        let result = wait_ocr(
            operation,
            TILE_WAIT.min(TOTAL_WAIT.saturating_sub(started.elapsed())),
        )?;
        if single_tile {
            // Preserve the engine's spacing and reading order, including RTL, when
            // there are no overlapping tile results to reconcile.
            native_text = result
                .Text()
                .map_err(|_| "Windows OCR returnerade ingen text.".to_string())?
                .to_string();
        }
        let lines = result
            .Lines()
            .map_err(|_| "Windows OCR returnerade inga textrader.".to_string())?;
        let line_count = lines
            .Size()
            .map_err(|_| "Windows OCR returnerade inga textrader.".to_string())?;
        for line_index in 0..line_count {
            let line = lines
                .GetAt(line_index)
                .map_err(|_| "Windows OCR returnerade inga textrader.".to_string())?;
            let words = line
                .Words()
                .map_err(|_| "Windows OCR returnerade inga ord.".to_string())?;
            let word_count = words
                .Size()
                .map_err(|_| "Windows OCR returnerade inga ord.".to_string())?;
            for word_index in 0..word_count {
                let word = words
                    .GetAt(word_index)
                    .map_err(|_| "Windows OCR returnerade inga ord.".to_string())?;
                let word_text = word
                    .Text()
                    .map_err(|_| "Windows OCR returnerade ett oläsbart ord.".to_string())?
                    .to_string();
                let bounds = word
                    .BoundingRect()
                    .map_err(|_| "Windows OCR returnerade ingen ordposition.".to_string())?;
                if let Some(region) = region_in_source(
                    WordBox {
                        text: word_text,
                        x: bounds.X,
                        y: bounds.Y,
                        width: bounds.Width,
                        height: bounds.Height,
                    },
                    tile,
                    width,
                    height,
                ) {
                    regions.push((tile_index, region));
                    if regions.len() > 8000 {
                        return Err(
                            "Bilden innehåller för många textregioner för säker OCR-bearbetning."
                                .into(),
                        );
                    }
                }
            }
        }
    }

    let (text, regions) = if single_tile {
        (
            native_text,
            regions.into_iter().map(|(_, region)| region).collect(),
        )
    } else {
        merge_regions(regions)
    };
    Ok(OcrOutput {
        text,
        language: Some(language),
        regions,
    })
}

#[cfg(any(windows, test))]
fn merge_regions(mut regions: Vec<(usize, OcrRegion)>) -> (String, Vec<OcrRegion>) {
    // A clipped word is usually contained in the complete word read by the next
    // tile. Prefer the larger box. Equal text at different positions is retained.
    regions.sort_by_key(|(_, region)| {
        std::cmp::Reverse(u64::from(region.width) * u64::from(region.height))
    });
    let mut unique: Vec<(usize, OcrRegion)> = Vec::new();
    for (tile, region) in regions {
        let duplicate = unique.iter().any(|(existing_tile, existing)| {
            if *existing_tile == tile {
                return false;
            }
            let width = (existing
                .x
                .saturating_add(existing.width)
                .min(region.x.saturating_add(region.width)))
            .saturating_sub(existing.x.max(region.x));
            let height = (existing
                .y
                .saturating_add(existing.height)
                .min(region.y.saturating_add(region.height)))
            .saturating_sub(existing.y.max(region.y));
            let intersection = u64::from(width) * u64::from(height);
            let smaller = (u64::from(existing.width) * u64::from(existing.height))
                .min(u64::from(region.width) * u64::from(region.height));
            smaller > 0 && (intersection as f64 / smaller as f64) >= 0.7
        });
        if !duplicate {
            unique.push((tile, region));
        }
    }
    let mut unique: Vec<OcrRegion> = unique.into_iter().map(|(_, region)| region).collect();
    unique.sort_by_key(|region| (region.y, region.x));
    let mut lines: Vec<Vec<OcrRegion>> = Vec::new();
    for region in unique {
        if let Some(line) = lines.iter_mut().rev().find(|line| {
            let first = &line[0];
            let distance = (f64::from(first.y) + f64::from(first.height) / 2.0
                - f64::from(region.y)
                - f64::from(region.height) / 2.0)
                .abs();
            distance <= f64::from(first.height.min(region.height)) / 2.0
        }) {
            line.push(region);
        } else {
            lines.push(vec![region]);
        }
    }
    for line in &mut lines {
        line.sort_by_key(|region| region.x);
    }
    let text = lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|region| region.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, lines.into_iter().flatten().collect())
}

/// Dropping alpha turns transparent pixels black, erasing black text on transparent
/// screenshots. Composite against white before passing the pixels to Windows OCR.
#[cfg(any(windows, test))]
fn grayscale_on_white(mut image: image::RgbaImage) -> image::GrayImage {
    for pixel in image.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for channel in 0..3 {
            pixel[channel] =
                ((u32::from(pixel[channel]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }
    image::DynamicImage::ImageRgba8(image).to_luma8()
}

#[cfg(windows)]
fn select_engine(
    requested: Option<&str>,
) -> Result<(windows::Media::Ocr::OcrEngine, String), String> {
    use windows::Media::Ocr::OcrEngine;

    if let Some(tag) = requested {
        return engine_for_tag(tag)?.ok_or_else(|| {
            format!("OCR-språket {tag} saknas. Kör installationen av GRIT Work för att installera svenska och engelska.")
        });
    }
    for tag in ["sv-SE", "en-US"] {
        if let Some(selected) = engine_for_tag(tag)? {
            return Ok(selected);
        }
    }

    let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| {
        "OCR-språk saknas. Kör installationen av GRIT Work för att installera svenska och engelska.".to_string()
    })?;
    let language = recognizer_tag(&engine)?;
    Ok((engine, language))
}

#[cfg(windows)]
fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(MAX_PIXELS.saturating_mul(4));
    limits
}

#[cfg(windows)]
fn engine_for_tag(tag: &str) -> Result<Option<(windows::Media::Ocr::OcrEngine, String)>, String> {
    use windows::core::HSTRING;
    use windows::Globalization::Language;
    use windows::Media::Ocr::OcrEngine;

    let language = Language::CreateLanguage(&HSTRING::from(tag))
        .map_err(|_| "Windows OCR kunde inte läsa språktaggen.".to_string())?;
    let supported = OcrEngine::IsLanguageSupported(&language)
        .map_err(|_| "Windows OCR kunde inte kontrollera språk.".to_string())?;
    if !supported {
        return Ok(None);
    }
    match OcrEngine::TryCreateFromLanguage(&language) {
        Ok(engine) => {
            let tag = recognizer_tag(&engine)?;
            Ok(Some((engine, tag)))
        }
        Err(_) => Ok(None),
    }
}

#[cfg(windows)]
fn recognizer_tag(engine: &windows::Media::Ocr::OcrEngine) -> Result<String, String> {
    let language = engine
        .RecognizerLanguage()
        .map_err(|_| "Windows OCR rapporterade inget språk.".to_string())?;
    let tag = language
        .LanguageTag()
        .map_err(|_| "Windows OCR rapporterade inget språk.".to_string())?
        .to_string();
    if tag.is_empty() {
        return Err("Windows OCR rapporterade inget språk.".to_string());
    }
    Ok(tag)
}

#[cfg(windows)]
fn gray_bitmap(
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Result<windows::Graphics::Imaging::SoftwareBitmap, String> {
    use windows::core::Interface;
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Storage::Streams::Buffer;
    use windows::Win32::System::WinRT::IBufferByteAccess;

    let expected = usize::try_from(width)
        .ok()
        .and_then(|value| value.checked_mul(usize::try_from(height).ok()?))
        .ok_or_else(|| "Bilden är för stor för OCR.".to_string())?;
    if pixels.len() != expected {
        return Err("Bilden kunde inte förberedas för OCR.".to_string());
    }
    let capacity =
        u32::try_from(pixels.len()).map_err(|_| "Bilden är för stor för OCR.".to_string())?;
    let buffer = Buffer::Create(capacity)
        .map_err(|_| "Windows kunde inte skapa bildbufferten.".to_string())?;
    buffer
        .SetLength(capacity)
        .map_err(|_| "Windows kunde inte skapa bildbufferten.".to_string())?;
    let access = Interface::cast::<IBufferByteAccess>(&buffer)
        .map_err(|_| "Windows kunde inte skriva bildbufferten.".to_string())?;
    unsafe {
        let destination = access
            .Buffer()
            .map_err(|_| "Windows kunde inte skriva bildbufferten.".to_string())?;
        if destination.is_null() {
            return Err("Windows kunde inte skriva bildbufferten.".to_string());
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), destination, pixels.len());
    }
    let pixel_width =
        i32::try_from(width).map_err(|_| "Bilden är för stor för OCR.".to_string())?;
    let pixel_height =
        i32::try_from(height).map_err(|_| "Bilden är för stor för OCR.".to_string())?;
    SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Gray8,
        pixel_width,
        pixel_height,
    )
    .map_err(|_| "Windows kunde inte skapa OCR-bilden.".to_string())
}

/// `S_OK` and `S_FALSE` both increment the WinRT apartment count and must be paired
/// with `RoUninitialize`. `RPC_E_CHANGED_MODE` means this thread is already in another
/// apartment, so this call must not uninitialize it.
#[cfg(any(windows, test))]
fn mta_outcome(hr: i32) -> MtaOutcome {
    const RPC_E_CHANGED_MODE: i32 = 0x80010106_u32 as i32;
    if hr >= 0 {
        MtaOutcome::Acquired
    } else if hr == RPC_E_CHANGED_MODE {
        MtaOutcome::AlreadyDifferent
    } else {
        MtaOutcome::Failed
    }
}

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MtaOutcome {
    Acquired,
    AlreadyDifferent,
    Failed,
}

#[cfg(windows)]
struct MtaGuard {
    uninitialize: bool,
}

#[cfg(windows)]
impl Drop for MtaGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { windows::Win32::System::WinRT::RoUninitialize() };
        }
    }
}

#[cfg(windows)]
fn enter_mta() -> Result<MtaGuard, String> {
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    let outcome = match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        Ok(()) => MtaOutcome::Acquired,
        Err(error) => mta_outcome(error.code().0),
    };
    match outcome {
        MtaOutcome::Acquired => Ok(MtaGuard { uninitialize: true }),
        MtaOutcome::AlreadyDifferent => Ok(MtaGuard {
            uninitialize: false,
        }),
        MtaOutcome::Failed => Err("Windows OCR kunde inte initiera COM-lägenheten.".to_string()),
    }
}

#[cfg(windows)]
fn release_operation<T>(operation: &windows_future::IAsyncOperation<T>, cancel: bool)
where
    T: windows::core::RuntimeType + 'static,
{
    if cancel {
        let _ = operation.Cancel();
    }
    let _ = operation.Close();
}

#[cfg(windows)]
fn wait_ocr<T>(
    operation: windows_future::IAsyncOperation<T>,
    timeout: std::time::Duration,
) -> Result<T, String>
where
    T: windows::core::RuntimeType + 'static,
{
    use std::time::{Duration, Instant};
    use windows_future::AsyncStatus;

    if timeout.is_zero() {
        release_operation(&operation, true);
        return Err("OCR avbröts eftersom tidsgränsen överskreds.".to_string());
    }
    let deadline = Instant::now() + timeout;
    loop {
        let status = match operation.Status() {
            Ok(status) => status,
            Err(_) => {
                release_operation(&operation, true);
                return Err("Windows OCR tappade status för anropet.".to_string());
            }
        };
        match status {
            AsyncStatus::Completed => {
                let result = operation
                    .GetResults()
                    .map_err(|_| "Windows OCR misslyckades.".to_string());
                release_operation(&operation, false);
                return result;
            }
            AsyncStatus::Started => {
                if Instant::now() >= deadline {
                    release_operation(&operation, true);
                    return Err("OCR avbröts eftersom tidsgränsen överskreds.".to_string());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            AsyncStatus::Canceled => {
                release_operation(&operation, false);
                return Err("OCR avbröts eftersom tidsgränsen överskreds.".to_string());
            }
            AsyncStatus::Error => {
                release_operation(&operation, false);
                return Err("Windows OCR misslyckades.".to_string());
            }
            _ => {
                release_operation(&operation, true);
                return Err("Windows OCR misslyckades.".to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{mta_outcome, plan_tiles, region_in_source, MtaOutcome, OcrTile, WordBox};

    #[test]
    fn a_fitting_image_stays_one_tile() {
        let tiles = plan_tiles(800, 600, 2_000, 36).unwrap();
        assert_eq!(
            tiles,
            vec![OcrTile {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            }]
        );
    }

    #[test]
    fn a_wide_composite_is_tiled_without_shrinking() {
        let tiles = plan_tiles(2_500, 900, 1_000, 36).unwrap();
        assert_eq!(
            tiles,
            vec![
                OcrTile {
                    x: 0,
                    y: 0,
                    width: 1_000,
                    height: 900,
                },
                OcrTile {
                    x: 750,
                    y: 0,
                    width: 1_000,
                    height: 900,
                },
                OcrTile {
                    x: 1_500,
                    y: 0,
                    width: 1_000,
                    height: 900,
                },
            ]
        );
    }

    #[test]
    fn too_many_tiles_is_an_explicit_size_error() {
        let error = plan_tiles(10_000, 10_000, 1_000, 36).unwrap_err();
        assert!(error.contains("för stor"));
    }

    #[test]
    fn word_coordinates_keep_the_tile_origin() {
        let tile = OcrTile {
            x: 2_000,
            y: 0,
            width: 500,
            height: 900,
        };
        let region = region_in_source(
            WordBox {
                text: "Faktura".to_string(),
                x: 12.2,
                y: 40.0,
                width: 80.0,
                height: 18.0,
            },
            tile,
            2_500,
            900,
        )
        .unwrap();
        assert_eq!(region.x, 2_013);
        assert_eq!(region.y, 40);
        assert_eq!(region.width, 80);
        assert_eq!(region.height, 18);
        assert_eq!(region.text, "Faktura");
    }

    #[test]
    fn a_word_past_the_tile_is_clamped_to_the_source_image() {
        let tile = OcrTile {
            x: 1_000,
            y: 1_000,
            width: 100,
            height: 50,
        };
        let region = region_in_source(
            WordBox {
                text: "kant".to_string(),
                x: 10.0,
                y: 30.0,
                width: 40.0,
                height: 40.0,
            },
            tile,
            1_100,
            1_040,
        )
        .unwrap();
        assert_eq!(region.x, 1_010);
        assert_eq!(region.y, 1_030);
        assert_eq!(region.width, 40);
        assert_eq!(region.height, 10);
        assert!(region.x + region.width <= 1_100);
        assert!(region.y + region.height <= 1_040);
    }

    #[test]
    fn a_word_outside_the_image_is_dropped() {
        let tile = OcrTile {
            x: 1_000,
            y: 1_000,
            width: 100,
            height: 50,
        };
        let region = region_in_source(
            WordBox {
                text: "utanför".to_string(),
                x: 90.0,
                y: 40.0,
                width: 40.0,
                height: 30.0,
            },
            tile,
            1_050,
            1_040,
        );
        assert!(region.is_none());
    }

    #[test]
    fn mta_success_is_balanced_and_a_different_apartment_is_left_alone() {
        assert_eq!(mta_outcome(0), MtaOutcome::Acquired);
        assert_eq!(mta_outcome(1), MtaOutcome::Acquired);
        assert_eq!(
            mta_outcome(0x80010106_u32 as i32),
            MtaOutcome::AlreadyDifferent
        );
        assert_eq!(mta_outcome(0x800401F0_u32 as i32), MtaOutcome::Failed);
    }

    #[cfg(not(windows))]
    #[test]
    fn recognize_reports_that_windows_ocr_is_unavailable() {
        let error = super::recognize(std::path::Path::new("saknas.png")).unwrap_err();
        assert!(error.contains("stöds inte"));
        assert!(!error.contains("saknas.png"));
    }
}

#[cfg(test)]
mod robustness_tests {
    use super::*;

    #[test]
    fn overlapping_word_boxes_recover_cut_identifiers_without_duplicate_text() {
        let region = |text: &str, x, y, width| OcrRegion {
            text: text.into(),
            x,
            y,
            width,
            height: 30,
        };
        let words = vec![
            (0, region("SAPPHIF", 9870, 50, 120)),
            (1, region("SAPPHIRE-729", 9870, 50, 250)),
            (2, region("RE-729", 10000, 51, 120)),
            (0, region("ÅÄÖ", 50, 100, 70)),
            (0, region("ÅÄÖ", 250, 100, 70)),
        ];
        let (text, regions) = merge_regions(words);
        assert_eq!(text, "SAPPHIRE-729\nÅÄÖ ÅÄÖ");
        assert_eq!(regions.len(), 3);
        let (_, same_tile) = merge_regions(vec![
            (0, region("x", 10, 10, 50)),
            (0, region("²", 15, 10, 15)),
        ]);
        assert_eq!(
            same_tile.len(),
            2,
            "Never delete overlapping words from the same OCR result"
        );
    }

    #[test]
    fn transparency_is_composited_without_changing_opaque_screenshots() {
        let mut pixels = image::RgbaImage::new(4, 1);
        pixels.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
        pixels.put_pixel(1, 0, image::Rgba([0, 0, 0, 255]));
        pixels.put_pixel(2, 0, image::Rgba([255, 255, 255, 255]));
        pixels.put_pixel(3, 0, image::Rgba([0, 0, 0, 128]));
        let gray = grayscale_on_white(pixels);
        assert_eq!(gray.into_raw(), vec![255, 0, 255, 127]);
    }

    #[test]
    fn randomized_tiles_and_word_positions_never_escape_image_bounds() {
        let mut state = 0x1234_5678u32;
        for _ in 0..10_000 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let width = state % 20000 + 1;
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let height = state % 20000 + 1;
            if let Ok(tiles) = plan_tiles(width, height, 10000, 36) {
                assert!(tiles.len() <= 36);
                for tile in tiles {
                    assert!(tile.x + tile.width <= width && tile.y + tile.height <= height);
                    for (x, y, w, h) in [
                        (-10.0, -10.0, 25.0, 25.0),
                        (12.5, 20.5, 60.0, 80.0),
                        (f32::NAN, 0.0, 3.0, 4.0),
                        (0.0, 0.0, f32::INFINITY, 4.0),
                    ] {
                        if let Some(region) = region_in_source(
                            WordBox {
                                text: "Å🧑".into(),
                                x,
                                y,
                                width: w,
                                height: h,
                            },
                            tile,
                            width,
                            height,
                        ) {
                            assert!(
                                region.x + region.width <= width
                                    && region.y + region.height <= height
                            );
                            assert!(region.width > 0 && region.height > 0);
                            assert_eq!(region.text, "Å🧑");
                        }
                    }
                }
            }
        }
        for (w, h, d, n) in [
            (0, 1, 1, 1),
            (1, 0, 1, 1),
            (1, 1, 0, 1),
            (1, 1, 1, 0),
            (u32::MAX, u32::MAX, 1, 36),
        ] {
            assert!(plan_tiles(w, h, d, n).is_err());
        }
    }
}
