use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use vitrine_core::Project;
use vitrine_core::io::disk_revision;
use vitrine_core::ops::Op;

use super::*;
use crate::events::RecordingSink;
use crate::testutil::example_copy;

fn op(json: Value) -> Op {
    serde_json::from_value(json).unwrap()
}

fn open() -> (tempfile::TempDir, Arc<Session>, Arc<RecordingSink>) {
    let (tmp, dir) = example_copy();
    let sink = Arc::new(RecordingSink::default());
    let s = Session::open(&dir, sink.clone()).unwrap();
    (tmp, s, sink)
}

#[test]
fn open_reports_missing_project() {
    let tmp = tempfile::tempdir().unwrap();
    let err = Session::open(tmp.path(), Arc::new(crate::events::NullSink)).unwrap_err();
    assert_eq!(err.code, ErrorCode::Load);
}

#[test]
fn apply_saves_with_new_revision_and_reports_changes() {
    let (_tmp, s, _) = open();
    let rev = s.revision();
    let r = s
        .apply_ops(
            vec![
                op(json!({"op": "update_element", "id": "band", "patch": {"rotation": 5}})),
                op(json!({"op": "set_string", "lang": "de", "key": "plan.title", "value": "Neu"})),
            ],
            None,
        )
        .unwrap();
    assert!(r.applied);
    assert_eq!(r.revision, rev + 1);
    assert_eq!(disk_revision(s.dir()).unwrap(), Some(rev + 1));
    assert_eq!(r.changes.elements, ["band"]);
    assert_eq!(r.changes.canvases, ["phone"]);
    assert_eq!(
        r.changes.strings,
        [StringRef {
            lang: "de".into(),
            key: "plan.title".into()
        }]
    );
    assert_eq!(r.changes.targets, Vec::<String>::new());
    assert_eq!(r.history.undo, ["update_element band (+1 more)"]);

    // The saved folder is what the session holds.
    let disk = Project::load(s.dir()).unwrap();
    assert_eq!(disk, s.snapshot());
    assert_eq!(disk.string("de", "plan.title"), Some("Neu"));
}

#[test]
fn rejected_ops_change_nothing() {
    let (_tmp, s, _) = open();
    let rev = s.revision();
    let err = s
        .apply_ops(
            vec![op(json!({"op": "remove_element", "id": "nope"}))],
            None,
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::OpRejected);
    assert_eq!(s.revision(), rev);
    assert_eq!(disk_revision(s.dir()).unwrap(), Some(rev));
    assert!(!s.history_state().can_undo);
}

#[test]
fn undo_redo_save_every_step() {
    let (_tmp, s, _) = open();
    let rev = s.revision();
    s.apply_ops(
        vec![op(json!({"op": "set_name", "name": "Renamed"}))],
        Some("Rename".into()),
    )
    .unwrap();
    let u = s.undo().unwrap();
    assert!(u.applied);
    assert!(u.changes.name);
    assert_eq!(u.revision, rev + 2);
    assert_eq!(Project::load(s.dir()).unwrap().name, "Fernwood");
    assert_eq!(u.history.redo, ["Rename"]);
    assert!(!u.history.can_undo);
    let r = s.redo().unwrap();
    assert_eq!(Project::load(s.dir()).unwrap().name, "Renamed");
    assert_eq!(r.history.undo, ["Rename"]);
    assert!(!s.redo().unwrap().applied);
}

#[test]
fn conflict_reloads_and_reports() {
    let (_tmp, s, sink) = open();
    // Someone else (MCP, a text editor) saves a newer revision.
    let mut other = Project::load(s.dir()).unwrap();
    other.name = "From MCP".into();
    other.save(s.dir()).unwrap();

    let err = s
        .apply_ops(
            vec![op(json!({"op": "set_name", "name": "From GUI"}))],
            None,
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(s.snapshot().name, "From MCP");
    assert_eq!(s.revision(), other.revision);
    let events = sink.named(PROJECT_CHANGED);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["reason"], "conflict");
    assert_eq!(events[0]["changes"]["name"], true);
    // Retrying after the reload works.
    s.apply_ops(
        vec![op(json!({"op": "set_name", "name": "From GUI"}))],
        None,
    )
    .unwrap();
    assert_eq!(Project::load(s.dir()).unwrap().name, "From GUI");
}

#[test]
fn own_writes_are_not_external_changes() {
    let (_tmp, s, sink) = open();
    s.apply_ops(vec![op(json!({"op": "set_name", "name": "Mine"}))], None)
        .unwrap();
    s.handle_fs_changes(&[s.dir().join("project.yaml"), s.dir().join("i18n/en.yaml")]);
    assert_eq!(sink.named(PROJECT_CHANGED), Vec::<Value>::new());
    assert!(s.history_state().can_undo);
}

#[test]
fn external_changes_reload_and_emit() {
    let (_tmp, s, sink) = open();
    let yaml = std::fs::read_to_string(s.dir().join("i18n/de.yaml")).unwrap();
    std::fs::write(
        s.dir().join("i18n/de.yaml"),
        format!("{yaml}extra.key: Hallo\n"),
    )
    .unwrap();
    s.handle_fs_changes(&[s.dir().join("i18n/de.yaml")]);
    assert_eq!(s.snapshot().string("de", "extra.key"), Some("Hallo"));
    let ev = sink.named(PROJECT_CHANGED);
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0]["reason"], "external");
    assert_eq!(ev[0]["changes"]["strings"][0]["key"], "extra.key");
    assert_eq!(ev[0]["history_cleared"], true);

    // Asset change: event, no reload.
    s.handle_fs_changes(&[s.dir().join("screenshots/en/ios-phone/home.png")]);
    let ev = sink.named(PROJECT_CHANGED);
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[1]["reason"], "assets");
    assert_eq!(
        ev[1]["changes"]["assets"][0],
        "screenshots/en/ios-phone/home.png"
    );

    // A broken file is reported, the session keeps its model.
    std::fs::write(s.dir().join("project.yaml"), "format: [").unwrap();
    s.handle_fs_changes(&[s.dir().join("project.yaml")]);
    assert_eq!(sink.named(PROJECT_ERROR).len(), 1);
    assert_eq!(s.snapshot().string("de", "extra.key"), Some("Hallo"));
}

#[test]
fn watcher_detects_external_edits() {
    let (_tmp, s, sink) = open();
    s.watch().unwrap();
    // Give the backend a moment to start (FSEvents on macOS).
    std::thread::sleep(Duration::from_millis(300));
    let mut other = Project::load(s.dir()).unwrap();
    other.name = "Watched".into();
    other.save(s.dir()).unwrap();
    let start = Instant::now();
    while s.snapshot().name != "Watched" && start.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(s.snapshot().name, "Watched");
    assert!(
        sink.named(PROJECT_CHANGED)
            .iter()
            .any(|e| e["reason"] == "external")
    );
    s.stop_watching();
}

#[test]
fn labels() {
    assert_eq!(
        label_for(&[op(json!({"op": "remove_element", "id": "x"}))]),
        "remove_element x"
    );
    assert_eq!(
        label_for(&[op(json!({"op": "set_name", "name": "x"}))]),
        "set_name"
    );
}

#[test]
fn actor_labels_history_and_edits_are_announced() {
    let (_tmp, s, sink) = open();
    let claude = Actor::assistant("claude-code", None);
    let rot =
        |deg: i32| op(json!({"op": "update_element", "id": "band", "patch": {"rotation": deg}}));

    // Explicit options and the thread's current actor give the same label.
    s.apply_ops_with(
        vec![rot(5)],
        ApplyOptions {
            label: Some("rotate band".into()),
            actor: claude.clone(),
        },
    )
    .unwrap();
    let r = claude
        .clone()
        .scope(|| s.apply_ops(vec![rot(6)], None))
        .unwrap();
    assert_eq!(
        r.history.undo,
        [
            "Claude Code: rotate band",
            "Claude Code: update_element band"
        ]
    );
    // Default: the user, no prefix.
    let r = s.apply_ops(vec![rot(7)], None).unwrap();
    assert_eq!(r.history.undo.last().unwrap(), "update_element band");
    s.undo().unwrap();

    let ev = sink.named(PROJECT_EDITED);
    assert_eq!(ev.len(), 4);
    assert_eq!(ev[0]["action"], "apply");
    assert_eq!(
        ev[0]["actor"],
        json!({"kind": "assistant", "client": "claude-code", "name": "Claude Code"})
    );
    assert_eq!(ev[0]["label"], "Claude Code: rotate band");
    assert_eq!(ev[0]["changes"]["elements"], json!(["band"]));
    assert_eq!(ev[2]["actor"], json!({"kind": "user"}));
    assert_eq!(ev[3]["action"], "undo");
    assert_eq!(ev[3]["label"], "update_element band");
    assert_eq!(ev[3]["revision"], s.revision());
    // Edits through the session are not reported as external changes.
    assert_eq!(sink.named(PROJECT_CHANGED), Vec::<Value>::new());

    // Nothing to redo after a new step: no event.
    let before = sink.named(PROJECT_EDITED).len();
    s.apply_ops(vec![rot(8)], None).unwrap();
    assert!(!s.redo().unwrap().applied);
    assert_eq!(sink.named(PROJECT_EDITED).len(), before + 1);
}
