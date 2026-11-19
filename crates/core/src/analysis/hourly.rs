// 小时摘要生成器：按小时聚合活动，供时间线小时摘要使用。

use crate::database::{Activity, HourlySummary};
use std::collections::HashMap;

fn calculate_activity_covered_duration(activities: &[Activity]) -> i64 {
    let mut ranges = activities
        .iter()
        .filter_map(|activity| {
            if activity.duration <= 0 {
                return None;
            }

            let start = activity.timestamp.saturating_sub(activity.duration);
            let end = activity.timestamp;
            (end > start).then_some((start, end))
        })
        .collect::<Vec<_>>();

    if ranges.is_empty() {
        return 0;
    }

    ranges.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let mut total = 0;
    let mut current_start = ranges[0].0;
    let mut current_end = ranges[0].1;

    for (start, end) in ranges.into_iter().skip(1) {
        if start <= current_end {
            current_end = current_end.max(end);
        } else {
            total += current_end - current_start;
            current_start = start;
            current_end = end;
        }
    }

    total + (current_end - current_start)
}

/// 小时活动统计
#[derive(Debug, Clone)]
pub struct HourlyStats {
    app_durations: HashMap<String, i64>,
    pub total_duration: i64,
    pub activity_count: i32,
    pub representative_screenshots: Vec<String>,
    pub urls: Vec<String>,
    pub ocr_keywords: Vec<String>,
}

impl HourlyStats {
    /// 从活动列表构建统计
    pub fn from_activities(activities: Vec<Activity>) -> Self {
        let activities: Vec<_> = activities
            .into_iter()
            .filter(|activity| activity.duration > 0)
            .collect();
        let mut app_durations: HashMap<String, i64> = HashMap::new();
        let mut representative_screenshots = Vec::new();
        let mut urls = Vec::new();
        let mut last_app: Option<String> = None;

        // 先计算 activity_count
        let activity_count = activities.len() as i32;

        for activity in &activities {
            // 统计应用时长
            *app_durations.entry(activity.app_name.clone()).or_insert(0) += activity.duration;

            // 收集浏览器 URL
            if let Some(url) = &activity.browser_url {
                if !url.is_empty() && !urls.contains(url) {
                    urls.push(url.clone());
                }
            }

            // 挑选代表性截图（应用切换时的截图）
            let current_app = Some(activity.app_name.clone());
            if current_app != last_app && !activity.screenshot_path.is_empty() {
                representative_screenshots.push(activity.screenshot_path.clone());
            }
            last_app = current_app;
        }

        // 限制代表性截图数量
        if representative_screenshots.len() > 5 {
            representative_screenshots = representative_screenshots.into_iter().take(5).collect();
        }

        // 提取 OCR 关键词
        let mut ocr_keywords: Vec<String> = Vec::new();
        let mut seen_keywords: std::collections::HashSet<String> = std::collections::HashSet::new();
        for activity in &activities {
            if let Some(ocr_text) = &activity.ocr_text {
                // 提取有意义的词（长度 > 3 的中英文词）
                for word in ocr_text.split(|c: char| !c.is_alphanumeric() && c != '-') {
                    let word = word.trim();
                    if word.chars().count() >= 2
                        && word.chars().count() <= 20
                        && !seen_keywords.contains(word)
                    {
                        // 只保留中文或英文词
                        if word.chars().all(|c| c.is_alphabetic() || c >= '\u{4e00}') {
                            seen_keywords.insert(word.to_string());
                            ocr_keywords.push(word.to_string());
                            if ocr_keywords.len() >= 20 {
                                break;
                            }
                        }
                    }
                }
            }
            if ocr_keywords.len() >= 20 {
                break;
            }
        }

        let total_duration = calculate_activity_covered_duration(&activities);

        Self {
            app_durations,
            total_duration,
            activity_count,
            representative_screenshots,
            urls,
            ocr_keywords,
        }
    }

    /// 获取主要应用（按时长排序）
    pub fn get_main_apps(&self) -> Vec<String> {
        let mut apps: Vec<_> = self.app_durations.iter().collect();
        apps.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        apps.into_iter()
            .map(|(name, _)| name.clone())
            .take(5)
            .collect()
    }
}

/// Swedish text generated from recorded activity, without guessing missing URLs.
pub fn generate_fallback_summary(stats: &HourlyStats) -> String {
    let main_apps = stats.get_main_apps();
    if main_apps.is_empty() {
        return "Ingen registrerad aktivitet".to_string();
    }

    let duration_min = (stats.total_duration + 30) / 60;
    let mut summary = format!("Aktiv i {} i {duration_min} min", main_apps.join(", "));
    let mut domains = Vec::new();
    for url in &stats.urls {
        // Only name hosts supported by a recorded URL. Keep raw URL/OCR data intact.
        if let Ok(url) = reqwest::Url::parse(url) {
            if matches!(url.scheme(), "https" | "http") {
                if let Some(host) = url.host_str() {
                    if !domains.contains(&host.to_string()) {
                        domains.push(host.to_string());
                    }
                }
            }
        }
    }
    if !domains.is_empty() {
        summary.push_str(&format!(
            ". Webbplatser: {}",
            domains.into_iter().take(5).collect::<Vec<_>>().join(", ")
        ));
    }
    if !stats.ocr_keywords.is_empty() {
        summary.push_str(&format!(
            ". Nyckelord från skärmen: {}",
            stats
                .ocr_keywords
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    summary
}

/// Recognize only the exact historical template, corroborated by its source rows.
/// The replacement is for display; stored prose and original evidence are never changed.
pub fn regenerated_legacy_fallback(summary: &HourlySummary, stats: &HourlyStats) -> Option<String> {
    if summary.activity_count != stats.activity_count
        || summary.total_duration != stats.total_duration
    {
        return None;
    }
    let main_apps: Vec<String> = if summary.main_apps.is_empty() {
        Vec::new()
    } else {
        summary.main_apps.split(", ").map(str::to_string).collect()
    };
    // Historical HashMap ties had arbitrary order. Validate membership/ranking while
    // using the cached order to reconstruct the exact old generated text.
    if main_apps.len() != stats.app_durations.len().min(5) {
        return None;
    }
    let mut previous = i64::MAX;
    let mut seen = std::collections::HashSet::new();
    for app in &main_apps {
        let duration = *stats.app_durations.get(app)?;
        if duration > previous || !seen.insert(app) {
            return None;
        }
        previous = duration;
    }
    if stats
        .app_durations
        .iter()
        .any(|(app, duration)| !seen.contains(app) && *duration > previous)
    {
        return None;
    }
    (summary.summary == legacy_fallback_summary(stats, &main_apps))
        .then(|| generate_fallback_summary(stats))
}

// Kept solely to recognize existing generated cache entries, never used for new output.
/// 生成小时摘要（无 AI 时的备选方案）
fn legacy_fallback_summary(stats: &HourlyStats, main_apps: &[String]) -> String {
    if main_apps.is_empty() {
        return "无活动记录".to_string();
    }

    let apps_str = main_apps.join("、");
    // 四舍五入，与前端 Math.round() 保持一致，避免 "1分钟" vs "0分钟" 的矛盾
    let duration_min = (stats.total_duration + 30) / 60;

    let mut summary = format!("使用 {apps_str} 等应用 {duration_min}分钟");

    // 添加 URL 信息（提取域名，避免完整 URL 过长导致断句）
    if !stats.urls.is_empty() {
        let domain = stats
            .urls
            .first()
            .map(|url| {
                // 提取域名部分
                url.split("//")
                    .nth(1)
                    .unwrap_or(url)
                    .split('/')
                    .next()
                    .unwrap_or(url)
                    .split(':')
                    .next()
                    .unwrap_or(url)
            })
            .unwrap_or("");
        let extra = if stats.urls.len() > 1 {
            format!(" 等 {} 个网站", stats.urls.len())
        } else {
            " 等网站".to_string()
        };
        summary.push_str(&format!("，访问了 {domain}{extra}"));
    }

    // 添加 OCR 关键词
    if !stats.ocr_keywords.is_empty() {
        let keywords_str = stats
            .ocr_keywords
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("、");
        summary.push_str(&format!("，涉及: {keywords_str}"));
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::{generate_fallback_summary, HourlyStats};
    use crate::database::Activity;

    fn sample_activity(app_name: &str, timestamp: i64, duration: i64) -> Activity {
        Activity {
            id: None,
            timestamp,
            app_name: app_name.to_string(),
            window_title: format!("{app_name} window"),
            screenshot_path: String::new(),
            ocr_text: None,
            category: "development".to_string(),
            duration,
            browser_url: None,
            executable_path: None,
            semantic_category: None,
            semantic_confidence: None,
            screenshot_url: None,
        }
    }

    #[test]
    fn 小时摘要总时长应按小时内覆盖时长计算而不是重叠累加() {
        let activities = vec![
            sample_activity("Chrome", 10 * 3600 + 40 * 60, 40 * 60),
            sample_activity("WeChat", 10 * 3600 + 55 * 60, 35 * 60),
        ];

        let stats = HourlyStats::from_activities(activities);

        assert_eq!(stats.total_duration, 55 * 60);
        assert!(generate_fallback_summary(&stats).contains("55 min"));
    }
    fn cached_summary(stats: &HourlyStats, apps: &[String]) -> crate::database::HourlySummary {
        crate::database::HourlySummary {
            id: Some(7),
            date: "2026-11-19".into(),
            hour: 10,
            summary: super::legacy_fallback_summary(stats, apps),
            main_apps: apps.join(", "),
            activity_count: stats.activity_count,
            total_duration: stats.total_duration,
            representative_screenshots: None,
            created_at: 0,
        }
    }

    #[test]
    fn swedish_fallback_preserves_evidence_and_does_not_guess_urls() {
        let mut activity = sample_activity("Editor", 60, 60);
        activity.browser_url = Some("not a captured URL".into());
        activity.ocr_text = Some("Projekt ritningar".into());
        let stats = HourlyStats::from_activities(vec![activity]);
        assert_eq!(
            generate_fallback_summary(&stats),
            "Aktiv i Editor i 1 min. Nyckelord från skärmen: Projekt, ritningar"
        );
        assert_eq!(stats.urls, ["not a captured URL"]);
        assert_eq!(
            generate_fallback_summary(&HourlyStats::from_activities(vec![])),
            "Ingen registrerad aktivitet"
        );
    }

    #[test]
    fn legacy_fallback_requires_exact_template_and_corroborating_evidence() {
        let mut activity = sample_activity("Firefox", 60, 60);
        activity.browser_url = Some("https://example.com/docs".into());
        activity.ocr_text = Some("Ritningar".into());
        let stats = HourlyStats::from_activities(vec![activity]);
        let mut cached = cached_summary(&stats, &stats.get_main_apps());
        assert_eq!(super::regenerated_legacy_fallback(&cached, &stats), Some("Aktiv i Firefox i 1 min. Webbplatser: example.com. Nyckelord från skärmen: Ritningar".into()));
        cached.summary.push_str("。这是我写的备注");
        assert_eq!(super::regenerated_legacy_fallback(&cached, &stats), None);
        cached = cached_summary(&stats, &stats.get_main_apps());
        cached.activity_count += 1;
        assert_eq!(super::regenerated_legacy_fallback(&cached, &stats), None);
        cached = cached_summary(&stats, &stats.get_main_apps());
        cached.total_duration += 60;
        assert_eq!(super::regenerated_legacy_fallback(&cached, &stats), None);
    }

    #[test]
    fn legacy_fallback_accepts_historical_tie_order_without_changing_prose() {
        let stats = HourlyStats::from_activities(vec![
            sample_activity("Alpha", 60, 60),
            sample_activity("Beta", 120, 60),
        ]);
        let cached = cached_summary(&stats, &["Beta".into(), "Alpha".into()]);
        assert!(super::regenerated_legacy_fallback(&cached, &stats).is_some());
        assert_eq!(stats.get_main_apps(), ["Alpha", "Beta"]);
        let mut prose = cached.clone();
        prose.summary = "使用 Alpha 完成了项目设计，这是模型撰写的说明。".into();
        assert_eq!(super::regenerated_legacy_fallback(&prose, &stats), None);
    }
}
