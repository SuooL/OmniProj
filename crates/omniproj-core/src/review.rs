//! Deterministic, state-derived review signals for the R0 project index.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};

use crate::ids::{CommitmentTransitionId, WorkItemId};
use crate::project::{ProjectSource, ProjectSourceStatus};
use crate::project_state::{
    CommitmentTransition, CommitmentTransitionKind, ProjectStateDoc, ProjectStatus,
};

pub const REVIEW_RULE_VERSION: &str = "r0-v1";
pub const DEFAULT_COMMITMENT_REVIEW_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReviewReasonCode {
    SourceUnavailable,
    CompleteSetup,
    NeedsCommitment,
    ReviewAction,
    ScheduledReview,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReviewReason {
    pub code: ReviewReasonCode,
    pub label: String,
    pub evidence: Vec<String>,
    pub rule_version: String,
}

struct EffectiveCommitment {
    work_item_id: WorkItemId,
    set_at: String,
    review_at: String,
}

struct EffectiveHistory {
    current: Option<EffectiveCommitment>,
    latest_transition: Option<(CommitmentTransitionKind, String)>,
}

/// Derives R0 review reasons from human state, source availability, and commitment history.
///
/// The function intentionally has no repository-observation inputs: activity, dirty files,
/// cached facts, and `WorkItem::updated_at` cannot change its result.
pub fn derive_review_reasons(
    state: &ProjectStateDoc,
    source: &ProjectSource,
    now: DateTime<Utc>,
    commitment_review_days: i64,
) -> Vec<ReviewReason> {
    let source_unavailable = source.status != ProjectSourceStatus::Available;
    let history = effective_history(&state.commitment_transitions);
    let mut reasons = Vec::new();

    if source_unavailable {
        reasons.push(reason(
            ReviewReasonCode::SourceUnavailable,
            vec![
                format!("source status: {}", source_status_name(source.status)),
                format!(
                    "last successful refresh: {}",
                    source
                        .last_successful_refresh_at
                        .as_deref()
                        .unwrap_or("none recorded")
                ),
                format!(
                    "source error category: {}",
                    source
                        .last_error_category
                        .as_deref()
                        .unwrap_or("none recorded")
                ),
            ],
        ));
    }

    let setup_incomplete = state.status == ProjectStatus::Setup
        && (missing(&state.objective)
            || missing(&state.desired_outcome)
            || state.current_next_action_id.is_none());
    if setup_incomplete && !source_unavailable {
        let mut evidence = Vec::new();
        if missing(&state.objective) {
            evidence.push("missing objective".into());
        }
        if missing(&state.desired_outcome) {
            evidence.push("missing desired outcome".into());
        }
        if state.current_next_action_id.is_none() {
            evidence.push("missing first commitment".into());
        }
        reasons.push(reason(ReviewReasonCode::CompleteSetup, evidence));
    }

    if state.status == ProjectStatus::Active && state.current_next_action_id.is_none() {
        let latest = history
            .latest_transition
            .as_ref()
            .map(|(kind, occurred_at)| {
                format!(
                    "last effective commitment transition: {} at {occurred_at}",
                    transition_name(*kind)
                )
            })
            .unwrap_or_else(|| "no effective commitment transition recorded".into());
        reasons.push(reason(ReviewReasonCode::NeedsCommitment, vec![latest]));
    }

    if state.status == ProjectStatus::Active && !source_unavailable {
        if let Some(current) = history.current {
            if review_due(&current.review_at, now, commitment_review_days) {
                reasons.push(reason(
                    ReviewReasonCode::ReviewAction,
                    vec![
                        format!("review interval: {commitment_review_days} days"),
                        format!("commitment set at: {}", current.set_at),
                        format!("last effective set/confirmation: {}", current.review_at),
                        format!("current commitment: {}", current.work_item_id),
                    ],
                ));
            }
        }
    }

    if matches!(state.status, ProjectStatus::Waiting | ProjectStatus::Parked)
        && !source_unavailable
        && state
            .review_at
            .as_deref()
            .is_some_and(|review_at| timestamp_at_or_before(review_at, now))
    {
        reasons.push(reason(
            ReviewReasonCode::ScheduledReview,
            vec![
                format!(
                    "status reason: {}",
                    state.status_reason.as_deref().unwrap_or("none recorded")
                ),
                format!(
                    "review date: {}",
                    state.review_at.as_deref().unwrap_or_default()
                ),
            ],
        ));
    }

    reasons
}

fn effective_history(transitions: &[CommitmentTransition]) -> EffectiveHistory {
    let corrected: HashSet<CommitmentTransitionId> = transitions
        .iter()
        .filter(|transition| transition.kind == CommitmentTransitionKind::Correction)
        .filter_map(|transition| transition.corrects_transition_id.clone())
        .collect();
    let mut current: Option<EffectiveCommitment> = None;
    let mut latest_transition = None;

    for transition in transitions {
        if transition.kind == CommitmentTransitionKind::Correction
            || corrected.contains(&transition.id)
        {
            continue;
        }
        match transition.kind {
            CommitmentTransitionKind::Set => {
                if let Some(work_item_id) = transition.next_work_item_id.clone() {
                    current = Some(EffectiveCommitment {
                        work_item_id,
                        set_at: transition.occurred_at.clone(),
                        review_at: transition.occurred_at.clone(),
                    });
                    latest_transition = Some((transition.kind, transition.occurred_at.clone()));
                }
            }
            CommitmentTransitionKind::Confirmed => {
                if current.as_ref().is_some_and(|current| {
                    Some(&current.work_item_id) == transition.previous_work_item_id.as_ref()
                }) {
                    if let Some(current) = current.as_mut() {
                        current.review_at = transition.occurred_at.clone();
                    }
                    latest_transition = Some((transition.kind, transition.occurred_at.clone()));
                }
            }
            CommitmentTransitionKind::Completed | CommitmentTransitionKind::Cleared => {
                if current.as_ref().is_some_and(|current| {
                    Some(&current.work_item_id) == transition.previous_work_item_id.as_ref()
                }) {
                    current = None;
                    latest_transition = Some((transition.kind, transition.occurred_at.clone()));
                }
            }
            CommitmentTransitionKind::Replaced => {
                if current.as_ref().is_some_and(|current| {
                    Some(&current.work_item_id) == transition.previous_work_item_id.as_ref()
                }) {
                    if let Some(work_item_id) = transition.next_work_item_id.clone() {
                        current = Some(EffectiveCommitment {
                            work_item_id,
                            set_at: transition.occurred_at.clone(),
                            review_at: transition.occurred_at.clone(),
                        });
                        latest_transition = Some((transition.kind, transition.occurred_at.clone()));
                    }
                }
            }
            CommitmentTransitionKind::Correction => unreachable!("corrections are filtered"),
        }
    }

    EffectiveHistory {
        current,
        latest_transition,
    }
}

fn review_due(review_at: &str, now: DateTime<Utc>, days: i64) -> bool {
    let Some(review_at) = parse_timestamp(review_at) else {
        return false;
    };
    now.signed_duration_since(review_at) >= Duration::days(days)
}

fn timestamp_at_or_before(value: &str, now: DateTime<Utc>) -> bool {
    parse_timestamp(value).is_some_and(|timestamp| timestamp <= now)
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

fn missing(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|value| value.trim().is_empty())
}

fn reason(code: ReviewReasonCode, evidence: Vec<String>) -> ReviewReason {
    ReviewReason {
        label: match code {
            ReviewReasonCode::SourceUnavailable => "Source unavailable",
            ReviewReasonCode::CompleteSetup => "Complete setup",
            ReviewReasonCode::NeedsCommitment => "Needs commitment",
            ReviewReasonCode::ReviewAction => "Review action",
            ReviewReasonCode::ScheduledReview => "Scheduled review",
        }
        .into(),
        code,
        evidence,
        rule_version: REVIEW_RULE_VERSION.into(),
    }
}

fn source_status_name(status: ProjectSourceStatus) -> &'static str {
    match status {
        ProjectSourceStatus::Available => "available",
        ProjectSourceStatus::Moved => "moved",
        ProjectSourceStatus::Unreadable => "unreadable",
        ProjectSourceStatus::Missing => "missing",
    }
}

fn transition_name(kind: CommitmentTransitionKind) -> &'static str {
    match kind {
        CommitmentTransitionKind::Set => "set",
        CommitmentTransitionKind::Confirmed => "confirmed",
        CommitmentTransitionKind::Completed => "completed",
        CommitmentTransitionKind::Replaced => "replaced",
        CommitmentTransitionKind::Cleared => "cleared",
        CommitmentTransitionKind::Correction => "correction",
    }
}
