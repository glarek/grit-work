//! Synthetic end-to-end vision test, never uses the user's captured activity.
#[path = "../src/grit_cli.rs"]
mod grit_cli;
use serde_json::json;
fn main() -> Result<(), String> {
    let provider = std::env::args().nth(1).unwrap_or("codex".into());
    let model = std::env::args().nth(2).unwrap_or("gpt-6.2-sol".into());
    let work = std::env::temp_dir().join(format!("grit-vision-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let image =
        std::path::PathBuf::from(std::env::args().nth(3).ok_or(
            "Usage: grit-agent-check PROVIDER MODEL IMAGE (image must show SAPPHIRE-729)",
        )?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let activities = json!([{"id":1,"timestamp":1768208400_i64,"duration":3600,"app_name":"Editor","window_title":"Task board","screenshot_path":image}]);
    let mut projects = json!({"projects":[{"id":42,"name":"ORBIT","number":"42","active":true}],"activities":[{"id":2,"name":"Development"}],"articles":[{"id":3,"name":"Hour"}],"default_activity_id":2,"default_article_id":3});
    let analysis = std::env::args().nth(4).as_deref() == Some("--analysis");
    if analysis {
        projects["analysis_question"] = json!("Vilket är det exakta ärendenumret som syns i bilden? Svara med numret och vad underlaget visar.");
    }
    let result = grit_cli::run_agent(
        &provider,
        &model,
        &work,
        &activities,
        &projects,
        180,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    if result.is_ok() {
        let _ = std::fs::remove_dir_all(&work);
    } else {
        eprintln!("Synthetic diagnostic retained at {}", work.display());
    }
    let result = result?;
    let mut description = result["rows"]
        .as_array()
        .ok_or("rows missing")?
        .iter()
        .map(|r| r["description"].as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ");
    if analysis {
        if !result["rows"].as_array().is_some_and(|r| r.is_empty()) {
            return Err("Analysis created report rows".into());
        }
        description = result["summary"].as_str().unwrap_or("").into();
    }
    if !description.contains("SAPPHIRE-729") {
        return Err(format!(
            "Vision identifier not present in description: {description}"
        ));
    }
    println!("PASS: {provider} read synthetic image identifier and produced valid JSON (analysis={analysis})");
    Ok(())
}
