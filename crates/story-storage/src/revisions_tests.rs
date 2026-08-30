use super::*;
use serde_json::json;

fn repository() -> (tempfile::TempDir, RevisionRepository, Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let package: Value = serde_json::from_slice(
        &std::fs::read(root.join(
            "eval/baselines/baseline-deepseek-v4-pro-20260727/family_001.story-package.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let repository = RevisionRepository::new(
        temporary.path().join("revisions"),
        root.join("schemas/story-package-v1.json"),
    )
    .unwrap();
    (temporary, repository, package)
}

#[test]
fn targeted_revision_populates_correspondence_and_preserves_origin() {
    let (_temporary, repository, package) = repository();
    let origin = repository.ensure_origin("run_test", &package).unwrap();
    let span = ArtifactSpanRef::parse("story-package/logline-1").unwrap();
    let mut replacement = repository
        .read_span(&origin.record.revision_id, &span)
        .unwrap();
    replacement["text"] = json!("新的故事梗概");
    let revision = repository
        .create_targeted(
            &origin.record.revision_id,
            &span,
            replacement,
            "让梗概更明确",
        )
        .unwrap();
    assert_eq!(revision.record.kind, RevisionKind::Targeted);
    assert_eq!(revision.record.round, 1);
    assert!(revision.record.node_correspondence_count > 0);
    assert_eq!(
        repository
            .read_span(&origin.record.revision_id, &span)
            .unwrap()["text"],
        package["logline"]["text"]
    );
    assert_eq!(
        repository
            .read_span(&revision.record.revision_id, &span)
            .unwrap()["text"],
        "新的故事梗概"
    );
}

#[test]
fn third_targeted_round_requires_explicit_input() {
    let (_temporary, repository, package) = repository();
    let origin = repository.ensure_origin("run_test", &package).unwrap();
    let span = ArtifactSpanRef::parse("story-package/logline-1").unwrap();
    let mut base = origin;
    for round in 1..=2 {
        let mut replacement = repository
            .read_span(&base.record.revision_id, &span)
            .unwrap();
        replacement["text"] = json!(format!("revision {round}"));
        base = repository
            .create_targeted(&base.record.revision_id, &span, replacement, "revise")
            .unwrap();
    }
    let replacement = repository
        .read_span(&base.record.revision_id, &span)
        .unwrap();
    assert!(matches!(
        repository.create_targeted(&base.record.revision_id, &span, replacement, "third"),
        Err(RevisionError::InputRequired)
    ));
}

#[test]
fn approval_rollback_and_export_are_append_only() {
    let (temporary, repository, package) = repository();
    let origin = repository.ensure_origin("run_test", &package).unwrap();
    let span = ArtifactSpanRef::parse("story-package/logline-1").unwrap();
    let mut replacement = repository
        .read_span(&origin.record.revision_id, &span)
        .unwrap();
    replacement["text"] = json!("changed");
    let revision = repository
        .create_targeted(&origin.record.revision_id, &span, replacement, "change")
        .unwrap();
    let rollback = repository
        .rollback(
            &revision.record.revision_id,
            &origin.record.revision_id,
            "restore origin",
        )
        .unwrap();
    assert_eq!(rollback.record.kind, RevisionKind::Rollback);
    assert_ne!(rollback.record.package_id, origin.record.package_id);
    assert_eq!(repository.list().unwrap().len(), 3);
    let approved = repository
        .approve(
            &rollback.record.revision_id,
            ApprovalDecision::Approved,
            "operator",
            "ready",
        )
        .unwrap();
    assert_eq!(
        approved.approval.unwrap().decision,
        ApprovalDecision::Approved
    );
    assert!(matches!(
        repository.approve(
            &rollback.record.revision_id,
            ApprovalDecision::Rejected,
            "operator",
            ""
        ),
        Err(RevisionError::ApprovalFinal)
    ));
    let export = temporary.path().join("approved.json");
    repository
        .export_approved(&rollback.record.revision_id, &export)
        .unwrap();
    let exported: Value = serde_json::from_slice(&std::fs::read(export).unwrap()).unwrap();
    assert_eq!(exported["schema"], "story-package/v1");
    assert_eq!(repository.list().unwrap().len(), 3);
}
