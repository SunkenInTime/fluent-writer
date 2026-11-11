//! Port of Tests/WriterCoreTests/RemoteTests.swift.

use std::fs;

use writer_core::remote::describe_failure;
use writer_core::{
    DraftLibrary, RemoteEntry, RemoteKind, RemoteLocation, SaveResult, SshTransport,
};

fn make_library(root: &tempfile::TempDir) -> DraftLibrary {
    DraftLibrary::new(root.path().join("drafts"), root.path().join("support")).unwrap()
}

fn root() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("fw-remote-")
        .tempdir()
        .unwrap()
}

fn loc(host: &str, port: Option<u16>, path: &str) -> RemoteLocation {
    RemoteLocation::new(host, port, path)
}

#[test]
fn parses_scp_style() {
    assert_eq!(
        RemoteLocation::parse("me@box:~/notes/a.md"),
        Some(loc("me@box", None, "~/notes/a.md"))
    );
    assert_eq!(
        RemoteLocation::parse(" box:/srv/x.md "),
        Some(loc("box", None, "/srv/x.md"))
    );
    assert_eq!(RemoteLocation::parse("box:"), Some(loc("box", None, "~")));
}

#[test]
fn parses_ssh_url() {
    assert_eq!(
        RemoteLocation::parse("ssh://me@box:2222/srv/a.md"),
        Some(loc("me@box", Some(2222), "/srv/a.md"))
    );
    assert_eq!(
        RemoteLocation::parse("ssh://box/~/a.md"),
        Some(loc("box", None, "~/a.md"))
    );
    assert_eq!(
        RemoteLocation::parse("ssh://box"),
        Some(loc("box", None, "~"))
    );
    assert_eq!(
        RemoteLocation::parse("SSH://box/srv/a%20b.md"),
        Some(loc("box", None, "/srv/a b.md"))
    );
    assert_eq!(RemoteLocation::parse("ssh://box:notaport/a"), None);
}

#[test]
fn rejects_unsafe_or_local_input() {
    assert_eq!(RemoteLocation::parse("/Users/me/a.md"), None);
    assert_eq!(RemoteLocation::parse("-oProxyCommand=x:a"), None);
    assert_eq!(RemoteLocation::parse("box;rm:a"), None);
    assert_eq!(RemoteLocation::parse("no colon"), None);
}

#[test]
fn shell_path_quotes_but_keeps_tilde() {
    assert_eq!(
        loc("b", None, "~/it's here.md").shell_path(),
        "~/'it'\\''s here.md'"
    );
    assert_eq!(loc("b", None, "/a b/$x").shell_path(), "'/a b/$x'");
    assert_eq!(loc("b", None, "~").shell_path(), "~");
}

#[test]
fn display_name_and_appending() {
    let l = loc("me@box", Some(2222), "/srv/notes/");
    assert_eq!(l.display(), "ssh://me@box:2222/srv/notes/");
    assert_eq!(l.host_name(), "box");
    assert_eq!(l.name(), "notes");
    assert_eq!(l.appending("a.md").path, "/srv/notes/a.md");
    assert_eq!(loc("box", None, "~").display(), "box:~");
    assert_eq!(loc("box", None, "~").appending("a.md").path, "~/a.md");
}

#[test]
fn arguments_are_non_interactive() {
    let args = SshTransport::default().arguments(&loc("me@box", Some(2222), "~"), "true");
    assert!(args.iter().any(|a| a == "BatchMode=yes"));
    assert_eq!(&args[args.len() - 4..], ["-p", "2222", "me@box", "true"]);
}

#[test]
fn parses_listing_and_skips_hidden() {
    let entries =
        SshTransport::parse_listing(".git/\n.DS_Store\nchapters/\nnotes.md\nhello world.txt\n");
    assert_eq!(
        entries,
        [
            RemoteEntry::new("chapters", true),
            RemoteEntry::new("notes.md", false),
            RemoteEntry::new("hello world.txt", false)
        ]
    );
}

#[test]
fn describes_failures() {
    assert!(
        describe_failure(
            "Warning: Permanently added 'box'\nme@box: Permission denied (publickey).\n",
            "box",
            255
        )
        .contains("refused the login")
    );
    assert!(
        describe_failure("Host key verification failed.\n", "box", 255)
            .contains("isn't a known host")
    );
    assert_eq!(
        describe_failure("", "box", 255),
        "ssh to box failed (exit 255)."
    );
    assert_eq!(
        describe_failure("cat: x: No such file or directory\n", "box", 1),
        "box: cat: x: No such file or directory"
    );
}

#[test]
fn missing_ssh_binary_reports_cleanly() {
    let ssh = SshTransport::new("/nonexistent/ssh-binary", 1);
    let err = ssh.read(&loc("box", None, "~/a.md")).unwrap_err();
    assert!(
        err.message.starts_with("Couldn't start ssh: "),
        "{}",
        err.message
    );
}

#[test]
fn mirror_path_stays_inside_support_directory() {
    let root = root();
    let lib = make_library(&root);
    let mirror = lib.mirror_path(&loc("me@box", Some(22), "/../../etc/passwd"));
    assert!(mirror.starts_with(lib.remote_mirror_directory()));
    assert_eq!(mirror.file_name().unwrap(), "passwd");
    assert_ne!(
        lib.mirror_path(&loc("box", None, "~/a.md")),
        lib.mirror_path(&loc("box", None, "/a.md"))
    );
}

#[test]
fn remote_save_marks_upload_pending_until_uploaded() {
    let root = root();
    let mut lib = make_library(&root);
    let l = loc("box", None, "~/notes/a.md");
    let mut doc = lib.open_remote(&l, b"hello").unwrap();
    assert_eq!(doc.body, "hello");
    assert_eq!(doc.title, "a.md");
    assert!(!doc.remote_upload_pending());
    doc.body = "hello there".into();
    assert_eq!(lib.save(&mut doc).unwrap(), SaveResult::Saved);
    assert!(doc.remote_upload_pending());
    assert_eq!(
        fs::read_to_string(lib.mirror_path(&l)).unwrap(),
        "hello there"
    );
    lib.remote_upload_finished(&mut doc, "hello", true);
    assert!(
        doc.remote_upload_pending(),
        "an older upload must not clear newer saves"
    );
    lib.remote_upload_finished(&mut doc, "hello there", true);
    assert!(!doc.remote_upload_pending());
    assert_eq!(lib.save(&mut doc).unwrap(), SaveResult::Unchanged);
}

#[test]
fn pending_local_edits_survive_reopen() {
    let root = root();
    let l = loc("box", None, "/srv/a.md");
    {
        let mut lib = make_library(&root);
        let mut doc = lib.open_remote(&l, b"v1").unwrap();
        doc.body = "local edit".into();
        lib.save(&mut doc).unwrap();
        lib.remote_upload_finished(&mut doc, "local edit", false);
    }
    let mut lib = make_library(&root);
    let mut reopened = lib.open_remote(&l, b"v1").unwrap();
    assert_eq!(reopened.body, "local edit");
    assert!(reopened.remote_upload_pending());
    assert_eq!(
        lib.save(&mut reopened).unwrap(),
        SaveResult::Saved,
        "pending text is offered for upload again"
    );
    assert_eq!(
        lib.record_for_remote(&l)
            .and_then(|r| r.remote_upload_pending),
        Some(true)
    );
}

#[test]
fn remote_drafts_appear_in_recents_with_location() {
    let root = root();
    let mut lib = make_library(&root);
    let l = loc("box", None, "~/a.md");
    lib.open_remote(&l, b"hi\nthere").unwrap();
    let summary = lib
        .recent_drafts(200)
        .into_iter()
        .next()
        .expect("a recent draft");
    assert_eq!(summary.remote.as_ref(), Some(&l));
    assert_eq!(summary.title, "a.md");
    assert_eq!(summary.preview, "there");
    let via_mirror = lib.open(&summary.path).unwrap();
    assert_eq!(via_mirror.remote(), Some(&l));
}

#[test]
fn renaming_remote_file_keeps_mirror_path() {
    let root = root();
    let mut lib = make_library(&root);
    let l = loc("box", None, "~/a.md");
    let mut doc = lib.open_remote(&l, b"hi").unwrap();
    lib.rename(&mut doc, "Better").unwrap();
    assert_eq!(doc.title, "Better");
    assert_eq!(doc.path(), Some(lib.mirror_path(&l).as_path()));
}

/// Runs against a real server when FLUENT_WRITER_SSH_TEST_DIR is set, e.g. `ssh://me@localhost:2222/tmp/fw`.
#[test]
fn round_trip_over_ssh() {
    let Some(dir) = std::env::var("FLUENT_WRITER_SSH_TEST_DIR")
        .ok()
        .and_then(|raw| RemoteLocation::parse(&raw))
    else {
        eprintln!("skipped: FLUENT_WRITER_SSH_TEST_DIR not set");
        return;
    };
    let ssh = SshTransport::default();
    let file = dir.appending("round trip 'quoted'.md");
    ssh.write("# Hi\n\nüñï".as_bytes(), &file).unwrap();
    assert_eq!(ssh.kind(&file).unwrap(), RemoteKind::File);
    assert_eq!(ssh.kind(&dir).unwrap(), RemoteKind::Directory);
    assert_eq!(
        ssh.kind(&dir.appending("nope")).unwrap(),
        RemoteKind::Missing
    );
    assert_eq!(
        String::from_utf8(ssh.read(&file).unwrap()).unwrap(),
        "# Hi\n\nüñï"
    );
    assert!(
        ssh.list(&dir)
            .unwrap()
            .contains(&RemoteEntry::new("round trip 'quoted'.md", false))
    );
    assert!(
        !ssh.list(&dir)
            .unwrap()
            .iter()
            .any(|e| e.name.contains("fwtmp"))
    );
}
