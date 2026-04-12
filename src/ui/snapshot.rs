use std::fs;
use std::path::{Path, PathBuf};

use crate::app::{AppState, Screen};
use crate::error::{Result, RingmasterError};
use crate::tui::render_snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapshotScenario {
    Strong,
    Weak,
    Empty,
    Stale,
    Error,
    MissingScope,
    RateLimited,
}

impl SnapshotScenario {
    pub const ALL: [Self; 7] = [
        Self::Strong,
        Self::Weak,
        Self::Empty,
        Self::Stale,
        Self::Error,
        Self::MissingScope,
        Self::RateLimited,
    ];

    pub const FIXTURE_BACKED: [Self; 3] = [Self::Strong, Self::Weak, Self::Empty];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Weak => "weak",
            Self::Empty => "empty",
            Self::Stale => "stale",
            Self::Error => "error",
            Self::MissingScope => "missing-scope",
            Self::RateLimited => "rate-limited",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSize {
    Compact,
    Medium,
    Wide,
}

impl SnapshotSize {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Medium, Self::Wide];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Medium => "medium",
            Self::Wide => "wide",
        }
    }

    pub const fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Compact => (90, 28),
            Self::Medium => (120, 36),
            Self::Wide => (160, 44),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotRequest {
    pub screen: Screen,
    pub size: SnapshotSize,
    pub scenario: Option<SnapshotScenario>,
}

impl SnapshotRequest {
    #[must_use]
    pub fn artifact_name(self) -> String {
        let screen = self.screen.title().to_ascii_lowercase();
        self.scenario.map_or_else(
            || format!("{screen}-{}.txt", self.size.label()),
            |scenario| format!("{screen}-{}-{}.txt", scenario.label(), self.size.label()),
        )
    }
}

#[must_use]
pub fn build_requests(
    screens: &[Screen],
    sizes: &[SnapshotSize],
    scenarios: Option<&[SnapshotScenario]>,
) -> Vec<SnapshotRequest> {
    let mut requests = Vec::new();
    for screen in screens {
        for size in sizes {
            if let Some(scenarios) = scenarios {
                for scenario in scenarios {
                    requests.push(SnapshotRequest {
                        screen: *screen,
                        size: *size,
                        scenario: Some(*scenario),
                    });
                }
            } else {
                requests.push(SnapshotRequest {
                    screen: *screen,
                    size: *size,
                    scenario: None,
                });
            }
        }
    }
    requests
}

#[must_use]
pub fn is_scenario_fixture_root(path: &Path) -> bool {
    SnapshotScenario::FIXTURE_BACKED
        .into_iter()
        .all(|scenario| path.join(scenario.label()).is_dir())
}

/// Renders and writes snapshot artifacts for the requested screens and sizes.
///
/// # Errors
///
/// Returns an error when the output directory cannot be created, when the snapshot
/// app builder fails, when rendering fails, or when an artifact cannot be written.
pub fn write_snapshots<F>(
    out_dir: &Path,
    requests: &[SnapshotRequest],
    mut app_for_request: F,
) -> Result<Vec<PathBuf>>
where
    F: FnMut(SnapshotRequest) -> Result<AppState>,
{
    fs::create_dir_all(out_dir)
        .map_err(|error| RingmasterError::io("creating UI snapshot output directory", error))?;

    let mut paths = Vec::new();

    for request in requests {
        let mut app = app_for_request(*request)?;
        app.active_screen = request.screen;
        let (width, height) = request.size.dimensions();
        let rendered = render_snapshot(&app, width, height)?;
        let path = out_dir.join(request.artifact_name());
        fs::write(&path, rendered)
            .map_err(|error| RingmasterError::io("writing UI snapshot artifact", error))?;
        paths.push(path);
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{
        SnapshotRequest, SnapshotScenario, SnapshotSize, build_requests, is_scenario_fixture_root,
        write_snapshots,
    };
    use crate::{
        app::{Screen, build_demo_state},
        config::{AppPaths, Config, LoggingConfig, OuraConfig, RefreshConfig, WebhookConfig},
        test_support::ok,
    };

    fn test_config() -> Config {
        let paths = ok(
            AppPaths::from_roots(
                PathBuf::from("/home/tester"),
                PathBuf::from("/tmp/config"),
                PathBuf::from("/tmp/state"),
                PathBuf::from("/tmp/cache"),
            ),
            "paths should resolve",
        );
        Config {
            app_name: "ringmaster",
            paths,
            logging: LoggingConfig {
                filter: "ringmaster=info".to_owned(),
            },
            oura: OuraConfig {
                client_id: Some("test-client".to_owned()),
                client_secret: None,
                authorize_url: "https://example.invalid/auth".to_owned(),
                token_url: "https://example.invalid/token".to_owned(),
                api_base_url: "https://example.invalid/api".to_owned(),
                secret_backend: crate::config::OuraSecretBackend::Keyring,
                secret_file: PathBuf::from("/tmp/state/ringmaster/secrets/oura-tokens.json"),
                callback_bind: "127.0.0.1:8788"
                    .parse()
                    .unwrap_or_else(|error| panic!("socket address should parse: {error}")),
                callback_path: "/callback".to_owned(),
                requested_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                    "workout".to_owned(),
                    "tag".to_owned(),
                    "session".to_owned(),
                ],
                auth_timeout_secs: 120,
            },
            refresh: RefreshConfig {
                personal_interval_secs: 3_600,
                daily_interval_secs: 300,
                heartrate_interval_secs: 60,
                workout_interval_secs: 600,
                enhanced_tag_interval_secs: 300,
                session_interval_secs: 300,
                personal_stale_after_secs: 72 * 60 * 60,
                daily_stale_after_secs: 12 * 60 * 60,
                heartrate_stale_after_secs: 15 * 60,
                workout_stale_after_secs: 24 * 60 * 60,
                enhanced_tag_stale_after_secs: 12 * 60 * 60,
                session_stale_after_secs: 12 * 60 * 60,
                daily_history_days: 90,
                daily_overlap_days: 2,
                heartrate_history_days: 7,
                heartrate_overlap_minutes: 60,
                workout_history_days: 90,
                workout_overlap_days: 2,
                enhanced_tag_history_days: 90,
                enhanced_tag_overlap_days: 2,
                session_history_days: 90,
                session_overlap_days: 2,
                max_backoff_secs: 60 * 60,
                demo_fixture_dir: None,
            },
            webhook: WebhookConfig {
                bind: "127.0.0.1:8799"
                    .parse()
                    .unwrap_or_else(|error| panic!("socket address should parse: {error}")),
                path: "/webhooks/oura".to_owned(),
                public_base_url: Some("https://example.test".to_owned()),
                verification_token: Some("verify-me".to_owned()),
                signature_tolerance_secs: 300,
                heartbeat_secs: 15,
                renewal_lead_secs: 7 * 24 * 60 * 60,
                subscriptions: crate::webhook::default_desired_subscriptions(),
            },
            guidance: crate::config::GuidanceConfig::default(),
            ai: crate::config::AiConfig::default(),
        }
    }

    #[test]
    fn writes_snapshot_artifacts_for_requested_matrix() {
        let config = test_config();
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let requests = build_requests(
            &[Screen::Dashboard, Screen::Review],
            &[SnapshotSize::Compact, SnapshotSize::Wide],
            None,
        );

        let paths = write_snapshots(out_dir.path(), &requests, |_| Ok(build_demo_state(&config)))
            .unwrap_or_else(|error| panic!("snapshots should write: {error}"));

        assert_eq!(paths.len(), 4);
        assert!(out_dir.path().join("dashboard-compact.txt").exists());
        assert!(out_dir.path().join("review-wide.txt").exists());
    }

    #[test]
    fn snapshot_size_dimensions_are_deterministic() {
        assert_eq!(SnapshotSize::Compact.dimensions(), (90, 28));
        assert_eq!(SnapshotSize::Medium.dimensions(), (120, 36));
        assert_eq!(SnapshotSize::Wide.dimensions(), (160, 44));
    }

    #[test]
    fn scenario_fixture_root_detection_requires_fixture_backed_scenarios() {
        let temp_root = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        assert!(!is_scenario_fixture_root(temp_root.path()));

        for scenario in SnapshotScenario::FIXTURE_BACKED {
            std::fs::create_dir_all(temp_root.path().join(scenario.label()))
                .unwrap_or_else(|error| panic!("fixture dir should create: {error}"));
        }

        assert!(is_scenario_fixture_root(temp_root.path()));
    }

    #[test]
    fn build_requests_tags_scenario_matrix_artifact_names() {
        let requests = build_requests(
            &[Screen::Dashboard],
            &[SnapshotSize::Compact],
            Some(&SnapshotScenario::ALL),
        );

        let names = requests
            .iter()
            .map(|request| request.artifact_name())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "dashboard-strong-compact.txt",
                "dashboard-weak-compact.txt",
                "dashboard-empty-compact.txt",
                "dashboard-stale-compact.txt",
                "dashboard-error-compact.txt",
                "dashboard-missing-scope-compact.txt",
                "dashboard-rate-limited-compact.txt",
            ]
        );
    }

    #[test]
    fn artifact_names_stay_single_source_when_no_scenario_is_requested() {
        let request = SnapshotRequest {
            screen: Screen::Timeline,
            size: SnapshotSize::Wide,
            scenario: None,
        };

        assert_eq!(request.artifact_name(), "timeline-wide.txt");
    }
}
