# Known differences from the Swift app

Running list of what the Rust rewrite does not yet match, with the reason. Remove an entry
when it is closed and checked. PARITY.md marks each item; this file says why the open ones are
open.

## Editor
- The caret doesn't blink. Rune draws a steady caret in text fields (nothing in its paint or
  runtime code turns it off and on); AppKit's caret blinks.
- Scrolling to the caret glides on a critically damped 0.14 s spring rather than AppKit's
  0.14 s ease-out curve: Rune's scroll glides (`Tx::scroll_to_with`) take only springs. Under
  Reduce Motion both are instant.
- Not built, blocked in Rune: dragging selected text to move it, dragging text out of the
  draft, and dropping text (Rune's drops are files only); Look Up / the dictionary panel;
  ⌘⌫ deletes to the paragraph's start rather than the visual line's; ⌃V and ⌥PageUp caret
  paging.
- Undo names for edits AppKit names itself (the Swift sets none) are guesses not checked on a
  Mac: ⌃K ⌃Y ⌃T ⌃O and the substitutions "Typing", the find bar's "Replace" and "Replace
  All", Edit › Cut "Cut".
- Text replacement reads the Mac's list with `defaults export -g`; that read was never run
  on a Mac. Windows and Linux have no system list, so the setting does nothing there.

## Spelling and grammar
- Linux: Rune has no spell checker there.
- No grammar checking on any platform (Rune has none), so Check Grammar With Spelling isn't in
  the menu.
- Show Spelling and Grammar (the panel) and Check Document Now aren't built: Rune has no
  spelling panel, and Check Document Now needs Rune to make `spell::misspelled(text)` public.
  Correct Spelling Automatically needs `spell::misspelled` and `spell::suggest`. Rune's menu
  has no Ignore Spelling.

## Images
- Copy Image works for image files only, not for images loaded from the web
  (`platform::copy_image`).
- Local images are decoded at full size (`ImageCache::load`), not downsampled to the column as
  the Swift's ImageIO thumbnail was: the same picture, more memory for very large photos.
  Whether Rune's decoder reads TIFF and HEIC wasn't checked.
- Dropping PNG or TIFF image data (rather than files) does nothing: Rune's drops are files
  only.

## Version preview
- The diff's accessibility value is the diff's own text, not the version's text as the Swift
  set it: Rune gives a text field's value as its text and ignores `a11y_value` on it
  (`a11y/node.rs` sets the field's text after the element's own value).

## Locale
- Thousands separators in the counts and in Copy for Twitter are always commas
  (`text_stats::format`, `platform::grouped`); the Swift's `NumberFormatter` followed the
  user's locale. Relative dates and short times are US English for the same reason. Fixing it
  needs a way to read the locale's number and date formats on all three platforms.

## Window
- Opening the Assist panel in a narrow window widens it to 720 pt (980 with the sidebar) but
  doesn't cap that to the screen's visible width: Rune gives the app no screen size. Rune keeps
  the window on its screen.
- The sidebar peek starts even while a mouse button is held: Rune doesn't tell the app
  whether one is down.
- Save As is a sheet on macOS, not the app-modal panel the Swift used (oddity 27): Rune has no
  app-modal save panel.
- View › Enter Full Screen (F11 off macOS) did nothing visible in a real X11 window on the
  t3os box at the end of this audit; not explained (the window manager there may not support
  it). Not tried on Windows or a Mac.

## Preferences
- Preferences are stored in `preferences.json` in the support folder. On macOS, the first
  launch without that file imports the Swift app's `UserDefaults` (`defaults export
  com.sunkenintime.fluentwriter -`, or `.dev`): every key `Preferences.swift` wrote,
  `assist.provider`, and the window frame AppKit saved under `FluentWriterMain` (kept only when
  it was on the main display, then placed in points once the window is up, since its scale
  isn't known). Unit-tested against a sample plist (`prefs::tests`); never run on a Mac.

## Packaging
- There is no packaging for the Rust app. `script/build-app.sh` still builds the Swift app;
  the Rust app needs its own bundle on macOS (Info.plist with `FWVariant`, the bundle ids,
  document types, `fluent-writer-mcp` beside the binary, `bridge/claude`, ad-hoc signing) and
  installers for Windows and Linux. That is a design decision (formats, signing) for the
  owner. Fonts need no bundling: they are compiled in. The variant already reads `FWVariant`
  from a bundle's Info.plist when there is one.

## Platforms
- macOS: the native menu bar is not wired yet (Rune's native menus have no submenus), so the
  app menu (About, Hide, Hide Others, Show All, Quit), the Window menu's window list and the
  system's Edit additions (Start Dictation, Emoji & Symbols) aren't there; nothing has been
  built or run on a Mac. Mac-only behaviours not checked: the transparent title bar and
  traffic lights (fading them with the chrome needs a Rune API), the proxy icon
  (`representedURL`), window tabbing (Rune has no `tabbingMode`), Finder open events (Rune
  passes none), the recent-documents list, app activation at launch, IME and dictation.
- Windows: file dialogs show neither the Open Folder button label nor a panel message (rfd
  doesn't reach `IFileDialog::SetOkButtonLabel`); the message is the dialog's title instead.
  The Linux portal takes the button label but has no message either.

## Windows (checked on a real PC at 11c5070)
- Typing in a 10,000-word draft no longer stalls: catch-up was 0.10 s at 12 and at 40
  characters a second in three fresh runs each, and about 1 s at 150 a second; saves were
  exact; spell check on or off made no difference.
- The keyboard context menu (Menu key, Shift+F10) now opens under the caret, except right
  after opening a long draft, before its caret is scrolled into view, when it appeared near the
  bottom of the screen. Cause not established.
- Ctrl+Alt+M (Add Comment) is taken system-wide on that PC by NVIDIA Overlay's microphone
  toggle (`RegisterHotKey` fails with 1409 even with the app closed). Assist › Add Comment…
  from the menu works, and Ctrl+Alt+J, V, C, L, 0 and O all work, so this is a conflict with
  another program, not an app bug.
- The test helper's folder lock now denies `DE` instead of `D` (which also denied Synchronize
  and so blocked listing the folder); with it all 149 app tests passed on Windows.
- The system beep was requested but not confirmed by ear.
- `writer-agents`' process tests are Unix-only, so child-process handling is untested on
  Windows.
- Folder and file dialogs can't show a custom button label or message (the system dialog
  has no way to set them through the library Rune uses).
