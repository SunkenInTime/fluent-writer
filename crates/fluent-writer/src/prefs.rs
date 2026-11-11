//! Settings that persist between launches. The Swift app kept these in `UserDefaults`; here
//! they live in `preferences.json` in the support directory, under the same key names.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use writer_core::FocusUnit;

use crate::theme::{Appearance, DEFAULT_TEXT_SIZE};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Stored {
    focus: Option<String>,
    last_dimming_focus: Option<String>,
    text_size: Option<f64>,
    appearance: Option<String>,
    show_counts: Option<bool>,
    spell_check: Option<bool>,
    /// Edit › Substitutions. AppKit's toggles, which the Swift app set at each launch and never
    /// saved: they last until the app quits. `EditorController.swift:164-167`
    #[serde(skip)]
    smart_quotes: Option<bool>,
    #[serde(skip)]
    smart_dashes: Option<bool>,
    #[serde(skip)]
    text_replacement: Option<bool>,
    show_sidebar: Option<bool>,
    sidebar_mode: Option<String>,
    folder_local: Option<String>,
    folder_remote: Option<String>,
    #[serde(rename = "lastSSHLocation")]
    last_ssh_location: Option<String>,
    /// The window's content rect in screen points and the scale it drew at: x, y, w, h, scale.
    window_frame: Option<[f64; 5]>,
    /// How far the window's title bar and borders put its content from its outer corner.
    window_decoration: Option<[f64; 2]>,
    #[serde(rename = "assist.provider")]
    assist_provider: Option<String>,
}

/// The app's settings, read once at launch and written on every change.
#[derive(Clone, Debug)]
pub struct Prefs {
    path: Option<PathBuf>,
    stored: Stored,
}

fn focus_from(raw: Option<&str>) -> Option<FocusUnit> {
    FocusUnit::ALL.into_iter().find(|u| Some(focus_raw(*u)) == raw)
}

fn focus_raw(unit: FocusUnit) -> &'static str {
    match unit {
        FocusUnit::Off => "off",
        FocusUnit::Sentence => "sentence",
        FocusUnit::Paragraph => "paragraph",
        FocusUnit::Typewriter => "typewriter",
    }
}

impl Prefs {
    /// Settings kept only in memory, for tests.
    #[cfg(test)]
    pub fn in_memory() -> Prefs {
        Prefs { path: None, stored: Stored::default() }
    }

    pub fn load(support_directory: &Path) -> Prefs {
        let path = support_directory.join("preferences.json");
        let stored = std::fs::read(&path).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default();
        Prefs { path: Some(path), stored }
    }

    /// [`Prefs::load`], except that the first launch on a Mac that ran the Swift app carries its
    /// settings over: with no `preferences.json` yet, the old `UserDefaults` of `bundle_id`
    /// (`com.sunkenintime.fluentwriter`, or `.dev`) are read with `defaults export` and written
    /// to `preferences.json`, so it happens once. Runs before the window opens.
    pub fn load_or_import(support_directory: &Path, bundle_id: &str) -> Prefs {
        let mut prefs = Prefs::load(support_directory);
        if prefs.path.as_ref().is_some_and(|p| !p.exists())
            && let Some(stored) = read_user_defaults(bundle_id).as_deref().and_then(stored_from_plist)
        {
            prefs.stored = stored;
            prefs.save();
        }
        prefs
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Ok(data) = serde_json::to_vec_pretty(&self.stored) {
            let _ = writer_core::atomic_file::write(&data, path);
        }
    }

    pub fn focus(&self) -> FocusUnit {
        focus_from(self.stored.focus.as_deref()).unwrap_or(FocusUnit::Off)
    }
    pub fn set_focus(&mut self, unit: FocusUnit) {
        self.stored.focus = Some(focus_raw(unit).into());
        self.save();
    }

    /// The dimming mode Toggle Focus returns to when turning focus back on.
    pub fn last_dimming_focus(&self) -> FocusUnit {
        focus_from(self.stored.last_dimming_focus.as_deref()).unwrap_or(FocusUnit::Sentence)
    }
    pub fn set_last_dimming_focus(&mut self, unit: FocusUnit) {
        self.stored.last_dimming_focus = Some(focus_raw(unit).into());
        self.save();
    }

    pub fn text_size(&self) -> f32 {
        self.stored.text_size.filter(|v| *v > 0.0).map_or(DEFAULT_TEXT_SIZE, |v| v as f32)
    }
    pub fn set_text_size(&mut self, size: f32) {
        self.stored.text_size = Some(size as f64);
        self.save();
    }

    pub fn appearance(&self) -> Appearance {
        self.stored.appearance.as_deref().and_then(Appearance::from_raw).unwrap_or(Appearance::System)
    }
    pub fn set_appearance(&mut self, a: Appearance) {
        self.stored.appearance = Some(a.raw().into());
        self.save();
    }

    pub fn show_counts(&self) -> bool {
        self.stored.show_counts.unwrap_or(true)
    }
    pub fn set_show_counts(&mut self, on: bool) {
        self.stored.show_counts = Some(on);
        self.save();
    }

    pub fn spell_check(&self) -> bool {
        self.stored.spell_check.unwrap_or(true)
    }
    pub fn set_spell_check(&mut self, on: bool) {
        self.stored.spell_check = Some(on);
        self.save();
    }

    /// Edit › Substitutions. The Swift app set these at each launch: quotes and dashes off,
    /// text replacement on. Here they are those defaults until changed, and then kept.
    pub fn smart_quotes(&self) -> bool {
        self.stored.smart_quotes.unwrap_or(false)
    }
    pub fn set_smart_quotes(&mut self, on: bool) {
        self.stored.smart_quotes = Some(on);
        self.save();
    }

    pub fn smart_dashes(&self) -> bool {
        self.stored.smart_dashes.unwrap_or(false)
    }
    pub fn set_smart_dashes(&mut self, on: bool) {
        self.stored.smart_dashes = Some(on);
        self.save();
    }

    pub fn text_replacement(&self) -> bool {
        self.stored.text_replacement.unwrap_or(true)
    }
    pub fn set_text_replacement(&mut self, on: bool) {
        self.stored.text_replacement = Some(on);
        self.save();
    }

    pub fn show_sidebar(&self) -> bool {
        self.stored.show_sidebar.unwrap_or(false)
    }
    pub fn set_show_sidebar(&mut self, on: bool) {
        self.stored.show_sidebar = Some(on);
        self.save();
    }

    /// `"recent"` or `"folder"`.
    pub fn sidebar_mode(&self) -> &str {
        self.stored.sidebar_mode.as_deref().unwrap_or("recent")
    }
    pub fn set_sidebar_mode(&mut self, mode: &str) {
        self.stored.sidebar_mode = Some(mode.into());
        self.save();
    }

    pub fn folder_local(&self) -> Option<&str> {
        self.stored.folder_local.as_deref()
    }
    pub fn folder_remote(&self) -> Option<&str> {
        self.stored.folder_remote.as_deref()
    }
    /// Sets the sidebar's folder: a local path, a remote location's display string, or neither.
    pub fn set_folder(&mut self, local: Option<String>, remote: Option<String>) {
        self.stored.folder_local = local;
        self.stored.folder_remote = remote;
        self.save();
    }

    /// Assist's provider, `codex` or `claude`; Codex unless set. `AssistController.swift:138`
    pub fn assist_provider(&self) -> writer_agents::AgentProvider {
        self.stored.assist_provider.as_deref().and_then(|p| p.parse().ok()).unwrap_or_default()
    }
    pub fn set_assist_provider(&mut self, raw: &str) {
        self.stored.assist_provider = Some(raw.into());
        self.save();
    }

    pub fn last_ssh_location(&self) -> &str {
        self.stored.last_ssh_location.as_deref().unwrap_or("")
    }
    pub fn set_last_ssh_location(&mut self, s: &str) {
        self.stored.last_ssh_location = Some(s.into());
        self.save();
    }

    /// The saved content rect `[x, y, w, h, scale]`. A scale of 0 marks a frame carried over
    /// from the Swift app, whose display scale isn't known: see [`crate::window::opening`].
    pub fn window_frame(&self) -> Option<[f64; 5]> {
        self.stored.window_frame
    }
    pub fn set_window_frame(&mut self, frame: [f64; 5]) {
        self.stored.window_frame = Some(frame);
        self.save();
    }

    pub fn window_decoration(&self) -> Option<[f64; 2]> {
        self.stored.window_decoration
    }
    pub fn set_window_decoration(&mut self, offset: [f64; 2]) {
        self.stored.window_decoration = Some(offset);
        self.save();
    }
}

/// The old app's `UserDefaults` as an XML property list, on macOS; nothing elsewhere or when the
/// domain doesn't exist.
fn read_user_defaults(bundle_id: &str) -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("/usr/bin/defaults").args(["export", bundle_id, "-"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A value in an XML property list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PlistValue {
    String(String),
    Number(f64),
    Bool(bool),
    Other,
}

/// The top-level dictionary of an XML property list, as `defaults export` and Info.plist
/// files write it. `None` when it isn't one.
pub(crate) fn plist_dict(xml: &str) -> Option<Vec<(String, PlistValue)>> {
    let doc = roxmltree::Document::parse_with_options(xml, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() }).ok()?;
    let dict = doc.root_element().children().find(|n| n.has_tag_name("dict"))?;
    let mut out = Vec::new();
    let mut key: Option<String> = None;
    for node in dict.children().filter(|n| n.is_element()) {
        if node.has_tag_name("key") {
            key = Some(node.text().unwrap_or("").to_string());
            continue;
        }
        let Some(k) = key.take() else { continue };
        let text = node.text().unwrap_or("").trim();
        let value = match node.tag_name().name() {
            "string" => PlistValue::String(node.text().unwrap_or("").to_string()),
            "real" | "integer" => text.parse().map_or(PlistValue::Other, PlistValue::Number),
            "true" => PlistValue::Bool(true),
            "false" => PlistValue::Bool(false),
            _ => PlistValue::Other,
        };
        out.push((k, value));
    }
    Some(out)
}

/// The settings the Swift app wrote (`Preferences.swift`, `assist.provider`, and the window
/// frame AppKit saved under `FluentWriterMain`), from the property list `defaults export`
/// prints. `None` when it holds none of them.
fn stored_from_plist(xml: &str) -> Option<Stored> {
    let dict = plist_dict(xml)?;
    let mut s = Stored::default();
    let mut any = false;
    for (key, value) in dict {
        let string = match &value {
            PlistValue::String(v) => Some(v.clone()),
            _ => None,
        };
        let boolean = match value {
            PlistValue::Bool(b) => Some(b),
            PlistValue::Number(n) => Some(n != 0.0),
            _ => None,
        };
        let number = match value {
            PlistValue::Number(n) => Some(n),
            _ => None,
        };
        let before = any;
        any = true;
        match key.as_str() {
            "focus" if string.is_some() => s.focus = string,
            "lastDimmingFocus" if string.is_some() => s.last_dimming_focus = string,
            "textSize" if number.is_some() => s.text_size = number,
            "appearance" if string.is_some() => s.appearance = string,
            "showCounts" if boolean.is_some() => s.show_counts = boolean,
            "spellCheck" if boolean.is_some() => s.spell_check = boolean,
            "showSidebar" if boolean.is_some() => s.show_sidebar = boolean,
            "sidebarMode" if string.is_some() => s.sidebar_mode = string,
            "folderLocal" if string.is_some() => s.folder_local = string,
            "folderRemote" if string.is_some() => s.folder_remote = string,
            "lastSSHLocation" if string.is_some() => s.last_ssh_location = string,
            "assist.provider" if string.is_some() => s.assist_provider = string,
            "NSWindow Frame FluentWriterMain" if string.as_deref().and_then(appkit_frame).is_some() => {
                s.window_frame = string.as_deref().and_then(appkit_frame);
            }
            _ => any = before,
        }
    }
    // The Swift app kept one folder at a time: a local one wins, as its getter read it first.
    if s.folder_local.is_some() {
        s.folder_remote = None;
    }
    any.then_some(s)
}

/// AppKit's saved frame, "x y w h screenX screenY screenW screenH" in points with the origin
/// at the bottom-left of the main display, as the content rect from the main display's
/// top-left that `preferences.json` keeps. With a full-size content view the window's frame is
/// its content. Only a frame on the main display (whose origin is 0, 0) gives the main
/// display's height, so a frame saved on another display isn't carried over and the window
/// opens centred. The scale is unknown, so it is written as 0.
fn appkit_frame(s: &str) -> Option<[f64; 5]> {
    let n: Vec<f64> = s.split_whitespace().map(str::parse).collect::<Result<_, _>>().ok()?;
    let [x, y, w, h, sx, sy, _sw, sh] = n[..] else { return None };
    (w > 0.0 && h > 0.0 && sx == 0.0 && sy == 0.0).then_some([x, sh - (y + h), w, h, 0.0])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `defaults export com.sunkenintime.fluentwriter -` prints for a Mac that used the
    /// Swift app: every key `Preferences.swift` writes, `assist.provider`, AppKit's own keys,
    /// and the window frame it saved under `FluentWriterMain`.
    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSNavLastRootDirectory</key>
	<string>~/Documents</string>
	<key>NSWindow Frame FluentWriterMain</key>
	<string>210 118 1060 760 0 0 1512 944 </string>
	<key>appearance</key>
	<string>dark</string>
	<key>assist.provider</key>
	<string>claude</string>
	<key>focus</key>
	<string>typewriter</string>
	<key>folderLocal</key>
	<string>/Users/me/Notes</string>
	<key>lastDimmingFocus</key>
	<string>paragraph</string>
	<key>lastSSHLocation</key>
	<string>me@server:~/notes</string>
	<key>showCounts</key>
	<false/>
	<key>showSidebar</key>
	<true/>
	<key>sidebarMode</key>
	<string>folder</string>
	<key>spellCheck</key>
	<false/>
	<key>textSize</key>
	<real>24</real>
	<key>NSSplitView Subview Frames</key>
	<array><string>0.000000, 0.000000, 260.000000</string></array>
</dict>
</plist>
"#;

    #[test]
    fn every_setting_the_swift_app_saved_carries_over() {
        let p = Prefs { path: None, stored: stored_from_plist(SAMPLE).expect("settings found") };
        assert_eq!(p.focus(), FocusUnit::Typewriter);
        assert_eq!(p.last_dimming_focus(), FocusUnit::Paragraph);
        assert_eq!(p.text_size(), 24.0);
        assert_eq!(p.appearance(), Appearance::Dark);
        assert!(!p.show_counts());
        assert!(!p.spell_check());
        assert!(p.show_sidebar());
        assert_eq!(p.sidebar_mode(), "folder");
        assert_eq!((p.folder_local(), p.folder_remote()), (Some("/Users/me/Notes"), None));
        assert_eq!(p.last_ssh_location(), "me@server:~/notes");
        assert_eq!(p.assist_provider(), writer_agents::AgentProvider::Claude);
        // AppKit's frame is bottom-left based: 944 − (118 + 760) = 66 from the top.
        assert_eq!(p.window_frame(), Some([210.0, 66.0, 1060.0, 760.0, 0.0]));
    }

    #[test]
    fn a_remote_folder_integer_sizes_and_frames_off_the_main_display_import_as_the_swift_read_them() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>
            <key>folderRemote</key><string>me@host:~/notes</string>
            <key>textSize</key><integer>17</integer>
            <key>NSWindow Frame FluentWriterMain</key><string>1600 100 900 700 1512 0 1920 1080 </string>
        </dict></plist>"#;
        let p = Prefs { path: None, stored: stored_from_plist(xml).unwrap() };
        assert_eq!(p.folder_remote(), Some("me@host:~/notes"));
        assert_eq!(p.text_size(), 17.0);
        assert_eq!(p.window_frame(), None, "on a second display: opens centred");
        assert!(stored_from_plist("<plist><dict><key>other</key><string>x</string></dict></plist>").is_none(), "nothing of ours");
        assert!(stored_from_plist("not xml").is_none());
    }

    #[test]
    fn the_import_runs_once_and_only_without_preferences_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("preferences.json"), r#"{"textSize": 15}"#).unwrap();
        let p = Prefs::load_or_import(dir.path(), "com.sunkenintime.fluentwriter.test-nonexistent");
        assert_eq!(p.text_size(), 15.0, "an existing file wins");
        let empty = tempfile::tempdir().unwrap();
        let p = Prefs::load_or_import(empty.path(), "com.sunkenintime.fluentwriter.test-nonexistent");
        assert_eq!(p.text_size(), 19.0, "nothing to import: the defaults");
    }

    #[test]
    fn an_imported_frame_opens_at_its_size_and_is_placed_in_points() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = Prefs::load(dir.path());
        p.set_window_frame([210.0, 66.0, 1000.0, 700.0, 0.0]);
        let o = crate::window::opening(&p);
        assert_eq!((o.size, o.frame, o.place, o.options.position), ((1000.0, 700.0), Some([210.0, 66.0, 1000.0, 700.0]), true, None));
        p.set_window_frame([210.0, 66.0, 1000.0, 700.0, 2.0]);
        let o = crate::window::opening(&p);
        assert_eq!((o.place, o.options.position), (false, Some((420, 132))), "a frame the Rust app saved has its scale");
    }
}
