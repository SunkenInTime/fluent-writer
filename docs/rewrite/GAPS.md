# Known differences from the Swift app

Running list of what the Rust rewrite does not yet match, with the reason. Remove an entry
when it is closed and checked.

## Images
- Format › Image… picks one file at a time. The Swift panel allowed several. Rune's file
  dialog returns a single path; the fork needs a multi-select open.

- Copy Image works for image files only, not for images loaded from the web.
- Previews are decoded at 2× regardless of the screen's scale.
- Pasting or dropping an image while a version is being previewed should beep and do nothing;
  to wire up once the version preview is merged.

## Editor
- One keystroke on a 10,000-word draft takes about 20 ms on the t3os box (release build).
  The whole draft is re-parsed, re-styled and re-shaped on every edit; the Swift app restyled
  only the edited lines.
- Preferences are stored in `preferences.json` in the support folder. A Mac user's existing
  settings in `UserDefaults` are not read.

## Platforms
- macOS: the native menu bar is not wired yet (Rune's native menus have no submenus), and
  nothing has been built or run on a Mac.
- Windows: nothing has been built or run on Windows.
- Linux: Rune has no spell checker there.
- No grammar checking on any platform (Rune has none).
