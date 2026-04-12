use std::collections::BTreeSet;

use serde::Serialize;
use time::Date;

use crate::error::{Result, RingmasterError};
use crate::evidence::PopulationProfile;
use crate::evidence::policy::{append_required_disclaimers, guidance_comparison_text};
use crate::oura::models::{AuthStatus, CapabilityKind};
use crate::review::features::ReviewSufficiency;
use crate::review::registry::{
    EvidenceKind, ReviewFocus, SignalDefinition, SignalDirectionality, WeeklyAggregation,
    signal_definition, signal_definitions,
};
use crate::review::templates::{
    confidence_badge, headline_for_signal, sufficiency_line, summary_for_signal, why_this_is_shown,
};
use crate::store::queries::{
    ContextEventFamily, ContextEventRecord, PatternMetric, PatternSummaryRecord,
    RestModePeriodRecord, ReviewSignalDayRecord, SleepTimeRecord,
};

const DEVIATION_THRESHOLD: f64 = 0.5;
const MAX_TOP_ITEMS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReviewMode {
    Today,
    Week,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReviewSection {
    Observation,
    PositiveChange,
    NegativeDrift,
    UnresolvedAnomaly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum ReviewConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewCard {
    pub id: String,
    pub signal_key: String,
    pub headline: String,
    pub summary: String,
    pub why_this_is_shown: String,
    pub confidence: ReviewConfidence,
    pub sufficiency: ReviewSufficiency,
    pub confidence_label: String,
    pub section: ReviewSection,
    pub score: i32,
    pub anchor_day: String,
    pub evidence: Vec<String>,
    pub counterevidence: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewDeck {
    pub mode: ReviewMode,
    pub anchor_day: String,
    pub observations: Vec<ReviewCard>,
    pub positive_changes: Vec<ReviewCard>,
    pub negative_drifts: Vec<ReviewCard>,
    pub unresolved_anomalies: Vec<ReviewCard>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewInputs<'a> {
    pub auth_status: &'a AuthStatus,
    pub active_population_profile: PopulationProfile,
    pub signal_days: &'a [ReviewSignalDayRecord],
    pub context_events: &'a [ContextEventRecord],
    pub pattern_summaries: &'a [PatternSummaryRecord],
    pub sleep_time: &'a [SleepTimeRecord],
    pub rest_mode_periods: &'a [RestModePeriodRecord],
}

#[derive(Debug, Clone, PartialEq)]
struct AggregateMeasurement<'a> {
    definition: &'a SignalDefinition,
    anchor_day: String,
    numeric_value: Option<f64>,
    baseline_mean: Option<f64>,
    baseline_stddev: Option<f64>,
    delta: Option<f64>,
    z_score: Option<f64>,
    persistence_days: u32,
    sufficiency: ReviewSufficiency,
    stale_days: u32,
    week_day_count: usize,
}

/// # Errors
///
/// Returns an error if review measurements cannot be derived from the provided inputs.
pub fn build_review_deck(
    mode: ReviewMode,
    anchor_day: &str,
    inputs: &ReviewInputs<'_>,
) -> Result<ReviewDeck> {
    let measurements = match mode {
        ReviewMode::Today => build_today_measurements(anchor_day, inputs),
        ReviewMode::Week => build_week_measurements(anchor_day, inputs)?,
    };
    let warnings = capability_warnings(inputs.auth_status);
    let mut cards = measurements
        .into_iter()
        .filter_map(|measurement| build_card(mode, measurement, inputs))
        .collect::<Vec<_>>();
    sort_cards(&mut cards);

    let observations = cards
        .iter()
        .take(MAX_TOP_ITEMS)
        .cloned()
        .collect::<Vec<_>>();
    let positive_changes = cards
        .iter()
        .filter(|card| card.section == ReviewSection::PositiveChange)
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    let negative_drifts = cards
        .iter()
        .filter(|card| card.section == ReviewSection::NegativeDrift)
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_anomalies = cards
        .iter()
        .filter(|card| card.section == ReviewSection::UnresolvedAnomaly)
        .take(3)
        .cloned()
        .collect::<Vec<_>>();

    Ok(ReviewDeck {
        mode,
        anchor_day: anchor_day.to_owned(),
        observations,
        positive_changes,
        negative_drifts,
        unresolved_anomalies,
        warnings,
    })
}

#[must_use]
pub fn focus_cards(focus: ReviewFocus, cards: &[ReviewCard]) -> Vec<&ReviewCard> {
    let focus_keys = focus.primary_signal_keys();
    cards
        .iter()
        .filter(|card| focus_keys.contains(&card.signal_key.as_str()))
        .collect()
}

#[must_use]
pub fn ranked_cards(deck: &ReviewDeck) -> Vec<&ReviewCard> {
    let mut seen = BTreeSet::new();
    let mut cards = Vec::new();

    for collection in [
        &deck.observations,
        &deck.positive_changes,
        &deck.negative_drifts,
        &deck.unresolved_anomalies,
    ] {
        for card in collection {
            if seen.insert(card.id.as_str()) {
                cards.push(card);
            }
        }
    }

    cards
}

fn build_today_measurements<'a>(
    anchor_day: &str,
    inputs: &'a ReviewInputs<'a>,
) -> Vec<AggregateMeasurement<'a>> {
    signal_definitions()
        .iter()
        .filter(|definition| definition.evidence_kind == EvidenceKind::Direct)
        .filter(|definition| {
            definition
                .suitable_surfaces
                .contains(&crate::review::registry::ReviewSurface::Today)
        })
        .filter_map(|definition| {
            inputs
                .signal_days
                .iter()
                .find(|row| row.signal_key == definition.key && row.day == anchor_day)
                .and_then(|row| {
                    row.numeric_value.map(|numeric_value| AggregateMeasurement {
                        definition,
                        anchor_day: anchor_day.to_owned(),
                        numeric_value: Some(numeric_value),
                        baseline_mean: row.baseline_mean,
                        baseline_stddev: row.baseline_stddev,
                        delta: row.delta,
                        z_score: row.z_score,
                        persistence_days: row.persistence_days,
                        sufficiency: row.sufficiency,
                        stale_days: row.stale_days,
                        week_day_count: 1,
                    })
                })
        })
        .collect()
}

fn build_week_measurements<'a>(
    anchor_day: &str,
    inputs: &'a ReviewInputs<'a>,
) -> Result<Vec<AggregateMeasurement<'a>>> {
    let anchor_date = parse_day(anchor_day)?;
    let week_start = anchor_date
        .checked_sub(time::Duration::days(6))
        .ok_or_else(|| {
            RingmasterError::Config("weekly review underflowed anchor day".to_owned())
        })?;

    let mut measurements = Vec::new();
    for definition in signal_definitions()
        .iter()
        .filter(|definition| definition.evidence_kind == EvidenceKind::Direct)
        .filter(|definition| {
            definition
                .suitable_surfaces
                .contains(&crate::review::registry::ReviewSurface::Week)
        })
    {
        let baseline_window_days =
            i64::try_from(definition.baseline_window_days).map_err(|error| {
                RingmasterError::Config(format!(
                    "baseline window overflowed i64 for {}: {error}",
                    definition.key
                ))
            })?;
        let baseline_start = week_start
            .checked_sub(time::Duration::days(baseline_window_days))
            .ok_or_else(|| {
                RingmasterError::Config(format!(
                    "weekly baseline underflowed anchor day for {}",
                    definition.key
                ))
            })?;
        let current_values = inputs
            .signal_days
            .iter()
            .filter(|row| row.signal_key == definition.key)
            .filter_map(|row| parse_day(&row.day).ok().map(|day| (day, row)))
            .filter(|(day, _)| *day >= week_start && *day <= anchor_date)
            .collect::<Vec<_>>();
        let baseline_values = inputs
            .signal_days
            .iter()
            .filter(|row| row.signal_key == definition.key)
            .filter_map(|row| parse_day(&row.day).ok().map(|day| (day, row)))
            .filter(|(day, _)| *day >= baseline_start && *day < week_start)
            .collect::<Vec<_>>();

        let current_numeric = current_values
            .iter()
            .filter_map(|(_, row)| row.numeric_value)
            .collect::<Vec<_>>();
        if current_numeric.is_empty() {
            continue;
        }

        measurements.push(build_week_measurement(
            anchor_day,
            definition,
            &current_values,
            &baseline_values,
            &current_numeric,
            baseline_start,
        )?);
    }

    Ok(measurements)
}

fn build_week_measurement<'a>(
    anchor_day: &str,
    definition: &'a SignalDefinition,
    current_values: &[(Date, &ReviewSignalDayRecord)],
    baseline_values: &[(Date, &ReviewSignalDayRecord)],
    current_numeric: &[f64],
    baseline_start: Date,
) -> Result<AggregateMeasurement<'a>> {
    let numeric_value = aggregate_values(definition.weekly_aggregation, current_numeric);
    let baseline_aggregates = aggregate_baseline_weeks(
        definition.weekly_aggregation,
        baseline_values,
        baseline_start,
        definition.baseline_window_days,
    )?;
    let baseline_mean = mean_value(&baseline_aggregates);
    let baseline_stddev = standard_deviation(&baseline_aggregates);
    let delta = match (numeric_value, baseline_mean) {
        (Some(current_value), Some(mean_value)) => Some(current_value - mean_value),
        _ => None,
    };
    let z_score = match (delta, baseline_stddev) {
        (Some(delta_value), Some(stddev)) if stddev >= 0.01 => Some(delta_value / stddev),
        _ => None,
    };
    let persistence_days = weekly_persistence_days(current_values, delta);
    let stale_days = current_values
        .iter()
        .map(|(_, row)| row.stale_days)
        .min()
        .unwrap_or_default();
    let sufficiency = ReviewSufficiency::from_comparable_weeks(baseline_aggregates.len());

    Ok(AggregateMeasurement {
        definition,
        anchor_day: anchor_day.to_owned(),
        numeric_value,
        baseline_mean,
        baseline_stddev,
        delta,
        z_score,
        persistence_days: u32::try_from(persistence_days).map_err(|error| {
            RingmasterError::Config(format!(
                "weekly persistence overflowed u32 for {}: {error}",
                definition.key
            ))
        })?,
        sufficiency,
        stale_days,
        week_day_count: current_numeric.len(),
    })
}

fn weekly_persistence_days(
    current_values: &[(Date, &ReviewSignalDayRecord)],
    weekly_delta: Option<f64>,
) -> usize {
    current_values
        .iter()
        .filter(|(_, row)| {
            row.z_score
                .is_some_and(|value| value.abs() >= DEVIATION_THRESHOLD)
                && weekly_delta.is_some_and(|delta| {
                    row.delta
                        .is_some_and(|day_delta| same_direction(day_delta, delta))
                })
        })
        .count()
}

fn build_card(
    mode: ReviewMode,
    measurement: AggregateMeasurement<'_>,
    inputs: &ReviewInputs<'_>,
) -> Option<ReviewCard> {
    let deviation_bucket = deviation_bucket(measurement.z_score, measurement.delta);
    if deviation_bucket == 0 {
        return None;
    }

    let persistence_bucket = persistence_bucket(measurement.persistence_days);
    let recency = recency_points(mode, measurement.stale_days);
    let corroboration_points = corroboration_points(
        mode,
        measurement.definition,
        &measurement.anchor_day,
        inputs,
    );
    let counterevidence = counterevidence_lines(mode, &measurement, inputs);
    let counterevidence_penalty = i32::try_from(counterevidence.len()).ok()?.min(2);
    let freshness_penalty = freshness_penalty(measurement.stale_days);
    let sufficiency_penalty = sufficiency_penalty(measurement.sufficiency);
    let score = deviation_bucket * 3 + persistence_bucket * 2 + recency + corroboration_points
        - counterevidence_penalty
        - freshness_penalty
        - sufficiency_penalty;
    if score <= 0 {
        return None;
    }

    let confidence = classify_confidence(
        measurement.sufficiency,
        freshness_penalty,
        counterevidence_penalty,
        corroboration_points,
    );
    let evidence = evidence_lines(mode, &measurement, corroboration_points, inputs);
    let mut warnings = Vec::new();
    if measurement.sufficiency != ReviewSufficiency::Strong {
        warnings.push(sufficiency_line(measurement.sufficiency));
    }
    if freshness_penalty > 0 {
        warnings.push(format!(
            "Freshness is reduced because the latest supporting data is {} day(s) old.",
            measurement.stale_days
        ));
    }
    append_required_disclaimers(
        measurement.definition.key,
        inputs.active_population_profile,
        &mut warnings,
    );

    let section = classify_section(
        measurement.definition.directionality,
        measurement.delta,
        measurement.z_score,
    );
    Some(ReviewCard {
        id: format!("{}:{}", measurement.definition.key, measurement.anchor_day),
        signal_key: measurement.definition.key.to_owned(),
        headline: headline_for_signal(
            measurement.definition,
            mode,
            measurement.delta,
            measurement.z_score,
        ),
        summary: summary_for_signal(
            measurement.definition,
            mode,
            measurement.definition.baseline_window_days,
            measurement.persistence_days,
        ),
        why_this_is_shown: why_this_is_shown(
            measurement.definition.baseline_window_days,
            deviation_bucket,
            persistence_bucket,
            corroboration_points,
        ),
        confidence,
        sufficiency: measurement.sufficiency,
        confidence_label: confidence_badge(confidence, measurement.sufficiency),
        section,
        score,
        anchor_day: measurement.anchor_day,
        evidence,
        counterevidence,
        warnings,
    })
}

fn sort_cards(cards: &mut [ReviewCard]) {
    cards.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(right.confidence.rank().cmp(&left.confidence.rank()))
            .then(left.signal_key.cmp(&right.signal_key))
    });
}

fn evidence_lines(
    mode: ReviewMode,
    measurement: &AggregateMeasurement<'_>,
    corroboration_points: i32,
    inputs: &ReviewInputs<'_>,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(guidance) = guidance_comparison_text(
        measurement.definition.key,
        inputs.active_population_profile,
        measurement.numeric_value,
    ) {
        lines.push(guidance.summary);
    }
    if let (Some(numeric_value), Some(baseline_mean)) =
        (measurement.numeric_value, measurement.baseline_mean)
    {
        lines.push(format!(
            "{} is {:.1} versus a {:.1} recent baseline.",
            measurement.definition.label, numeric_value, baseline_mean
        ));
    }
    if let Some(z_score) = measurement.z_score {
        lines.push(format!(
            "Deviation strength is {z_score:.1} standard deviations from baseline."
        ));
    }
    if measurement.persistence_days > 1 {
        lines.push(format!(
            "This pattern has persisted for {} recent days.",
            measurement.persistence_days
        ));
    }
    lines.extend(context_support_lines(
        mode,
        measurement.definition,
        &measurement.anchor_day,
        inputs,
    ));
    if corroboration_points == 0 {
        lines.push(match mode {
            ReviewMode::Today => {
                "No strong contextual corroboration was found for today.".to_owned()
            }
            ReviewMode::Week => {
                "No strong contextual corroboration was found for this week.".to_owned()
            }
        });
    }
    lines
}

fn counterevidence_lines(
    mode: ReviewMode,
    measurement: &AggregateMeasurement<'_>,
    inputs: &ReviewInputs<'_>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let sibling_keys = sibling_keys(measurement.definition.key);
    for sibling_key in sibling_keys {
        let sibling_rows = sibling_signal_rows_in_review_window(
            mode,
            &measurement.anchor_day,
            sibling_key,
            inputs.signal_days,
        );
        if !sibling_rows.is_empty()
            && sibling_rows.iter().all(|row| {
                row.z_score
                    .is_some_and(|value| value.abs() < DEVIATION_THRESHOLD)
            })
            && let Some(definition) = signal_definition(sibling_key)
        {
            lines.push(format!(
                "{} stayed near baseline, so the signal is mixed.",
                definition.label
            ));
        }
    }

    if measurement.sufficiency == ReviewSufficiency::Missing {
        lines.push(
            "Evidence is limited because no comparable baseline days are available.".to_owned(),
        );
    }

    lines
}

fn sibling_signal_rows_in_review_window<'a>(
    mode: ReviewMode,
    anchor_day: &str,
    sibling_key: &str,
    signal_days: &'a [ReviewSignalDayRecord],
) -> Vec<&'a ReviewSignalDayRecord> {
    let Some((window_start, window_end)) = review_window_bounds(mode, anchor_day) else {
        return Vec::new();
    };

    signal_days
        .iter()
        .filter(|row| row.signal_key == sibling_key)
        .filter(|row| {
            parse_day(&row.day)
                .ok()
                .is_some_and(|day| day >= window_start && day <= window_end)
        })
        .collect()
}

fn context_support_lines(
    mode: ReviewMode,
    definition: &SignalDefinition,
    anchor_day: &str,
    inputs: &ReviewInputs<'_>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let related_events = context_events_in_review_window(mode, anchor_day, inputs.context_events);
    for event in related_events {
        lines.push(format!(
            "{} context nearby: {}.",
            event_family_label(event.family),
            event.title
        ));
    }

    if matches!(
        definition.key,
        "sleep_score" | "readiness_score" | "stress_high" | "recovery_high"
    ) && let Some(record) = inputs
        .sleep_time
        .iter()
        .find(|record| record.day == anchor_day)
        && let Some(status) = &record.status
    {
        lines.push(format!(
            "Sleep timing status on the anchor day was {}.",
            status.replace('_', " ")
        ));
    }

    if matches!(
        definition.key,
        "readiness_score" | "stress_high" | "recovery_high" | "resilience_level"
    ) {
        let rest_mode_days = overlapping_rest_mode_days(mode, anchor_day, inputs.rest_mode_periods);
        if rest_mode_days > 0 {
            lines.push(format!(
                "Rest mode overlapped {rest_mode_days} day(s) in the current review window."
            ));
        }
    }

    if let Some(metric) = pattern_metric_for_signal(definition.key)
        && let Some(pattern) = inputs
            .pattern_summaries
            .iter()
            .find(|summary| summary.metric == metric)
    {
        lines.push(format!(
            "Historical pattern support exists for {} events and {}.",
            event_family_label(pattern.family),
            metric.label()
        ));
    }

    lines
}

fn corroboration_points(
    mode: ReviewMode,
    definition: &SignalDefinition,
    anchor_day: &str,
    inputs: &ReviewInputs<'_>,
) -> i32 {
    let context_match = i32::from(
        !context_events_in_review_window(mode, anchor_day, inputs.context_events).is_empty(),
    );
    let pattern_match = i32::from(
        pattern_metric_for_signal(definition.key)
            .and_then(|metric| {
                inputs
                    .pattern_summaries
                    .iter()
                    .find(|summary| summary.metric == metric)
            })
            .is_some(),
    );
    let rest_mode_match = i32::from(
        matches!(
            definition.key,
            "readiness_score" | "stress_high" | "recovery_high" | "resilience_level"
        ) && overlapping_rest_mode_days(mode, anchor_day, inputs.rest_mode_periods) > 0,
    );
    (context_match + pattern_match + rest_mode_match).min(2)
}

fn classify_confidence(
    sufficiency: ReviewSufficiency,
    freshness_penalty: i32,
    counterevidence_penalty: i32,
    corroboration_points: i32,
) -> ReviewConfidence {
    if matches!(
        sufficiency,
        ReviewSufficiency::Missing | ReviewSufficiency::Thin
    ) || freshness_penalty >= 2
    {
        ReviewConfidence::Low
    } else if sufficiency == ReviewSufficiency::Strong
        && freshness_penalty == 0
        && counterevidence_penalty == 0
        && corroboration_points >= 1
    {
        ReviewConfidence::High
    } else {
        ReviewConfidence::Medium
    }
}

fn classify_section(
    directionality: SignalDirectionality,
    delta: Option<f64>,
    z_score: Option<f64>,
) -> ReviewSection {
    let comparator = z_score.or(delta).unwrap_or_default();
    match directionality {
        SignalDirectionality::HigherBetter => {
            if comparator.is_sign_positive() {
                ReviewSection::PositiveChange
            } else {
                ReviewSection::NegativeDrift
            }
        }
        SignalDirectionality::LowerBetter => {
            if comparator.is_sign_negative() {
                ReviewSection::PositiveChange
            } else {
                ReviewSection::NegativeDrift
            }
        }
        SignalDirectionality::Neutral | SignalDirectionality::Contextual => {
            ReviewSection::UnresolvedAnomaly
        }
    }
}

fn aggregate_values(aggregation: WeeklyAggregation, values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }

    match aggregation {
        WeeklyAggregation::Mean => {
            Some(values.iter().sum::<f64>() / crate::numeric::usize_to_f64(values.len()))
        }
        WeeklyAggregation::Sum | WeeklyAggregation::Count => Some(values.iter().sum()),
        WeeklyAggregation::Latest => values.last().copied(),
    }
}

fn aggregate_baseline_weeks(
    aggregation: WeeklyAggregation,
    baseline_values: &[(Date, &ReviewSignalDayRecord)],
    baseline_start: Date,
    baseline_window_days: usize,
) -> Result<Vec<f64>> {
    let mut weekly_aggregates = Vec::new();
    let week_count = baseline_window_days.div_ceil(7);

    for week_offset in 0..week_count {
        let week_offset = i64::try_from(week_offset).map_err(|error| {
            RingmasterError::Config(format!(
                "weekly baseline week offset overflowed i64: {error}"
            ))
        })?;
        let window_start = baseline_start
            .checked_add(time::Duration::days(week_offset * 7))
            .ok_or_else(|| {
                RingmasterError::Config(
                    "weekly baseline window overflowed supported date range".to_owned(),
                )
            })?;
        let window_end = window_start
            .checked_add(time::Duration::days(6))
            .ok_or_else(|| {
                RingmasterError::Config(
                    "weekly baseline window exceeded supported date range".to_owned(),
                )
            })?;
        let window_values = baseline_values
            .iter()
            .filter(|(day, _)| *day >= window_start && *day <= window_end)
            .filter_map(|(_, row)| row.numeric_value)
            .collect::<Vec<_>>();
        if let Some(aggregate) = aggregate_values(aggregation, &window_values) {
            weekly_aggregates.push(aggregate);
        }
    }

    Ok(weekly_aggregates)
}

fn same_direction(left: f64, right: f64) -> bool {
    (left > 0.0 && right > 0.0) || (left < 0.0 && right < 0.0)
}

fn mean_value(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / crate::numeric::usize_to_f64(values.len()))
    }
}

fn standard_deviation(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mean_value = values.iter().sum::<f64>() / crate::numeric::usize_to_f64(values.len());
    let variance = values
        .iter()
        .map(|value| (*value - mean_value).powi(2))
        .sum::<f64>()
        / crate::numeric::usize_to_f64(values.len());
    Some(variance.sqrt())
}

fn deviation_bucket(z_score: Option<f64>, delta: Option<f64>) -> i32 {
    let absolute = z_score
        .map(f64::abs)
        .or_else(|| delta.map(f64::abs))
        .unwrap_or_default();
    if absolute >= 2.0 {
        4
    } else if absolute >= 1.5 {
        3
    } else if absolute >= 1.0 {
        2
    } else {
        i32::from(absolute >= DEVIATION_THRESHOLD)
    }
}

const fn persistence_bucket(persistence_days: u32) -> i32 {
    match persistence_days {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        _ => 3,
    }
}

const fn recency_points(mode: ReviewMode, stale_days: u32) -> i32 {
    match mode {
        ReviewMode::Today => 2,
        ReviewMode::Week if stale_days <= 3 => 1,
        ReviewMode::Week => 0,
    }
}

fn freshness_penalty(stale_days: u32) -> i32 {
    if stale_days >= 3 {
        2
    } else {
        i32::from(stale_days >= 2)
    }
}

const fn sufficiency_penalty(sufficiency: ReviewSufficiency) -> i32 {
    match sufficiency {
        ReviewSufficiency::Missing => 2,
        ReviewSufficiency::Thin => 1,
        ReviewSufficiency::Medium | ReviewSufficiency::Strong => 0,
    }
}

fn sibling_keys(signal_key: &str) -> &'static [&'static str] {
    match signal_key {
        "sleep_score" => &["readiness_score"],
        "readiness_score" => &["sleep_score", "activity_score"],
        "activity_score" => &["readiness_score", "steps"],
        "stress_high" => &["recovery_high", "resilience_level"],
        "recovery_high" => &["stress_high", "resilience_level"],
        _ => &[],
    }
}

fn pattern_metric_for_signal(signal_key: &str) -> Option<PatternMetric> {
    match signal_key {
        "sleep_score" => Some(PatternMetric::Sleep),
        "readiness_score" => Some(PatternMetric::Readiness),
        "activity_score" | "active_calories" | "steps" => Some(PatternMetric::Activity),
        _ => None,
    }
}

fn overlapping_rest_mode_days(
    mode: ReviewMode,
    anchor_day: &str,
    rest_mode_periods: &[RestModePeriodRecord],
) -> usize {
    let Some((window_start, window_end)) = review_window_bounds(mode, anchor_day) else {
        return 0;
    };
    let mut overlapped_days = BTreeSet::new();

    for period in rest_mode_periods {
        let Some(start_day) = parse_day(&period.start_day).ok() else {
            continue;
        };
        let end_day = period
            .end_day
            .as_deref()
            .and_then(|day| parse_day(day).ok())
            .unwrap_or(window_end);

        let overlap_start = start_day.max(window_start);
        let overlap_end = end_day.min(window_end);
        if overlap_start > overlap_end {
            continue;
        }

        let mut day = overlap_start;
        loop {
            overlapped_days.insert(day);
            if day >= overlap_end {
                break;
            }
            let Some(next_day) = day.next_day() else {
                break;
            };
            day = next_day;
        }
    }

    overlapped_days.len()
}

fn context_events_in_review_window<'a>(
    mode: ReviewMode,
    anchor_day: &str,
    context_events: &'a [ContextEventRecord],
) -> Vec<&'a ContextEventRecord> {
    let Some((window_start, window_end)) = review_window_bounds(mode, anchor_day) else {
        return Vec::new();
    };

    context_events
        .iter()
        .filter(|event| {
            parse_day(&event.anchor_day)
                .ok()
                .is_some_and(|event_day| event_day >= window_start && event_day <= window_end)
        })
        .take(2)
        .collect()
}

fn review_window_bounds(mode: ReviewMode, anchor_day: &str) -> Option<(Date, Date)> {
    let anchor_date = parse_day(anchor_day).ok()?;
    let window_start = match mode {
        ReviewMode::Today => anchor_date,
        ReviewMode::Week => anchor_date
            .checked_sub(time::Duration::days(6))
            .unwrap_or(anchor_date),
    };
    Some((window_start, anchor_date))
}

const fn event_family_label(family: ContextEventFamily) -> &'static str {
    match family {
        ContextEventFamily::Workout => "Workout",
        ContextEventFamily::Tag => "Tag",
        ContextEventFamily::EnhancedTag => "Enhanced tag",
        ContextEventFamily::Session => "Session",
    }
}

fn capability_warnings(auth_status: &AuthStatus) -> Vec<String> {
    let mut warnings = Vec::new();
    if !auth_status
        .capability_report
        .is_granted(CapabilityKind::Daily)
    {
        warnings.push("Daily scope is missing, so review evidence may be incomplete.".to_owned());
    }
    warnings
}

fn parse_day(day: &str) -> Result<Date> {
    Date::parse(
        day,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| {
        RingmasterError::Config(format!("failed to parse review day `{day}`: {error}"))
    })
}

impl ReviewConfidence {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    const fn rank(self) -> i32 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use time::Month;

    use crate::evidence::PopulationProfile;
    use crate::oura::models::CapabilityReport;
    use crate::review::engine::{
        ReviewInputs, ReviewMode, build_review_deck, build_week_measurements,
        overlapping_rest_mode_days, ranked_cards,
    };
    use crate::review::features::ReviewSufficiency;
    use crate::store::queries::{
        ContextEventFamily, ContextEventRecord, PatternMetric, PatternRelationWindow,
        PatternSummaryRecord, RestModePeriodRecord, ReviewSignalDayRecord,
    };

    fn auth_status() -> crate::oura::models::AuthStatus {
        crate::oura::models::AuthStatus {
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
        }
    }

    #[test]
    fn today_review_ranks_negative_drift_before_weaker_items() {
        let auth_status = auth_status();
        let signal_days = vec![
            ReviewSignalDayRecord {
                signal_key: "readiness_score".to_owned(),
                day: "2026-04-08".to_owned(),
                numeric_value: Some(64.0),
                text_value: None,
                baseline_mean: Some(81.0),
                baseline_stddev: Some(6.0),
                delta: Some(-17.0),
                z_score: Some(-2.8),
                persistence_days: 4,
                sufficiency: ReviewSufficiency::Strong,
                stale_days: 0,
                metadata_json: "{}".to_owned(),
                updated_at: "2026-04-08T12:00:00Z".to_owned(),
            },
            ReviewSignalDayRecord {
                signal_key: "activity_score".to_owned(),
                day: "2026-04-08".to_owned(),
                numeric_value: Some(76.0),
                text_value: None,
                baseline_mean: Some(78.0),
                baseline_stddev: Some(4.0),
                delta: Some(-2.0),
                z_score: Some(-0.5),
                persistence_days: 1,
                sufficiency: ReviewSufficiency::Strong,
                stale_days: 0,
                metadata_json: "{}".to_owned(),
                updated_at: "2026-04-08T12:00:00Z".to_owned(),
            },
        ];

        let deck = build_review_deck(
            ReviewMode::Today,
            "2026-04-08",
            &ReviewInputs {
                auth_status: &auth_status,
                active_population_profile: PopulationProfile::GeneralAdult,
                signal_days: &signal_days,
                context_events: &[ContextEventRecord {
                    context_event_id: "workout:1".to_owned(),
                    family: ContextEventFamily::Workout,
                    source_id: "1".to_owned(),
                    anchor_day: "2026-04-08".to_owned(),
                    start_at: "2026-04-08T18:00:00Z".to_owned(),
                    end_at: Some("2026-04-08T19:00:00Z".to_owned()),
                    time_semantics: crate::store::queries::TimeSemantics::Interval,
                    title: "Late workout".to_owned(),
                    subtype: Some("running".to_owned()),
                    notes: None,
                    intensity: Some("high".to_owned()),
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T20:00:00Z".to_owned(),
                }],
                pattern_summaries: &[PatternSummaryRecord {
                    summary_id: "summary-1".to_owned(),
                    family: ContextEventFamily::Workout,
                    normalized_key: "sport:running".to_owned(),
                    relation_window: PatternRelationWindow::NextDayReadiness,
                    metric: PatternMetric::Readiness,
                    sample_count: 6,
                    median_delta: -4.0,
                    effect_direction: crate::store::queries::EffectDirection::Lower,
                    confidence: crate::store::queries::DataSufficiency::Strong,
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T20:00:00Z".to_owned(),
                }],
                sleep_time: &[],
                rest_mode_periods: &[],
            },
        )
        .unwrap_or_else(|error| panic!("today review should build: {error}"));

        assert_eq!(
            deck.observations
                .first()
                .map(|card| card.signal_key.as_str()),
            Some("readiness_score")
        );
        assert!(
            deck.observations
                .first()
                .is_some_and(|card| card.headline.contains("below your baseline"))
        );
    }

    #[test]
    fn weekly_sum_metrics_compare_against_prior_weekly_windows() {
        let auth_status = auth_status();
        let start_day = time::Date::from_calendar_date(2026, Month::March, 5)
            .unwrap_or_else(|error| panic!("test start day should be valid: {error}"));
        let signal_days = (0_i64..35_i64)
            .map(|offset| {
                let day = start_day
                    .checked_add(time::Duration::days(offset))
                    .unwrap_or_else(|| panic!("test day should stay in range"));
                let day = day
                    .format(&time::macros::format_description!("[year]-[month]-[day]"))
                    .unwrap_or_else(|error| panic!("test day should format: {error}"));
                ReviewSignalDayRecord {
                    signal_key: "steps".to_owned(),
                    day,
                    numeric_value: Some(100.0),
                    text_value: None,
                    baseline_mean: Some(100.0),
                    baseline_stddev: Some(10.0),
                    delta: Some(0.0),
                    z_score: Some(0.0),
                    persistence_days: 0,
                    sufficiency: ReviewSufficiency::Strong,
                    stale_days: 0,
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                }
            })
            .collect::<Vec<_>>();

        let deck = build_review_deck(
            ReviewMode::Week,
            "2026-04-08",
            &ReviewInputs {
                auth_status: &auth_status,
                active_population_profile: PopulationProfile::GeneralAdult,
                signal_days: &signal_days,
                context_events: &[],
                pattern_summaries: &[],
                sleep_time: &[],
                rest_mode_periods: &[],
            },
        )
        .unwrap_or_else(|error| panic!("weekly review should build: {error}"));

        assert!(
            ranked_cards(&deck)
                .iter()
                .all(|card| card.signal_key.as_str() != "steps"),
            "equal weekly step totals should not be ranked as a drift"
        );
    }

    #[test]
    fn weekly_measurements_respect_signal_baseline_window() {
        let auth_status = auth_status();
        let start_day = time::Date::from_calendar_date(2026, Month::March, 5)
            .unwrap_or_else(|error| panic!("test start day should be valid: {error}"));
        let signal_days = (0_i64..35_i64)
            .map(|offset| {
                let day = start_day
                    .checked_add(time::Duration::days(offset))
                    .unwrap_or_else(|| panic!("test day should stay in range"));
                let day = day
                    .format(&time::macros::format_description!("[year]-[month]-[day]"))
                    .unwrap_or_else(|error| panic!("test day should format: {error}"));
                let baseline_value = match offset {
                    0..=6 => 200.0,
                    7..=27 => 100.0,
                    28..=34 => 150.0,
                    _ => 0.0,
                };
                ReviewSignalDayRecord {
                    signal_key: "steps".to_owned(),
                    day,
                    numeric_value: Some(baseline_value),
                    text_value: None,
                    baseline_mean: Some(baseline_value),
                    baseline_stddev: Some(10.0),
                    delta: Some(0.0),
                    z_score: Some(0.0),
                    persistence_days: 0,
                    sufficiency: ReviewSufficiency::Strong,
                    stale_days: 0,
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                }
            })
            .collect::<Vec<_>>();

        let deck = build_review_deck(
            ReviewMode::Week,
            "2026-04-08",
            &ReviewInputs {
                auth_status: &auth_status,
                active_population_profile: PopulationProfile::GeneralAdult,
                signal_days: &signal_days,
                context_events: &[],
                pattern_summaries: &[],
                sleep_time: &[],
                rest_mode_periods: &[],
            },
        )
        .unwrap_or_else(|error| panic!("weekly review should build: {error}"));

        let steps_card = ranked_cards(&deck)
            .into_iter()
            .find(|card| card.signal_key == "steps")
            .unwrap_or_else(|| panic!("steps card should be ranked"));

        assert!(
            steps_card
                .evidence
                .iter()
                .any(|line| line.contains("700.0 recent baseline")),
            "steps should compare against the configured 21-day baseline, not an older fourth week"
        );
    }

    #[test]
    fn weekly_persistence_only_counts_days_in_the_weekly_drift_direction() {
        let auth_status = auth_status();
        let start_day = time::Date::from_calendar_date(2026, Month::March, 5)
            .unwrap_or_else(|error| panic!("test start day should be valid: {error}"));
        let signal_days = (0_i64..35_i64)
            .map(|offset| {
                let day = start_day
                    .checked_add(time::Duration::days(offset))
                    .unwrap_or_else(|| panic!("test day should stay in range"));
                let day = day
                    .format(&time::macros::format_description!("[year]-[month]-[day]"))
                    .unwrap_or_else(|error| panic!("test day should format: {error}"));
                let (numeric_value, delta, z_score) = if (28..=34).contains(&offset) {
                    match offset {
                        28..=30 => (Some(130.0), Some(30.0), Some(3.0)),
                        31..=34 => (Some(90.0), Some(-10.0), Some(-1.0)),
                        _ => (Some(100.0), Some(0.0), Some(0.0)),
                    }
                } else {
                    (Some(100.0), Some(0.0), Some(0.0))
                };
                ReviewSignalDayRecord {
                    signal_key: "steps".to_owned(),
                    day,
                    numeric_value,
                    text_value: None,
                    baseline_mean: Some(100.0),
                    baseline_stddev: Some(10.0),
                    delta,
                    z_score,
                    persistence_days: 0,
                    sufficiency: ReviewSufficiency::Strong,
                    stale_days: 0,
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                }
            })
            .collect::<Vec<_>>();

        let inputs = ReviewInputs {
            auth_status: &auth_status,
            active_population_profile: PopulationProfile::GeneralAdult,
            signal_days: &signal_days,
            context_events: &[],
            pattern_summaries: &[],
            sleep_time: &[],
            rest_mode_periods: &[],
        };
        let measurements = build_week_measurements("2026-04-08", &inputs)
            .unwrap_or_else(|error| panic!("weekly measurements should build: {error}"));

        let steps_measurement = measurements
            .into_iter()
            .find(|measurement| measurement.definition.key == "steps")
            .unwrap_or_else(|| panic!("steps weekly measurement should exist"));

        assert_eq!(
            steps_measurement.persistence_days, 3,
            "weekly persistence should only count in-window days that drift in the same direction as the weekly aggregate"
        );
    }

    #[test]
    fn weekly_sufficiency_uses_comparable_weekly_aggregates() {
        let auth_status = auth_status();
        let start_day = time::Date::from_calendar_date(2026, Month::March, 5)
            .unwrap_or_else(|error| panic!("test start day should be valid: {error}"));
        let signal_days = [(0_i64, 40.0), (8_i64, 41.0), (16_i64, 42.0), (28_i64, 43.0)]
            .into_iter()
            .map(|(offset, value)| {
                let day = start_day
                    .checked_add(time::Duration::days(offset))
                    .unwrap_or_else(|| panic!("test day should stay in range"))
                    .format(&time::macros::format_description!("[year]-[month]-[day]"))
                    .unwrap_or_else(|error| panic!("test day should format: {error}"));
                ReviewSignalDayRecord {
                    signal_key: "vo2_max".to_owned(),
                    day,
                    numeric_value: Some(value),
                    text_value: None,
                    baseline_mean: Some(value),
                    baseline_stddev: Some(1.0),
                    delta: Some(0.0),
                    z_score: Some(0.0),
                    persistence_days: 0,
                    sufficiency: ReviewSufficiency::Strong,
                    stale_days: 0,
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                }
            })
            .collect::<Vec<_>>();

        let deck = build_review_deck(
            ReviewMode::Week,
            "2026-04-08",
            &ReviewInputs {
                auth_status: &auth_status,
                active_population_profile: PopulationProfile::GeneralAdult,
                signal_days: &signal_days,
                context_events: &[],
                pattern_summaries: &[],
                sleep_time: &[],
                rest_mode_periods: &[],
            },
        )
        .unwrap_or_else(|error| panic!("weekly review should build: {error}"));

        let vo2_card = ranked_cards(&deck)
            .into_iter()
            .find(|card| card.signal_key == "vo2_max")
            .unwrap_or_else(|| panic!("vo2 max card should be ranked"));

        assert_eq!(vo2_card.sufficiency, ReviewSufficiency::Medium);
    }

    #[test]
    fn weekly_review_uses_context_events_from_the_full_week_window() {
        let auth_status = auth_status();
        let start_day = time::Date::from_calendar_date(2026, Month::March, 5)
            .unwrap_or_else(|error| panic!("test start day should be valid: {error}"));
        let signal_days = (0_i64..35_i64)
            .map(|offset| {
                let day = start_day
                    .checked_add(time::Duration::days(offset))
                    .unwrap_or_else(|| panic!("test day should stay in range"));
                let day = day
                    .format(&time::macros::format_description!("[year]-[month]-[day]"))
                    .unwrap_or_else(|error| panic!("test day should format: {error}"));
                let in_current_week = offset >= 28;
                ReviewSignalDayRecord {
                    signal_key: "readiness_score".to_owned(),
                    day,
                    numeric_value: Some(if in_current_week { 60.0 } else { 80.0 }),
                    text_value: None,
                    baseline_mean: Some(80.0),
                    baseline_stddev: Some(5.0),
                    delta: Some(if in_current_week { -20.0 } else { 0.0 }),
                    z_score: Some(if in_current_week { -4.0 } else { 0.0 }),
                    persistence_days: if in_current_week { 4 } else { 0 },
                    sufficiency: ReviewSufficiency::Strong,
                    stale_days: 0,
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                }
            })
            .collect::<Vec<_>>();

        let deck = build_review_deck(
            ReviewMode::Week,
            "2026-04-08",
            &ReviewInputs {
                auth_status: &auth_status,
                active_population_profile: PopulationProfile::GeneralAdult,
                signal_days: &signal_days,
                context_events: &[ContextEventRecord {
                    context_event_id: "workout:week-window".to_owned(),
                    family: ContextEventFamily::Workout,
                    source_id: "week-window".to_owned(),
                    anchor_day: "2026-04-04".to_owned(),
                    start_at: "2026-04-04T18:00:00Z".to_owned(),
                    end_at: Some("2026-04-04T18:40:00Z".to_owned()),
                    time_semantics: crate::store::queries::TimeSemantics::Interval,
                    title: "Tempo run".to_owned(),
                    subtype: Some("running".to_owned()),
                    notes: None,
                    intensity: Some("moderate".to_owned()),
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-04T19:00:00Z".to_owned(),
                }],
                pattern_summaries: &[],
                sleep_time: &[],
                rest_mode_periods: &[],
            },
        )
        .unwrap_or_else(|error| panic!("weekly review should build: {error}"));

        let readiness_card = ranked_cards(&deck)
            .into_iter()
            .find(|card| card.signal_key == "readiness_score")
            .unwrap_or_else(|| panic!("readiness card should be ranked"));

        assert!(
            readiness_card
                .evidence
                .iter()
                .any(|line| line.contains("Workout context nearby: Tempo run."))
        );
        assert!(
            readiness_card
                .evidence
                .iter()
                .all(|line| line != "No strong contextual corroboration was found for this week.")
        );
    }

    #[test]
    fn overlapping_rest_mode_days_counts_interior_anchor_days() {
        let periods = vec![RestModePeriodRecord {
            period_id: "rest-mode-1".to_owned(),
            start_day: "2026-04-01".to_owned(),
            start_time: Some("2026-04-01T00:00:00Z".to_owned()),
            end_day: Some("2026-04-05".to_owned()),
            end_time: Some("2026-04-05T23:59:59Z".to_owned()),
            episode_count: 1,
            tags_json: "[]".to_owned(),
            raw_cache_key: None,
            updated_at: "2026-04-05T23:59:59Z".to_owned(),
        }];

        assert_eq!(
            overlapping_rest_mode_days(ReviewMode::Today, "2026-04-03", &periods),
            1
        );
    }

    #[test]
    fn overlapping_rest_mode_days_counts_week_window_overlap() {
        let periods = vec![RestModePeriodRecord {
            period_id: "rest-mode-1".to_owned(),
            start_day: "2026-04-01".to_owned(),
            start_time: Some("2026-04-01T00:00:00Z".to_owned()),
            end_day: Some("2026-04-03".to_owned()),
            end_time: Some("2026-04-03T23:59:59Z".to_owned()),
            episode_count: 1,
            tags_json: "[]".to_owned(),
            raw_cache_key: None,
            updated_at: "2026-04-03T23:59:59Z".to_owned(),
        }];

        assert_eq!(
            overlapping_rest_mode_days(ReviewMode::Week, "2026-04-05", &periods),
            3
        );
    }

    #[test]
    fn overlapping_rest_mode_days_treats_open_periods_as_active_through_anchor_day() {
        let periods = vec![RestModePeriodRecord {
            period_id: "rest-mode-open".to_owned(),
            start_day: "2026-04-01".to_owned(),
            start_time: Some("2026-04-01T00:00:00Z".to_owned()),
            end_day: None,
            end_time: None,
            episode_count: 1,
            tags_json: "[]".to_owned(),
            raw_cache_key: None,
            updated_at: "2026-04-01T23:59:59Z".to_owned(),
        }];

        assert_eq!(
            overlapping_rest_mode_days(ReviewMode::Today, "2026-04-03", &periods),
            1
        );
        assert_eq!(
            overlapping_rest_mode_days(ReviewMode::Week, "2026-04-05", &periods),
            5
        );
    }

    #[test]
    fn weekly_counterevidence_checks_the_full_review_window() {
        let auth_status = auth_status();
        let start_day = time::Date::from_calendar_date(2026, Month::March, 5)
            .unwrap_or_else(|error| panic!("test start day should be valid: {error}"));
        let signal_days = (0_i64..35_i64)
            .flat_map(|offset| {
                let day = start_day
                    .checked_add(time::Duration::days(offset))
                    .unwrap_or_else(|| panic!("test day should stay in range"))
                    .format(&time::macros::format_description!("[year]-[month]-[day]"))
                    .unwrap_or_else(|error| panic!("test day should format: {error}"));
                let in_current_week = offset >= 28;
                let readiness = ReviewSignalDayRecord {
                    signal_key: "readiness_score".to_owned(),
                    day: day.clone(),
                    numeric_value: Some(if in_current_week { 60.0 } else { 80.0 }),
                    text_value: None,
                    baseline_mean: Some(80.0),
                    baseline_stddev: Some(5.0),
                    delta: Some(if in_current_week { -20.0 } else { 0.0 }),
                    z_score: Some(if in_current_week { -4.0 } else { 0.0 }),
                    persistence_days: if in_current_week { 4 } else { 0 },
                    sufficiency: ReviewSufficiency::Strong,
                    stale_days: 0,
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                };
                let activity_z_score = match offset {
                    28..=33 => -1.5,
                    _ => 0.0,
                };
                let activity = ReviewSignalDayRecord {
                    signal_key: "activity_score".to_owned(),
                    day,
                    numeric_value: Some(if (28..=33).contains(&offset) {
                        65.0
                    } else {
                        78.0
                    }),
                    text_value: None,
                    baseline_mean: Some(78.0),
                    baseline_stddev: Some(4.0),
                    delta: Some(if (28..=33).contains(&offset) {
                        -13.0
                    } else {
                        0.0
                    }),
                    z_score: Some(activity_z_score),
                    persistence_days: if (28..=33).contains(&offset) { 3 } else { 0 },
                    sufficiency: ReviewSufficiency::Strong,
                    stale_days: 0,
                    metadata_json: "{}".to_owned(),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                };
                [readiness, activity]
            })
            .collect::<Vec<_>>();

        let deck = build_review_deck(
            ReviewMode::Week,
            "2026-04-08",
            &ReviewInputs {
                auth_status: &auth_status,
                active_population_profile: PopulationProfile::GeneralAdult,
                signal_days: &signal_days,
                context_events: &[],
                pattern_summaries: &[],
                sleep_time: &[],
                rest_mode_periods: &[],
            },
        )
        .unwrap_or_else(|error| panic!("weekly review should build: {error}"));

        let readiness_card = ranked_cards(&deck)
            .into_iter()
            .find(|card| card.signal_key == "readiness_score")
            .unwrap_or_else(|| panic!("readiness card should be ranked"));

        assert!(
            readiness_card
                .counterevidence
                .iter()
                .all(|line| !line.contains("stayed near baseline")),
            "weekly sibling drift elsewhere in the window should prevent a mixed-signal penalty"
        );
    }

    #[test]
    fn sibling_counterevidence_ignores_unknown_baselines() {
        let auth_status = auth_status();
        let signal_days = vec![
            ReviewSignalDayRecord {
                signal_key: "readiness_score".to_owned(),
                day: "2026-04-08".to_owned(),
                numeric_value: Some(62.0),
                text_value: None,
                baseline_mean: Some(80.0),
                baseline_stddev: Some(5.0),
                delta: Some(-18.0),
                z_score: Some(-3.6),
                persistence_days: 3,
                sufficiency: ReviewSufficiency::Strong,
                stale_days: 0,
                metadata_json: "{}".to_owned(),
                updated_at: "2026-04-08T12:00:00Z".to_owned(),
            },
            ReviewSignalDayRecord {
                signal_key: "activity_score".to_owned(),
                day: "2026-04-08".to_owned(),
                numeric_value: Some(77.0),
                text_value: None,
                baseline_mean: None,
                baseline_stddev: None,
                delta: None,
                z_score: None,
                persistence_days: 0,
                sufficiency: ReviewSufficiency::Missing,
                stale_days: 0,
                metadata_json: "{}".to_owned(),
                updated_at: "2026-04-08T12:00:00Z".to_owned(),
            },
        ];

        let deck = build_review_deck(
            ReviewMode::Today,
            "2026-04-08",
            &ReviewInputs {
                auth_status: &auth_status,
                active_population_profile: PopulationProfile::GeneralAdult,
                signal_days: &signal_days,
                context_events: &[],
                pattern_summaries: &[],
                sleep_time: &[],
                rest_mode_periods: &[],
            },
        )
        .unwrap_or_else(|error| panic!("today review should build: {error}"));

        let readiness_card = ranked_cards(&deck)
            .into_iter()
            .find(|card| card.signal_key == "readiness_score")
            .unwrap_or_else(|| panic!("readiness card should be ranked"));

        assert!(
            readiness_card
                .counterevidence
                .iter()
                .all(|line| !line.contains("stayed near baseline")),
            "siblings without usable z-scores should not produce mixed-signal counterevidence"
        );
    }
}
