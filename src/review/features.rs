use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::{Result, RingmasterError};
use crate::review::registry::{SignalDirectionality, signal_definitions};
use crate::store::queries::{
    DailyActivityRecord, DailyCardiovascularAgeRecord, DailyOverviewRow, DailyReadinessRecord,
    DailyResilienceRecord, DailyStressRecord, RestModePeriodRecord, ReviewSignalDayRecord,
    SleepTimeRecord, Vo2MaxRecord,
};

const COMPARABLE_MEDIUM_DAYS: usize = 7;
const COMPARABLE_STRONG_DAYS: usize = 14;
const MIN_STDDEV: f64 = 0.01;
const PERSISTENCE_Z_THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewSufficiency {
    Missing,
    Thin,
    Medium,
    Strong,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureInputs<'a> {
    pub daily_history: &'a [DailyOverviewRow],
    pub daily_activity: &'a [DailyActivityRecord],
    pub daily_readiness: &'a [DailyReadinessRecord],
    pub daily_stress: &'a [DailyStressRecord],
    pub daily_resilience: &'a [DailyResilienceRecord],
    pub daily_cardiovascular_age: &'a [DailyCardiovascularAgeRecord],
    pub vo2_max: &'a [Vo2MaxRecord],
    pub sleep_time: &'a [SleepTimeRecord],
    pub rest_mode_periods: &'a [RestModePeriodRecord],
    pub captured_at: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
struct SeedPoint {
    day: String,
    numeric_value: Option<f64>,
    text_value: Option<String>,
    metadata_json: String,
}

impl ReviewSufficiency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Thin => "thin",
            Self::Medium => "medium",
            Self::Strong => "strong",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Missing => "Missing",
            Self::Thin => "Thin",
            Self::Medium => "Medium",
            Self::Strong => "Strong",
        }
    }

    pub fn from_comparable_days(comparable_days: usize) -> Self {
        if comparable_days == 0 {
            Self::Missing
        } else if comparable_days < COMPARABLE_MEDIUM_DAYS {
            Self::Thin
        } else if comparable_days < COMPARABLE_STRONG_DAYS {
            Self::Medium
        } else {
            Self::Strong
        }
    }
}

pub fn build_review_signal_days(inputs: &FeatureInputs<'_>) -> Result<Vec<ReviewSignalDayRecord>> {
    let captured_day = parse_day(&captured_day(inputs.captured_at)?)?;
    let mut series: BTreeMap<&'static str, Vec<SeedPoint>> = BTreeMap::new();

    for row in inputs.daily_history {
        insert_numeric_seed(
            &mut series,
            "sleep_score",
            &row.day,
            row.sleep_score.map(f64::from),
            json!({ "source_family": "daily_sleep" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "readiness_score",
            &row.day,
            row.readiness_score.map(f64::from),
            json!({ "source_family": "daily_readiness" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "activity_score",
            &row.day,
            row.activity_score.map(f64::from),
            json!({ "source_family": "daily_activity" }),
        )?;
    }

    for row in inputs.daily_activity {
        insert_numeric_seed(
            &mut series,
            "active_calories",
            &row.day,
            Some(row.active_calories as f64),
            json!({ "source_family": "daily_activity" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "steps",
            &row.day,
            Some(row.steps as f64),
            json!({ "source_family": "daily_activity" }),
        )?;
    }

    for row in inputs.daily_readiness {
        insert_numeric_seed(
            &mut series,
            "temperature_deviation",
            &row.day,
            row.temperature_deviation,
            json!({
                "source_family": "daily_readiness",
                "temperature_trend_deviation": row.temperature_trend_deviation,
            }),
        )?;
    }

    for row in inputs.daily_stress {
        insert_numeric_seed(
            &mut series,
            "stress_high",
            &row.day,
            row.stress_high.map(|value| value as f64),
            json!({
                "source_family": "daily_stress",
                "day_summary": row.day_summary,
            }),
        )?;
        insert_numeric_seed(
            &mut series,
            "recovery_high",
            &row.day,
            row.recovery_high.map(|value| value as f64),
            json!({
                "source_family": "daily_stress",
                "day_summary": row.day_summary,
            }),
        )?;
    }

    for row in inputs.daily_resilience {
        insert_numeric_seed(
            &mut series,
            "resilience_level",
            &row.day,
            Some(f64::from(resilience_level_score(&row.level))),
            json!({
                "source_family": "daily_resilience",
                "level": row.level,
            }),
        )?;
        insert_numeric_seed(
            &mut series,
            "sleep_recovery",
            &row.day,
            Some(row.sleep_recovery),
            json!({ "source_family": "daily_resilience" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "daytime_recovery",
            &row.day,
            Some(row.daytime_recovery),
            json!({ "source_family": "daily_resilience" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "resilience_stress",
            &row.day,
            Some(row.stress),
            json!({ "source_family": "daily_resilience" }),
        )?;
    }

    for row in inputs.daily_cardiovascular_age {
        insert_numeric_seed(
            &mut series,
            "cardiovascular_age",
            &row.day,
            row.vascular_age.map(|value| value as f64),
            json!({ "source_family": "daily_cardiovascular_age" }),
        )?;
    }

    for row in inputs.vo2_max {
        insert_numeric_seed(
            &mut series,
            "vo2_max",
            &row.day,
            row.vo2_max,
            json!({
                "source_family": "vo2_max",
                "recorded_at": row.recorded_at,
            }),
        )?;
    }

    for row in inputs.sleep_time {
        insert_text_seed(
            &mut series,
            "sleep_time_status",
            &row.day,
            row.status.clone(),
            json!({
                "source_family": "sleep_time",
                "recommendation": row.recommendation,
                "optimal_bedtime_start_offset": row.optimal_bedtime_start_offset,
                "optimal_bedtime_end_offset": row.optimal_bedtime_end_offset,
                "optimal_bedtime_day_tz": row.optimal_bedtime_day_tz,
            }),
        )?;
    }

    let rest_days = expand_rest_mode_days(inputs.rest_mode_periods)?;
    for day in rest_days {
        insert_numeric_seed(
            &mut series,
            "rest_mode_active",
            &day,
            Some(1.0),
            json!({ "source_family": "rest_mode_period" }),
        )?;
    }

    let mut rows = Vec::new();
    for definition in signal_definitions() {
        let Some(seed_points) = series.get(definition.key) else {
            continue;
        };
        let numeric_series = numeric_series(seed_points);

        for seed in seed_points {
            let day = parse_day(&seed.day)?;
            let stale_days = (captured_day - day).whole_days().max(0);
            let comparable_values =
                prior_numeric_values(&numeric_series, &seed.day, definition.baseline_window_days);
            let sufficiency = ReviewSufficiency::from_comparable_days(comparable_values.len());
            let baseline_mean = mean(&comparable_values);
            let baseline_stddev = standard_deviation(&comparable_values);
            let delta = match (seed.numeric_value, baseline_mean) {
                (Some(numeric_value), Some(mean_value)) => Some(numeric_value - mean_value),
                _ => None,
            };
            let z_score = match (delta, baseline_stddev) {
                (Some(delta_value), Some(stddev)) if stddev >= MIN_STDDEV => {
                    Some(delta_value / stddev)
                }
                _ => None,
            };
            let persistence_days = persistence_days(
                &numeric_series,
                &seed.day,
                z_score,
                definition.directionality,
                definition.baseline_window_days,
            )?;
            let metadata_json = serde_json::to_string(&json!({
                "comparable_days": comparable_values.len(),
                "source_family": definition.family,
                "seed_metadata": serde_json::from_str::<serde_json::Value>(&seed.metadata_json)
                    .unwrap_or_else(|_| json!({})),
                "wording_constraint": definition.wording_constraint,
            }))?;

            rows.push(ReviewSignalDayRecord {
                signal_key: definition.key.to_owned(),
                day: seed.day.clone(),
                numeric_value: seed.numeric_value,
                text_value: seed.text_value.clone(),
                baseline_mean,
                baseline_stddev,
                delta,
                z_score,
                persistence_days,
                sufficiency,
                stale_days: u32::try_from(stale_days).map_err(|error| {
                    RingmasterError::Config(format!(
                        "stale_days overflowed u32 for {} {}: {error}",
                        definition.key, seed.day
                    ))
                })?,
                metadata_json,
                updated_at: inputs.captured_at.to_owned(),
            });
        }
    }

    rows.sort_by(|left, right| {
        left.day
            .cmp(&right.day)
            .then(left.signal_key.cmp(&right.signal_key))
    });
    Ok(rows)
}

fn insert_numeric_seed(
    series: &mut BTreeMap<&'static str, Vec<SeedPoint>>,
    signal_key: &'static str,
    day: &str,
    numeric_value: Option<f64>,
    metadata: serde_json::Value,
) -> Result<()> {
    let seed_point = SeedPoint {
        day: day.to_owned(),
        numeric_value,
        text_value: None,
        metadata_json: serde_json::to_string(&metadata)?,
    };
    upsert_seed_point(series.entry(signal_key).or_default(), seed_point);
    Ok(())
}

fn insert_text_seed(
    series: &mut BTreeMap<&'static str, Vec<SeedPoint>>,
    signal_key: &'static str,
    day: &str,
    text_value: Option<String>,
    metadata: serde_json::Value,
) -> Result<()> {
    let seed_point = SeedPoint {
        day: day.to_owned(),
        numeric_value: None,
        text_value,
        metadata_json: serde_json::to_string(&metadata)?,
    };
    upsert_seed_point(series.entry(signal_key).or_default(), seed_point);
    Ok(())
}

fn upsert_seed_point(seed_points: &mut Vec<SeedPoint>, seed_point: SeedPoint) {
    if let Some(existing) = seed_points
        .iter_mut()
        .find(|existing| existing.day == seed_point.day)
    {
        *existing = seed_point;
    } else {
        seed_points.push(seed_point);
    }
}

fn numeric_series(seed_points: &[SeedPoint]) -> Vec<(String, f64)> {
    seed_points
        .iter()
        .filter_map(|seed| seed.numeric_value.map(|value| (seed.day.clone(), value)))
        .collect()
}

fn prior_numeric_values(
    numeric_series: &[(String, f64)],
    day: &str,
    baseline_window_days: usize,
) -> Vec<f64> {
    let mut values = numeric_series
        .iter()
        .filter(|(series_day, _)| series_day.as_str() < day)
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();

    if values.len() > baseline_window_days {
        let drain_count = values.len().saturating_sub(baseline_window_days);
        values.drain(..drain_count);
    }

    values
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn standard_deviation(values: &[f64]) -> Option<f64> {
    let mean_value = mean(values)?;
    let variance = values
        .iter()
        .map(|value| (*value - mean_value).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    Some(variance.sqrt())
}

fn persistence_days(
    numeric_series: &[(String, f64)],
    anchor_day: &str,
    z_score: Option<f64>,
    directionality: SignalDirectionality,
    baseline_window_days: usize,
) -> Result<u32> {
    let Some(anchor_sign) = signal_sign(z_score, directionality) else {
        return Ok(0);
    };
    let mut streak = 0_u32;
    let mut current_day = parse_day(anchor_day)?;

    loop {
        let day_label = current_day.to_string();
        let Some((_, value)) = numeric_series
            .iter()
            .find(|(series_day, _)| series_day == &day_label)
        else {
            break;
        };
        let comparable_values =
            prior_numeric_values(numeric_series, &day_label, baseline_window_days);
        let comparable_mean = mean(&comparable_values);
        let comparable_stddev = standard_deviation(&comparable_values);
        let day_z_score = match (comparable_mean, comparable_stddev) {
            (Some(mean_value), Some(stddev)) if stddev >= MIN_STDDEV => {
                Some((*value - mean_value) / stddev)
            }
            _ => None,
        };
        let Some(day_sign) = signal_sign(day_z_score, directionality) else {
            break;
        };
        if day_sign != anchor_sign {
            break;
        }

        streak = streak.saturating_add(1);
        current_day = current_day.previous_day().ok_or_else(|| {
            RingmasterError::Config(
                "review persistence walked before supported date range".to_owned(),
            )
        })?;
    }

    Ok(streak)
}

fn signal_sign(z_score: Option<f64>, directionality: SignalDirectionality) -> Option<i8> {
    let z_score = z_score?;
    if z_score.abs() < PERSISTENCE_Z_THRESHOLD {
        return None;
    }

    match directionality {
        SignalDirectionality::HigherBetter => Some(if z_score.is_sign_positive() { 1 } else { -1 }),
        SignalDirectionality::LowerBetter => Some(if z_score.is_sign_positive() { -1 } else { 1 }),
        SignalDirectionality::Neutral | SignalDirectionality::Contextual => {
            Some(if z_score.is_sign_positive() { 1 } else { -1 })
        }
    }
}

fn resilience_level_score(level: &str) -> u8 {
    match level {
        "limited" => 1,
        "adequate" => 2,
        "solid" => 3,
        "strong" => 4,
        "exceptional" => 5,
        _ => 0,
    }
}

fn expand_rest_mode_days(rest_mode_periods: &[RestModePeriodRecord]) -> Result<Vec<String>> {
    let mut days = BTreeSet::new();

    for period in rest_mode_periods {
        let start_day = parse_day(&period.start_day)?;
        let end_day = period
            .end_day
            .as_deref()
            .map(parse_day)
            .transpose()?
            .unwrap_or(start_day);
        let mut current_day = start_day;
        loop {
            days.insert(current_day.to_string());
            if current_day >= end_day {
                break;
            }
            current_day = current_day.next_day().ok_or_else(|| {
                RingmasterError::Config(
                    "rest mode expansion exceeded supported date range".to_owned(),
                )
            })?;
        }
    }

    Ok(days.into_iter().collect())
}

fn captured_day(timestamp: &str) -> Result<String> {
    let parsed = OffsetDateTime::parse(timestamp, &Rfc3339).map_err(|error| {
        RingmasterError::Config(format!(
            "failed to parse captured_at timestamp `{timestamp}` for review features: {error}"
        ))
    })?;
    Ok(parsed.date().to_string())
}

fn parse_day(day: &str) -> Result<Date> {
    Date::parse(
        day,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| {
        RingmasterError::Config(format!(
            "failed to parse day `{day}` for review features: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        FeatureInputs, ReviewSufficiency, build_review_signal_days, persistence_days, signal_sign,
    };
    use crate::review::registry::SignalDirectionality;
    use crate::review::registry::signal_definition;
    use crate::store::queries::{
        DailyActivityRecord, DailyCardiovascularAgeRecord, DailyOverviewRow, DailyReadinessRecord,
        DailyResilienceRecord, DailyStressRecord, RestModePeriodRecord, SleepTimeRecord,
        Vo2MaxRecord,
    };

    #[test]
    fn sufficiency_bucket_thresholds_are_explicit() {
        assert_eq!(
            ReviewSufficiency::from_comparable_days(0),
            ReviewSufficiency::Missing
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_days(3),
            ReviewSufficiency::Thin
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_days(8),
            ReviewSufficiency::Medium
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_days(20),
            ReviewSufficiency::Strong
        );
    }

    #[test]
    fn feature_builder_emits_phase5_signal_rows() {
        let daily_history = vec![
            DailyOverviewRow {
                day: "2026-04-01".to_owned(),
                sleep_score: Some(80),
                readiness_score: Some(82),
                activity_score: Some(79),
                updated_at: "2026-04-01T08:00:00Z".to_owned(),
            },
            DailyOverviewRow {
                day: "2026-04-02".to_owned(),
                sleep_score: Some(74),
                readiness_score: Some(76),
                activity_score: Some(70),
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            },
        ];

        let rows = build_review_signal_days(&FeatureInputs {
            daily_history: &daily_history,
            daily_activity: &[DailyActivityRecord {
                oura_id: Some("act-1".to_owned()),
                day: "2026-04-02".to_owned(),
                activity_score: Some(70),
                active_calories: 400,
                steps: 5000,
                total_calories: 2200,
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            daily_readiness: &[DailyReadinessRecord {
                oura_id: Some("ready-1".to_owned()),
                day: "2026-04-02".to_owned(),
                readiness_score: Some(76),
                temperature_deviation: Some(0.4),
                temperature_trend_deviation: Some(0.2),
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            daily_stress: &[DailyStressRecord {
                oura_id: Some("stress-1".to_owned()),
                day: "2026-04-02".to_owned(),
                stress_high: Some(180),
                recovery_high: Some(40),
                day_summary: Some("stressed".to_owned()),
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            daily_resilience: &[DailyResilienceRecord {
                oura_id: Some("res-1".to_owned()),
                day: "2026-04-02".to_owned(),
                level: "solid".to_owned(),
                sleep_recovery: 78.0,
                daytime_recovery: 64.0,
                stress: 55.0,
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            daily_cardiovascular_age: &[DailyCardiovascularAgeRecord {
                day: "2026-04-02".to_owned(),
                vascular_age: Some(37),
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            vo2_max: &[Vo2MaxRecord {
                oura_id: Some("vo2-1".to_owned()),
                day: "2026-04-02".to_owned(),
                recorded_at: "2026-04-02T08:00:00Z".to_owned(),
                vo2_max: Some(42.5),
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            sleep_time: &[SleepTimeRecord {
                oura_id: Some("sleep-time-1".to_owned()),
                day: "2026-04-02".to_owned(),
                status: Some("optimal_found".to_owned()),
                recommendation: Some("follow_optimal_bedtime".to_owned()),
                optimal_bedtime_start_offset: Some(79200),
                optimal_bedtime_end_offset: Some(82800),
                optimal_bedtime_day_tz: Some(0),
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            rest_mode_periods: &[RestModePeriodRecord {
                period_id: "rest-1".to_owned(),
                start_day: "2026-04-02".to_owned(),
                start_time: Some("2026-04-02T00:00:00Z".to_owned()),
                end_day: Some("2026-04-03".to_owned()),
                end_time: Some("2026-04-03T12:00:00Z".to_owned()),
                episode_count: 1,
                tags_json: "[]".to_owned(),
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            captured_at: "2026-04-03T10:00:00Z",
        })
        .unwrap_or_else(|error| panic!("feature build should succeed: {error}"));

        for key in [
            "sleep_score",
            "readiness_score",
            "stress_high",
            "resilience_level",
            "cardiovascular_age",
            "vo2_max",
            "sleep_time_status",
            "rest_mode_active",
        ] {
            assert!(
                rows.iter().any(|row| row.signal_key == key),
                "expected derived review signal row for {key}"
            );
            assert!(
                signal_definition(key).is_some(),
                "{key} should exist in the registry"
            );
        }
    }

    #[test]
    fn feature_builder_keeps_latest_vo2_max_per_day() {
        let rows = build_review_signal_days(&FeatureInputs {
            daily_history: &[],
            daily_activity: &[],
            daily_readiness: &[],
            daily_stress: &[],
            daily_resilience: &[],
            daily_cardiovascular_age: &[],
            vo2_max: &[
                Vo2MaxRecord {
                    oura_id: Some("vo2-1".to_owned()),
                    day: "2026-04-02".to_owned(),
                    recorded_at: "2026-04-02T08:00:00Z".to_owned(),
                    vo2_max: Some(41.5),
                    raw_cache_key: None,
                    updated_at: "2026-04-02T08:00:00Z".to_owned(),
                },
                Vo2MaxRecord {
                    oura_id: Some("vo2-2".to_owned()),
                    day: "2026-04-02".to_owned(),
                    recorded_at: "2026-04-02T12:00:00Z".to_owned(),
                    vo2_max: Some(42.0),
                    raw_cache_key: None,
                    updated_at: "2026-04-02T12:00:00Z".to_owned(),
                },
            ],
            sleep_time: &[],
            rest_mode_periods: &[],
            captured_at: "2026-04-03T10:00:00Z",
        })
        .unwrap_or_else(|error| panic!("feature build should succeed: {error}"));

        let vo2_rows = rows
            .iter()
            .filter(|row| row.signal_key == "vo2_max" && row.day == "2026-04-02")
            .collect::<Vec<_>>();

        assert_eq!(vo2_rows.len(), 1);
        assert_eq!(vo2_rows[0].numeric_value, Some(42.0));
    }

    #[test]
    fn persistence_uses_signal_baseline_window() {
        let start_day = time::Date::from_calendar_date(2026, time::Month::March, 1)
            .unwrap_or_else(|error| panic!("test start day should be valid: {error}"));
        let numeric_series = (0_i64..35_i64)
            .map(|offset| {
                let day = start_day
                    .checked_add(time::Duration::days(offset))
                    .unwrap_or_else(|| panic!("test day should stay in range"));
                let value = match offset {
                    0..=15 if offset % 2 == 0 => 50.0,
                    0..=15 => 150.0,
                    16..=30 => 100.0,
                    _ => 110.0,
                };
                (day.to_string(), value)
            })
            .collect::<Vec<_>>();

        let z_score = Some(1.0);
        assert_eq!(
            signal_sign(z_score, SignalDirectionality::HigherBetter),
            Some(1)
        );

        let short_window = persistence_days(
            &numeric_series,
            "2026-04-04",
            z_score,
            SignalDirectionality::HigherBetter,
            14,
        )
        .unwrap_or_else(|error| panic!("short baseline persistence should build: {error}"));
        let long_window = persistence_days(
            &numeric_series,
            "2026-04-04",
            z_score,
            SignalDirectionality::HigherBetter,
            30,
        )
        .unwrap_or_else(|error| panic!("long baseline persistence should build: {error}"));

        assert!(short_window >= 3);
        assert_eq!(long_window, 0);
    }
}
