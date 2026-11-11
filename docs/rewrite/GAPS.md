# Known differences from the Swift app

Running list of what the Rust rewrite does not yet match, with the reason. Remove an entry
when it is closed and checked.

## Images
- Copy Image works for image files only, not for images loaded from the web.
- Previews are decoded at 2× regardless of the screen's scale.

## Editor
- One keystroke on a 10,000-word draft takes about 20 ms on the t3os box (release build).
  The whole draft is re-parsed, re-styled and re-shaped on every edit; the Swift app restyled
  only the edited lines.
- Opening the draft's context menu from the keyboard (Menu key, Shift+F10) puts it at the
  draft field's bottom-left, which is the window's lower-left, not at the caret. The cause is
  in Rune: `Runtime::context_menu_key` (`runtime/input/secondary.rs`) anchors every keyboard
  menu at `drawn_box(focused)`'s bottom-left, and the draft field fills the editor. For a text
  field it should anchor at the caret, as AppKit and Windows edit controls do. The app's own
  menu on Linux gets the same point from Rune.
- Inline code, comment and diff backgrounds paint over the selection highlight (Rune paints
  span and highlight backgrounds after the field's selection); AppKit draws the selection
  over a `.backgroundColor`, so selected inline code shows the selection colour.
- Undo names for edits AppKit names itself (the Swift sets none) are guesses not checked on a
  Mac: ⌃K ⌃Y ⌃T ⌃O and the substitutions "Typing", the find bar's "Replace" and "Replace
  All", Edit › Cut "Cut".
- Preferences are stored in `preferences.json` in the support folder. A Mac user's existing
  settings in `UserDefaults` are not read.

## Platforms
- macOS: the native menu bar is not wired yet (Rune's native menus have no submenus), and
  nothing has been built or run on a Mac.
- Windows: nothing has been built or run on Windows. Its file dialogs show neither the Open
  Folder button label nor a panel message (rfd doesn't reach `IFileDialog::SetOkButtonLabel`);
  the message is the dialog's title instead. The Linux portal takes the button label but has no
  message either.
- Linux: Rune has no spell checker there.
- No grammar checking on any platform (Rune has none).
