use serde::Serialize;

use crate::error::Result;
use crate::review::engine::{
    ReviewConfidence, ReviewDeck, ReviewInputs, ReviewMode, build_review_deck, ranked_cards,
};
use crate::review::features::ReviewSufficiency;
use crate::review::registry::ReviewFocus;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InvestigationReport {
    pub focus: ReviewFocus,
    pub anchor_day: String,
    pub headline: String,
    pub summary: String,
    pub confidence: ReviewConfidence,
    pub sufficiency: ReviewSufficiency,
    pub evidence: Vec<String>,
    pub counterevidence: Vec<String>,
    pub warnings: Vec<String>,
    pub look_at: Vec<String>,
}

pub fn build_investigation_report(
    focus: ReviewFocus,
    anchor_day: &str,
    inputs: &ReviewInputs<'_>,
) -> Result<InvestigationReport> {
    let today = build_review_deck(ReviewMode::Today, anchor_day, inputs)?;
    let week = build_review_deck(ReviewMode::Week, anchor_day, inputs)?;
    Ok(report_from_decks(focus, anchor_day, &today, &week))
}

fn report_from_decks(
    focus: ReviewFocus,
    anchor_day: &str,
    today: &ReviewDeck,
    week: &ReviewDeck,
) -> InvestigationReport {
    let focus_keys = focus.primary_signal_keys();
    let mut related_cards = ranked_cards(today)
        .into_iter()
        .filter(|card| focus_keys.contains(&card.signal_key.as_str()))
        .collect::<Vec<_>>();
    related_cards.extend(
        ranked_cards(week)
            .into_iter()
            .filter(|card| focus_keys.contains(&card.signal_key.as_str())),
    );
    related_cards.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.signal_key.cmp(&right.signal_key))
            .then_with(|| left.id.cmp(&right.id))
    });

    let headline = related_cards.first().map_or_else(
        || {
            format!(
                "{} investigation has limited direct evidence.",
                focus.label()
            )
        },
        |card| format!("{}: {}", focus.label(), card.headline),
    );
    let summary = related_cards.first().map_or_else(
        || format!(
            "Evidence is limited for {} because the current review windows do not contain a strong focus-specific signal.",
            focus.as_str()
        ),
        |card| card.summary.clone(),
    );
    let confidence = related_cards
        .first()
        .map_or(ReviewConfidence::Low, |card| card.confidence);
    let sufficiency = related_cards
        .first()
        .map_or(ReviewSufficiency::Missing, |card| card.sufficiency);

    let evidence = related_cards
        .iter()
        .flat_map(|card| card.evidence.iter().take(2).cloned())
        .take(6)
        .collect::<Vec<_>>();
    let counterevidence = related_cards
        .iter()
        .flat_map(|card| card.counterevidence.iter().take(1).cloned())
        .take(4)
        .collect::<Vec<_>>();
    let mut warnings = today.warnings.clone();
    warnings.extend(week.warnings.clone());
    if related_cards.is_empty() {
        warnings.push(format!(
            "No ranked {} observations were available for {}.",
            focus.as_str(),
            anchor_day
        ));
    }

    InvestigationReport {
        focus,
        anchor_day: anchor_day.to_owned(),
        headline,
        summary,
        confidence,
        sufficiency,
        evidence,
        counterevidence,
        warnings,
        look_at: look_at_lines(focus),
    }
}

fn look_at_lines(focus: ReviewFocus) -> Vec<String> {
    let mut lines = vec![
        "Open Review Today to compare the anchor day against the 30-day baseline.".to_owned(),
        "Open Explain for the anchor day to inspect the local evidence bundle.".to_owned(),
        "Open Patterns to see descriptive workout, tag, and session associations.".to_owned(),
    ];
    if matches!(
        focus,
        ReviewFocus::Sleep | ReviewFocus::Stress | ReviewFocus::Recovery
    ) {
        lines.push(
            "Open Timeline to inspect nearby workouts, tags, and sessions around the same day."
                .to_owned(),
        );
    }
    lines
}

#[cfg(test)]
mod tests {
    use crate::oura::models::CapabilityReport;
    use crate::review::engine::ReviewInputs;
    use crate::review::engine::{
        ReviewCard, ReviewConfidence, ReviewDeck, ReviewMode, ReviewSection,
    };
    use crate::review::features::ReviewSufficiency;
    use crate::review::registry::ReviewFocus;
    use crate::store::queries::ReviewSignalDayRecord;

    fn make_card(id: &str, signal_key: &str, score: i32) -> ReviewCard {
        ReviewCard {
            id: id.to_owned(),
            signal_key: signal_key.to_owned(),
            headline: format!("{signal_key} changed"),
            summary: format!("{signal_key} summary"),
            why_this_is_shown: "why".to_owned(),
            confidence: ReviewConfidence::Medium,
            sufficiency: ReviewSufficiency::Medium,
            confidence_label: "Medium confidence / Medium data".to_owned(),
            section: ReviewSection::NegativeDrift,
            score,
            anchor_day: "2026-04-08".to_owned(),
            evidence: vec![format!("{signal_key} evidence")],
            counterevidence: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn investigation_focuses_on_related_signal_keys() {
        let auth_status = crate::oura::models::AuthStatus {
            configured: true,
            callback_url: "http://localhost".to_owned(),
            requested_scopes: vec!["daily".to_owned()],
            granted_scopes: vec!["daily".to_owned()],
            missing_fields: Vec::new(),
            capability_report: CapabilityReport::demo(),
            auth_timeout_secs: 30,
            secret_backend: "memory".to_owned(),
            access_token_stored: true,
            refresh_token_stored: true,
            access_token_expires_at: None,
            last_authenticated_at: None,
            last_refresh_at: None,
            account_id: None,
            account_email: None,
            last_error: None,
        };
        let signal_days = vec![ReviewSignalDayRecord {
            signal_key: "stress_high".to_owned(),
            day: "2026-04-08".to_owned(),
            numeric_value: Some(220.0),
            text_value: None,
            baseline_mean: Some(90.0),
            baseline_stddev: Some(30.0),
            delta: Some(130.0),
            z_score: Some(4.3),
            persistence_days: 3,
            sufficiency: ReviewSufficiency::Strong,
            stale_days: 0,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T12:00:00Z".to_owned(),
        }];

        let report = super::build_investigation_report(
            ReviewFocus::Stress,
            "2026-04-08",
            &ReviewInputs {
                auth_status: &auth_status,
                signal_days: &signal_days,
                context_events: &[],
                pattern_summaries: &[],
                sleep_time: &[],
                rest_mode_periods: &[],
            },
        )
        .unwrap_or_else(|error| panic!("investigation should build: {error}"));

        assert!(report.headline.contains("Stress"));
        assert!(report.summary.contains("stress") || report.summary.contains("Stress"));
    }

    #[test]
    fn investigation_uses_all_ranked_cards_not_just_top_observations() {
        let today = ReviewDeck {
            mode: ReviewMode::Today,
            anchor_day: "2026-04-08".to_owned(),
            observations: vec![
                make_card("1", "sleep_score", 10),
                make_card("2", "readiness_score", 9),
                make_card("3", "activity_score", 8),
                make_card("4", "active_calories", 7),
                make_card("5", "steps", 6),
            ],
            positive_changes: Vec::new(),
            negative_drifts: vec![make_card("6", "stress_high", 5)],
            unresolved_anomalies: Vec::new(),
            warnings: Vec::new(),
        };
        let week = ReviewDeck {
            mode: ReviewMode::Week,
            anchor_day: "2026-04-08".to_owned(),
            observations: Vec::new(),
            positive_changes: Vec::new(),
            negative_drifts: Vec::new(),
            unresolved_anomalies: Vec::new(),
            warnings: Vec::new(),
        };

        let report = super::report_from_decks(ReviewFocus::Stress, "2026-04-08", &today, &week);

        assert!(report.headline.contains("stress_high changed"));
        assert!(
            report
                .warnings
                .iter()
                .all(|warning| !warning.contains("No ranked stress observations"))
        );
        assert!(
            report
                .evidence
                .iter()
                .any(|line| line.contains("stress_high"))
        );
    }

    #[test]
    fn investigation_breaks_equal_scores_deterministically() {
        let mut lower_confidence = make_card("stress-low", "stress_high", 8);
        lower_confidence.confidence = ReviewConfidence::Low;
        lower_confidence.headline = "stress low confidence".to_owned();
        let mut higher_confidence = make_card("stress-high", "stress_high", 8);
        higher_confidence.confidence = ReviewConfidence::High;
        higher_confidence.headline = "stress high confidence".to_owned();

        let today = ReviewDeck {
            mode: ReviewMode::Today,
            anchor_day: "2026-04-08".to_owned(),
            observations: vec![lower_confidence],
            positive_changes: Vec::new(),
            negative_drifts: Vec::new(),
            unresolved_anomalies: Vec::new(),
            warnings: Vec::new(),
        };
        let week = ReviewDeck {
            mode: ReviewMode::Week,
            anchor_day: "2026-04-08".to_owned(),
            observations: vec![higher_confidence],
            positive_changes: Vec::new(),
            negative_drifts: Vec::new(),
            unresolved_anomalies: Vec::new(),
            warnings: Vec::new(),
        };

        let report = super::report_from_decks(ReviewFocus::Stress, "2026-04-08", &today, &week);

        assert!(report.headline.contains("stress high confidence"));
        assert_eq!(report.confidence, ReviewConfidence::High);
    }
}
