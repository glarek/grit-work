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
    recognize_platform(path)
}

#[cfg(not(windows))]
fn recognize_platform(_path: &Path) -> Result<OcrOutput, String> {
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

    let columns = width.div_ceil(max_dimension);
    let rows = height.div_ceil(max_dimension);
    let tile_count = columns.saturating_mul(rows);
    if tile_count > max_tiles {
        return Err("Bilden är för stor för OCR utan att förminska texten.".to_string());
    }

    let mut tiles = Vec::with_capacity(tile_count as usize);
    for row in 0..rows {
        for column in 0..columns {
            let x = column.saturating_mul(max_dimension);
            let y = row.saturating_mul(max_dimension);
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
fn recognize_platform(path: &Path) -> Result<OcrOutput, String> {
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
    let (engine, language) = select_engine()?;
    let mut text_parts = Vec::new();
    let mut regions = Vec::new();
    let started = Instant::now();

    for tile in tiles {
        if started.elapsed() >= TOTAL_WAIT {
            return Err("OCR avbröts eftersom den sammanlagda tidsgränsen överskreds.".to_string());
        }
        let view = crop_imm(&rgba, tile.x, tile.y, tile.width, tile.height).to_image();
        let gray = image::DynamicImage::ImageRgba8(view).to_luma8();
        let bitmap = gray_bitmap(gray.as_raw(), tile.width, tile.height)?;
        let operation = engine
            .RecognizeAsync(&bitmap)
            .map_err(|_| "Windows OCR kunde inte starta tolkningen.".to_string())?;
        let result = wait_ocr(
            operation,
            TILE_WAIT.min(TOTAL_WAIT.saturating_sub(started.elapsed())),
        )?;
        let tile_text = result
            .Text()
            .map_err(|_| "Windows OCR returnerade ingen text.".to_string())?
            .to_string();
        if !tile_text.trim().is_empty() {
            text_parts.push(tile_text);
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
                    regions.push(region);
                }
            }
        }
    }

    Ok(OcrOutput {
        text: text_parts.join("\n"),
        language: Some(language),
        regions,
    })
}

#[cfg(windows)]
fn select_engine() -> Result<(windows::Media::Ocr::OcrEngine, String), String> {
    use windows::Media::Ocr::OcrEngine;

    for tag in ["sv-SE", "en-US"] {
        if let Some(selected) = engine_for_tag(tag)? {
            return Ok(selected);
        }
    }

    let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| {
        "Ingen OCR-språkmodul är installerad. Windows behöver sv-SE eller en-US.".to_string()
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
        Ok(engine) => Ok(Some((engine, recognizer_tag(&engine)?))),
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
                    x: 1_000,
                    y: 0,
                    width: 1_000,
                    height: 900,
                },
                OcrTile {
                    x: 2_000,
                    y: 0,
                    width: 500,
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
