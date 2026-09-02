use chrono::{DateTime, Utc};
use omniproj_core::{
    derive_review_reasons, CommitmentTransition, CommitmentTransitionId, CommitmentTransitionKind,
    ProjectId, ProjectSource, ProjectSourceId, ProjectSourceKind, ProjectSourceStatus,
    ProjectStateDoc, ProjectStatus, ReviewReasonCode, WorkItem, WorkItemId, WorkItemStatus,
    DEFAULT_COMMITMENT_REVIEW_DAYS, REVIEW_RULE_VERSION,
};

const CREATED_AT: &str = "2026-08-01T00:00:00Z";

fn at(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}

fn project_id() -> ProjectId {
    ProjectId::parse("project-review").unwrap()
}

fn source(status: ProjectSourceStatus) -> ProjectSource {
    ProjectSource {
        id: ProjectSourceId::parse("source-review").unwrap(),
        project_id: project_id(),
        kind: ProjectSourceKind::GitRepo,
        location: "/projects/review".into(),
        is_primary: true,
        status,
        created_at: CREATED_AT.into(),
        last_observed_at: Some("2026-08-10T00:00:00Z".into()),
        last_successful_refresh_at: Some("2026-08-09T00:00:00Z".into()),
        last_error_category: Some("permission_denied".into()),
        revision: 1,
    }
}

fn state(status: ProjectStatus) -> ProjectStateDoc {
    let mut state = ProjectStateDoc::new_setup(CREATED_AT).unwrap();
    state.status = status;
    state.status_changed_at = CREATED_AT.into();
    state.updated_at = "2026-08-10T00:00:00Z".into();
    state
}

fn work_item_id(value: &str) -> WorkItemId {
    WorkItemId::parse(value).unwrap()
}

fn transition_id(value: &str) -> CommitmentTransitionId {
    CommitmentTransitionId::parse(value).unwrap()
}

fn transition(
    id: &str,
    kind: CommitmentTransitionKind,
    occurred_at: &str,
    previous_work_item_id: Option<WorkItemId>,
    next_work_item_id: Option<WorkItemId>,
    corrects_transition_id: Option<CommitmentTransitionId>,
) -> CommitmentTransition {
    CommitmentTransition {
        id: transition_id(id),
        project_id: project_id(),
        document_revision: 1,
        kind,
        previous_work_item_id,
        next_work_item_id,
        reason: None,
        occurred_at: occurred_at.into(),
        corrects_transition_id,
    }
}

fn codes(state: &ProjectStateDoc, source: &ProjectSource, now: &str) -> Vec<ReviewReasonCode> {
    derive_review_reasons(
        state,
        source,
        at(now),
        at(now).date_naive(),
        DEFAULT_COMMITMENT_REVIEW_DAYS,
    )
    .into_iter()
    .map(|reason| reason.code)
    .collect()
}

#[test]
fn source_failure_is_first_and_suppresses_setup_review_and_scheduled_reasons() {
    let mut state = state(ProjectStatus::Setup);
    state.review_at = Some("2026-08-01T00:00:00Z".into());
    let unavailable = source(ProjectSourceStatus::Unreadable);

    let reasons = derive_review_reasons(
        &state,
        &unavailable,
        at("2026-08-10T00:00:00Z"),
        at("2026-08-10T00:00:00Z").date_naive(),
        DEFAULT_COMMITMENT_REVIEW_DAYS,
    );

    assert_eq!(
        reasons
            .iter()
            .map(|reason| &reason.code)
            .collect::<Vec<_>>(),
        vec![&ReviewReasonCode::SourceUnavailable]
    );
    assert_eq!(reasons[0].label, "Source unavailable");
    assert_eq!(reasons[0].rule_version, REVIEW_RULE_VERSION);
    assert!(reasons[0]
        .evidence
        .iter()
        .any(|evidence| evidence.contains("permission_denied")));
}

#[test]
fn setup_requires_framing_or_first_commitment_but_not_when_complete() {
    let source = source(ProjectSourceStatus::Available);
    let mut incomplete = state(ProjectStatus::Setup);
    incomplete.objective = Some("Improve review quality".into());
    incomplete.desired_outcome = Some("A reliable project loop".into());
    assert_eq!(
        codes(&incomplete, &source, "2026-08-10T00:00:00Z"),
        vec![ReviewReasonCode::CompleteSetup]
    );

    incomplete.current_next_action_id = Some(work_item_id("work-setup"));
    assert!(codes(&incomplete, &source, "2026-08-10T00:00:00Z").is_empty());
}

#[test]
fn needs_commitment_is_active_only_and_remains_visible_with_source_failure() {
    let active = state(ProjectStatus::Active);
    let unavailable = source(ProjectSourceStatus::Missing);
    assert_eq!(
        codes(&active, &unavailable, "2026-08-10T00:00:00Z"),
        vec![
            ReviewReasonCode::SourceUnavailable,
            ReviewReasonCode::NeedsCommitment,
        ]
    );

    assert_eq!(
        codes(
            &state(ProjectStatus::Setup),
            &source(ProjectSourceStatus::Available),
            "2026-08-10T00:00:00Z",
        ),
        vec![ReviewReasonCode::CompleteSetup]
    );
    for status in [
        ProjectStatus::Waiting,
        ProjectStatus::Parked,
        ProjectStatus::Archived,
    ] {
        assert!(codes(
            &state(status),
            &source(ProjectSourceStatus::Available),
            "2026-08-10T00:00:00Z"
        )
        .is_empty());
    }
}

#[test]
fn review_action_starts_at_exactly_seven_days_and_not_one_second_earlier() {
    let item = work_item_id("work-review");
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(item.clone());
    state.commitment_transitions.push(transition(
        "transition-set-review",
        CommitmentTransitionKind::Set,
        "2026-08-01T00:00:00Z",
        None,
        Some(item),
        None,
    ));
    let available = source(ProjectSourceStatus::Available);

    assert!(codes(&state, &available, "2026-08-07T23:59:59Z").is_empty());
    assert_eq!(
        codes(&state, &available, "2026-08-08T00:00:00Z"),
        vec![ReviewReasonCode::ReviewAction]
    );
}

#[test]
fn confirmation_resets_review_age_without_replacing_the_original_set_time() {
    let item = work_item_id("work-confirm");
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(item.clone());
    state.commitment_transitions = vec![
        transition(
            "transition-set-confirm",
            CommitmentTransitionKind::Set,
            "2026-08-01T00:00:00Z",
            None,
            Some(item.clone()),
            None,
        ),
        transition(
            "transition-confirm",
            CommitmentTransitionKind::Confirmed,
            "2026-08-05T00:00:00Z",
            Some(item.clone()),
            Some(item),
            None,
        ),
    ];
    let available = source(ProjectSourceStatus::Available);

    assert!(codes(&state, &available, "2026-08-11T23:59:59Z").is_empty());
    let reasons = derive_review_reasons(
        &state,
        &available,
        at("2026-08-12T00:00:00Z"),
        at("2026-08-12T00:00:00Z").date_naive(),
        DEFAULT_COMMITMENT_REVIEW_DAYS,
    );
    assert_eq!(reasons[0].code, ReviewReasonCode::ReviewAction);
    assert!(reasons[0]
        .evidence
        .iter()
        .any(|evidence| evidence.contains("2026-08-01T00:00:00Z")));
    assert!(reasons[0]
        .evidence
        .iter()
        .any(|evidence| evidence.contains("2026-08-05T00:00:00Z")));
}

#[test]
fn replacement_starts_a_new_commitment_clock() {
    let original = work_item_id("work-original");
    let replacement = work_item_id("work-replacement");
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(replacement.clone());
    state.commitment_transitions = vec![
        transition(
            "transition-set-original",
            CommitmentTransitionKind::Set,
            "2026-08-01T00:00:00Z",
            None,
            Some(original.clone()),
            None,
        ),
        transition(
            "transition-replace",
            CommitmentTransitionKind::Replaced,
            "2026-08-09T00:00:00Z",
            Some(original),
            Some(replacement),
            None,
        ),
    ];
    let available = source(ProjectSourceStatus::Available);

    assert!(codes(&state, &available, "2026-08-15T23:59:59Z").is_empty());
    assert_eq!(
        codes(&state, &available, "2026-08-16T00:00:00Z"),
        vec![ReviewReasonCode::ReviewAction]
    );
}

#[test]
fn corrections_mask_transition_effects_and_never_become_review_anchors() {
    let original = work_item_id("work-correction-original");
    let replacement = work_item_id("work-correction-replacement");
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(original.clone());
    state.commitment_transitions = vec![
        transition(
            "transition-correction-set",
            CommitmentTransitionKind::Set,
            "2026-08-01T00:00:00Z",
            None,
            Some(original.clone()),
            None,
        ),
        transition(
            "transition-correction-replace",
            CommitmentTransitionKind::Replaced,
            "2026-08-09T00:00:00Z",
            Some(original.clone()),
            Some(replacement),
            None,
        ),
        transition(
            "transition-correction-undo-replace",
            CommitmentTransitionKind::Correction,
            "2026-08-10T00:00:00Z",
            Some(work_item_id("work-correction-replacement")),
            Some(original),
            Some(transition_id("transition-correction-replace")),
        ),
    ];
    let available = source(ProjectSourceStatus::Available);

    assert_eq!(
        codes(&state, &available, "2026-08-08T00:00:00Z"),
        vec![ReviewReasonCode::ReviewAction],
        "the corrected replacement must not provide a newer clock"
    );
}

#[test]
fn corrected_confirmation_does_not_reset_the_review_age() {
    let item = work_item_id("work-corrected-confirm");
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(item.clone());
    state.commitment_transitions = vec![
        transition(
            "transition-set-corrected-confirm",
            CommitmentTransitionKind::Set,
            "2026-08-01T00:00:00Z",
            None,
            Some(item.clone()),
            None,
        ),
        transition(
            "transition-confirm-corrected",
            CommitmentTransitionKind::Confirmed,
            "2026-08-09T00:00:00Z",
            Some(item.clone()),
            Some(item.clone()),
            None,
        ),
        transition(
            "transition-undo-confirm",
            CommitmentTransitionKind::Correction,
            "2026-08-10T00:00:00Z",
            Some(item.clone()),
            Some(item),
            Some(transition_id("transition-confirm-corrected")),
        ),
    ];

    assert_eq!(
        codes(
            &state,
            &source(ProjectSourceStatus::Available),
            "2026-08-10T00:00:00Z",
        ),
        vec![ReviewReasonCode::ReviewAction]
    );
}

#[test]
fn undoing_complete_replace_and_clear_restores_the_prior_pointer_and_clock() {
    let available = source(ProjectSourceStatus::Available);
    for (target_kind, target_id, after_pointer) in [
        (
            CommitmentTransitionKind::Completed,
            "transition-undo-complete",
            Some(work_item_id("work-undo")),
        ),
        (
            CommitmentTransitionKind::Replaced,
            "transition-undo-replace",
            Some(work_item_id("work-undo")),
        ),
        (
            CommitmentTransitionKind::Cleared,
            "transition-undo-clear",
            Some(work_item_id("work-undo")),
        ),
    ] {
        let original = work_item_id("work-undo");
        let replacement = work_item_id("work-undo-replacement");
        let mut state = state(ProjectStatus::Active);
        state.current_next_action_id = after_pointer.clone();
        let (previous, next) = match target_kind {
            CommitmentTransitionKind::Completed | CommitmentTransitionKind::Cleared => {
                (Some(original.clone()), None)
            }
            CommitmentTransitionKind::Replaced => {
                (Some(original.clone()), Some(replacement.clone()))
            }
            _ => unreachable!(),
        };
        state.commitment_transitions = vec![
            transition(
                "transition-set-undo",
                CommitmentTransitionKind::Set,
                "2026-08-01T00:00:00Z",
                None,
                Some(original.clone()),
                None,
            ),
            transition(
                target_id,
                target_kind,
                "2026-08-09T00:00:00Z",
                previous,
                next,
                None,
            ),
            transition(
                &format!("correction-{target_id}"),
                CommitmentTransitionKind::Correction,
                "2026-08-10T00:00:00Z",
                match target_kind {
                    CommitmentTransitionKind::Replaced => Some(replacement),
                    _ => None,
                },
                after_pointer,
                Some(transition_id(target_id)),
            ),
        ];
        assert_eq!(
            codes(&state, &available, "2026-08-08T00:00:00Z"),
            vec![ReviewReasonCode::ReviewAction],
            "corrected {target_kind:?} must restore the August 1 clock"
        );
    }
}

#[test]
fn scheduled_review_is_due_at_its_exact_time_and_suppressed_for_archived_or_source_failure() {
    let mut waiting = state(ProjectStatus::Waiting);
    waiting.status_reason = Some("Waiting on ethics review".into());
    waiting.review_at = Some("2026-08-10T00:00:00Z".into());
    let available = source(ProjectSourceStatus::Available);

    assert!(codes(&waiting, &available, "2026-08-09T23:59:59Z").is_empty());
    assert_eq!(
        codes(&waiting, &available, "2026-08-10T00:00:00Z"),
        vec![ReviewReasonCode::ScheduledReview]
    );
    assert_eq!(
        codes(
            &waiting,
            &source(ProjectSourceStatus::Moved),
            "2026-08-10T00:00:00Z"
        ),
        vec![ReviewReasonCode::SourceUnavailable]
    );

    waiting.status = ProjectStatus::Archived;
    assert!(codes(&waiting, &available, "2026-08-10T00:00:00Z").is_empty());
}

#[test]
fn reasons_aggregate_in_fixed_priority_order_and_never_include_actual_changed() {
    let active = state(ProjectStatus::Active);
    let reasons = derive_review_reasons(
        &active,
        &source(ProjectSourceStatus::Missing),
        at("2026-08-10T00:00:00Z"),
        at("2026-08-10T00:00:00Z").date_naive(),
        DEFAULT_COMMITMENT_REVIEW_DAYS,
    );

    assert_eq!(
        reasons
            .iter()
            .map(|reason| &reason.code)
            .collect::<Vec<_>>(),
        vec![
            &ReviewReasonCode::SourceUnavailable,
            &ReviewReasonCode::NeedsCommitment,
        ]
    );
    assert!(reasons
        .iter()
        .all(|reason| reason.rule_version == REVIEW_RULE_VERSION));
    assert!(reasons.iter().all(|reason| {
        !reason.label.to_ascii_lowercase().contains("actual changed")
            && reason
                .evidence
                .iter()
                .all(|evidence| !evidence.to_ascii_lowercase().contains("actual changed"))
    }));
}

#[test]
fn review_interval_is_a_visible_parameter_not_work_item_or_state_update_age() {
    let item = work_item_id("work-parameter");
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(item.clone());
    state.updated_at = "2026-08-10T00:00:00Z".into();
    state.commitment_transitions.push(transition(
        "transition-set-parameter",
        CommitmentTransitionKind::Set,
        "2026-08-01T00:00:00Z",
        None,
        Some(item),
        None,
    ));

    let reasons = derive_review_reasons(
        &state,
        &source(ProjectSourceStatus::Available),
        at("2026-08-04T00:00:00Z"),
        at("2026-08-04T00:00:00Z").date_naive(),
        3,
    );
    assert_eq!(reasons[0].code, ReviewReasonCode::ReviewAction);
    assert!(reasons[0]
        .evidence
        .iter()
        .any(|evidence| evidence.contains("3 days")));
}

// --- R1: overdue work ------------------------------------------------------

fn work_item(id: &str, status: WorkItemStatus, due: Option<&str>) -> WorkItem {
    WorkItem {
        id: work_item_id(id),
        project_id: project_id(),
        text: format!("task {id}"),
        status,
        unclear: false,
        due: due.map(str::to_owned),
        note: None,
        tags: Vec::new(),
        commits: Vec::new(),
        blocker: None,
        blocked_at: None,
        created_at: CREATED_AT.into(),
        updated_at: CREATED_AT.into(),
        adopted_from_proposal_id: None,
        source_task_id: None,
    }
}

#[test]
fn overdue_work_fires_only_after_the_due_day_has_fully_passed() {
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(work_item_id("work-a"));
    state.work_items = vec![work_item(
        "work-a",
        WorkItemStatus::Planned,
        Some("2026-08-10"),
    )];
    let available = source(ProjectSourceStatus::Available);

    // On the due day itself, nothing fires.
    assert!(codes(&state, &available, "2026-08-10T23:00:00Z").is_empty());
    // The morning after, it does.
    let reasons = derive_review_reasons(
        &state,
        &available,
        at("2026-08-11T08:00:00Z"),
        at("2026-08-11T08:00:00Z").date_naive(),
        DEFAULT_COMMITMENT_REVIEW_DAYS,
    );
    assert_eq!(reasons[0].code, ReviewReasonCode::OverdueWork);
    assert_eq!(reasons[0].label, "Overdue work");
    assert_eq!(reasons[0].rule_version, REVIEW_RULE_VERSION);
    assert!(reasons[0]
        .evidence
        .iter()
        .any(|line| line == "overdue items: 1"));
    assert!(reasons[0]
        .evidence
        .iter()
        .any(|line| line.contains("due 2026-08-10 (1 days overdue)")));
}

#[test]
fn done_abandoned_and_undated_items_are_never_overdue() {
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(work_item_id("work-done"));
    state.work_items = vec![
        work_item("work-done", WorkItemStatus::Done, Some("2026-08-01")),
        work_item("work-gone", WorkItemStatus::Abandoned, Some("2026-08-01")),
        work_item("work-open", WorkItemStatus::Planned, None),
    ];

    assert!(codes(
        &state,
        &source(ProjectSourceStatus::Available),
        "2026-08-20T00:00:00Z"
    )
    .is_empty());
}

#[test]
fn waiting_and_parked_projects_suppress_overdue_work() {
    for status in [ProjectStatus::Waiting, ProjectStatus::Parked] {
        let mut suspended = state(status);
        suspended.status_reason = Some("deliberately deferred".into());
        if status == ProjectStatus::Waiting {
            suspended.review_at = Some("2026-12-01T00:00:00Z".into());
        }
        suspended.work_items = vec![work_item(
            "work-late",
            WorkItemStatus::Doing,
            Some("2026-08-01"),
        )];
        assert!(codes(
            &suspended,
            &source(ProjectSourceStatus::Available),
            "2026-08-20T00:00:00Z"
        )
        .is_empty());
    }
}

#[test]
fn overdue_evidence_names_the_three_oldest_and_folds_the_rest() {
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(work_item_id("work-1"));
    state.work_items = vec![
        work_item("work-1", WorkItemStatus::Planned, Some("2026-08-05")),
        work_item("work-2", WorkItemStatus::Doing, Some("2026-08-01")),
        work_item("work-3", WorkItemStatus::Blocked, Some("2026-08-03")),
        work_item("work-4", WorkItemStatus::Planned, Some("2026-08-07")),
        work_item("work-5", WorkItemStatus::Planned, Some("2026-08-06")),
    ];

    let reasons = derive_review_reasons(
        &state,
        &source(ProjectSourceStatus::Available),
        at("2026-08-10T00:00:00Z"),
        at("2026-08-10T00:00:00Z").date_naive(),
        DEFAULT_COMMITMENT_REVIEW_DAYS,
    );
    let overdue = reasons
        .iter()
        .find(|reason| reason.code == ReviewReasonCode::OverdueWork)
        .expect("overdue reason");
    assert_eq!(overdue.evidence[0], "overdue items: 5");
    // Oldest debt first: 08-01, 08-03, 08-05; the rest folds into a count.
    assert!(overdue.evidence[1].starts_with("due 2026-08-01 (9 days overdue)"));
    assert!(overdue.evidence[2].starts_with("due 2026-08-03 (7 days overdue)"));
    assert!(overdue.evidence[3].starts_with("due 2026-08-05 (5 days overdue)"));
    assert_eq!(overdue.evidence[4], "and 2 more overdue items");
}

#[test]
fn overdue_evidence_truncates_long_task_text_on_a_char_boundary() {
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(work_item_id("work-long"));
    let mut long = work_item("work-long", WorkItemStatus::Planned, Some("2026-08-01"));
    long.text = "长".repeat(80);
    state.work_items = vec![long];

    let reasons = derive_review_reasons(
        &state,
        &source(ProjectSourceStatus::Available),
        at("2026-08-10T00:00:00Z"),
        at("2026-08-10T00:00:00Z").date_naive(),
        DEFAULT_COMMITMENT_REVIEW_DAYS,
    );
    let overdue = reasons
        .iter()
        .find(|reason| reason.code == ReviewReasonCode::OverdueWork)
        .expect("overdue reason");
    let line = &overdue.evidence[1];
    assert!(line.ends_with('…'));
    assert_eq!(line.chars().filter(|c| *c == '长').count(), 60);
}

#[test]
fn overdue_sits_between_needs_commitment_and_review_action() {
    // A project with no commitment AND overdue work: NeedsCommitment first, overdue second.
    let mut state = state(ProjectStatus::Active);
    state.work_items = vec![work_item(
        "work-late",
        WorkItemStatus::Planned,
        Some("2026-08-01"),
    )];
    assert_eq!(
        codes(
            &state,
            &source(ProjectSourceStatus::Available),
            "2026-08-20T00:00:00Z"
        ),
        vec![
            ReviewReasonCode::NeedsCommitment,
            ReviewReasonCode::OverdueWork,
        ]
    );

    // With a stale commitment AND overdue work: overdue outranks the routine review.
    let item = work_item_id("work-late");
    state.current_next_action_id = Some(item.clone());
    state.commitment_transitions.push(transition(
        "transition-set-overdue",
        CommitmentTransitionKind::Set,
        "2026-08-01T00:00:00Z",
        None,
        Some(item),
        None,
    ));
    assert_eq!(
        codes(
            &state,
            &source(ProjectSourceStatus::Available),
            "2026-08-20T00:00:00Z"
        ),
        vec![
            ReviewReasonCode::OverdueWork,
            ReviewReasonCode::ReviewAction,
        ]
    );
}

#[test]
fn overdue_work_survives_source_failure_like_other_human_state_reasons() {
    let mut state = state(ProjectStatus::Active);
    state.current_next_action_id = Some(work_item_id("work-late"));
    state.work_items = vec![work_item(
        "work-late",
        WorkItemStatus::Planned,
        Some("2026-08-01"),
    )];
    assert_eq!(
        codes(
            &state,
            &source(ProjectSourceStatus::Missing),
            "2026-08-20T00:00:00Z"
        ),
        vec![
            ReviewReasonCode::SourceUnavailable,
            ReviewReasonCode::OverdueWork,
        ]
    );
}
