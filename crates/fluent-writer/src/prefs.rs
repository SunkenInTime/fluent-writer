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
    smart_quotes: Option<bool>,
    smart_dashes: Option<bool>,
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
