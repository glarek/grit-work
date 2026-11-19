//! Read a supplied synthetic image through the production Windows OCR path.
#[path = "../src/image_ocr.rs"]
mod image_ocr;

fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Usage: grit-ocr-check SYNTHETIC_PNG [EXPECTED_TEXT]")?;
    let expected = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "SAPPHIRE-729".into());
    let output = image_ocr::recognize(std::path::Path::new(&path))?;
    if !output.text.contains(&expected) || output.regions.is_empty() {
        return Err("Synthetic identifier or word positions missing in OCR output".into());
    }
    println!(
        "PASS: Windows OCR read synthetic identifier; language={}, regions={}",
        output.language.unwrap_or_default(),
        output.regions.len()
    );
    Ok(())
}
