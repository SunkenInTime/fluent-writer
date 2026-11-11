//! Port of Tests/WriterCoreTests/DraftStoreTests.swift.

use std::fs;
use std::path::{Path, PathBuf};

use writer_core::{DraftLibrary, SaveResult, atomic_file, text_stats, utf16_to_byte};

struct Root {
    dir: tempfile::TempDir,
}

impl Root {
    fn new() -> Self {
        Root {
            dir: tempfile::Builder::new()
                .prefix("fw-tests-")
                .tempdir()
                .unwrap(),
        }
    }
    fn drafts(&self) -> PathBuf {
        self.dir.path().join("drafts")
    }
    fn make_library(&self) -> DraftLibrary {
        DraftLibrary::new(self.drafts(), self.dir.path().join("support")).unwrap()
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        set_mode(&self.drafts(), 0o755);
    }
}

#[cfg(unix)]
fn set_mode(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_mode(_p: &Path, _mode: u32) {}

fn long_draft() -> String {
    let sentence = "The quick brown fox jumps over the lazy dog while the writer keeps typing. ";
    (0..215)
        .map(|i| {
            if i % 5 == 4 {
                format!("{sentence}\n\n")
            } else {
                sentence.to_string()
            }
        })
        .collect()
}

fn file_name(p: Option<&Path>) -> String {
    p.and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[test]
fn empty_new_draft_is_not_written() {
    let root = Root::new();
    let mut lib = root.make_library();
    let mut doc = lib.new_draft();
    assert_eq!(lib.save(&mut doc).unwrap(), SaveResult::Unchanged);
    assert!(doc.path().is_none());
}

#[test]
fn save_reopen_preserves_body_and_cursor() {
    let root = Root::new();
    let mut lib = root.make_library();
    let mut doc = lib.new_draft();
    doc.body = format!("# My post\n\n{}", long_draft());
    doc.refresh_derived_title();
    doc.selection = 1234..1234;
    lib.save(&mut doc).unwrap();
    assert_eq!(file_name(doc.path()), "My post.md");
    lib.remember_selection(&doc);
    assert!(text_stats::counts_for_markdown(&doc.body).words > 3000);

    let mut lib2 = root.make_library();
    assert_eq!(lib2.last_opened_path().as_deref(), doc.path());
    let reopened = lib2.open(doc.path().unwrap()).unwrap();
    assert_eq!(reopened.body, doc.body);
    assert_eq!(reopened.selection.start, 1234);
    assert!(!reopened.is_dirty());
}

#[test]
fn cursor_round_trips_through_utf16_on_disk() {
    let root = Root::new();
    let mut lib = root.make_library();
    let mut doc = lib.new_draft();
    doc.body = "# 你好 👍🏽\n\nmore 中文 text".to_string();
    doc.refresh_derived_title();
    let at = doc.body.find("中文").unwrap();
    doc.selection = at..at + "中文".len();
    lib.save(&mut doc).unwrap();
    lib.remember_selection(&doc);

    // state.json holds the selection in UTF-16 units, as the Swift app wrote it.
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(root.dir.path().join("support/state.json")).unwrap())
            .unwrap();
    let key = doc.path().unwrap().to_string_lossy().into_owned();
    let record = &state["records"][&key];
    assert_eq!(record["selectionLocation"], 16);
    assert_eq!(record["selectionLength"], 2);
    assert!(record["lastOpened"].is_f64());
    assert_eq!(utf16_to_byte(&doc.body, 16), at);

    let mut lib2 = root.make_library();
    let reopened = lib2.open(doc.path().unwrap()).unwrap();
    assert_eq!(&reopened.body[reopened.selection.clone()], "中文");
}

#[test]
fn reads_state_written_by_swift() {
    let root = Root::new();
    let draft = root.drafts().join("Old.md");
    fs::create_dir_all(root.drafts()).unwrap();
    fs::write(&draft, "😀 hi").unwrap();
    let support = root.dir.path().join("support");
    fs::create_dir_all(&support).unwrap();
    let p = draft
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('/', "\\/");
    let json = format!(
        r#"{{"records":{{"{p}":{{"path":"{p}","title":"Kept","hasManualTitle":true,"selectionLocation":3,"selectionLength":2,"lastOpened":784111777.12345603}}}},"lastOpenedPath":"{p}"}}"#
    );
    fs::write(support.join("state.json"), json).unwrap();
    let mut lib = root.make_library();
    assert_eq!(lib.last_opened_path(), Some(draft.clone()));
    let doc = lib.open(&draft).unwrap();
    assert_eq!(doc.title, "Kept");
    assert!(doc.has_manual_title);
    assert_eq!(&doc.body[doc.selection.clone()], "hi");
    assert_eq!(DraftLibrary::recent_paths(&support)[0].title, "Kept");
}

#[test]
fn derived_title_renames_file_but_manual_title_sticks() {
    let root = Root::new();
    let mut lib = root.make_library();
    let mut doc = lib.new_draft();
    doc.body = "First idea".into();
    doc.refresh_derived_title();
    lib.save(&mut doc).unwrap();
    assert_eq!(file_name(doc.path()), "First idea.md");
    doc.body = "Better idea\nmore".into();
    doc.refresh_derived_title();
    lib.save(&mut doc).unwrap();
    assert_eq!(file_name(doc.path()), "Better idea.md");
    lib.rename(&mut doc, "Launch thread").unwrap();
    assert_eq!(file_name(doc.path()), "Launch thread.md");
    doc.body = "Changed first line".into();
    doc.refresh_derived_title();
    lib.save(&mut doc).unwrap();
    assert_eq!(doc.title, "Launch thread");
    assert_eq!(file_name(doc.path()), "Launch thread.md");
}

#[test]
fn title_collision_gets_suffix() {
    let root = Root::new();
    let mut lib = root.make_library();
    for _ in 0..2 {
        let mut d = lib.new_draft();
        d.body = "Same".into();
        d.refresh_derived_title();
        lib.save(&mut d).unwrap();
    }
    let mut names: Vec<String> = fs::read_dir(lib.drafts_directory())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Same 2.md", "Same.md"]);
}

#[cfg(unix)]
#[test]
fn save_failure_keeps_recovery_and_reopen_restores_it() {
    let root = Root::new();
    let mut lib = root.make_library();
    let mut doc = lib.new_draft();
    doc.body = "Saved version".into();
    doc.refresh_derived_title();
    lib.save(&mut doc).unwrap();
    let path = doc.path().unwrap().to_path_buf();

    set_mode(lib.drafts_directory(), 0o555);
    doc.body = "Saved version plus unsaved words".into();
    let err = lib.save(&mut doc).unwrap_err();
    assert!(matches!(err, writer_core::Error::FileWrite(_)));
    assert!(err.to_string().contains("Couldn't save"), "{err}");
    assert!(doc.is_dirty());
    assert!(lib.has_recovery(&doc));
    assert_eq!(fs::read_to_string(&path).unwrap(), "Saved version");

    // Simulate quitting and relaunching.
    set_mode(lib.drafts_directory(), 0o755);
    let mut lib2 = root.make_library();
    let mut reopened = lib2.open(&path).unwrap();
    assert_eq!(reopened.body, "Saved version plus unsaved words");
    assert!(reopened.recovered_at().is_some());
    assert!(reopened.is_dirty());
    lib2.save(&mut reopened).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "Saved version plus unsaved words"
    );
    assert!(lib2.recovery_snapshots().is_empty());
}

#[test]
fn interrupted_write_leaves_previous_revision() {
    let root = Root::new();
    let mut lib = root.make_library();
    let mut doc = lib.new_draft();
    doc.body = "Complete revision".into();
    doc.refresh_derived_title();
    lib.save(&mut doc).unwrap();
    // A crash mid-write leaves only a partial temporary file next to the draft.
    let tmp = lib.drafts_directory().join(format!(
        ".Complete revision.md{}crash",
        atomic_file::TEMPORARY_MARKER
    ));
    fs::write(&tmp, "Compl").unwrap();
    let mut lib2 = root.make_library();
    assert!(!tmp.exists());
    assert_eq!(
        lib2.open(doc.path().unwrap()).unwrap().body,
        "Complete revision"
    );
}

#[cfg(unix)]
#[test]
fn orphaned_untitled_recovery() {
    let root = Root::new();
    let mut lib = root.make_library();
    set_mode(lib.drafts_directory(), 0o555);
    let mut doc = lib.new_draft();
    doc.body = "never reached disk 👍🏽".into();
    doc.selection = doc.body.len()..doc.body.len();
    assert!(lib.save(&mut doc).is_err());
    set_mode(lib.drafts_directory(), 0o755);

    // The journal stores the caret in UTF-16 units.
    let journal = lib
        .recovery_directory()
        .join(format!("{}.json", doc.id().to_string().to_uppercase()));
    let v: serde_json::Value = serde_json::from_slice(&fs::read(journal).unwrap()).unwrap();
    assert_eq!(v["selectionLocation"], 23);

    let lib2 = root.make_library();
    let orphans = lib2.orphaned_recoveries();
    assert_eq!(orphans.len(), 1);
    let restored = lib2.restore(&orphans[0]);
    assert_eq!(restored.body, "never reached disk 👍🏽");
    assert_eq!(restored.selection, restored.body.len()..restored.body.len());
    assert!(restored.is_dirty());
}

#[test]
fn recent_drafts() {
    let root = Root::new();
    let mut lib = root.make_library();
    for t in ["One", "Two"] {
        let mut d = lib.new_draft();
        d.body = t.into();
        d.refresh_derived_title();
        lib.save(&mut d).unwrap();
    }
    let mut titles: Vec<String> = lib
        .recent_drafts(200)
        .into_iter()
        .map(|s| s.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["One", "Two"]);
}
