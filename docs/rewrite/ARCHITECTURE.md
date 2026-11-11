# How the Rust rewrite is put together

Fluent Writer is being rewritten from Swift/AppKit (`Sources/`, `Tests/`, the spec) to Rust
on the Rune UI framework, to run on macOS, Windows and Linux. The owner's requirement is exact
parity with the Swift app. `docs/rewrite/PARITY.md` lists every behaviour to match, with the
Swift file and line for each.

## Crates

- `crates/writer-core`: UI-free logic (Markdown parsing, text units, edits, drafts on disk,
  notes, SSH). A port of `Sources/WriterCore`. Offsets are UTF-8 bytes; files on disk keep the
  Swift formats.
- `crates/writer-agents`, `crates/fluent-writer-mcp`: ports of `Sources/WriterAgents` and
  `Sources/FluentWriterMCP`.
- `crates/fluent-writer`: the app.

## Rune

The UI framework is a fork: https://github.com/SunkenInTime/rune, branch `fluent`, pinned by
revision in `crates/fluent-writer/Cargo.toml`. Its docs are `crates/rune/README.md` in that
repo (long; search it by heading) and `crates/rune-kit/README.md`. A checkout is at
`~/Code/rune`. Do not edit the fork from a feature task: if Rune can't do something, build it
in the app when that is reasonable, otherwise stop and report exactly what is missing.

Rune in one paragraph: the app is one state struct (`App`), a pure `view(&App, &mut Cx) -> El`
that rebuilds the element tree, and handlers (`.on_press(|s: &mut App| ..)`,
`.on_press_tx(|s, tx| ..)`) that mutate the state between frames. `Tx` carries requests to the
shell (scroll, focus, clipboard, dialogs, timers). Elements are positioned by flexbox
(`row()`, `col()`, `.grow()`, `.gap()`) or `.absolute(x, y).size(w, h)`. Text fields are
`text_input(&edit, style)` bound to a `rune::TextEdit` in the state with `.on_edit(..)`.

## The app crate

| File | Holds |
| --- | --- |
| `main.rs` | Arguments, picking the library and first draft, `rune::run`. |
| `app.rs` | `App` (all state), the window `Frame` (sidebar, editor, Assist panel widths), loading and saving, status and counts, focus modes, scrolling, chrome fading, `poll`. |
| `view.rs` | The window's element tree: top bar, draft field, footer. |
| `styler.rs` | Markdown lines to spans, hanging indents and strikes for the draft field. |
| `commands.rs` | Every menu command: title, menu position, shortcut per platform, enabled and checked rules, and `App::run(command, tx)`. |
| `keys.rs` | Key handling for the draft field and the window. |
| `theme.rs` | Colours (`Palette::new(dark)`), fonts, column metrics. |
| `prefs.rs` | Persisted settings. |
| `platform.rs` | Per-OS helpers outside the UI. |
| `tests.rs` | Headless tests that drive the whole app. |

Conventions a feature follows:

- One module per feature, mirroring the Swift file (`Sidebar.swift` → `sidebar.rs`). Its
  state is one struct in that module, held as one field of `App`. Its elements come from one
  `pub fn view(s: &App, p: &Palette, f: &Frame) -> El` (or similar) that `view.rs` adds as a
  child. Keep the lines you add to `app.rs`, `view.rs` and `commands.rs` few, so parallel work
  merges cleanly.
- Commands: fill in the arm for your command in `App::run` (replacing its fall-through to the
  "isn't built yet" message), and its rules in `command_enabled`, `command_checked` and
  `command_title`. Don't add shortcuts anywhere else.
- Timers: there is no timer object. Store a deadline (`Option<f64>` on the app clock,
  `self.now`), ask for a wake with `tx.wake_after(..)`, and act on it in `App::poll`. See
  `autosave_at` in `app.rs`.
- Slow work (file listings, ssh, network, agents) never runs on the UI thread. Run it on a
  thread and bring the result back with `rune::Latest` or an `AppHandle`/`Waker`
  (README: "Requests off the main thread").
- Layout numbers, colours, strings and timings come from the Swift code, via PARITY.md. Use
  `Palette` for every colour and `ui_font`/`draft_font` for every font.
- Text offsets are UTF-8 bytes everywhere in Rust.
- Platform differences: the Mac's ⌘ is `mod` in a chord (Ctrl elsewhere). Anything
  Mac-only in the Swift (Finder, sheets, `NSWorkspace`) needs a Windows and Linux equivalent.

## Checking work

`cargo test -p fluent-writer` runs headless tests through Rune's `Runtime`: the real view,
layout and key routing on a virtual clock. `tests.rs` has the harness (`harness`, `settle`,
`type_text`, `press`, `command`). Add tests there, or in a `#[cfg(test)]` module of your
feature file, for each behaviour you build.

To look at the app: `cargo build -p fluent-writer`, then
`target/debug/fluent-writer --drafts DIR --support DIR [--open FILE] --snap out.png [--theme
light|dark] [--scale 1] [--size 1060x760] [--script steps.json]` renders a PNG with no window
(README: "Agent tooling"). `--tree out.json` dumps the element tree. Scripts send clicks,
keys and typing. Read the PNG to check your work against the screenshots in
`docs/screenshots/` and the numbers in PARITY.md. On this machine a real window can also be
opened on the session's private X display.

A feature is done when its PARITY.md items are each built and checked this way, and
`cargo clippy -p fluent-writer --all-targets` is clean.
