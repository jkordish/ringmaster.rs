use serde::Serialize;

use crate::error::Result;
use crate::review::engine::{
    ReviewConfidence, ReviewDeck, ReviewInputs, ReviewMode, build_review_deck, focus_cards,
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
    let mut related_cards = focus_cards(focus, &today.observations);
    related_cards.extend(focus_cards(focus, &week.observations));
    related_cards.sort_by(|left, right| right.score.cmp(&left.score));

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
    use crate::review::features::ReviewSufficiency;
    use crate::review::registry::ReviewFocus;
    use crate::store::queries::ReviewSignalDayRecord;

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
}
