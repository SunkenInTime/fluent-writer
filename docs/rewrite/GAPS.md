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

## Windows (checked on a real PC at a9c1d6f)
- Typing in a 10,000-word draft sometimes stalls for many seconds: the first runs took 30 s
  to catch up after 300 characters at 12 a second, later runs 1 to 2 s. No characters are
  lost. Not yet explained; the Windows spell checker running over the whole draft on each
  edit was the likely cause: Rune checked the whole text on every edit. The fork now checks a
  paragraph at a time; not yet re-measured on Windows.
- A context menu opened from the keyboard (Menu key, Shift+F10) appeared at the window's
  lower-left corner. Fixed in the fork (it opens under the caret); not yet re-checked on
  Windows.
- Ctrl+Alt+M (Add Comment) did not open the composer with real key events, while
  Ctrl+Alt+O worked. Not yet explained: Ctrl+Alt is AltGr on Windows, or another program
  may hold the chord.
- The system beep was requested but not confirmed by ear.
- One test (`opening_from_the_sidebar_or_the_picker_runs_the_leave_check`) fails because the
  `icacls` deny rule the tests use to block saves also blocks listing the folder.
- Folder and file dialogs can't show a custom button label or message (the system dialog
  has no way to set them through the library Rune uses).
