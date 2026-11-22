//! Read a supplied synthetic image through the production Windows OCR path.
#[path = "../src/image_ocr.rs"]
mod image_ocr;

fn main() -> Result<(), String> {
    let path = std::env::args().nth(1).ok_or(
        "Usage: grit-ocr-check SYNTHETIC_IMAGE [EXPECTED_TEXT] [--json] [--language sv-SE|en-US]",
    )?;
    let mut args = std::env::args().skip(2);
    let mut json_output = false;
    let mut expected = None;
    let mut language = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json_output = true,
            "--language" => language = Some(args.next().ok_or("Missing --language value")?),
            _ if arg.starts_with("--") || expected.is_some() => {
                return Err("Unknown OCR check argument".into())
            }
            _ => expected = Some(arg),
        }
    }
    let path = std::path::Path::new(&path);
    let output = match language {
        Some(language) => image_ocr::recognize_in_language(path, &language)?,
        None => image_ocr::recognize(path)?,
    };
    if json_output {
        println!(
            "{}",
            serde_json::to_string(&output).map_err(|_| "Could not encode OCR result")?
        );
        if expected.is_none() {
            return Ok(());
        }
    }
    let expected = expected.as_deref().unwrap_or("SAPPHIRE-729");
    if !output.text.contains(expected) || output.regions.is_empty() {
        return Err("Synthetic identifier or word positions missing in OCR output".into());
    }
    if !json_output {
        println!(
            "PASS: Windows OCR read synthetic identifier; language={}, regions={}",
            output.language.unwrap_or_default(),
            output.regions.len()
        );
    }
    Ok(())
}
