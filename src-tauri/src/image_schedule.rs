//! Shared clock for the background job and the sidebar countdown.
use crate::image_interpretation::{ImageSettings, Mode};

pub struct ImageSchedule {
    configuration: Option<(ImageSettings, bool)>,
    pub next_run_at: Option<i64>,
}
impl ImageSchedule {
    pub const fn new() -> Self {
        Self {
            configuration: None,
            next_run_at: None,
        }
    }
    pub fn configure(&mut self, settings: &ImageSettings, consent: bool, now: i64) {
        let configuration = (settings.clone(), consent);
        if self.configuration.as_ref() == Some(&configuration) {
            return;
        }
        let enabled = settings.mode == Mode::Ocr || (settings.mode == Mode::Ai && consent);
        self.next_run_at = enabled.then(|| now.saturating_add(settings.interval_seconds as i64));
        self.configuration = Some(configuration);
    }
    pub fn due(&self, now: i64) -> bool {
        self.next_run_at.is_some_and(|at| at <= now)
    }
    pub fn started(&mut self, now: i64) {
        if let Some((settings, _)) = &self.configuration {
            if self.next_run_at.is_some() {
                self.next_run_at = Some(now.saturating_add(settings.interval_seconds as i64));
            }
        }
    }
    pub fn defer_if_due(&mut self, now: i64) {
        if self.due(now) {
            self.next_run_at = Some(now.saturating_add(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn countdown_is_the_actual_job_deadline_and_polling_does_not_reset_it() {
        let mut clock = ImageSchedule::new();
        let settings = ImageSettings {
            mode: Mode::Ocr,
            interval_seconds: 300,
            ..ImageSettings::default()
        };
        clock.configure(&settings, false, 1000);
        assert_eq!(clock.next_run_at, Some(1300));
        clock.configure(&settings, false, 1250);
        assert!(!clock.due(1299));
        assert!(clock.due(1300));
        clock.started(1300);
        assert_eq!(clock.next_run_at, Some(1600));
        clock.defer_if_due(1400);
        assert_eq!(clock.next_run_at, Some(1600));
        clock.defer_if_due(1600);
        assert_eq!(clock.next_run_at, Some(1605));
    }
    #[test]
    fn interval_and_mode_changes_reconfigure_without_enabling_unconsented_ai() {
        let mut clock = ImageSchedule::new();
        let mut settings = ImageSettings::default();
        clock.configure(&settings, true, 1000);
        assert_eq!(clock.next_run_at, None);
        settings.mode = Mode::Ai;
        clock.configure(&settings, false, 1001);
        assert_eq!(clock.next_run_at, None);
        clock.configure(&settings, true, 1002);
        assert_eq!(clock.next_run_at, Some(1302));
        settings.interval_seconds = 60;
        clock.configure(&settings, true, 1010);
        assert_eq!(clock.next_run_at, Some(1070));
        clock.configure(&settings, false, 1020);
        assert_eq!(clock.next_run_at, None);
        settings.mode = Mode::Ocr;
        clock.configure(&settings, false, 1030);
        assert_eq!(clock.next_run_at, Some(1090));
    }
}
