//! Read-only connection diagnostic. Credentials arrive on stdin, never command arguments.
use std::io::Read;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let entry = keyring::Entry::new("se.grit.work", "spiris")?;
    if !input.trim().is_empty() {
        let _: serde_json::Value = serde_json::from_str(&input)?;
        entry.set_password(input.trim())?;
        if entry.get_password()? != input.trim() {
            return Err("OS keyring verification failed".into());
        }
        println!("OS keyring readback verified");
    }
    let saved = entry.get_password()?;
    let v: serde_json::Value = serde_json::from_str(&saved)?;
    let c = reqwest::Client::builder()
        .user_agent("GritWork/0.1")
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let r = c
        .post("https://publicapi.tid.vismaspcs.se/v1/Auth/Token")
        .basic_auth(
            v["application_id"].as_str().unwrap_or(""),
            v["secret"].as_str(),
        )
        .json(&serde_json::json!({}))
        .send()
        .await?;
    println!("Auth HTTP {}", r.status());
    if !r.status().is_success() {
        return Ok(());
    }
    let response: serde_json::Value = r.json().await?;
    let token = response["accessToken"].as_str().ok_or("Missing token")?;
    for (path, extra) in [
        ("Core/Projects", ""),
        ("Admin/Users", ""),
        ("Admin/Activities", ""),
        ("Admin/TimeCodes", ""),
        ("Admin/Articles", "&type=time"),
    ] {
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        let r = c
            .get(format!(
                "https://publicapi.tid.vismaspcs.se/v1/{path}?page=1&pageSize=100{extra}"
            ))
            .bearer_auth(token)
            .send()
            .await?;
        println!("{path}: HTTP {}", r.status());
        if !r.status().is_success() {
            continue;
        }
        let v: serde_json::Value = r.json().await?;
        println!("total={}, pages={}", v["totalItemCount"], v["totalPages"]);
        if let Some(first) = v["items"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_object())
        {
            println!("fields={:?}", first.keys().collect::<Vec<_>>());
        }
    }
    Ok(())
}
