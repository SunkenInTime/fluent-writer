# Fluent Writer parity checklist

Everything the Swift/AppKit app does, one observable behaviour per item, so the Rust/Rune rewrite can be checked item by item. Sources are `File.swift:line` under `Sources/FluentWriter/` unless a longer path is given. `WriterCore/…` and `WriterAgents/…` are under `Sources/`.

Items marked **(AppKit built-in)** come from AppKit with no app code beyond turning them on. The rewrite has to build them by hand.

Colours are sRGB 0–255 from `Theme.swift`, written `light / dark`. Alpha is given when it isn't 1.

---

## 1. Window and chrome

### Theme colours (each is a light/dark pair that follows the effective appearance)

- [x] `canvas` 247,247,247 / 26,26,26 — window background, top bar, footer, Assist panel background. `Theme.swift:35`
- [x] `text` 25,25,25 / 218,218,216 — body text. `Theme.swift:36`
- [x] `dimmed` 199,196,194 / 78,78,76 — text outside the focused sentence or paragraph. `Theme.swift:37`
- [x] `markup` 172,170,168 / 112,112,110 — Markdown markers, link URLs, underline colour. `Theme.swift:38`
- [x] `quiet` 150,148,146 / 120,120,118 — chrome labels, code-block text, secondary text. `Theme.swift:39`
- [x] `caret` 0,190,255 / 0,175,245 — caret, active toolbar button tint. `Theme.swift:40`
- [x] `selection` 178,228,250 / 22,78,104 — selected-text background. `Theme.swift:41`
- [x] `hairline` 228,228,226 / 44,44,44 — 1 px dividers, card borders, image frame. `Theme.swift:42`
- [x] `sidebar` 240,240,239 / 32,32,32 — sidebar background. `Theme.swift:43`
- [x] `sidebarSelection` 222,222,220 / 52,52,52 — sidebar selected row; toolbar button plate. `Theme.swift:44`
- [x] `sidebarCurrent` 230,230,228 / 44,44,44 — sidebar row of the open file. `Theme.swift:45`
- [x] `codeBackground` 236,236,234 / 38,38,38 — inline code background, image placeholder fill. `Theme.swift:46`
- [x] `warning` 196,54,36 / 240,110,90 — failed-save and failed-request text. `Theme.swift:47`
- [x] `inserted` 30,130,70 / 110,200,140 — diff inserted text. `Theme.swift:48`
- [x] `insertedBackground` 30,160,80 α0.13 / 90,200,120 α0.18. `Theme.swift:49`
- [x] `deleted` 180,50,40 / 235,120,105 — diff deleted text and its strikethrough. `Theme.swift:50`
- [x] `deletedBackground` 210,60,40 α0.11 / 230,90,70 α0.17. `Theme.swift:51`
- [x] `commentHighlight` 255,196,0 α0.20 / 255,196,40 α0.16. `Theme.swift:52`
- [x] `commentHighlightActive` 255,180,0 α0.42 / 255,190,40 α0.34. `Theme.swift:53`
- [x] `card` 255,255,255 / 34,34,34. `Theme.swift:54`
- [x] `cardHover` 252,252,251 / 40,40,40. `Theme.swift:55`
- [x] `cardSelected` 234,246,253 / 26,54,70. `Theme.swift:56`
- [x] `icon` 120,118,116 / 172,172,170 — toolbar icons at rest. `Theme.swift:57`
- [x] `banner` 238,238,236 / 36,36,36 — version preview bar. `Theme.swift:58`

### Window

- [x] One window per app. Initial content size 1060×760 pt. `EditorController.swift:111-115`
- [x] Window style: titled, closable, miniaturizable, resizable, full-size content view. `EditorController.swift:113` — *Rust: Rune's window; checked on Linux (X11).*
- [ ] Title bar is transparent and the window title text is hidden; content draws under the title bar. `EditorController.swift:140-141` — *Rust: Built (`Chrome { full_size_content, drag_regions }`, the top bar is the drag region); not run on a Mac. Windows draws its own 40 pt title bar with the system buttons over its right end (`Titlebar`); Linux keeps the window manager's frame, which Rune can't replace there.*
- [x] Window background colour is `canvas`. `EditorController.swift:142`
- [x] Minimum window size 420×320 pt. `EditorController.swift:143`
- [ ] Window tabbing is disabled (no tab bar, no "Merge All Windows"). `EditorController.swift:144` — *Rust: Rune has no option for `NSWindow.tabbingMode`; not built.*
- [x] Window frame is saved and restored under the autosave name `FluentWriterMain`; with no saved frame the window is centred on screen. **(AppKit built-in)** `EditorController.swift:146-147` — *Rust: Saved in preferences.json as the content rect, scale and (off macOS) the decoration offset; restored at launch; centred on the first frame when none was saved. Checked in a real X11 window. Not kept on screen if the saved display is gone.*
- [x] Mouse-moved events are delivered to the window (needed for chrome reveal and sidebar peek). `EditorController.swift:149`
- [x] Window title (used by the Window menu, Mission Control and accessibility) is the draft title. `EditorController.swift:356`, `EditorController.swift:440`
- [ ] Window `representedURL` is the draft's file for local files and nil for remote files and unsaved drafts. **(AppKit built-in: Window-menu/proxy behaviour of representedURL)** `EditorController.swift:355`, `EditorController.swift:441` — *Rust: Rune has no API for a window's represented file (proxy icon).*
- [x] Closing the window first runs the leave-document check (save, or ask if the save failed); cancelling keeps the window open. `EditorController.swift:841-843` — *Rust: `App::on_close_request` runs the check before anything hides; when it asks, the window stays up as it was with the alert, Keep Editing leaves it open and Continue quits. Test: `a_refused_close_keeps_the_window_up_with_the_alert`. The new flow was not run in a real window (the hide-then-reopen flow it replaces was checked in X11).*
- [x] When the window closes, the app quits. `EditorController.swift:845-847`, `main.swift:48`
- [ ] Full screen via View › Enter Full Screen (⌃⌘F); entering and leaving full screen re-lays out the window. **(AppKit built-in: full screen itself)** `MainMenu.swift:131`, `EditorController.swift:838-839` — *Rust: Wired to `Tx::toggle_full_screen` (F11 off macOS), title follows; not observed in a real window.* Sent F11 to a real X11 window on this box at the end of the audit: the window stayed as it was (the window manager here may not do full screen); not explained.
- [x] In full screen the traffic lights are hidden by the system. **(AppKit built-in)** — *Rust: The system's.*
- [x] Window resize re-lays out every region (sidebar, top bar, footer, editor, Assist panel, preview). `EditorController.swift:826`, `EditorController.swift:238-259`

### Layout regions

- [x] Top bar is 40 pt tall, filled with `canvas`, spans only the editor column (between sidebar and Assist panel). `EditorController.swift:92`, `EditorController.swift:250`, `EditorController.swift:34-40`
- [x] Footer is 34 pt tall, filled with `canvas`, at the bottom of the editor column, drawn over the bottom of the text scroll view. `EditorController.swift:93`, `EditorController.swift:251`
- [x] Editor scroll view starts 40 pt from the top and runs to the bottom of the window (under the footer). `EditorController.swift:252`
- [x] Assist panel width: 0 when hidden; when shown `min(380, max(260, round(windowWidth × 0.5)))`. `EditorController.swift:84`, `EditorController.swift:241`
- [x] Pinned sidebar width: `min(260, max(0, windowWidth − assistWidth − 320))` so the editor keeps at least 320 pt when possible. `EditorController.swift:242`
- [x] While the sidebar peeks it is always 260 pt wide and does not push the editor. `EditorController.swift:244-246`
- [x] Editor column width = window width − Assist width − pinned sidebar width. `EditorController.swift:243`
- [x] Assist panel sits at the right edge, full window height. `EditorController.swift:253` Test `the_assist_panel_runs_down_the_right_edge`.

### Chrome fade

- [ ] Chrome = title field, status label, counts label, focus label, sidebar button, the four toolbar buttons, and the traffic lights (traffic lights only when the sidebar is not pinned). The bars themselves do not fade. `EditorController.swift:682` — *Rust: All built (plus the in-window menu bar) except the traffic lights: Rune has no API to fade them.*
- [x] Chrome fades out over 0.6 s and in over 0.25 s; with Reduce Motion both are instant. `EditorController.swift:683-687` — *Rust: Ease-in-out tweens. Reduce Motion asks for an instant change; Rune may still give opacity its 0.15 s reduced-motion fade.*
- [x] Any key press in the editor hides the chrome immediately, unless the save state is failed or the Assist panel is open. `EditorController.swift:650-655`, `EditorTextView.swift:61-64`
- [x] Chrome does not hide while the title field is being edited. `EditorController.swift:659`
- [x] Moving the mouse over the window shows the chrome and starts a 3.0 s auto-hide timer. `EditorController.swift:150-153`, `EditorController.swift:664-679`
- [x] When the 3.0 s timer fires it hides the chrome only if: the window is key, the editor text view is first responder, the save state is not failed, and the pointer is not over the top bar, the footer, or the sidebar panel. `EditorController.swift:672-676`
- [x] When the window loses key status, the draft is saved and the chrome is shown with no auto-hide. `EditorController.swift:828-831`
- [x] When the window becomes key, the chrome is shown with auto-hide, and the sidebar list reloads if the sidebar is pinned. `EditorController.swift:833-836` — *Rust: The sidebar reload is left to the sidebar port (marked in `window.rs`).*
- [x] A failed save (local or remote upload) shows the chrome with no auto-hide. `EditorController.swift:469`, `EditorController.swift:485` — *Rust: Local; the remote upload is the SSH port's.*
- [x] Every transient footer message shows the chrome with auto-hide. `EditorController.swift:494`

### Toolbar buttons (ChromeButton)

- [x] Four buttons pinned to the window's top-right corner, right to left: Assist, Versions, Comments, then Add Comment. They do not move when panels open. `ChromeControls.swift:98-109`
- [x] Button slot is 28×24 pt; the rightmost slot's right edge is 12 pt from the window edge; 2 pt between Assist/Versions/Comments, 10 pt between Comments and Add Comment. `ChromeControls.swift:10`, `ChromeControls.swift:101-108`
- [x] Buttons are vertically centred on the sidebar button. `ChromeControls.swift:102`, `ChromeControls.swift:106`
- [x] Buttons always draw above other views (re-raised on every layout). `ChromeControls.swift:109`
- [x] Add Comment: SF Symbol `plus.bubble`, accessibility label "Add comment", tooltip "Comment on the selection or this line (⌥⌘M)", runs Add Comment. `ChromeControls.swift:71`, `ChromeControls.swift:83-84` — *Rust: Drawn as SVG; runs Add Comment (placeholder until the comments port).* Audit: on an empty draft the button did nothing; as an unvalidated button it now runs Add Comment and beeps (test `the_add_comment_button_beeps_on_an_empty_draft`).
- [x] Comments: `text.bubble`, label "Comments", tooltip "Comments (⇧⌘M)", toggles the Assist panel on its Comments tab. `ChromeControls.swift:72`, `ChromeControls.swift:85-86`, `ChromeControls.swift:159-166` — *Rust: Drawn as SVG; the toggle is `App::toggle_panel`, a placeholder until the Assist port.*
- [x] Versions: `clock.arrow.circlepath`, label "Versions", tooltip "Versions (⌥⌘V)", toggles the Assist panel on its Versions tab. `ChromeControls.swift:73`, `ChromeControls.swift:87-88` — *Rust: As Comments.*
- [x] Assist: `sparkles`, label "Assist", tooltip "Assist (⌥⌘J)", toggles the Assist panel on its Ask tab. `ChromeControls.swift:74`, `ChromeControls.swift:89-90` — *Rust: As Comments.*
- [x] Toggle rule: if the panel is open on that tab, close it; otherwise open it on that tab (keeping the current provider). `ChromeControls.swift:159-162` — *Rust: Needs the Assist panel; `App::toggle_panel` is the hook.* Filled in by the Assist port: `App::assist_toggle_tab`; tested (`toolbar_buttons_toggle_their_tab_and_show_state`).
- [x] Symbols are drawn at 13 pt, regular weight, image only, no border. `ChromeControls.swift:14-18` — *Rust: SVG bodies in a 16 pt box with 1.1 pt strokes, the size a 13 pt symbol draws at.*
- [x] Icon tint: `caret` when active, `text` while hovered, `icon` otherwise. `ChromeControls.swift:47`
- [x] Hover/active plate: rounded rect, corner radius 6, filled with `sidebarSelection` at α0.7 on hover and α0.9 when active; clear otherwise. `ChromeControls.swift:26`, `ChromeControls.swift:49-50`
- [x] Hover tracking only while the window is key. `ChromeControls.swift:33`
- [x] Colours refresh when the appearance changes. `ChromeControls.swift:41-44`
- [x] Count badge: IBM Plex Mono Medium 9 pt, centred, in the button's top-right corner, 11 pt tall, width `max(12, textWidth + 2)`, 1 pt from the right edge. `ChromeControls.swift:19-21`, `ChromeControls.swift:53-57` — *Rust: Built; no count shows until the comments port fills `open_comment_count`.*
- [x] Badge text is the count, or "99+" above 99; hidden at 0. `ChromeControls.swift:60-66`
- [x] Badge colour is `caret` when the button is active, `text` otherwise. `ChromeControls.swift:48`
- [x] Button accessibility value is the count when > 0, none otherwise. `ChromeControls.swift:63`
- [x] Comments badge shows the number of open comments (detached ones included). `ChromeControls.swift:136-137` — *Rust: Hook `App::open_comment_count` for the comments port.* Filled in by the Assist port; tested.
- [x] Comments button is active while the Assist panel is open on Comments. `ChromeControls.swift:140` — *Rust: Hook `App::open_panel_tab` for the Assist port.* Filled in by the Assist port; tested.
- [x] Versions button is active while the panel is open on Versions, or while a version preview is showing. `ChromeControls.swift:141` — *Rust: Hooks `open_panel_tab` and `previewing_version`.* Filled in by the Assist port; tested.
- [x] Assist button is active while the panel is open on Ask. `ChromeControls.swift:142` — *Rust: Hook `open_panel_tab`.* Filled in by the Assist port; tested.

### Sidebar button

- [x] SF Symbol `sidebar.left`, label "Toggle sidebar", tooltip "Show or hide the sidebar (⌃⌘S). Point at the left edge to peek." `EditorController.swift:77` — Rust: the chrome's button (`chrome.rs`), drawn as a vector icon; off macOS the tooltip names that platform's shortcut.
- [x] Placed 12 pt right of the zoom (green) traffic light, vertically centred on it; in full screen at x 12, y 8. Size 28×24. `ChromeControls.swift:121-128` — Rust: `chrome::layout`, centred on the traffic lights where Rune reports them. Off macOS the sidebar panel's fill and hairline start under the 40 pt top band, so the in-window menu bar, which runs past a pinned sidebar's edge, sits on one bar.
- [x] Always drawn above the sidebar panel. `ChromeControls.swift:129-132`
- [x] Active (caret tint and plate) while the sidebar is pinned. `ChromeControls.swift:143`
- [x] Click toggles the sidebar; if the sidebar is peeking, click pins it instead. `RemoteEditing.swift:49-62`, `ChromeControls.swift:249-258`

### Title field position

- [x] Title field is centred in the top bar, 18 pt tall, at `midY − 9` of the sidebar button. `ChromeControls.swift:116`
- [x] Title width = `max(0, min(420, topBarWidth − 2 × reserve))`, where reserve = the largest of: the part of the toolbar buttons over the top bar (+8 pt), the sidebar button's right edge + 12 pt inside the top bar, and 24 pt. `ChromeControls.swift:110-115` — *Rust: Off macOS the reserve also clears the in-window menu bar.*
- [x] Title field is hidden when its width is under 60 pt. `ChromeControls.swift:117`

### Opening the Assist panel in a narrow window

- [ ] Opening the Assist panel widens the window to 720 pt (980 pt if the sidebar is pinned) when it is narrower, capped to the screen's visible width, shifted left to stay on screen, animated unless Reduce Motion. Not done in full screen. `ChromeControls.swift:147-157`, `AssistController.swift:380-384` `App::make_room_for_panel` asks Rune (`Tx::resize_window`) for the width; Rune keeps the window on its screen and jumps under Reduce Motion. Checked in a real window on Linux: 600 pt wide became 720. Not done: the cap to the screen's visible width (Rune gives no screen size) and the full-screen exception. Audit: the full-screen exception is now built (test `opening_assist_widens_a_narrow_window_except_in_full_screen`); still not capped to the screen's visible width, as Rune gives no screen size (GAPS).

---

## 2. Editor surface and typography

- [x] Body font is IBM Plex Mono (bundled OTFs, registered for the process at launch). `Theme.swift:68-77`, `main.swift:8` Audit: the faces are compiled into the binary (`theme::FONTS`) and registered with Rune at launch, so there is nothing to look up; snapshot of a styled draft shows regular, bold, italic and bold italic.
- [x] Font faces: regular `IBMPlexMono`, bold `IBMPlexMono-Bold`, italic `IBMPlexMono-Italic`, bold italic `IBMPlexMono-BoldItalic`. `Theme.swift:94-102` Audit: the faces are compiled into the binary (`theme::FONTS`) and registered with Rune at launch, so there is nothing to look up; snapshot of a styled draft shows regular, bold, italic and bold italic.
- [x] Fallback when the font is missing: the system monospaced font (bold weight for bold; italic trait for italic). `Theme.swift:103-106` Audit: can't happen, the faces are compiled in.
- [x] UI font is IBM Plex Mono regular, or IBM Plex Mono Medium when weight is medium; any other weight uses regular. Fallback system monospaced at that weight. Default UI size 12 pt. `Theme.swift:109-113` Audit: `theme::ui_font` (400 or 500 only; SemiBold is bundled and unused, oddity 16 kept).
- [x] Fonts are looked up in `FluentWriter_FluentWriter.bundle` under `Fonts` or `Contents/Resources/Fonts`, searched in the app's Resources, the bundle root, then beside the executable. `Theme.swift:79-92` Audit: the faces are compiled into the binary (`theme::FONTS`) and registered with Rune at launch, so there is nothing to look up; snapshot of a styled draft shows regular, bold, italic and bold italic.
- [x] Default text size 19 pt. `Theme.swift:60` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Allowed text sizes: 14, 15, 16, 17, 18, 19, 20, 22, 24, 26, 28, 32. `Theme.swift:61` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Line height = `round(fontSize × 1.55)`, fixed (min = max line height). `Theme.swift:62`, `MarkdownStyler.swift:34`, `MarkdownStyler.swift:41-42` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Baseline offset = `max(0, round((lineHeight − naturalLineHeight) / 2))` so text sits centred in the tall line. `MarkdownStyler.swift:35-36`, `MarkdownStyler.swift:56` Audit: snapshot of a styled draft (light and dark): text sits centred in the 29 pt line, wraps by word, links aren't links, the canvas shows through.
- [x] Line wrapping by word; no hyphenation/line-break strategy; no tightening. `MarkdownStyler.swift:45-47` Audit: snapshot of a styled draft (light and dark): text sits centred in the 29 pt line, wraps by word, links aren't links, the canvas shows through.
- [x] Character width = width of "0" in the regular font. `Theme.swift:115-117`, `MarkdownStyler.swift:33` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Hang width (space left of the prose edge for block markers) = 4 × character width. `MarkdownStyler.swift:17` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Every paragraph has first-line and head indent = hang width, so plain prose starts at the prose edge. `MarkdownStyler.swift:43-44` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Prose column width = `max(160, min(round(68 × charWidth), visibleWidth − 2 × minPad))`, minPad = `max(36, hangWidth + 12)`. `Theme.swift:63-64`, `EditorController.swift:296-299` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Horizontal inset = `max(4, round((visibleWidth − prose) / 2 − hangWidth))`; the column is centred. `EditorController.swift:300` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Text container width = prose + hang width; line fragment padding 0. `EditorController.swift:107`, `EditorController.swift:309` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Vertical inset 56 pt top and bottom when focus is Off. `EditorController.swift:306` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] With focus Off, extra scrollable space below the last line = `round(visibleHeight × 0.35)`. `EditorController.swift:307`, `EditorTextView.swift:19-26` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] With any focus mode on (Sentence, Paragraph, Typewriter) vertical inset = `max(40, round(visibleHeight / 2 − lineHeight / 2))` and no extra bottom space, so the first and last lines can reach the middle. `EditorController.swift:302-304` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Text view is at least as tall as the visible area. `EditorController.swift:311`, `EditorTextView.swift:23` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] The selection is preserved across any relayout. `EditorController.swift:294`, `EditorController.swift:315` Test `text_sizes_step_through_the_list_keep_the_selection_and_flash`.
- [x] Text colour `text`; selection background `selection` (no selected-text colour change). `MarkdownStyler.swift:54`, `EditorController.swift:176` — Rewrite: while another field has the keyboard or the window isn't key, the selection stays drawn in AppKit's `unemphasizedSelectedTextBackgroundColor` (220,220,220 / 70,70,70) through `SelectionStyle::unfocused` and `inactive`, as an `NSTextView` that isn't first responder draws it; checked in snapshots with the comment composer open. Audit: snapshot of a styled draft (light and dark): text sits centred in the 29 pt line, wraps by word, links aren't links, the canvas shows through.
- [x] Links get no special link attributes (not clickable as links). `EditorController.swift:177` Audit: snapshot of a styled draft (light and dark): text sits centred in the 29 pt line, wraps by word, links aren't links, the canvas shows through.
- [x] Editor background is transparent over `canvas`. `EditorController.swift:170` Audit: snapshot of a styled draft (light and dark): text sits centred in the 29 pt line, wraps by word, links aren't links, the canvas shows through.
- [x] Caret: filled rounded rect (radius 1) in `caret`, width `max(2, round(fontSize / 9))`, height `round(naturalLineHeight × 1.12)`, vertically centred in the line. `EditorController.swift:283-285`, `EditorTextView.swift:71-89` Test `the_column_and_caret_follow_the_swift_metrics`.
- [x] Caret blink-off redraws a rect 1 pt larger than the caret so no trace is left; narrow invalidations are widened by caretWidth + 1. `EditorTextView.swift:86-95` Audit: nothing to do in Rune, which repaints the caret's whole rect.
- [ ] Caret blinks. **(AppKit built-in)** Audit: Rune draws a steady caret; it doesn't blink (GAPS).
- [x] Plain text only: no rich text, no pasted graphics as attachments. `EditorController.swift:158-159` Audit: `text_input` holds plain text; pasted images become Markdown (test `pasting_image_files_inserts_them`).
- [x] Vertical scroller is an overlay scroller that auto-hides; no border; no scroll view background. `EditorController.swift:191-195` Audit: Rune's scroll view, overlay and hidden at rest (no bar in any snapshot).
- [x] Changing text size keeps the selection, re-centres the caret in focus modes (otherwise scrolls the selection into view), and flashes "Text size N pt". `Commands.swift:129-138` Test `text_sizes_step_through_the_list_keep_the_selection_and_flash`.
- [x] Layout is contiguous (whole document laid out), font leading not used. `EditorController.swift:102-103` Audit: Rune lays the whole draft out, paragraph by paragraph; nothing observable differs.
- [ ] Text input: IME/marked text, dead keys, emoji picker, dictation. **(AppKit built-in)** — Rewrite: IME, dead keys and composition are Rune's (`edit/ime.rs`); the emoji picker and dictation are the system's and reach the draft as IME commits on macOS and Windows. Not checked by hand on either.
- [ ] Standard text navigation and selection keys (arrows, ⌥/⌘ arrows, ⇧ extend, Home/End, Page Up/Down, Emacs ⌃A/⌃E/⌃K etc.), double-click word, triple-click paragraph, drag-select, drag-to-move selected text. **(AppKit built-in)** — Rewrite: arrows, ⇧ extend, ⌥/⌘ arrows (Ctrl off macOS), word/line deletion, Page Up/Down, ⌃B ⌃F ⌃N ⌃P ⌃D ⌃H, double-click word, triple-click paragraph and drag-select are Rune's; ⌃A ⌃E (paragraph), ⌃K/⌃Y kill and yank, ⌃T, ⌃O, ⌃L, ⌥↑/⌥↓ (Ctrl+↑/↓ elsewhere), Home/End scrolling and Escape keeping focus are built in `editing.rs`. Tests: `word_and_line_keys_follow_the_platform`, `paragraph_keys_move_by_paragraph`, `double_and_triple_click_select_a_word_and_a_paragraph_and_drag_selects`, `escape_keeps_the_keyboard_in_the_draft`. ⌥←/→ and ⌥⌫ stop at Unicode word boundaries as AppKit's do (`TextEdit::word_motion(WordMotion::Unicode)` on every field; test `word_keys_stop_at_word_boundaries_as_appkit_does`). Not done: drag-to-move selected text (Rune has no text drag source or drop target in a field); ⌘⌫ deletes to the paragraph's start, not the visual line's; ⌃V/⌥PageUp caret paging.
- [x] Undo/redo with typing coalescing; undo history cleared when a document loads. **(AppKit built-in for the stack)** `EditorController.swift:160`, `EditorController.swift:328` — Rewrite: Rune coalesces a typing run until the caret moves; a new draft gets a new `TextEdit`. Tests: `typing_coalesces_until_the_caret_moves`, `loading_a_draft_clears_the_undo_history`.
- [x] Undo menu titles name the action ("Undo Typing", "Undo Bold", …) from the action names the app sets. **(AppKit built-in titles)** `EditorController.swift:767` — Rewrite: `App::apply` makes each edit a named step (`TextEdit::undo_group_named`); the in-window Edit menu reads "Undo <name>" / "Redo <name>" from the focused field and disables them with nothing to undo or redo. Rune names typing "Typing" and its paste and cut "Paste" and "Cut". Names AppKit gives its own edits, which the Swift doesn't set, are the rewrite's choice and not checked on a Mac: ⌃K ⌃Y ⌃T ⌃O and substitutions "Typing", find bar "Replace" and "Replace All", Edit › Cut "Cut". Test: `undo_and_redo_name_the_step_and_are_disabled_with_nothing_to_do`. The macOS menu bar is the native-menu work.
- [ ] Dragging text in and out of the editor, and dropping text from other apps. **(AppKit built-in)** — Rewrite: blocked: Rune fields have no text drag out, and drops are files only (`on_drop_files`).
- [x] Continuous spell checking on by default (preference). `EditorController.swift:163` — Rewrite: preference defaults on and drives `El::spellcheck`; the draft's menu also has Rune's Spellcheck toggle. Rune checks on macOS and Windows only; Linux has no checker. Test: `check_spelling_while_typing_is_on_by_default_and_toggles`.
- [x] Automatic spelling correction off at launch. `EditorController.swift:164` — Rewrite: Rune has no automatic correction, so it is always off.
- [x] Smart quotes off at launch. `EditorController.swift:165` — Rewrite: `Prefs::smart_quotes` defaults off. The rewrite keeps the setting between launches; the Swift app reset it each launch. Audit: the setting was kept between launches; it now resets each launch as in the Swift (test `substitution_toggles_last_until_the_app_quits`).
- [x] Smart dashes off at launch. `EditorController.swift:166` — Rewrite: as Smart Quotes. Audit: as Smart Quotes, fixed.
- [ ] Text replacement (system text shortcuts) on at launch. **(AppKit built-in: the replacement itself)** `EditorController.swift:167` — Rewrite: built in `substitutions.rs`: on by default, reads `NSUserDictionaryReplacementItems` through `defaults export -g - | plutil -extract` on a thread at launch, replaces a shortcut when a space or punctuation follows it. Matching is tested with a sample list; the macOS read was not run (no Mac reachable from the build box). Windows and Linux have no system list, so the setting does nothing there.
- [x] Automatic link detection off. `EditorController.swift:168` Audit: Rune has no such substitution, so it is off.
- [x] Smart insert/delete (automatic spaces around pasted words) off. `EditorController.swift:169` Audit: Rune has no such substitution, so it is off.
- [x] Find bar (not a panel), shown above the text, with incremental search highlighting. **(AppKit built-in)** `EditorController.swift:161-162`, `EditorController.swift:196` — Rewrite: `find.rs`. Tests: `find_shows_the_bar_searches_as_you_type_and_escape_returns_to_the_match`, `the_find_bar_pushes_the_draft_down_and_its_buttons_work`, `a_far_match_is_scrolled_into_view`.
- [ ] Look Up / dictionary on force-click or ⌃⌘D. **(AppKit built-in)** — Rewrite: blocked: Rune has no Look Up / dictionary panel API.

---

## 3. Markdown styling

- [x] Restyling covers only the lines touched by an edit, unless the number of code-fence lines changed, in which case the whole document is restyled. `EditorController.swift:527-536` Audit: the parse follows only the edited lines (`MarkdownDocument::update`; writer-core `typing_in_a_long_draft_reparses_only_the_lines_around_the_caret`, `opening_a_fence_reparses_to_the_end_and_closing_it_restores_the_lines`) and styling is recomputed from it each frame; nothing observable differs.
- [x] Styling runs after the edit transaction finishes (deferred to the next main-loop turn). `EditorController.swift:538-549` Audit: the view styles the text after the edit has landed.
- [x] Base attributes for every character: regular font, `text` colour, base paragraph style, baseline offset. `MarkdownStyler.swift:51-58` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Headings (`#`–`######` followed by space/tab or end of line): whole line bold. `MarkdownStyler.swift:90-91`, `WriterCore/Markdown.swift:64` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Heading marker (`## ` including trailing space) coloured `markup`. `MarkdownStyler.swift:92` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Heading marker hangs into the left margin: first-line indent `max(0, hangWidth − markerWidth)`, so heading text starts at the prose edge; wrapped lines indent to the prose edge. `MarkdownStyler.swift:93-94` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Bullet items (`-`, `*`, `+` then whitespace), ordered items (1–9 digits then `.` or `)` then whitespace), task items (`- [ ]`, `- [x]`, `- [X]`), blockquotes (one or more `>` with optional space): marker coloured `markup`. `MarkdownStyler.swift:95-96`, `WriterCore/Markdown.swift:65-68` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] List and quote markers hang: first-line indent `max(0, hangWidth − markerWidth)`; wrapped lines align under the text after the marker (head indent = first-line indent + leading indent width + marker width). `MarkdownStyler.swift:97-100` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Code fence lines (``` or ~~~, 3+, up to 3 leading spaces) coloured `markup`; no inline styling. `MarkdownStyler.swift:101-103`, `WriterCore/Markdown.swift:70` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] A fence closes only on the same fence character it opened with. `WriterCore/Markdown.swift:128-138` writer-core test `fence_tracking`.
- [x] Lines inside a fence coloured `quiet`; no inline styling. `MarkdownStyler.swift:104-106` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Horizontal rules (3+ of `-`, `*` or `_`, optional spaces, up to 3 leading spaces) coloured `markup`. `MarkdownStyler.swift:101-103`, `WriterCore/Markdown.swift:69` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Bold (`**x**`, `__x__`): bold face; delimiters `markup`. `MarkdownStyler.swift:126-127`, `MarkdownStyler.swift:143` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Italic (`*x*`, `_x_`): italic face; delimiters `markup`. `MarkdownStyler.swift:128-129` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Bold italic (`***x***`): bold italic face (bold and italic combine with any existing trait). `MarkdownStyler.swift:113-121`, `WriterCore/Markdown.swift:235-242` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Strikethrough (`~~x~~`): single strikethrough in `quiet` over the content (not the delimiters); delimiters `markup`. `MarkdownStyler.swift:130-132` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Inline code (backtick runs): `codeBackground` behind the whole span including backticks; backticks `markup`. Nothing inside code is styled as emphasis. `MarkdownStyler.swift:133-134`, `WriterCore/Markdown.swift:207-213` — Rewrite: the background fills the line's whole height, square, as `.backgroundColor` does (`Span::line_bg`; test `backgrounds_fill_the_line_height`, light and dark snapshots). Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Links `[text](url)` and images `![alt](url)`: link text underlined (single, `markup` colour); `[`, `](`, `)` and the URL coloured `markup`. `MarkdownStyler.swift:135-138` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Link text may carry emphasis. `WriterCore/Markdown.swift:221-222` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Autolinks `<https://…>` / `<mailto:…>` and bare URLs (`http://`, `https://`, `www.`): underlined in `markup`; angle brackets `markup`. `MarkdownStyler.swift:139-141` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Headings keep the same font size as body text (one size throughout). `MarkdownStyler.swift:4-5`, `MarkdownStyler.swift:91` Audit: snapshot of a styled draft (`# heading`, emphasis, strike, code, links, autolink, bare URL, lists, task, quote, fence, code block, rule) matches the rule.
- [x] Image lines are additionally coloured `quiet` (see Images). `ImageBlocks.swift:168-169` Audit: `styler::style` colours a line with room under it `quiet`; test `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`.

---

## 4. Focus modes and scrolling

- [x] Four modes: Off, Sentence, Paragraph, Typewriter. `WriterCore/TextUnits.swift:4-12` Test `focus_modes_dim_flash_and_toggle`.
- [x] Sentence and Paragraph dim everything outside the current unit to `dimmed`. Typewriter dims nothing. `WriterCore/TextUnits.swift:10`, `EditorController.swift:587-602` Test `focus_modes_dim_flash_and_toggle`.
- [x] Sentence, Paragraph and Typewriter all keep the caret line at the vertical middle of the window. `WriterCore/TextUnits.swift:11` Tests `sentence_focus_centres_the_caret_line`, `typewriter_focus_keeps_the_caret_line_in_the_middle`.
- [x] Paragraph unit = the hard line (text between line breaks) containing the caret. `WriterCore/TextUnits.swift:16-26` writer-core tests `paragraph`, `sentence_containing_caret`, `caret_after_final_punctuation_belongs_to_sentence`.
- [x] Sentence unit = the natural-language sentence inside that line containing the caret, trimmed of trailing whitespace; a caret right after the final punctuation belongs to that sentence. `WriterCore/TextUnits.swift:28-58` writer-core tests `paragraph`, `sentence_containing_caret`, `caret_after_final_punctuation_belongs_to_sentence`.
- [x] With a selection, the bright range covers from the unit at the selection start to the unit at the selection end. `EditorController.swift:577-585` Test `focus_modes_dim_flash_and_toggle`.
- [x] Dimming is a display-only colour (does not change stored attributes) and is recomputed after every edit and on selection change when the range changed. `EditorController.swift:587-602` Test `focus_modes_dim_flash_and_toggle`.
- [x] Image previews outside the bright range fade to α0.28. `ImageBlocks.swift:218-224` Test `image_lines_are_quiet_and_their_previews_fade_outside_the_focus`.
- [x] Choosing a mode saves it, relays out, keeps the selection, re-centres the caret (animated unless Reduce Motion), sets the footer focus label, and flashes "Focus off" or "<Name> focus" ("Sentence focus", "Paragraph focus", "Typewriter focus"). `EditorController.swift:553-565` Test `focus_modes_dim_flash_and_toggle`.
- [x] Choosing Sentence or Paragraph also records it as the mode Toggle Focus returns to. `EditorController.swift:557` Test `focus_modes_dim_flash_and_toggle`.
- [x] Toggle Focus (⌘D): if Sentence or Paragraph is on, turn focus Off; otherwise switch to the last dimming mode (default Sentence). From Typewriter it switches to the last dimming mode. `Commands.swift:120-122`, `Preferences.swift:13-16` Test `focus_modes_dim_flash_and_toggle`.
- [x] Centring: scroll so the caret's line midpoint is at the middle of the visible area, clamped to the document. `EditorController.swift:607-613` Test `typewriter_focus_keeps_the_caret_line_in_the_middle`.
- [ ] Scroll animation lasts 0.14 s, ease-out; instant with Reduce Motion; no scroll if the target is within 0.5 pt. `EditorController.swift:630-646` Audit: Rune glides with a critically damped 0.14 s spring, not a 0.14 s ease-out curve (it has no tweened scroll); instant under Reduce Motion; the 0.5 pt rule is in `App::scroll_to` (GAPS).
- [x] In centring modes, typing re-centres with animation after each change. `EditorController.swift:709-713` Test `typewriter_focus_keeps_the_caret_line_in_the_middle`.
- [x] In centring modes, caret moves caused by key presses (arrows etc.) re-centre with animation; mouse clicks do not re-centre. `EditorController.swift:723-726` Tests `sentence_focus_centres_the_caret_line`, `typewriter_focus_keeps_the_caret_line_in_the_middle`.
- [x] With focus Off, after each change the view scrolls only when the caret is too close to an edge: bottom limit is `max(34 + 2 × lineHeight, 22% of visible height)` from the bottom; top limit is one line height from the top. `EditorController.swift:616-628` Test `with_focus_off_typing_keeps_the_caret_off_the_bottom_edge`.
- [x] On document load the selection is scrolled into view, then centred (focus modes) or kept readable (Off), without animation. `EditorController.swift:346-351` Test `typewriter_focus_keeps_the_caret_line_in_the_middle`.
- [x] Reduce Motion is read from the system accessibility setting. `EditorController.swift:604` Test `reduce_motion_makes_the_chrome_and_the_peek_instant`.

---

## 5. Documents, saving, recovery

### Launch

- [x] Drafts folder `~/Documents/Fluent Writer` (dev: `~/Documents/Fluent Writer Dev`), created if missing. `WriterCore/AppVariant.swift:23-26`, `WriterCore/DraftStore.swift:89-95` writer-core test `names_and_folders` (the macOS paths are checked on macOS). Linux uses the XDG documents and data folders, Windows Documents and %APPDATA%.
- [x] Support folder `~/Library/Application Support/Fluent Writer` (dev: `… Fluent Writer Dev`) with a `Recovery` subfolder, created if missing. `WriterCore/AppVariant.swift:18-21`, `WriterCore/DraftStore.swift:83`, `WriterCore/DraftStore.swift:94-95` Audit: `DraftLibrary::new` makes the support and Recovery folders; every draft-store test starts there.
- [x] Stale temporary files (`.<name>.fw-tmp-<uuid>`) from interrupted saves are deleted at launch in the drafts, support and recovery folders. `WriterCore/DraftStore.swift:96-98`, `WriterCore/AtomicFile.swift:15`, `WriterCore/AtomicFile.swift:61-62` writer-core test `writes_replaces_and_cleans_up`.
- [x] If the folders can't be created, an error alert is shown and the app quits. `main.swift:12-18` — *Rust: A small window with the message and OK; not run against a real failure.*
- [x] Launch document: `--open <path>` if it opens; else the newest recovery snapshot whose file is missing or that never had a file; else a new empty draft. `main.swift:19-27`, `WriterCore/DraftStore.swift:170-172`
- [ ] The app activates and comes to the front at launch. `main.swift:31` Audit: macOS activation not run here; on Linux the window opens with the keyboard.
- [ ] Opening files from Finder / Dock opens the first URL in the existing window (same rules as Open). `main.swift:34-36` — *Rust: Rune passes no file-open events (only `myapp://` links). Windows and Linux "Open With" pass the path as an argument, which is opened at launch; a second launch starts a second app.*

### Opening

- [x] Accepted extensions: md, markdown, txt, text. `WriterCore/DraftStore.swift:87`
- [x] Opening the file that is already open just focuses the editor. `Commands.swift:41`
- [x] Opening another file first runs the leave-document check. `Commands.swift:42`
- [x] Files are read as UTF-8, falling back to ISO Latin 1. `WriterCore/DraftStore.swift:134` Test `files_that_arent_utf8_are_read_as_latin_1`.
- [x] Error "Couldn't open “<name>”: <reason>" when the file can't be read; "“<name>” isn't a text file." when it can't be decoded; shown as a sheet alert. `WriterCore/DraftStore.swift:130-136`, `EditorController.swift:850-854` — *Rust: In the app's alert. "isn't a text file" can't happen: Latin 1 decodes any bytes, as in the Swift.*
- [x] The caret/selection saved for that file is restored on open; with no record, the caret goes to the end of the file. `WriterCore/DraftStore.swift:139-142`
- [x] A remembered title and manual-title flag are restored; with no record the title is the file name without extension. `WriterCore/DraftStore.swift:137-138`
- [ ] Opened files are added to the system recent-documents list (Dock menu). **(AppKit built-in: the list)** `Commands.swift:45` — *Rust: No Rune API.*
- [x] File › Open Other File… (⇧⌘O): open panel as a sheet, md/markdown/txt/text only, single file, starting in the drafts folder. `Commands.swift:26-38`

### Saving

- [x] Autosave 0.8 s after the last text change. `EditorController.swift:428-433`, `EditorController.swift:707` Test `autosave_waits_0_8_s_and_save_flashes_saved`.
- [x] Footer shows "Edited" while a save is pending. `EditorController.swift:475-477`, `EditorController.swift:706` Test `autosave_waits_0_8_s_and_save_flashes_saved`.
- [x] Saves are atomic (temp file in the same folder, fsync, rename). `WriterCore/AtomicFile.swift:17-50` writer-core tests `writes_replaces_and_cleans_up`, `interrupted_write_leaves_previous_revision`.
- [x] An empty, never-saved draft is never written. `WriterCore/DraftStore.swift:193` writer-core test `empty_new_draft_is_not_written`.
- [x] Nothing is written when the text equals what was last saved (and nothing else is pending). `WriterCore/DraftStore.swift:194` Test `a_draft_with_nothing_new_isnt_written_again`.
- [x] A new draft's first save creates `<title>.md` in the drafts folder; if taken, `<title> 2.md`, `<title> 3.md`, … `WriterCore/DraftStore.swift:197-199`, `WriterCore/DraftStore.swift:278-288` writer-core test `title_collision_gets_suffix`.
- [x] File-name stem: `/` and `:` become `-`, control characters removed, leading/trailing spaces and dots trimmed, "Untitled" if empty, cut to 120 characters if over 200 UTF-8 bytes. `WriterCore/TitleDeriver.swift:29-36` Test `file_name_stems_are_safe`.
- [x] A draft in the drafts folder without a manual title is renamed on save to follow its derived title (unless the current name is already that title, or that title plus " N"). Files elsewhere keep their name. `WriterCore/DraftStore.swift:232-241` Tests `derived_title_renames_file_but_manual_title_sticks`, `numbered_names_stay_and_files_elsewhere_keep_their_names`.
- [x] File › Save (⌘S) saves now and flashes "Saved"; does nothing for an empty new draft. `Commands.swift:51-54` Test `autosave_waits_0_8_s_and_save_flashes_saved`.
- [x] Save also happens when the window loses focus, before opening another document, before quitting, and before the Open picker appears. `EditorController.swift:828-831`, `EditorController.swift:386-387`, `Commands.swift:15` — *Rust: Window focus, opening, quitting; the Open picker is that port's.*
- [x] Caret position is remembered (in `state.json`, not in the file) 1.0 s after the selection stops changing, and on every save. `EditorController.swift:727-731`, `EditorController.swift:368` Test `the_caret_is_remembered_a_second_after_it_stops`.
- [x] A successful save in Recent sidebar mode refreshes the Recent list. `EditorController.swift:372-374` Audit: `App::after_save` refreshes the Recent list after a save that wrote; sidebar tests cover the list.
- [x] A save failure keeps the failure message in the footer as "Not saved — <message>" in `warning`, with the message as tooltip, and keeps the chrome visible. `EditorController.swift:378-381`, `EditorController.swift:481-485`
- [x] Save failure messages read "Couldn't save “<file>”: <system reason>." `WriterCore/AtomicFile.swift:8-11`
- [x] While a save has failed, further typing does not change the footer back to "Edited". `EditorController.swift:706` Test `a_failed_save_keeps_its_status_while_typing`.
- [x] Each failed save writes a recovery snapshot (`Recovery/<draft-uuid>.json`: path, title, manual-title flag, body, caret, time). `WriterCore/DraftStore.swift:212-222` Tests `recovery_snapshots_hold_the_draft_and_go_once_it_is_saved`, `save_failure_keeps_recovery_and_reopen_restores_it`.
- [x] A successful save deletes the draft's recovery snapshot and any other snapshot for the same path. `WriterCore/DraftStore.swift:207-208` Test `recovery_snapshots_hold_the_draft_and_go_once_it_is_saved`.

### Leaving a document

- [x] Before replacing the open draft (New, Open, open over SSH, sidebar click, close, quit) the app saves; if the save fails it shows a warning alert: title "This draft couldn't be saved.", text "<message>\n\nA recovery copy is kept and will be offered next time.", buttons "Keep Editing" (default), "Save As…", "Continue". `EditorController.swift:385-400` Tests `a_failed_save_asks_before_a_new_draft`, `save_as_from_the_alert_leaves_only_once_the_copy_is_written`; strings checked against the Swift.
- [x] Keep Editing cancels; Save As… runs Save As and proceeds only if it succeeds; Continue proceeds. `EditorController.swift:395-399` Tests `a_failed_save_asks_before_a_new_draft`, `save_as_from_the_alert_leaves_only_once_the_copy_is_written`; strings checked against the Swift.
- [x] For remote files with edits not yet uploaded, the leave check also uploads synchronously (see SSH). `EditorController.swift:387`, `RemoteEditing.swift:194-211` — Rust: uploads off the UI thread behind a "Saving to <host>…" sheet with Keep Editing (owner-approved fix of oddity 10).
- [x] Quitting stops any running Assist request, then runs the leave check; cancel aborts the quit. `main.swift:38-42` Audit: the request now stops before the check, even when the check keeps the app open (fixed this pass). Test `quitting_stops_assist_first`.
- [x] On quit the open draft's comments/versions are flushed to disk and the live-state file is deleted. `main.swift:44-46`, `NotesController.swift:254-257` Test `quitting_writes_the_notes_and_removes_the_live_state`.

### New draft

- [x] File › New (⌘N): if the open draft is empty and never saved, just focus the editor; otherwise leave-check then open an empty "Untitled" draft. `Commands.swift:7-11`

### Save As

- [ ] File › Save As… (⇧⌘S): app-modal save panel (not a sheet), `.md` only, name "<title stem>.md", starting in the drafts folder. `EditorController.swift:403-408` — *Rust: Rune's save dialog (a sheet on macOS, oddity 27 not kept: Rune has no app-modal panel). A name without `.md` gets it.* Audit: the panel is a sheet on macOS, not app-modal (oddity 27 not kept): Rune has no app-modal save panel (GAPS).
- [x] Writes the current text atomically to the chosen file, opens that file in place of the current one keeping the current selection, and discards the old draft's recovery snapshot. The old file stays where it was. `EditorController.swift:409-416`
- [x] Save As errors show as a sheet alert. `EditorController.swift:417-419`

### Recovery

- [x] Opening a file whose recovery snapshot differs from the file loads the snapshot's text, title and caret; footer shows "Recovered unsaved changes"; the recovered text is saved to the file 0.2 s later. `WriterCore/DraftStore.swift:145-156`, `EditorController.swift:334-336`, `EditorController.swift:478-480` Tests `a_snapshot_matching_the_file_is_deleted_on_open_and_a_different_one_is_loaded`, `a_recovered_draft_says_so_and_saves_after_a_moment`.
- [x] A snapshot identical to the file is deleted on open. `WriterCore/DraftStore.swift:153-155` Tests `a_snapshot_matching_the_file_is_deleted_on_open_and_a_different_one_is_loaded`, `a_recovered_draft_says_so_and_saves_after_a_moment`.
- [x] A recovered never-saved draft is saved as a new file 0.2 s after launch. `WriterCore/DraftStore.swift:162-167`, `EditorController.swift:334-336`

### Show in Finder

- [ ] File › Show in Finder (⌥⌘R) reveals the file in Finder; for an unsaved draft with text it saves first to create the file. `Commands.swift:64-68` Audit: built (`platform::reveal`: Finder, File Explorer, or the file manager's ShowItems over D-Bus on Linux; saves first); not run here, where it would open a file manager on a shared machine.
- [x] Enabled only for local drafts that have a file or have text. `Commands.swift:182` Test `show_in_finder_needs_a_local_file_or_text`.

---

## 6. Title and rename

- [x] Title field: borderless, no background, no focus ring, centred, IBM Plex Mono 13 pt, single line, truncates at the tail. `EditorController.swift:205-212` — *Rust: A label at rest, a text field while editing (a click puts the caret at the end rather than where clicked).*
- [x] Placeholder "Untitled". `EditorController.swift:214`, `WriterCore/TitleDeriver.swift:4` — *Rust: Drawn in `dimmed`.*
- [x] Tooltip "Click to rename"; accessibility label "Draft title". `EditorController.swift:215-216`
- [x] Field is empty (placeholder shows) when the title is "Untitled" and the draft is empty; otherwise it shows the title. `EditorController.swift:437`
- [x] Title colour: `text` at α0.75 when the writer named the draft, `quiet` when derived. `EditorController.swift:438`
- [x] The field isn't overwritten while it is being edited. `EditorController.swift:436`
- [x] Derived title = first line that isn't blank, a fence or a rule, with its block marker dropped, inline Markdown removed, whitespace collapsed. `WriterCore/TitleDeriver.swift:7-17` Tests `derived_titles_skip_fences_and_rules_and_drop_markup`, `derived_titles_are_cut_at_60_characters_on_a_late_space`.
- [x] Derived titles are cut to 60 characters; if a space falls past the halfway point the cut is at the last space and trailing " ,;:-" are trimmed. `WriterCore/TitleDeriver.swift:19-26` Tests `derived_titles_skip_fences_and_rules_and_drop_markup`, `derived_titles_are_cut_at_60_characters_on_a_late_space`.
- [x] Without a manual title, the title follows the first line as you type. `EditorController.swift:701-705`
- [x] File › Rename… (⇧⌘R): shows chrome (no auto-hide), focuses the title field and selects all its text. `Commands.swift:58-62`
- [x] Clicking the title field starts editing it and shows the chrome with no auto-hide. `EditorController.swift:791-793`
- [x] Ending the edit (Return, Tab, clicking away) with a changed title renames. `EditorController.swift:795-813`
- [x] Esc in the title field restores the current title and returns focus to the text without renaming. `EditorController.swift:815-822`
- [x] Rename trims whitespace; an empty title clears the manual title and goes back to the derived one. `WriterCore/DraftStore.swift:243-251`
- [x] Renaming a local file moves it in the same folder to "<new stem>.<same extension>" (" 2", " 3"… if taken). `WriterCore/DraftStore.swift:252-262`
- [x] Renaming a remote file changes only the title shown, not the remote file name. `WriterCore/DraftStore.swift:252` writer-core test `renaming_remote_file_keeps_mirror_path`.
- [x] Renaming a never-saved draft that has text saves it first (so it gets a file), then renames. `EditorController.swift:800`
- [x] A rename failure shows "Couldn't rename “<file>”: <reason>" as a sheet alert. `WriterCore/DraftStore.swift:257-258`, `EditorController.swift:803-805`
- [x] After a rename, comments and versions move with the file. `EditorController.swift:442`, `NotesController.swift:53-63` — *Rust: The comments port's; marked in `end_title_edit`.* Test `renaming_moves_the_notes_with_the_file`.
- [x] After the edit ends, focus returns to the text and chrome auto-hide restarts. `EditorController.swift:811-812`

---

## 7. Footer status and counts

- [x] Three labels in the footer: status (left), focus mode (centre), counts (right); IBM Plex Mono 12 pt, `quiet`, truncate at tail. `EditorController.swift:221-228`
- [x] Labels are 16 pt tall at y 9 in the 34 pt footer; side padding 20 pt. `EditorController.swift:264-273`
- [x] Counts width is its text width + 4, capped at 45% of the footer width; right-aligned to 20 pt from the right edge. `EditorController.swift:265-267`
- [x] Status width is its text width + 4, capped at the space left of the counts minus 16 pt gap. `EditorController.swift:268-270` — *Rust: Measured from the text on screen, flashes included (oddity 4 fixed).*
- [x] Focus label is centred and shown only when it fits with at least 12 pt clear of both status and counts. `EditorController.swift:271-274` — *Rust: Also when counts are hidden (the earlier Rust hid it then).*
- [x] Focus label text: "Sentence", "Paragraph", "Typewriter", or empty for Off. `EditorController.swift:562`, `EditorController.swift:567-574` — *Rust: Shown from launch for a saved focus mode; the Swift showed it only after the mode was next changed.*
- [ ] Counts text: "<w> word(s)  ·  <c> character(s)" (two spaces each side of the dot), numbers with locale grouping separators. `WriterCore/TextStats.swift:30-40` Audit: text and spacing tested (`counts_read_the_plain_text_in_grapheme_clusters`, writer-core `formats`), but the thousands separator is always a comma; the Swift followed the user's locale (GAPS).
- [x] With a selection: "<sw> of <w> words  ·  <sc> of <c> characters"; singular/plural follows the total. `WriterCore/TextStats.swift:36-38` Tests `counts_read_the_plain_text_in_grapheme_clusters`, `counts_wait_0_15_s_flashes_last_2_2_s_and_show_counts_toggles`.
- [x] Counts measure the plain-text export (Markdown delimiters excluded); characters are grapheme clusters. `WriterCore/TextStats.swift:17-28` Tests `counts_read_the_plain_text_in_grapheme_clusters`, `counts_wait_0_15_s_flashes_last_2_2_s_and_show_counts_toggles`.
- [x] Counts update at most every 0.15 s after typing or selection change. `EditorController.swift:520-523` Test `counts_wait_0_15_s_flashes_last_2_2_s_and_show_counts_toggles`.
- [x] View › Show Counts (⌥⌘C) toggles the counts label; checked when shown; persisted. `Commands.swift:156-160`, `Commands.swift:177` Test `counts_wait_0_15_s_flashes_last_2_2_s_and_show_counts_toggles`.
- [x] Status "Saved" for a saved local file (tooltip: full path). `EditorController.swift:471-474` Test `autosave_waits_0_8_s_and_save_flashes_saved`.
- [x] Status empty for a never-saved draft. `EditorController.swift:472` Test `autosave_waits_0_8_s_and_save_flashes_saved`.
- [x] Status "Edited" while a save is pending. `EditorController.swift:475-477` Test `autosave_waits_0_8_s_and_save_flashes_saved`.
- [x] Status "Recovered unsaved changes". `EditorController.swift:478-480` Test `a_recovered_draft_says_so_and_saves_after_a_moment`.
- [x] Status "Not saved — <message>" in `warning`. `EditorController.swift:481-485` Test `a_failed_save_keeps_its_status_while_typing`.
- [x] Remote, uploaded: "Saved to <host>". `EditorController.swift:457`
- [x] Remote, saved locally but not uploaded: "Saved locally · not yet on <host>". `EditorController.swift:457`
- [x] Remote, uploading: "Saving to <host>…". `EditorController.swift:459-461`
- [x] Remote, server unreachable at open: "Offline · editing the local copy". `EditorController.swift:462-464`
- [x] Remote, upload failed: "Not on <host> yet — <message>" in `warning`, tooltip the message, chrome kept visible. `EditorController.swift:465-469`
- [x] Remote status tooltip is the remote location (`host:path` or `ssh://host:port/path`). `EditorController.swift:454`
- [x] `<host>` in remote statuses is the host without `user@`. `WriterCore/Remote.swift:48`
- [x] Transient messages replace the status for 2.2 s, then the status returns. `EditorController.swift:491-501` Test `counts_wait_0_15_s_flashes_last_2_2_s_and_show_counts_toggles`.
- [x] Every transient message is announced to VoiceOver at medium priority. `EditorController.swift:495` — *Rust: `Tx::announce` (a polite announcement) in `flash_message`; test `a_flash_gets_its_own_width_and_returns_to_the_status`. Not checked with a screen reader.* Audit: tested as noted; not heard through a screen reader.

---

## 8. Menus

Every item, its shortcut, its enabled/checked rule and its action. "Always" means always enabled.

### App menu (titled with the app name: "Fluent Writer" or "Fluent Writer Dev")

- [x] "About <app name>", no shortcut, always; standard About panel (name, version 0.2.0 (2)). **(AppKit built-in panel)** `MainMenu.swift:25` — *Rust: Off macOS at the end of the in-window File menu, as an alert with the version. The macOS app menu is the native-menu work.*
- [ ] separator `MainMenu.swift:26` Audit: the macOS app menu is the native-menu work (GAPS).
- [ ] "Hide <app name>" ⌘H, always. **(AppKit built-in)** `MainMenu.swift:27` Audit: the macOS app menu is the native-menu work (GAPS).
- [ ] "Hide Others" ⌥⌘H, always. **(AppKit built-in)** `MainMenu.swift:28` Audit: the macOS app menu is the native-menu work (GAPS).
- [ ] "Show All", no shortcut. **(AppKit built-in)** `MainMenu.swift:29` Audit: the macOS app menu is the native-menu work (GAPS).
- [ ] separator `MainMenu.swift:30` Audit: the macOS app menu is the native-menu work (GAPS).
- [x] "Quit <app name>" ⌘Q; runs the quit flow (stop Assist, leave check). `MainMenu.swift:31`, `main.swift:38-42`
- [ ] No Settings/Preferences item and no Services submenu in the app menu. `MainMenu.swift:24-32` Audit: the macOS app menu is the native-menu work (GAPS).

### File

- [x] "New" ⌘N, always → New draft. `MainMenu.swift:35`, `Commands.swift:7-11` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Open…" ⌘O, always → Open picker sheet. `MainMenu.swift:36`, `Commands.swift:13-24`
- [x] "Open Other File…" ⇧⌘O, always → file open panel. `MainMenu.swift:37` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Open Folder…" ⌥⌘O, always → folder panel for the sidebar. `MainMenu.swift:38`
- [x] "Open over SSH…" ⌃⌘O, always → SSH sheet. `MainMenu.swift:39`
- [x] separator `MainMenu.swift:40` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Save" ⌘S, always → save now. `MainMenu.swift:41`
- [x] "Save As…" ⇧⌘S, always → Save As. `MainMenu.swift:42`
- [x] "Rename…" ⇧⌘R, always → edit title. `MainMenu.swift:43`
- [x] "Show in Finder" ⌥⌘R, enabled for local drafts with a file or text. `MainMenu.swift:44`, `Commands.swift:182` Tests `menus_match_the_swift_menu_bar`, `show_in_finder_needs_a_local_file_or_text`. Off macOS it reads "Show in File Explorer" or "Show in Files".
- [x] separator `MainMenu.swift:45` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Copy for Twitter" ⇧⌘C; title becomes "Copy Selection for Twitter" when text is selected; disabled when the draft is empty. `MainMenu.swift:46`, `Commands.swift:183-185` Test `menu_items_follow_the_swift_validation`.
- [x] separator `MainMenu.swift:47` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Close" ⌘W → close the window (leave check, then quit). `MainMenu.swift:48`

### Edit

- [x] "Undo" ⌘Z. **(AppKit built-in, with action names)** `MainMenu.swift:55` — Rewrite: through the draft's own history; from a menu, `App::run_menu` sends it to whichever field has focus. Titled "Undo <name>" and disabled with nothing to undo: see Undo menu titles above.
- [x] "Redo" ⇧⌘Z. **(AppKit built-in)** `MainMenu.swift:56`
- [x] separator `MainMenu.swift:57` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Cut" ⌘X. **(AppKit built-in)** `MainMenu.swift:58` — Rewrite: Test: `cut_copy_and_paste_go_through_the_clipboard`.
- [x] "Copy" ⌘C. **(AppKit built-in)** `MainMenu.swift:59`
- [x] "Paste" ⌘V; also enabled when the clipboard holds only an image (see Images). **(AppKit built-in for text)** `MainMenu.swift:60`, `EditorTextView.swift:33-37` — Rewrite: text paste works (test `cut_copy_and_paste_go_through_the_clipboard`); the image half belongs to Images. Audit: Paste is always enabled in the Rust menus, and an image-only clipboard or copied image files paste as images (test `pasting_image_files_inserts_them`, fixed this pass for files).
- [x] "Delete", no shortcut. **(AppKit built-in)** `MainMenu.swift:61` — Rewrite: deletes the selection only. Test: `menu_edit_items_act_on_the_focused_field`.
- [x] "Select All" ⌘A. **(AppKit built-in)** `MainMenu.swift:62`
- [x] separator `MainMenu.swift:63` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] Find › "Find…" ⌘F — shows the find bar. **(AppKit built-in)** `MainMenu.swift:65` — Rewrite: keeps the last search (AppKit kept it on the system find pasteboard, which Rune can't reach).
- [x] Find › "Find and Replace…" ⌥⌘F — find bar with replace. **(AppKit built-in)** `MainMenu.swift:66` — Rewrite: Ctrl+H off macOS.
- [x] Find › "Find Next" ⌘G. **(AppKit built-in)** `MainMenu.swift:67` — Rewrite: works with the bar hidden. Test: `find_next_and_previous_wrap_and_respect_the_options`.
- [x] Find › "Find Previous" ⇧⌘G. **(AppKit built-in)** `MainMenu.swift:68`
- [x] Find › "Use Selection for Find" ⌘E. **(AppKit built-in)** `MainMenu.swift:69` — Rewrite: Test: `use_selection_for_find_then_find_next`.
- [x] Find › "Jump to Selection" ⌥⌘J — scrolls the selection to the centre. **(AppKit built-in)** `MainMenu.swift:70` — Rewrite: no shortcut (see Oddities). Test: `jump_to_selection_centres_it`.
- [x] Find bar: Done button, match count, Esc closes, Return/⇧Return next/previous, replace and replace-all, case and contains/starts-with options. **(AppKit built-in)** — Rewrite: matches fill the line's whole height, square (`TextHighlight::line_bg`), as AppKit lights them; the current match keeps the app's rounded ring (test `find_matches_fill_the_line_height`). Count reads "2 of 13", "13 matches" or "Not found"; Replace replaces the selected match and moves to the next, each one undo step; Replace All is one step and reports "N replaced". Tests: `replace_replaces_the_match_and_moves_on_as_one_undo_step`, `replace_all_is_one_undo_step_and_reports_the_count`. Snapshots of the bar in light and dark were checked.
- [ ] Spelling and Grammar › "Show Spelling and Grammar" ⌘: (shift-semicolon) — spelling panel. **(AppKit built-in)** `MainMenu.swift:73` — Rewrite: blocked: Rune has no spelling panel. Left out of the menu rather than shown disabled.
- [ ] Spelling and Grammar › "Check Document Now" ⌘; — jumps to the next misspelling. **(AppKit built-in)** `MainMenu.swift:74` — Rewrite: blocked: needs Rune to make `spell::misspelled(text)` public so the app can select the next misspelled word.
- [ ] separator `MainMenu.swift:75` Audit: goes with the two Spelling and Grammar items above, which aren't built.
- [x] Spelling and Grammar › "Check Spelling While Typing", no shortcut, checked per the spell-check preference; toggles and saves it. `MainMenu.swift:76`, `Commands.swift:162-165`, `Commands.swift:178`
- [ ] Spelling and Grammar › "Check Grammar With Spelling", checked state from the text view; toggles grammar checking (not saved by the app). **(AppKit built-in)** `MainMenu.swift:77` — Rewrite: blocked: Rune's checker reports spelling only.
- [ ] Spelling and Grammar › "Correct Spelling Automatically", checked state from the text view; off at each launch. **(AppKit built-in)** `MainMenu.swift:78`, `EditorController.swift:164` — Rewrite: blocked: needs public `spell::misspelled` and `spell::suggest`; off in the Swift app at each launch, and absent here, so the default behaviour matches.
- [x] Substitutions › "Smart Quotes", checked from the text view; off at each launch. **(AppKit built-in)** `MainMenu.swift:81`, `EditorController.swift:165` — Rewrite: AppKit's rule: opening after the start, white space or an opening bracket or quote, closing elsewhere. Test: `substitutions_follow_their_settings`.
- [x] Substitutions › "Smart Dashes", checked from the text view; off at each launch. **(AppKit built-in)** `MainMenu.swift:82`, `EditorController.swift:166` — Rewrite: two hyphens become an em dash once the next character is typed, as its own undo step; three are left alone.
- [x] Substitutions › "Text Replacement", checked from the text view; on at each launch. **(AppKit built-in)** `MainMenu.swift:83`, `EditorController.swift:167` — Rewrite: macOS only; see Text replacement in section 2. Audit: on at each launch again (it was kept), test `substitution_toggles_last_until_the_app_quits`.
- [ ] Red underline under misspelled words and right-click suggestions (Guesses, Ignore Spelling, Learn Spelling). **(AppKit built-in)** — Rewrite: Rune draws the marks and offers guesses and Learn Spelling on macOS and Windows; there is no Ignore Spelling, and none of it exists on Linux. Not seen on a Mac or Windows machine.
- [ ] macOS adds "Start Dictation…" and "Emoji & Symbols" (⌃⌘Space) to the Edit menu. **(AppKit built-in)** — Rewrite: depends on the native Edit menu, which is not built yet.

### Format

- [x] "Bold" ⌘B → toggle `**`. `MainMenu.swift:88`, `Commands.swift:90` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Italic" ⌘I → toggle `*`. `MainMenu.swift:89`, `Commands.swift:91` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Strikethrough" ⇧⌘U → toggle `~~`. `MainMenu.swift:90`, `Commands.swift:92` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Link" ⌘K → insert link. `MainMenu.swift:91`, `Commands.swift:94-96` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Image…" ⇧⌘I → image open panel. `MainMenu.swift:92`, `ImageBlocks.swift:361-374` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] separator `MainMenu.swift:93` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Heading 1" ⌘1, "Heading 2" ⌘2, "Heading 3" ⌘3. `MainMenu.swift:94-96` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Body" ⌥⌘0 → remove block prefix. `MainMenu.swift:97` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] separator `MainMenu.swift:98` Test `menus_match_the_swift_menu_bar`.
- [x] "Bulleted List" ⌘L. `MainMenu.swift:99` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Numbered List" ⌥⌘L. `MainMenu.swift:100` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Blockquote" ⌘'. `MainMenu.swift:101` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] separator `MainMenu.swift:102` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Indent" ⌘]. `MainMenu.swift:103` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Outdent" ⌘[. `MainMenu.swift:104` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] All Format items are always enabled. `Commands.swift:169-194` Test `menu_items_follow_the_swift_validation`.

### View

- [x] "Show Sidebar" ⌃⌘S; title "Hide Sidebar" while the sidebar is pinned. `MainMenu.swift:113`, `Commands.swift:181`
- [x] "Recent Files" ⇧⌘E → pin the sidebar (if needed) and switch it to Recent. `MainMenu.swift:114`, `RemoteEditing.swift:64-69`
- [x] separator `MainMenu.swift:115` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Toggle Focus" ⌘D; title "Turn Focus Off" while Sentence or Paragraph is on, else "Turn <Sentence|Paragraph> Focus On" naming the last dimming mode. `MainMenu.swift:116`, `Commands.swift:175` Test `focus_modes_dim_flash_and_toggle`.
- [x] Focus › "Off" ⌃⌘0, checked when Off. `MainMenu.swift:118`, `Commands.swift:171` Test `focus_modes_dim_flash_and_toggle`.
- [x] Focus › "Sentence" ⌃⌘1, checked when Sentence. `MainMenu.swift:119`, `Commands.swift:172` Test `focus_modes_dim_flash_and_toggle`.
- [x] Focus › "Paragraph" ⌃⌘2, checked when Paragraph. `MainMenu.swift:120`, `Commands.swift:173` Test `focus_modes_dim_flash_and_toggle`.
- [x] Focus › "Typewriter" ⌃⌘3, checked when Typewriter. `MainMenu.swift:121`, `Commands.swift:174` Test `focus_modes_dim_flash_and_toggle`.
- [x] separator `MainMenu.swift:123` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Bigger Text" ⌘+ (key equivalent "+"); next larger size in the list; disabled at 32 pt. `MainMenu.swift:124`, `Commands.swift:140-142`, `Commands.swift:179` Test `text_sizes_step_through_the_list_keep_the_selection_and_flash` (⌘= and ⇧⌘= both, oddity 23 fixed).
- [x] "Smaller Text" ⌘−; next smaller size; disabled at 14 pt. `MainMenu.swift:125`, `Commands.swift:144-146`, `Commands.swift:180` Test `text_sizes_step_through_the_list_keep_the_selection_and_flash` (⌘= and ⇧⌘= both, oddity 23 fixed).
- [x] "Default Text Size" ⌘0 → 19 pt. `MainMenu.swift:126`, `Commands.swift:148` Test `text_sizes_step_through_the_list_keep_the_selection_and_flash` (⌘= and ⇧⌘= both, oddity 23 fixed).
- [x] separator `MainMenu.swift:127` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] Appearance › "Match System", "Light", "Dark"; the current one is checked; choosing one applies and saves it. `MainMenu.swift:107-111`, `MainMenu.swift:128`, `Commands.swift:150-154`, `Commands.swift:176`, `Theme.swift:15-21` — *Rust: In-app colours change live. Rune has no API to set the native appearance (macOS traffic lights, menus and panels follow the system).*
- [x] "Show Counts" ⌥⌘C, checked when counts are shown. `MainMenu.swift:129`, `Commands.swift:177` Test `counts_wait_0_15_s_flashes_last_2_2_s_and_show_counts_toggles`.
- [x] separator `MainMenu.swift:130` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Enter Full Screen" ⌃⌘F; the system retitles it "Exit Full Screen" in full screen. **(AppKit built-in)** `MainMenu.swift:131` — *Rust: Off macOS the in-window menu retitles it from the app's own state.*

### Assist

- [x] "Ask Codex…" ⌘J → open Assist on Ask with Codex selected; disabled when the draft is empty. `MainMenu.swift:137`, `AssistController.swift:870`, `Commands.swift:186`
- [x] "Ask Claude…" ⇧⌘J → open Assist on Ask with Claude selected; disabled when the draft is empty. `MainMenu.swift:138`, `AssistController.swift:871`, `Commands.swift:186`
- [x] "Show Assist" ⌥⌘J → close the panel if open, else open it on its last tab; disabled when the draft is empty. `MainMenu.swift:139`, `AssistController.swift:866-868`, `Commands.swift:186`
- [x] separator `MainMenu.swift:140` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Add Comment…" ⌥⌘M → comment composer; disabled when the draft is empty. `MainMenu.swift:141`, `Commands.swift:187` Test `menu_items_follow_the_swift_validation`.
- [x] "Show Comments" ⇧⌘M → open Assist on Comments; always enabled. `MainMenu.swift:142`, `AssistController.swift:872` Test `menu_items_follow_the_swift_validation`.
- [x] separator `MainMenu.swift:143` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Show Versions" ⌥⌘V → open Assist on Versions; always enabled. `MainMenu.swift:144`, `VersionPreview.swift:408-410` Test `menu_items_follow_the_swift_validation`.
- [x] "Older Version" ⌥⌘← → step back in the version timeline; enabled when any version exists. `MainMenu.swift:145`, `Commands.swift:188` Test `menu_items_follow_the_swift_validation`.
- [x] "Newer Version" ⌥⌘→ → step forward; enabled only while previewing. `MainMenu.swift:146`, `Commands.swift:189` Test `menu_items_follow_the_swift_validation`.
- [x] "Use This Version" ⌥⌘↩ → use the previewed version; enabled only while previewing. `MainMenu.swift:147`, `Commands.swift:190` Test `menu_items_follow_the_swift_validation`.
- [x] separator `MainMenu.swift:148` Test `menus_match_the_swift_menu_bar` (titles, order, separators, shortcuts).
- [x] "Connect an Outside Agent…", no shortcut, always → connect sheet. `MainMenu.swift:149`

### Window

- [x] "Minimize" ⌘M. **(AppKit built-in)** `MainMenu.swift:153`
- [x] "Zoom", no shortcut. **(AppKit built-in)** `MainMenu.swift:154`
- [ ] The menu is registered as the Window menu, so the system lists the open window in it. **(AppKit built-in)** `MainMenu.swift:156` — *Rust: Native macOS menus are separate work.*
- [x] No Help menu. `MainMenu.swift:20-159`

### Editor context menu (right-click in the text)

- [x] First item "Comment on Selection…" when text is selected, else "Comment on This Line…" → Add Comment. `EditorController.swift:774-776` — *Rust: Native pop-up via `tx.popup_menu`; drawn in the app where Rune has none (Linux).*
- [x] When the click is on a commented passage: that comment becomes active and a "Show Comment" item opens the Comments tab. `EditorController.swift:777-780` — *Rust: `App::draft_menu_items` (native menus) and `context_rows` (Linux). Tests: `the_drafts_own_menu_puts_the_comment_items_first`, `right_clicking_a_commented_passage_offers_show_comment`.*
- [ ] Then a separator, then the standard text-view menu: spelling suggestions, Look Up, Translate, Search With Google, Cut/Copy/Paste, Share, Spelling and Grammar, Substitutions, Transformations, Speech, Services. **(AppKit built-in)** `EditorController.swift:781-786` — *Rust: on macOS and Windows the menu is the field's own (`El::input_menu`): the app's items, a separator, then Rune's spelling suggestions and Learn Spelling, Cut, Copy, Paste (Select All off macOS) and the Spellcheck toggle; no Look Up, Translate, Share, Substitutions, Transformations, Speech or Services. Linux has no native menus: the app draws Cut, Copy, Paste and Check Spelling While Typing. Opened from the keyboard (Menu key, Shift+F10) the menu now opens under the caret (fork pinned at 11c5070; seen on Windows). Not seen on a Mac or Windows machine.*

---

## 9. Format commands

All format commands apply through the text system as one undo step with the named action, then reset typing attributes to the base style. `EditorController.swift:763-772`

- [x] Bold/Italic/Strikethrough with a caret inside a word (not at its edge): wraps the whole word. Word characters are letters, digits, `'` and `’`. `WriterCore/TextEditing.swift:133-137`, `WriterCore/TextEditing.swift:192-205` Test `a_caret_inside_a_word_wraps_the_word_with_apostrophes_in_it`.
- [x] With a caret not in a word: inserts the delimiter pair and puts the caret between them. `WriterCore/TextEditing.swift:147` Test `a_caret_at_a_word_edge_or_between_words_inserts_an_empty_pair`.
- [x] With a caret directly between an empty pair (`**|**`): removes the pair. `WriterCore/TextEditing.swift:139-146` Test `a_caret_between_an_empty_pair_removes_it`.
- [x] With a selection that includes the delimiters on both ends: removes them and selects the inner text. `WriterCore/TextEditing.swift:151-157` Test `a_selection_with_or_inside_the_delimiters_unwraps`.
- [x] With a selection directly surrounded by the delimiters: removes them and keeps the text selected. `WriterCore/TextEditing.swift:158-164` Test `a_selection_with_or_inside_the_delimiters_unwraps`.
- [x] A single `*` only counts as a delimiter when it isn't half of `**` (a `***` run counts). `WriterCore/TextEditing.swift:175-190` Test `a_single_star_is_a_delimiter_only_when_not_half_of_a_double`.
- [x] Otherwise wraps the selection, keeping leading/trailing spaces outside the delimiters, and keeps the wrapped text selected. `WriterCore/TextEditing.swift:165-172` Test `wrapping_keeps_spaces_outside_the_delimiters`.
- [x] Undo names: "Bold", "Italic", "Strikethrough". `Commands.swift:90-92` — *Rust: tested (`undo_and_redo_name_the_step_and_are_disabled_with_nothing_to_do`, `format_image_inserts_several_images_as_one_step`).*
- [x] Link with no selection: inserts `[]()` with the caret inside `[]`. `WriterCore/TextEditing.swift:210-212` Test `link_with_nothing_selected_or_a_url_selected_puts_the_caret_in_the_brackets`.
- [x] Link with a selection starting `http://`, `https://` or `www.`: `[](<selection>)` with the caret inside `[]`. `WriterCore/TextEditing.swift:213-215` Test `link_with_nothing_selected_or_a_url_selected_puts_the_caret_in_the_brackets`.
- [x] Link with other selected text: `[<selection>]()` with the caret inside `()`. `WriterCore/TextEditing.swift:216-217` Test `link_with_nothing_selected_or_a_url_selected_puts_the_caret_in_the_brackets`.
- [x] Heading 1/2/3, Bulleted List, Numbered List, Blockquote apply to every line the selection touches; each line's existing block marker is replaced. `WriterCore/TextEditing.swift:229-265` Tests `block_prefixes_replace_each_touched_lines_marker`, `numbered_list_prefix`.
- [x] If every touched line already has that exact prefix (same heading level, any bullet, any number, any quote), the command removes the prefixes (toggle off). `WriterCore/TextEditing.swift:234-240`, `WriterCore/TextEditing.swift:252-253` Test `the_same_prefix_on_every_line_toggles_off`.
- [x] Prefixes written: `# `/`## `/`### `, `- `, `1. ` `2. ` … numbered in order, `> `. `WriterCore/TextEditing.swift:255-261` Tests `block_prefixes_replace_each_touched_lines_marker`, `numbered_list_prefix`.
- [x] Body removes any block prefix. `WriterCore/TextEditing.swift:252-253` Test `the_same_prefix_on_every_line_toggles_off`.
- [x] Blank lines inside a multi-line selection are left as they are. `WriterCore/TextEditing.swift:247-250` Test `blank_lines_in_a_selection_stay_and_the_block_stays_selected`.
- [x] Caret-only: caret moves by the prefix length change; with a selection, the whole rewritten block is selected. `WriterCore/TextEditing.swift:263`, `WriterCore/TextEditing.swift:267-272` Tests `a_caret_moves_by_the_prefix_change`, `blank_lines_in_a_selection_stay_and_the_block_stays_selected`.
- [x] Undo names: "Heading", "Body", "Bulleted List", "Numbered List", "Blockquote". `Commands.swift:102-108` — *Rust: tested (`undo_and_redo_name_the_step_and_are_disabled_with_nothing_to_do`, `format_image_inserts_several_images_as_one_step`).*
- [x] Indent (⌘] or Tab in a list item): adds two spaces before every list item line touched; non-list lines unchanged; nothing happens (Tab inserts a tab) if no touched line is a list item. `Commands.swift:110-112`, `EditorController.swift:743-747`, `WriterCore/TextEditing.swift:68-109` Tests `indent_touches_only_list_items_and_tab_falls_through_without_one`, `list_continuation_and_indent_are_one_step_each`.
- [x] Outdent (⌘[ or Shift-Tab in a list item): removes up to two leading spaces, or one leading tab, from each list item line. `Commands.swift:114-116`, `EditorController.swift:748-752`, `WriterCore/TextEditing.swift:82-89` Tests `indent_touches_only_list_items_and_tab_falls_through_without_one`, `list_continuation_and_indent_are_one_step_each`.
- [x] Undo names "Indent", "Outdent". `Commands.swift:111`, `Commands.swift:115` — *Rust: tested (`undo_and_redo_name_the_step_and_are_disabled_with_nothing_to_do`, `format_image_inserts_several_images_as_one_step`).*
- [x] Return in a bullet, numbered, task or quote line (caret after the marker, no selection) continues it: same indent plus same bullet character; next number with the same `.`/`)`; task as `<bullet> [ ] `; quote with the same `>` marker. `EditorController.swift:738-742`, `WriterCore/TextEditing.swift:24-66` Tests `continuing_a_list_drops_spaces_before_the_caret_and_keeps_the_marker_style`, `bullet_continuation`, `ordered_continuation`, `task_continuation`.
- [x] Return on an item with no text removes the marker and indent (ends the list). `WriterCore/TextEditing.swift:38-42` writer-core test `empty_item_ends_list`.
- [x] Spaces right before the caret are dropped when continuing a list. `WriterCore/TextEditing.swift:60-62` Tests `continuing_a_list_drops_spaces_before_the_caret_and_keeps_the_marker_style`, `bullet_continuation`, `ordered_continuation`, `task_continuation`.
- [x] Undo name "Typing" for list continuation. `EditorController.swift:740` — *Rust: tested (`undo_and_redo_name_the_step_and_are_disabled_with_nothing_to_do`, `format_image_inserts_several_images_as_one_step`).*
- [x] Copy for Twitter (⇧⌘C): copies the selection, or the whole draft, as plain text; flashes "Copied selection as plain text · <n> characters" or "Copied draft as plain text · <n> characters" (number grouped, grapheme count). `Commands.swift:70-82` Test `copy_for_twitter_copies_plain_text_and_counts_characters` (the separator is always a comma; see section 7).
- [x] Plain-text export: headings, quotes and paragraphs lose their markers; bullets and tasks become "• "; numbered items keep "N. "/"N) "; indentation becomes two spaces per level (tab = 4 spaces); fences dropped; code-block lines kept as is; rules become blank lines; runs of blank lines collapse to one; trailing blank lines removed. `WriterCore/PlainTextExporter.swift:9-46` Test `export_drops_block_markers_and_keeps_list_shapes`.
- [x] Plain-text inline rules: delimiters removed; images removed; links become "label (url)", or just the url when the label is empty, equals the url, or starts with "http"; backslash escapes removed (not when the line has code). `WriterCore/PlainTextExporter.swift:59-113` Test `export_inline_rules`.

---

## 10. Sidebar, Recent and Folder

### Panel

- [x] Width 260 pt; background `sidebar`; 1 pt `hairline` on the right edge. `Sidebar.swift:58-66`, `Sidebar.swift:89`
- [x] Hidden by default; pinned state persisted. `Preferences.swift:38-41`, `EditorController.swift:130`
- [x] Pinning/unpinning sets the traffic lights fully opaque, shows chrome with auto-hide, and re-lays out. `RemoteEditing.swift:49-62` — Rust: chrome shown with auto-hide and the editor re-laid out; there are no traffic lights to force opaque until the window-chrome work draws them.
- [x] Pinning reloads the list. `RemoteEditing.swift:54-55`
- [x] Segmented control "Recent" | "Folder", small size, IBM Plex Mono 11 pt, equal widths, at x 14, y 42, width panel−29, height 22; accessibility label "Sidebar view". `Sidebar.swift:73`, `Sidebar.swift:101-108`, `Sidebar.swift:160` — Rust: drawn by the app (AppKit's control look approximated with `sidebarCurrent` track and `sidebarSelection` segment).
- [x] Mode is persisted; default Recent. `Preferences.swift:43-46`, `Sidebar.swift:299-304`
- [x] List rows: 42 pt with a subtitle, 28 pt without; no header; no grid; transparent background; no focus ring; accessibility label "Files". `Sidebar.swift:134-147`, `Sidebar.swift:341-343`
- [x] List starts at y 74 (y 96 when the folder header shows) and fills the rest; overlay auto-hiding scroller. `Sidebar.swift:161-168`, `Sidebar.swift:148-153` — Rust: the scroller is Rune's scroll view's own.
- [x] Row of the open file (when not selected): `sidebarCurrent` rounded rect inset 8 pt horizontally, 1 pt vertically, radius 6. `Sidebar.swift:35-39`
- [x] Selected row: `sidebarSelection` with the same shape. `Sidebar.swift:41-44`
- [x] Row icon (11 pt, `quiet`): `arrow.turn.left.up` for "..", `folder` for folders, `network` for remote files, `doc.text` for local files. `Sidebar.swift:349-357` — Rust: vector icons in a 14 pt box.
- [x] Row title IBM Plex Mono 13 pt, `text` for the open file, `text` α0.8 otherwise, truncates at tail. `Sidebar.swift:358-361`
- [x] Two-line row: icon 14×14 at (18, 22), title at (38, 21), subtitle IBM Plex Mono 11 pt `quiet` at (38, 4), truncating in the middle. `Sidebar.swift:363-371` — Rust: the Swift cell isn't flipped; measured from the top these are icon (18, 6), title (38, 4), subtitle (38, 23).
- [x] One-line row: icon at (18, 7), title at (38, 5). `Sidebar.swift:372-375` — Rust: from the top: icon (18, 7), title (38, 6).
- [x] Row accessibility label "<title>, <subtitle>". `Sidebar.swift:378`
- [x] Type-to-select by title. `Sidebar.swift:382-384` — Rust: prefix match, keys within 1 s add to it.
- [x] Single click on a row activates it. `Sidebar.swift:144-145`, `Sidebar.swift:310-314`
- [x] Return or keypad Enter activates the selected row. `Sidebar.swift:49-56`, `Sidebar.swift:146`
- [x] Activating a file deselects the row and opens it (remote files over SSH). `Sidebar.swift:325-327`
- [x] After every list load the selection is cleared and the open file is re-marked. `Sidebar.swift:284-289`
- [x] Empty-state message: IBM Plex Mono 12 pt, `quiet`, centred, wrapping, at listY + 24, 20 pt side margins, 60 pt tall. `Sidebar.swift:129-132`, `Sidebar.swift:170`

### Recent

- [x] Up to 100 files: every draft-extension file in the drafts folder plus every remembered file that still exists, newest first by max(modified, last opened). `Sidebar.swift:192-197`, `WriterCore/DraftStore.swift:321-345`
- [x] Row title is the remembered title (or remote name, or file name); subtitle is the remote location, or the parent folder with `~` for home. `Sidebar.swift:194-196`, `WriterCore/DraftStore.swift:340`
- [x] Empty message "Files you open show up here." `Sidebar.swift:198`
- [x] The Recent list refreshes after saves and document changes. `Sidebar.swift:187-190`, `EditorController.swift:372-374`, `EditorController.swift:354`

### Folder

- [x] With no folder chosen: message "Open a folder to see its files here." and a centred small rounded "Open Folder…" button at listY + 90. `Sidebar.swift:204-211`, `Sidebar.swift:171-173`
- [x] Header (only in Folder mode with a folder): IBM Plex Mono 11 pt `quiet`, truncating in the middle, at (16, 74), width panel−53; text is the folder name, or "root › sub › sub" when navigated into subfolders; tooltip is the full path (`~` abbreviated) or remote location. `Sidebar.swift:110-113`, `Sidebar.swift:161-166`, `Sidebar.swift:214-215`, `Sidebar.swift:15-20`
- [x] Remote folder name is the last path component, or "<host> ~" for the home folder. `Sidebar.swift:8-13`
- [x] Refresh button `arrow.clockwise`, borderless, `quiet` tint, 20×20 at (panel−33, 72), tooltip "Refresh", accessibility description "Refresh folder"; reloads the list. `Sidebar.swift:115-121`, `Sidebar.swift:166`, `Sidebar.swift:307`
- [x] Lists folders and files ending md/markdown/txt/text (case-insensitive); hidden files skipped. `Sidebar.swift:238-243`, `Sidebar.swift:251-252` — Rust: local listings run off the UI thread too.
- [x] Sort: folders first, then Finder-style name order. `Sidebar.swift:253` — Rust: case-insensitive with digit runs compared by value.
- [x] When inside a subfolder, the first row is ".." (no subtitle); clicking it goes up one level. `Sidebar.swift:269-271`, `Sidebar.swift:318-320`
- [x] Clicking a folder row goes into it. `Sidebar.swift:321-324`
- [x] Empty folder message "No Markdown or text files here." `Sidebar.swift:263`
- [x] Listing error: message is the error text, ".." row kept. `Sidebar.swift:247-249` — Rust: local errors use Foundation's wording ("The folder “x” doesn’t exist.").
- [x] Remote folder: shows "Loading <host>…" while listing in the background; a stale listing result (after navigating or reloading) is ignored. `Sidebar.swift:219-235`
- [x] Only the root folder is persisted (local path or remote location); subfolder navigation is not. `Preferences.swift:48-63`, `Sidebar.swift:96`, `Sidebar.swift:293-297`
- [x] File › Open Folder… (⌥⌘O): folder-only open panel as a sheet, button "Open Folder", message "Choose a folder to list its Markdown and text files in the sidebar.", starting in the current local root; on OK pins the sidebar and shows the folder. `RemoteEditing.swift:78-92` — Rust: `FileDialogOptions::prompt` and `message`, shown on the Mac's panel; the Linux portal takes the button label but has no message, and Windows shows neither, so the message is also the dialog's title there. Test: `open_folder_names_its_button_and_explains_itself`. Not seen in a real dialog.
- [x] The "Open Folder…" button in the sidebar runs the same command. `Sidebar.swift:308`

### Peek

- [ ] With the sidebar not pinned, holding the pointer within 4 pt of the window's left edge (no mouse button down, window key) for 0.12 s, and still within 10 pt then, slides the sidebar in over the text. `ChromeControls.swift:171-199` — Rust: Rune doesn't tell the app whether a mouse button is held, so the "no mouse button down" condition is missing. Audit: the "no mouse button down" condition is missing: Rune doesn't say whether a button is held (GAPS).
- [x] Peek slides in from x −260 to 0 over 0.18 s ease-out (instant with Reduce Motion), with a black shadow, opacity 0.22, radius 18, offset (4, 0); list reloads; chrome shown with no auto-hide. `ChromeControls.swift:201-226`
- [x] Peek closes 0.25 s after the pointer moves more than 296 pt (260 + 36) from the left edge; moving back cancels the timer. `ChromeControls.swift:174-186`
- [x] Clicking in the text closes the peek. `EditorController.swift:183-186` — Rust: detected as the caret moving with no key pressed and the pointer right of the panel; a click that leaves the caret where it was doesn't close it.
- [x] Peek slides out over 0.16 s ease-in (instant with Reduce Motion), then chrome auto-hide restarts. `ChromeControls.swift:228-246` Audit: under Reduce Motion it still slid out; now it goes at once (test `reduce_motion_makes_the_chrome_and_the_peek_instant`).
- [x] Clicking the sidebar button (or ⌃⌘S) while peeking pins the sidebar and saves it as shown. `ChromeControls.swift:249-258`, `RemoteEditing.swift:50`

---

## 11. Open picker (⌘O)

- [x] Saves the current draft first, then shows a titled sheet, 520×420 pt. `Commands.swift:13-16`, `OpenPicker.swift:18-24`, `OpenPicker.swift:36`
- [x] Search field at top (16 pt margins), IBM Plex Mono 14 pt, placeholder "Find a draft", no focus ring, accessibility label "Find a draft"; focused when the sheet appears. `OpenPicker.swift:37-41`, `OpenPicker.swift:76-79`, `OpenPicker.swift:95-98`
- [x] List 10 pt under the search, 8 pt side margins, 12 pt above the buttons; rows 50 pt; accessibility label "Recent drafts". `OpenPicker.swift:44-55`, `OpenPicker.swift:80-83`
- [x] Lists up to 200 drafts (drafts folder + remembered files), newest first. `OpenPicker.swift:91`, `WriterCore/DraftStore.swift:321`
- [x] Row: title IBM Plex Mono Medium 13 pt; below, IBM Plex Mono 11 pt secondary-label colour, "<relative date> — <preview>", 3 pt apart, 10 pt side insets, vertically centred; both truncate at tail. `OpenPicker.swift:135-158` — Rust: secondary-label colour is `text` at α0.5.
- [x] Relative date uses the short style ("2 hr. ago"); omitted when unknown. `OpenPicker.swift:141-143`
- [x] Preview = plain text of the first 600 bytes without the first line, joined with spaces, up to 140 characters. `WriterCore/DraftStore.swift:392-399`
- [x] Row accessibility label "<title>, <relative date>". `OpenPicker.swift:159`
- [x] Filtering as you type: case-insensitive substring of title or preview, query trimmed of spaces. `OpenPicker.swift:100-103`, `OpenPicker.swift:114`
- [x] Initial selection: the second row when the open draft is first, there is more than one row and the query is empty; otherwise the first row. `OpenPicker.swift:106-111`
- [x] Empty states centred over the list, IBM Plex Mono 13 pt `quiet`: "No drafts yet" or "No matching drafts". `OpenPicker.swift:62-64`, `OpenPicker.swift:104-105`
- [x] ↓/↑ in the search field move the selection (clamped). `OpenPicker.swift:116-131`
- [x] Return opens the selected draft; Esc cancels. `OpenPicker.swift:120-121`
- [x] Double-click a row opens it. `OpenPicker.swift:54`
- [x] Buttons along the bottom (16 pt margins, 14 pt from bottom): "Other File…", "New Draft", flexible space, "Cancel" (Esc), "Open" (Return, default). `OpenPicker.swift:66-73`, `OpenPicker.swift:86-88`
- [x] Other File… closes the sheet and shows the file open panel; New Draft closes and runs New. `OpenPicker.swift:175-176`, `Commands.swift:19-22` — Rust: A remote file's row opens over SSH (owner-approved fix of oddity 13).
- [x] The chosen action runs after the sheet has closed. `OpenPicker.swift:163-168`

---

## 12. SSH and remote editing

- [x] File › Open over SSH… (⌃⌘O): sheet alert, title "Open over SSH", text "Enter a file to edit or a folder to browse, like me@server:~/notes or ssh://me@server:2222/srv/notes/draft.md. Your SSH config and keys are used; password prompts aren't supported." `RemoteEditing.swift:96-100` — Rust: the app's alert (`ui.rs`) with the field as its accessory, 380 pt wide.
- [x] Text field 340×24, placeholder "user@host:~/notes/draft.md", prefilled with the last location, initially focused, accessibility label "SSH location"; buttons "Open", "Cancel". `RemoteEditing.swift:101-108`
- [x] Accepted forms: `host:path`, `user@host:path`, `ssh://[user@]host[:port]/path`; empty path means `~`; `ssh://host/~/x` means `~/x`. `WriterCore/Remote.swift:17-33`
- [x] Rejected: empty host, host starting with `-`, host containing whitespace or any of `/'"`$;|&<>()\`, path containing newline or NUL. `WriterCore/Remote.swift:35-40`
- [x] Invalid input: second sheet alert "That doesn't look like an SSH location." / "Use host:path, user@host:path, or ssh://user@host:port/path." `RemoteEditing.swift:112-118`
- [x] A valid location is remembered for next time. `RemoteEditing.swift:119`, `Preferences.swift:65-68`
- [x] Flashes "Connecting to <host>…" and checks in the background whether the path is a folder, file or missing. `RemoteEditing.swift:124-128`, `WriterCore/Remote.swift:113-121`
- [x] Folder → pin the sidebar and show it in Folder mode. File → open it. Missing → alert "Nothing at <location>." Error → alert with the ssh error. `RemoteEditing.swift:131-141`
- [x] Runs `/usr/bin/ssh -o BatchMode=yes -o ConnectTimeout=10 -o ServerAliveInterval=5 -o ServerAliveCountMax=2 [-p port] <host> <command>`, so `~/.ssh/config`, keys and agent apply and passwords are never prompted. `WriterCore/Remote.swift:86-100`
- [x] Remote paths starting `~` stay unquoted so the remote shell expands them; everything else is single-quoted. `WriterCore/Remote.swift:61-70`
- [x] SSH error "…Permission denied…" → "<host> refused the login. Fluent Writer can't type passwords, so use an SSH key or ssh-agent (ssh-copy-id <host>)." `WriterCore/Remote.swift:176-178`
- [x] SSH error "Host key verification failed" → "<host> isn't a known host yet. Run `ssh <host>` in Terminal once to trust it." `WriterCore/Remote.swift:179-181`
- [x] No stderr → "ssh to <host> failed (exit <status>)."; otherwise "<host>: <last stderr line>" (ignoring "Warning: Permanently added" lines). `WriterCore/Remote.swift:173-184` — Rust: the stderr case is checked against the test server ("<host>: ssh: connect to host … Connection refused"); the no-stderr case only by writer-core's tests.
- [x] ssh can't start → "Couldn't start ssh: <reason>". `WriterCore/Remote.swift:148-150`
- [x] Opening a remote file that is already open just focuses the editor. `RemoteEditing.swift:147`
- [x] Opening a remote file runs the leave check, flashes "Opening <name> from <host>…" and reads it in the background; only the latest open request is applied. `RemoteEditing.swift:146-160`
- [x] Remote text is kept in a local mirror under `<support>/Remote/<host>[-<port>]/home|root/<path components>`. `WriterCore/DraftStore.swift:350-359`
- [x] On a successful read the mirror is overwritten with the server's text, unless the mirror has saved edits that never reached the server; those are kept and the app flashes "Kept your edits that hadn't reached <host>" and uploads them 0.2 s later. `WriterCore/DraftStore.swift:366-379`, `RemoteEditing.swift:166-169`, `EditorController.swift:339`
- [x] Remote file title is the remote file name unless the writer named it. `WriterCore/DraftStore.swift:376`
- [x] If the server can't be reached and this file was opened before (record and mirror exist), the mirror opens and the footer shows "Offline · editing the local copy"; otherwise an error alert. `RemoteEditing.swift:170-176`
- [x] Each save writes the mirror first, then uploads in the background. `EditorController.swift:369-371`, `WriterCore/DraftStore.swift:201-204`
- [x] Upload writes `<path>.fwtmp-<8 hex>` beside the target, then `mv -f` over it; on failure the temp file is removed. `WriterCore/Remote.swift:106-111`
- [x] One upload at a time; while one runs, only the newest pending text is queued. `RemoteEditing.swift:9-35`
- [x] The "not yet uploaded" flag clears only if the uploaded text is still the last saved text; the flag is persisted per file. `WriterCore/DraftStore.swift:382-390`
- [x] Leaving a remote file with an un-uploaded save uploads it synchronously first; on failure a warning alert: "Your latest edits aren't on <host> yet." / "<error>\n\nThey're saved on this Mac and will be sent the next time you open this file.", buttons "Keep Editing" (stay), "Continue" (leave). `RemoteEditing.swift:194-211` — Rust: the upload runs off the UI thread behind a waiting sheet ("Saving to <host>…", Keep Editing cancels leaving); the warning alert and its buttons are as listed.
- [x] Show in Finder is disabled for remote files. `Commands.swift:182`
- [x] Remote files show a `network` icon in the sidebar. `Sidebar.swift:353`
- [x] Remote folder listing runs `cd -- <path> && ls -1Ap`; entries starting with `.` are skipped; trailing `/` marks folders. `WriterCore/Remote.swift:123-137`

---

## 13. Images

### Inline previews

- [x] A paragraph line that holds only an image (`![alt](src)` or `![alt](<src>)`, optional title, up to 3 leading spaces) gets a framed preview below it. Lines in code blocks don't. `WriterCore/MarkdownImages.swift:11-31` Tests `image_lines_say_why_they_show_no_image`, `finds_image_lines_only`.
- [x] The Markdown line stays editable above the preview and is coloured `quiet`. `ImageBlocks.swift:168-169` Test `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`.
- [x] Space for the preview is made with paragraph spacing after the line = preview height + 2 × gap; gap = `round(lineHeight × 0.45)`. `ImageBlocks.swift:135`, `ImageBlocks.swift:161-167` Test `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`.
- [x] Preview position: x = text origin + hang width (prose edge), y = bottom of the line's last fragment + gap. `ImageBlocks.swift:201-206` Test `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`.
- [x] Preview width = min(column width, natural width); height by aspect ratio; if taller than max height, height = max height and width scaled. Column width = `max(80, containerWidth − hangWidth)`; max height = `max(160, min(560, visibleHeight × 0.7))`. `ImageBlocks.swift:137-146`, `ImageBlocks.swift:179-185` Audit: `display_size` follows the Swift; test `image_lines_say_why_they_show_no_image` (an image narrower than the column keeps its size). The height cap and the unknown-size case weren't exercised.
- [x] Image with unknown size uses column width × 0.6 column width. `ImageBlocks.swift:180` Audit: `display_size` follows the Swift; test `image_lines_say_why_they_show_no_image` (an image narrower than the column keeps its size). The height cap and the unknown-size case weren't exercised.
- [x] Image is drawn clipped to a rounded rect (radius 6) with a 1 pt `hairline` border, high-quality interpolation. `ImageBlocks.swift:23-34` Audit: code against the Swift; placeholder size tested (`an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`).
- [x] Placeholder: min(column, 420) × 40 pt, `codeBackground` fill, radius 6, SF Symbol 12 pt + message in IBM Plex Mono 11 pt, both in `text` α0.62, 12 pt left inset, 8 pt gap, truncated. `ImageBlocks.swift:35-51`, `ImageBlocks.swift:177-178` Audit: code against the Swift; placeholder size tested (`an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`).
- [x] Remote (SSH) draft: placeholder `photo` "Images aren't shown for files opened over SSH". `ImageBlocks.swift:229` Test `a_remote_draft_takes_no_images`.
- [x] Unresolvable source: `questionmark.square.dashed` "Can't read image address “<source>”". `ImageBlocks.swift:230` Test `image_lines_say_why_they_show_no_image`.
- [x] Missing local file: `photo.badge.exclamationmark` "Image not found · <source>". `ImageBlocks.swift:233` Test `image_lines_say_why_they_show_no_image`.
- [x] Unreadable local file: `photo.badge.exclamationmark` "Can't open <filename>". `ImageBlocks.swift:236-238` Test `image_lines_say_why_they_show_no_image`.
- [x] Web image loading: `photo` "Loading <host>…". `ImageBlocks.swift:244-245` Test `an_error_page_is_not_an_image`. An HTTP error page now reads "Not an image" as with URLSession; it read "Couldn't load" before (fixed this pass).
- [x] Web image failed: `wifi.exclamationmark` "Couldn't load image from <host>" (network error) or "Not an image · <host>" (data isn't an image). Failures are remembered for the session. `ImageBlocks.swift:243`, `ImageBlocks.swift:283` Test `an_error_page_is_not_an_image`. An HTTP error page now reads "Not an image" as with URLSession; it read "Couldn't load" before (fixed this pass).
- [x] Sources resolve as: http/https/file URLs as is; other single-letter-or-longer schemes rejected; `/…` absolute; `~…` home; otherwise relative to the draft's folder (drafts folder for unsaved drafts); percent-encoding decoded. `WriterCore/MarkdownImages.swift:34-44`, `ImageBlocks.swift:152` writer-core test `resolve`.
- [ ] Local images are downsampled to the column width × the screen's backing scale (pixels), honouring EXIF orientation; size in points uses the image DPI (72 if absent), so 2× screenshots show at on-screen size. `ImageBlocks.swift:234`, `ImageBlocks.swift:249-266` Audit: images are decoded at full size rather than downsampled to the column (same look, more memory for large photos); EXIF orientation is Rune's (its test); points from PNG pHYs or JPEG JFIF density; the decode scale now follows the window's screen (fixed this pass, it was always 2). See GAPS.
- [ ] SVG and other formats ImageIO can't read are loaded with the generic image loader. `ImageBlocks.swift:267-269` Audit: SVG goes through Rune's rasteriser at the column width; whether Rune's decoder reads TIFF and HEIC wasn't checked.
- [x] Local images are cached and reused while the file's modification date is unchanged and the cached size is at least 90% of what's needed. `ImageBlocks.swift:235` Audit: `Images::content`, `fetch` and `poll` follow the Swift; tests `an_error_page_is_not_an_image`, `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`.
- [x] Web images are fetched once per URL per session and the previews refresh when they arrive. `ImageBlocks.swift:272-288` Audit: `Images::content`, `fetch` and `poll` follow the Swift; tests `an_error_page_is_not_an_image`, `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`.
- [x] Previews reflow as text is edited, resized or restyled. `ImageBlocks.swift:149-173`, `EditorController.swift:314` Audit: `Images::content`, `fetch` and `poll` follow the Swift; tests `an_error_page_is_not_an_image`, `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`.
- [x] Previews outside the focused sentence/paragraph fade to α0.28. `ImageBlocks.swift:218-224` Test `image_lines_are_quiet_and_their_previews_fade_outside_the_focus`.
- [ ] Clicking a preview puts the caret at the end of its Markdown line and focuses the editor (also on first click when the window isn't key). `ImageBlocks.swift:21`, `ImageBlocks.swift:54-59` Audit: a click puts the caret at the line's end and focuses the draft (code); a first click on a window that isn't key wasn't checked.
- [ ] Double-clicking opens the image in the default app, only for http/https links or existing local files whose type is an image. `ImageBlocks.swift:55`, `ImageBlocks.swift:82-90` Audit: built (only http(s) and existing image files); not run here, where it would open a viewer.
- [ ] Right-click menu: "Open Image" (when openable), "Show in Finder" (local files), "Copy Image" (when the image loaded; copies the full-resolution file for local images). No menu when the source didn't resolve. `ImageBlocks.swift:61-80`, `ImageBlocks.swift:94-99` Audit: menu built; Copy Image copies image files only, not images loaded from the web (GAPS).
- [x] Preview tooltip: local path with `~`, or the URL. `ImageBlocks.swift:211` Audit: `help` on each preview, `~` for the home folder (`platform::abbreviate_home`).
- [x] Preview accessibility role image, label = alt text or "Image". `ImageBlocks.swift:101-102`, `ImageBlocks.swift:212` Test `image_lines_say_why_they_show_no_image`.

### Adding images

- [x] Paste (⌘V) with image files on the clipboard, or image data (PNG/TIFF) and no text, inserts images; if there's any text on the clipboard the text is pasted instead. `EditorTextView.swift:28-31`, `ImageBlocks.swift:300-310` Test `pasting_image_files_inserts_them`. Image files on the clipboard were pasted as text before (fixed this pass).
- [ ] Dragging image files or PNG/TIFF data onto the text shows a copy cursor and inserts at the drop point. `EditorTextView.swift:39-59` Audit: dropped image files go in at the drop point (test `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`); dropped PNG or TIFF data doesn't, as Rune drops are files only (GAPS).
- [x] Image file extensions: png, jpg, jpeg, gif, heic, heif, webp, tiff, tif, bmp, svg. `WriterCore/MarkdownImages.swift:15` Audit: `markdown_images::IMAGE_EXTENSIONS`.
- [x] Format › Image… (⇧⌘I): open panel as a sheet, those image types, multiple selection, message "Images are copied into an images folder next to this draft." `ImageBlocks.swift:361-374` — *Rust: `Tx::open_files` with the message (also the title off the Mac, where Windows and the Linux portal show no message). Test: `format_image_inserts_several_images_as_one_step`. Not seen in a real dialog.*
- [x] While a version preview is showing, adding an image beeps and does nothing. `ImageBlocks.swift:311` — *Rust: `Tx::beep`; test `the_preview_beeps_at_typing_and_at_images`.*
- [x] Remote drafts: flashes "Images can't be added to files opened over SSH yet". `ImageBlocks.swift:312-315` Test `a_remote_draft_takes_no_images`.
- [x] A file already inside the draft's folder (any depth) is linked by relative path, not copied. `ImageBlocks.swift:323-326`, `WriterCore/MarkdownImages.swift:47-52` Test `an_image_already_beside_the_draft_is_linked_and_blank_lines_keep_it_on_its_own_line`.
- [x] Other files are copied into `images/` beside the draft (created if needed), renamed "name 2.ext", "name 3.ext" … if taken. `ImageBlocks.swift:327-331`, `WriterCore/MarkdownImages.swift:70-80` Tests `block_insertion_and_unique_names`, `an_image_line_gets_room_under_it_and_dropped_files_are_copied_beside_the_draft`.
- [x] Pasted image data is saved as `images/Pasted image yyyy-MM-dd at HH.mm.ss.png` (TIFF converted to PNG), with empty alt text. `ImageBlocks.swift:333-341` Audit: `insert_images` names it `Pasted image %Y-%m-%d at %H.%M.%S.png`; Rune hands the clipboard image over as PNG.
- [x] Alt text from a file name: extension dropped, runs of `-`/`_` become a space, trimmed. `WriterCore/MarkdownImages.swift:64-67` Tests `alt_and_relative_path`, `markdown_escapes_path_and_round_trips`, `pasting_image_files_inserts_them`.
- [x] Markdown inserted is `![alt](path)` with the path percent-encoded (spaces and parentheses escaped) and `[`/`]`/newlines removed from alt. `WriterCore/MarkdownImages.swift:55-61` Tests `alt_and_relative_path`, `markdown_escapes_path_and_round_trips`, `pasting_image_files_inserts_them`.
- [x] Several images are separated by a blank line; a blank line is added before/after when the insertion point isn't already at a line start/end; the caret goes after the inserted text. `ImageBlocks.swift:347-352` Test `an_image_already_beside_the_draft_is_linked_and_blank_lines_keep_it_on_its_own_line`.
- [x] Undo name "Insert Image" or "Insert Images". `ImageBlocks.swift:353-354` — *Rust: tested (`undo_and_redo_name_the_step_and_are_disabled_with_nothing_to_do`, `format_image_inserts_several_images_as_one_step`).*
- [x] Flash "Image added · copied to the images folder beside this draft" / "<n> images added · …", or "… · linked where it already is" when nothing was copied. `ImageBlocks.swift:356-357` Test `an_image_already_beside_the_draft_is_linked_and_blank_lines_keep_it_on_its_own_line`.
- [x] Copy failure flashes "Couldn't save the image: <reason>"; images already handled are still inserted. `ImageBlocks.swift:342-345` Audit: the reason now reads as Foundation's, without Rust's "(os error N)" (fixed this pass).

---

## 14. Assist panel

### Panel

- [x] Panel background `canvas`, 1 pt `hairline` on its left edge; accessibility group "Assist". `AssistController.swift:6-14`, `AssistController.swift:182-183` Built in `assist_view.rs`.
- [x] Created on first use. `AssistController.swift:857-864` The panel's state lives on `App` from launch (`assist.rs`); nothing visible differs.
- [x] Showing it adds it at the right edge (widening the window if narrow), selects the requested tab, refreshes the scope line and lists, and focuses the instruction field when on Ask. `AssistController.swift:377-391`
- [x] Ask Codex / Ask Claude switch the provider and open on Ask. `AssistController.swift:385`, `AssistController.swift:870-871`
- [x] Closing stops any running request, removes the panel and focuses the preview text (if previewing) or the editor. `AssistController.swift:393-400`
- [x] Esc in the editor: clears the active comment if any; otherwise closes the Assist panel if open. `EditorController.swift:753-755`
- [x] Chrome does not auto-hide on typing while the panel is open. `EditorController.swift:653`
- [x] Panel padding 22 pt. `AssistController.swift:294`
- [x] Provider pop-up at (18, 40), 120×24, borderless, IBM Plex Mono 12 pt, items "Codex", "Claude"; accessibility label "Provider". `AssistController.swift:185-191`, `AssistController.swift:296` A native menu where Rune has one (macOS, Windows); on Linux, where Rune has no pop-up menus, the app draws the menu itself with the current provider ticked over the button. The chevrons are a vector stand-in for the pop-up's arrows.
- [x] Provider choice persisted under `assist.provider`; default Codex. `AssistController.swift:138`, `AssistController.swift:668-674` Kept in `preferences.json` under the same key.
- [x] Close button: text "Close", borderless, IBM Plex Mono 12 pt, `quiet`, 50×20 at right edge − 22, y 42; accessibility label "Close assist". `AssistController.swift:193-199`, `AssistController.swift:297`
- [x] Tabs: segmented "Ask" | "Comments" | "Versions", small, IBM Plex Mono 11 pt, equal widths, at (22, 72), full inner width, 22 tall; accessibility label "Assist section". `AssistController.swift:109`, `AssistController.swift:201-207`, `AssistController.swift:298` The segmented control is drawn in the app: a grey track with the selected segment on a raised plate.
- [x] Tab labels show counts: "Comments <open count>" and "Versions <count>" when non-zero. `AssistController.swift:472-477`
- [x] Tab content starts at y 106. `AssistController.swift:299`
- [x] Switching tabs refreshes lists and toolbar button states. `AssistController.swift:423-427`

### Ask tab

- [x] Scope line (IBM Plex Mono 12 pt, `text`, 18 pt tall, truncating): "Full draft · <n> word(s) → <Provider>". `AssistController.swift:212-213`, `AssistController.swift:465`
- [x] With non-empty selected words: "Draft · focus on <n> selected word(s) → <Provider>". `AssistController.swift:460-463`
- [x] While previewing a version: "Version “<label>” · <n> words → <Provider>". `AssistController.swift:449-453` Refreshes on the Swift triggers only (selection, provider, show, a request ending), so previewing a version from its card leaves the line stale until one of those, as in the Swift app (Oddity 29).
- [x] Empty draft: "Write something first". `AssistController.swift:456`
- [x] Scope accessibility labels: "Will send the full draft, <n> words, to <Provider>"; "Will send the draft, <n> words, focused on <m> selected words, to <Provider>"; "Will send the previewed version, <n> words, to <Provider>". `AssistController.swift:452`, `AssistController.swift:463`, `AssistController.swift:466` Set on the scope text; checked in tests, not with a screen reader. Test `the_assist_panel_and_its_cards_name_themselves`.
- [x] Scope updates on selection change (not while a request runs) and on provider change. `AssistController.swift:438-439`, `AssistController.swift:672`
- [x] Privacy note (IBM Plex Mono 11 pt, `quiet`, wraps): "Nothing is sent until you press Send. The agent gets this draft, your selection and comments, with no files, commands or web access." `AssistController.swift:214-215`, `AssistController.swift:318-320`
- [x] Quick-action buttons "Critique", "Clarify", "Shorten", "Stronger opening": recessed style, IBM Plex Mono 12 pt, 22 pt tall, fitted width + 8, 6 pt apart, wrapping to new rows 28 pt apart; accessibility label "<label> with the selected provider". `AssistController.swift:216-224`, `AssistController.swift:321-328`, `WriterAgents/AgentModels.swift:25-36` The recessed bezel is drawn as a grey plate; widths (text + 24 + 8) match `docs/screenshots/assist-diff.png` to the point.
- [x] Quick actions and the provider pop-up are disabled while a request runs. `AssistController.swift:850-851`
- [x] Instruction field: 44 pt tall, width inner − 74, rounded bezel, IBM Plex Mono 13 pt, wraps, placeholder "Ask anything, or say what to change or add", no focus ring; accessibility label "Instruction". `AssistController.swift:225-235`, `AssistController.swift:330` The rounded bezel is drawn: `card` fill, a 1 pt edge, radius 6.
- [x] Return in the instruction field sends it (ignored when blank); Esc stops a running request, otherwise closes the panel. `AssistController.swift:693-703`, `AssistController.swift:684-687` Shift-Return sends too, as in an AppKit field editor.
- [x] Option-Return in the instruction field inserts a line break. **(AppKit built-in field-editor behaviour)**
- [x] Send button 66×28, rounded, IBM Plex Mono 12 pt, no key equivalent; "Send" (accessibility "Send instruction") or "Stop" while running (accessibility "Stop request"); Stop cancels the request. `AssistController.swift:236-240`, `AssistController.swift:331`, `AssistController.swift:689-691`, `AssistController.swift:848-849` The bezel is a 22 pt grey plate centred in the 28 pt frame, as in the screenshot.
- [x] Status line (IBM Plex Mono 12 pt, wraps) under the field. `AssistController.swift:241`, `AssistController.swift:333-335`
- [x] "Preview Version" small button appears under the status when the last reply's version still exists; it opens the preview. `AssistController.swift:257`, `AssistController.swift:336-342`, `AssistController.swift:656-658`
- [x] "Feedback" header (IBM Plex Mono 11 pt, `quiet`) shown while running or when there is feedback; feedback below it, read-only selectable, IBM Plex Mono 13 pt, `text`, line height ×1.35, filling to 20 pt above the panel bottom; accessibility label "Feedback". `AssistController.swift:242-256`, `AssistController.swift:343-345`, `AssistController.swift:839-845` Line height round(13 × 1.3 × 1.35) = 23 pt. Accessibility label set, not checked with a screen reader.

### Sending

- [x] The request uses the previewed version's text when previewing, otherwise the draft; nothing is sent if that text is blank. `AssistController.swift:705-709`
- [x] Sending stops any earlier request, clears the result area, and flushes notes to disk. `AssistController.swift:710-712`
- [x] The selection is sent as the passage to focus on only when not previewing and it isn't blank. `AssistController.swift:713-716`
- [x] Ask (custom) and Send Comments include all open, still-attached comments with their replies; quick actions send none. `AssistController.swift:717-722`
- [x] The last 4 Assist exchanges for this draft are sent as history. `AssistController.swift:724`
- [x] Status while waiting: "<Provider> is reading the draft…", "… the version…", or "… <n> comment(s)…". `AssistController.swift:736-738`
- [x] Send Comments switches to the Ask tab. `AssistController.swift:735`
- [x] Feedback streams in as the reply arrives; status changes to "Drafting edits…" once the reply contains `<edit`. `AssistController.swift:751-753`, `WriterAgents/AgentModels.swift:213-228`
- [x] Failure: feedback cleared, status shows the error in `warning`. `AssistController.swift:758-764`
- [x] Stop (button, Esc, closing the panel, opening another document, quitting): "Stopped. Nothing was changed." `AssistController.swift:402-412`, `AssistController.swift:431-436`, `main.swift:40` Quitting drops the request's `AgentHandle`, which stops it.
- [x] A reply from a stopped/older request is ignored. `AssistController.swift:743-746`
- [x] Replies to comments are added to those comments with the provider name as author. `AssistController.swift:774-776`
- [x] Edits are placed on the text as it is when the reply arrives (typing during the wait is kept); duplicate matches pick the occurrence nearest the selection or the edit's comment. `AssistController.swift:777-783`, `WriterCore/EditPlacement.swift:69-90`
- [x] If the edits change the text, a new version is created (draft untouched): label is the instruction (one line, 48 characters max: 47 + "…"), "Resolve <n> comment(s)", or the action label; "(from “<base label>”)" appended when based on a version; source is the provider; feedback attached; resolves the comments its edits named. `AssistController.swift:784-800`, `AssistController.swift:828-831`
- [x] Feedback shown: the reply's notes; "No feedback." when there are no notes, no version and no replies. `AssistController.swift:802-803`
- [x] Appended: "Answered <n> comment(s) — see Comments." when the agent replied to comments. `AssistController.swift:804-806`
- [x] Appended: "<n> edit(s) couldn't be placed:" then for each "• <reason>\n  “<first 300 chars of new text>”". Reasons: "The edit didn't say what text to change.", "Couldn't find the text this edit changes. It may have been edited since.", "This edit overlaps another one." `AssistController.swift:807-812`, `WriterCore/EditPlacement.swift:80-89`
- [x] Every completed request is added to the draft's Assist history (provider, action, instruction, feedback, version, comments). `AssistController.swift:814-815`
- [x] With a version: status "<n> edit(s) ready as a new version, shown in the editor. ← → switch versions · ↩ use it · esc back to your draft." and the preview opens; otherwise the status clears. `AssistController.swift:816-823`
- [x] Send Comments that got only replies (no version) switches to the Comments tab. `AssistController.swift:824`
- [x] Opening a different document stops the request and clears the result. `AssistController.swift:431-436`, `EditorController.swift:353`

### Providers

- [x] Codex runs `codex app-server` (found on PATH, Homebrew, /usr/local, ~/.local/bin, ~/.npm-global/bin, ~/.bun/bin, ~/.volta/bin, ~/.claude/local, ~/.codex/bin, or a login-shell lookup) in an empty temporary folder, ephemeral thread, read-only sandbox, approval "never". `WriterAgents/CodexAgent.swift:25-60`, `WriterAgents/CodexAgent.swift:121-143`, `WriterAgents/LineProcess.swift:92-122` From the panel: a real Codex request (codex-cli 0.167.1) ran end to end on Linux on 2026-11-10: the reply streamed, became a version, previewed as a diff and was used. The launch details are `writer-agents`' and weren't re-checked here. Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Codex declines every command, file-change, permission and elicitation request. `WriterAgents/CodexAgent.swift:185-198` Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Codex errors: "Codex isn't installed. Install it with `npm install -g @openai/codex`, then sign in with `codex login`."; sign-in errors → "Codex isn't signed in. Run `codex login` in Terminal, then try again."; "Codex stopped unexpectedly: <last stderr line>"; "Couldn't start Codex: <reason>"; "Codex didn't start a conversation."; interrupted → "Stopped." `WriterAgents/CodexAgent.swift:34`, `WriterAgents/CodexAgent.swift:47`, `WriterAgents/CodexAgent.swift:50`, `WriterAgents/CodexAgent.swift:133`, `WriterAgents/CodexAgent.swift:172`, `WriterAgents/CodexAgent.swift:200-206` Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Claude runs the bundled Node bridge (`Resources/bridge/claude/bridge.mjs`, or the repo copy) with `node`, in an empty temporary folder, passing the `claude` binary's path in `FLUENT_WRITER_CLAUDE_PATH`. `WriterAgents/ClaudeAgent.swift:13-56` From the panel: a real Claude request through `FLUENT_WRITER_BRIDGE` ran end to end on Linux on 2026-11-10 (it also answered a comment). The launch details are `writer-agents`' and weren't re-checked here. Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Claude errors: "Claude needs Node.js. Install it from nodejs.org or with `brew install node`."; "The Claude bridge is missing from this build."; "Claude stopped unexpectedly: <detail>"; "Couldn't start Claude: <reason>"; "Claude reported an error." `WriterAgents/ClaudeAgent.swift:29`, `WriterAgents/ClaudeAgent.swift:33`, `WriterAgents/ClaudeAgent.swift:47`, `WriterAgents/ClaudeAgent.swift:50`, `WriterAgents/ClaudeAgent.swift:69` Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Bridge: one turn, no tools, no MCP servers, no setting sources, every tool request denied ("Writing assistance has no tools."), system prompt "You are a careful writing editor. You only read the text you are given and reply in the requested format.", streams text deltas. `bridge/claude/bridge.mjs:37-73` Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Bridge errors: "The Claude bridge dependencies are missing. Run `npm install` in bridge/claude."; sign-in → "Claude isn't signed in. Run `claude` in Terminal and sign in, then try again."; missing binary → "Claude Code isn't installed. Install it with `npm install -g @anthropic-ai/claude-code`."; aborted → "Stopped." `bridge/claude/bridge.mjs:18-35`, `bridge/claude/bridge.mjs:77`, `bridge/claude/bridge.mjs:86-88` Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Bridge cancel on SIGTERM, `{"type":"cancel"}` or stdin close; exits 50–100 ms after finishing. `bridge/claude/bridge.mjs:11-16`, `bridge/claude/bridge.mjs:91-106` Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Temporary agent folders are deleted after each request; child processes are terminated (SIGKILL after 2 s if still running). `WriterAgents/ClaudeAgent.swift:92-100`, `WriterAgents/CodexAgent.swift:217-225`, `WriterAgents/LineProcess.swift:59-74` Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.
- [x] Prompt format (feedback tag, edit/old/new tags, `at="end"`, `for="ID"`, reply tags, history, selection block) is built by `AgentPrompt.build`; Critique asks for no edits. `WriterAgents/AgentModels.swift:130-209` Audit: writer-agents tests (`codex_over_a_real_process`, `declines_approvals`, `auth_error_is_friendly`, `missing_executable_fails`, `claude_bridge_protocol_over_a_fake_node`, `bridge_events_and_cancel`, `stop_kills_a_hanging_agent`, `fixture_replays_word_by_word_and_stop_silences`, the prompt tests); every error string compared with the Swift; the bridge is the same `bridge/claude/bridge.mjs`.

---

## 15. Comments

- [x] Add Comment (⌥⌘M, toolbar, Comments tab button, context menu): target is the selection, or the current line when nothing is selected, trimmed of surrounding whitespace; beeps if empty. `CommentComposer.swift:118-126`, `CommentComposer.swift:138-140` Built in `comment_composer.rs` and `notes.rs` (`App::add_comment_command`); the toolbar, Comments-tab and context-menu entry points call it from their own modules. The beep is `Tx::beep` (the system alert sound; tests count `Runtime::beeps`).
- [x] Adding a comment while previewing a version closes the preview first. `CommentComposer.swift:139`
- [x] The target text is selected and scrolled into view; chrome shown with no auto-hide. `CommentComposer.swift:142-144`
- [x] Composer popover, 320×150, transient (closes on click outside), animated, anchored to the first character of the target. `CommentComposer.swift:27-30`, `CommentComposer.swift:93-96`, `CommentComposer.swift:129-136` Drawn in the app (card, arrow, shadow) since Rune has no popover. Not done: the open and close animation. The card uses `card` and `hairline` in place of AppKit's popover material.
- [x] Quote line: “<text>” with newlines as spaces, cut to 90 characters + "…", IBM Plex Mono Italic 11 pt, secondary-label colour, at (14, 124). `CommentComposer.swift:32-38` Secondary label colour is AppKit's black α0.5 / white α0.55.
- [x] Text area 296×84 at (12, 34), IBM Plex Mono 13 pt, label colour, caret `caret`, insets 2×4; accessibility label "Comment"; focused on open. `CommentComposer.swift:40-61`, `CommentComposer.swift:97`
- [x] Placeholder "What should change here?" for a new comment, hidden once text is typed. `CommentComposer.swift:63-72`
- [x] Hint "↩ save · esc cancel", IBM Plex Mono 10 pt, secondary colour, tooltip "Shift-Return adds a new line". `CommentComposer.swift:74-79`
- [x] Button "Comment" (new) or "Save" (editing), small rounded, IBM Plex Mono 11 pt, bottom-right 12 pt in. `CommentComposer.swift:81-88` The small rounded bezel is approximated: `card` fill, 1 pt `hairline`, radius 5, 22 pt tall, title width + 20.
- [x] Return (not composing IME text) saves; Shift-Return or Option-Return inserts a newline; Esc cancels. `CommentComposer.swift:5-18`
- [x] Saving trims whitespace; empty body beeps and keeps the popover open. `CommentComposer.swift:100-107`
- [x] After saving a new comment: caret moves to the end of the commented text, the comment becomes active, and the footer flashes "<n> open comment(s) · ⇧⌘M to review and send". `CommentComposer.swift:145-154` The shortcut in the flash is the platform's ("Ctrl+Shift+M" off the Mac).
- [x] Comments are author "you", ordered by position. `NotesController.swift:117-128`
- [x] Open, attached comments are highlighted with `commentHighlight` behind the text; the active one with `commentHighlightActive`. Display-only colour. `NotesController.swift:162-171` Drawn with `Span::line_bg`, which fills the line's whole height, square, as AppKit does, and shifts with IME preedit (`El::text_highlight` does not). Checked in light and dark snapshots. The pinned fork now paints the selection over it, as AppKit does (11c5070).
- [x] Comments follow their text as it is edited; a comment whose text is deleted becomes detached (no highlight). `NotesController.swift:65-75` The draft's `TextEdit` reports each change (`report_changes` / `take_changes`): typing, paste, cut, undo and redo (as the exact inverse of each edit they take back), IME commits and the app's own edits. Test: `undoing_edits_on_both_sides_of_a_comment_keeps_it_in_place`.
- [x] A detached open comment re-attaches when its quoted text reappears (e.g. after undo). `NotesController.swift:98-107`
- [x] Comments re-anchor by their quote when a draft is opened. `NotesController.swift:43-44`
- [x] Comments tab: "Add Comment" small button top-left (tooltip "Comment on the selection or the current line (⌥⌘M)"); "Show resolved" small checkbox top-right, IBM Plex Mono 11 pt, hidden when there are no resolved comments. `AssistController.swift:263-269`, `AssistController.swift:351-355`, `AssistController.swift:492` The tooltip and the empty text name the platform's shortcut (Ctrl+Alt+M off the Mac).
- [x] List shows open comments, plus resolved ones when "Show resolved" is ticked. `AssistController.swift:486`, `AssistController.swift:648-651`
- [x] Empty text (centred, IBM Plex Mono 12 pt, `quiet`): "Select some text, or put the caret on a line, and press ⌥⌘M to leave a comment.\n\nWrite as many as you like, then send them all at once. The agent's fixes come back as a version you can preview." `AssistController.swift:273-275`
- [x] Send button pinned 52 pt from the bottom, full width, 30 pt tall, IBM Plex Mono 12 pt: "Send <n> Comment(s) to <Provider>", or "Send Comments" when none; enabled only with attached open comments and no request running. `AssistController.swift:270-272`, `AssistController.swift:359-361`, `AssistController.swift:488-490`
- [x] Cards: rounded rect radius 8, 8 pt apart, fill `card`, `cardHover` on hover, `cardSelected` when active; 1 pt border `hairline`, or `caret` α0.5 when active. `AssistController.swift:21-57`, `AssistController.swift:92` Hover is an overlay that turns `card` into `cardHover`.
- [x] Resolved cards at α0.6. `AssistController.swift:24`, `AssistController.swift:533`
- [x] Card quote “<text>” (newlines as spaces), IBM Plex Mono Italic 11 pt `quiet`, max 2 lines; detached adds " — no longer in the draft"; quote width leaves 72 pt for buttons. `AssistController.swift:535-536`, `AssistController.swift:561-562`
- [x] Card body IBM Plex Mono 12 pt `text`; each reply "<author>: <text>" 11 pt `quiet`. 12 pt padding, 10 pt top, 4 pt after the quote, 6 pt after others. `AssistController.swift:537-543`, `AssistController.swift:554-574`
- [x] Card buttons 18×18, top-right, 24 pt apart, `quiet`: resolve `checkmark.circle` "Resolve" (or `arrow.uturn.backward.circle` "Reopen" when resolved), `pencil` "Edit comment", `trash` "Delete comment". `AssistController.swift:544-547`, `AssistController.swift:568-572` The SF Symbols are vector stand-ins drawn at 15 pt.
- [x] Clicking a card selects its text in the editor, scrolls to it (centres in focus modes), makes it active and closes any preview. `AssistController.swift:549`, `CommentComposer.swift:168-175` `App::reveal_comment`; the card is the Assist panel's.
- [x] Card accessibility: role button, press = click, label "Comment on “<first 60 chars of quote>”: <body>". `AssistController.swift:54-56`, `AssistController.swift:550` Set; checked in tests, not with a screen reader. Test `the_assist_panel_and_its_cards_name_themselves`.
- [x] Edit opens the composer pre-filled, button "Save", no placeholder; not available for detached comments. `CommentComposer.swift:157-165` `App::edit_comment`; the card's pencil button is the Assist panel's.
- [x] Resolve/Reopen toggles the resolved state. `AssistController.swift:623-627` `App::set_comment_resolved`; the button is the Assist panel's.
- [x] Delete removes the comment immediately (no confirmation). `AssistController.swift:634-638` `App::delete_comment`; the button is the Assist panel's.
- [x] With the Comments tab open, moving the caret into a commented passage makes that comment active (shortest span wins when nested). `AssistController.swift:440-443`, `NotesController.swift:156-160`
- [x] Using a version resolves the comments that version addresses. `NotesController.swift:199-206`

---

## 16. Versions and the inline diff preview

### Versions tab

- [x] Header text (IBM Plex Mono 11 pt, `quiet`): "Each agent reply becomes a version. Click one to preview it as a diff, then use it or go back. Using one keeps your current draft here too." `AssistController.swift:280-281`
- [x] First card "Your draft" (Medium 12 pt) with "What you're editing now", or "Back to editing (→ or esc)" while previewing; selected when not previewing; click closes the preview. `AssistController.swift:576-587`
- [x] Then one card per version, newest first: label (Medium 12 pt, 2 lines max, 20 pt narrower for the delete button); detail "<Saved draft | source> · <relative time>[ · used]"; feedback up to 3 lines, 11 pt `quiet`. `AssistController.swift:588-595`, `AssistController.swift:605-621`
- [x] Relative time: "just now" under 30 s, otherwise the short relative style. `VersionPreview.swift:330-335` `notes::relative_time`, in US English ("2 min. ago", "3 days ago").
- [x] Version card delete button `xmark` 14×14 "Delete version"; deleting the previewed version closes the preview. `AssistController.swift:596-598`, `AssistController.swift:640-646`
- [x] Clicking a version card previews it; the previewed card is selected. `AssistController.swift:579`, `AssistController.swift:600`
- [x] Version card accessibility label "<label>, <detail parts joined by ', '>". `AssistController.swift:601` Set; not checked with a screen reader. Test `the_assist_panel_and_its_cards_name_themselves`.
- [x] At most 40 versions per draft; past that the oldest unused version is dropped. `WriterCore/DocumentNotes.swift:168`, `WriterCore/DocumentNotes.swift:177-182` writer-core test `version_cap_drops_oldest_unused`.

### Preview

- [x] The preview replaces the editor (editor hidden) with a 44 pt bar on top and a read-only diff below. `VersionPreview.swift:169-178`, `VersionPreview.swift:127-131` The preview is drawn over the draft, which stays mounted so its scroll position survives.
- [x] Bar background `banner`, 1 pt `hairline` along its bottom; accessibility role toolbar, label "Version preview". `VersionPreview.swift:22-30`, `VersionPreview.swift:121-122`
- [x] Bar left: `chevron.left` (tooltip "Older version (←)") at (12, 12) 20×20; position "<n>/<total>" IBM Plex Mono 11 pt `quiet` centred at x 34, min 34 pt wide; `chevron.right` ("Newer version (→)") after it. Chevrons tinted `text`. `VersionPreview.swift:95-103`, `VersionPreview.swift:142-145` Chevrons are SVG stand-ins for the SF Symbols.
- [x] Position accessibility label "Version <n> of <total>"; position counts oldest = 1. `VersionPreview.swift:290-297`
- [x] Older button disabled at position 1; Newer is always enabled. `VersionPreview.swift:300`
- [x] Title (version label, Medium 12 pt `text`) at y 5 and detail (11 pt `quiet`) at y 22, starting 12 pt after the Newer button. `VersionPreview.swift:89-94`, `VersionPreview.swift:146-149`
- [x] Detail: "Saved draft" or "From <source>" · relative time · "same as your draft" or "+<added> −<removed> words" · "used before" if used. `VersionPreview.swift:293-299`
- [x] Right: "Use This Version" (Medium 11 pt, small rounded, tooltip "Replace your draft with this version (↩). Your current draft is kept in Versions."), 16 pt from the right; "Back to Draft" (11 pt, tooltip "Keep your draft as it is (esc)") 8 pt left of it. Both 22 pt tall at y 11. `VersionPreview.swift:104-116`, `VersionPreview.swift:138-141` Button bezel approximated as in the composer.
- [x] Use button disabled when the version is the same as the draft. `VersionPreview.swift:301`
- [x] Under 620 pt wide, "Back to Draft" is hidden; under 460 pt "Use This Version" becomes "Use". `VersionPreview.swift:132-137` **Not ticked:** Built; not checked at those widths. Test `the_preview_bar_gives_way_in_narrow_windows`.
- [x] Title and detail hidden when their width is under 48 pt; when under 140 pt the bar's tooltip shows title and detail. `VersionPreview.swift:150-152` **Not ticked:** Built; not checked at those widths. Test `the_preview_bar_gives_way_in_narrow_windows`.
- [x] Diff text uses the same column and horizontal inset as the editor, 40 pt top/bottom inset, 35% extra bottom space, no typewriter centring. `VersionPreview.swift:154-161`
- [x] Diff is word-level between the current draft and the version, with Markdown styling applied to the combined text. `VersionPreview.swift:242-270`
- [x] Inserted text: `inserted` colour on `insertedBackground`. `VersionPreview.swift:276-277` The tints of both fill the line's whole height, square (`Span::line_bg`); checked in light and dark snapshots.
- [x] Deleted text: `deleted` colour on `deletedBackground`, struck through in `deleted`. `VersionPreview.swift:278-282`
- [x] Line breaks inside changes are not painted. `VersionPreview.swift:306-320`
- [x] A space is added between a deletion and the insertion that follows when neither side has whitespace at the join. `VersionPreview.swift:251-254`
- [x] Word counts for +/− use the plain-text word count of each changed segment. `VersionPreview.swift:262-263`
- [x] Opening a preview scrolls to the first change (or top when none) and focuses the diff. `VersionPreview.swift:180-181`, `VersionPreview.swift:322-328` **Not ticked:** Built (`preview_poll`) and focus checked in tests; the scroll to a change below the fold was not checked on a long draft. Test `a_preview_opens_at_its_first_change_on_a_long_draft`.
- [x] Diff text is selectable and copyable but not editable. `VersionPreview.swift:69-71` A read-only `text_input`: typing, paste and cut beep and change nothing.
- [x] Keys in the preview: ← older, → newer, Esc back to the draft, Return/keypad Enter use; other keys behave normally. `VersionPreview.swift:6-20`
- [x] Timeline: versions oldest → newest, then your draft. ← from the draft opens the newest version; ← at the oldest beeps; → at the newest returns to the draft; → while not previewing beeps. `VersionPreview.swift:166-226`
- [x] Older/Newer/Use are also ⌥⌘←/⌥⌘→/⌥⌘↩ from anywhere. `MainMenu.swift:145-147`
- [x] Opening a preview announces "Previewing <label>. Left and right arrows switch versions, Return uses it, Escape goes back." `VersionPreview.swift:185-188` **Not ticked:** Built with `Tx::announce` (test `opening_a_preview_is_announced`); not checked with a screen reader. Test `opening_a_preview_is_announced`; not heard through a screen reader.
- [ ] Diff accessibility: label "Version preview", value = the version's full text, help "Inserted text is green, removed text is struck through." `VersionPreview.swift:78`, `VersionPreview.swift:302-303` **Not ticked:** Set (`label`, `a11y_value`, `description`); not checked with a screen reader. Audit: label and help set (test `the_composer_and_the_preview_name_themselves`); the value reads the diff, not the version's text, because Rune gives a text field's value as its own text (GAPS).
- [x] Preview re-renders when the draft or version list changes; closes if its version is deleted. `VersionPreview.swift:205-210`, `EditorController.swift:121-125`
- [x] Closing the preview shows the editor again and focuses it. `VersionPreview.swift:191-202`
- [x] Opening another document closes the preview. `EditorController.swift:323`
- [x] Requests sent while previewing build on that version. `AssistController.swift:707-708` Exposed as `App::previewed_version` and `App::assist_source_text`; the sending is the Assist panel's.

### Using a version

- [x] Using a version that differs from the draft first keeps the current draft as a version "Your draft · <short time>" (source "you"), unless the draft is blank or an identical version exists. `VersionPreview.swift:340-345`, `NotesController.swift:189-197` The short time is US English with macOS 14's narrow no-break space before AM/PM.
- [x] The draft is rewritten with word-level edits as one undo step "Use Version", so untouched text and comments on it stay put; falls back to replacing the whole text if the edits don't land exactly. `VersionPreview.swift:353-406`
- [x] Comments are settled after the rewrite (tracked ones keep their place, lost ones are searched by quote). `NotesController.swift:208-224`
- [x] The version is marked used, comments it addresses resolve, the preview closes, and the footer flashes "Version used · ⌘Z or Versions to go back". `VersionPreview.swift:348-350`
- [x] Caret goes to the first changed place. `VersionPreview.swift:402-404`

---

## 17. Outside agents (MCP) and the connect sheet

### Connect sheet

- [x] Assist › Connect an Outside Agent… with no `fluent-writer-mcp` executable beside the app binary: sheet "The agent connector is missing from this build." / "Build the app with script/build-app.sh, which bundles fluent-writer-mcp." `ConnectAgent.swift:8-12`, `ConnectAgent.swift:33-39` Shown with the shared `ui::Alert`. The server is `fluent-writer-mcp` (`.exe` on Windows) beside the running executable on every platform, which on macOS is inside the bundle as before.
- [x] Otherwise a sheet titled "Connect an outside agent" with text "Claude Code or Codex can then see the draft open here, your selection, comments, versions and Assist history. They can leave comments and propose versions, which you preview and choose here, the same as Assist's. Run one of these in Terminal, or click a button to do it now." `ConnectAgent.swift:43-45` Drawn in `ui::sheet` with `ui::button`s (four buttons and the commands are more than `ui::Alert` holds).
- [x] Accessory 460×150: "Claude Code" heading (system 11 pt medium) and its command; "Codex" heading and its command; commands in IBM Plex Mono 10 pt, secondary colour, wrapping, selectable. `ConnectAgent.swift:46-62` Headings are IBM Plex Mono Medium 11 pt (the app embeds no system font). Each command box is at least 46 pt and grows for long support paths instead of clipping. No app icon: the sheet is 580 pt wide so the four buttons fit in one row.
- [x] Claude command: `claude mcp add --scope user <server name> -- '<server path>' '--support' '<support folder>'` (each argument single-quoted). `ConnectAgent.swift:16-22` The support folder is made absolute first, so a relative `--support` still works.
- [x] Codex command: `codex mcp add <server name> -- '<server path>' '--support' '<support folder>'`. `ConnectAgent.swift:24-26`
- [x] Server name `fluent-writer`, or `fluent-writer-dev` in the dev build. `ConnectAgent.swift:14`
- [x] Buttons "Add to Claude Code", "Add to Codex", "Copy Commands", "Done". `ConnectAgent.swift:63-66` Escape does nothing, as the alert had no Cancel button.
- [ ] Add to …: runs the CLI with those arguments; success flashes "Added to <Claude Code|Codex>. Start a new <name> session to use it." `ConnectAgent.swift:69-72`, `ConnectAgent.swift:83-101` Built (on a thread, `App::connect_poll`), not run against the real CLIs: that would register the server in this machine's own Claude Code and Codex settings.
- [x] CLI not found: flash "<name> isn't installed". CLI can't start: flash "Couldn't run <tool>: <reason>". `ConnectAgent.swift:84-87`, `ConnectAgent.swift:110` The not-found flash is tested; the can't-start flash is built but not exercised.
- [ ] CLI non-zero exit: sheet "<name> didn't add the connector." with the last 600 characters of its output, or "It exited with status <n>." `ConnectAgent.swift:102-107` Built with `ui::Alert`; not exercised.
- [x] Copy Commands copies "# Claude Code\n<claude cmd>\n\n# Codex\n<codex cmd>\n" and flashes "Copied the connect commands". `ConnectAgent.swift:73-77`

### What the app publishes and accepts

- [x] Live state file `<support>/Live.json` (pid, app name, path, remote location, title, selection, selected text, previewed version id), written 0.4 s after selection changes, comment adds, preview changes and document loads. `NotesController.swift:237-252`
- [x] Live state is deleted on quit; readers treat it as absent when the pid isn't running. `NotesController.swift:254-257`, `WriterCore/DocumentNotes.swift:308-312` Deleted when the app's state is dropped (window closed); a killed process leaves it, and readers ignore it because the pid is gone.
- [x] The app watches `<support>/Inbox/` and applies each command file (oldest first), deleting it and writing `Inbox/Results/<id>.json`. `NotesController.swift:261-308`, `WriterCore/DocumentNotes.swift:316-336` Watched with the `notify` crate; reading, deleting and answering run on the notes worker thread.
- [x] Commands for the open draft apply to the live text and notes; a new version flashes "<agent> proposed a version · ⌥⌘V to see it"; a new comment flashes "<agent> added a comment". `NotesController.swift:279-293`
- [x] Commands for other files apply to that file's notes under the notes lock; if the file can't be read the result is "Couldn't read <path>." `NotesController.swift:294-305`
- [x] The inbox is also processed whenever a document loads. `NotesController.swift:49`

### fluent-writer-mcp server

- [x] Usage `fluent-writer-mcp [--support <dir>] [--variant standard|dev]`; `--help`/`-h` prints usage and exits 0; default support folder is the standard app's. `FluentWriterMCP/main.swift:13-24` Test `help_prints_usage_and_exits_0`; `--support` and `--variant` in `main.rs`.
- [x] Speaks JSON-RPC 2.0, one message per line on stdio; accepts batches; ignores blank or unparsable lines. `FluentWriterMCP/main.swift:403-406` Test `the_protocol_answers_as_the_swift_server_did`.
- [x] `initialize`: echoes the client's protocol version (default "2025-06-18"), tools capability, server info name "fluent-writer", title "Fluent Writer", version "0.2.0", and the instructions text. `FluentWriterMCP/main.swift:371-382`, `FluentWriterMCP/main.swift:357-363` Test `the_protocol_answers_as_the_swift_server_did`.
- [x] Agent name for comments/versions: "Claude Code" if the client name/title contains "claude", "Codex CLI" if "codex", else the client's title or name, else "Outside agent". `FluentWriterMCP/main.swift:27`, `FluentWriterMCP/main.swift:372-376` Test `comments_are_signed_with_the_clients_name`.
- [x] `ping` → empty result; `notifications/*` ignored; unknown methods → error -32601 "Method not found: <method>". `FluentWriterMCP/main.swift:383-400` Test `the_protocol_answers_as_the_swift_server_did`.
- [x] Tool errors return `isError: true` with the message text. `FluentWriterMCP/main.swift:387-396` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] Tools: get_context, read_document, list_comments, add_comment, reply_to_comment, resolve_comment, propose_version, list_versions, read_version, get_assist_history, list_documents, with the descriptions and input schemas in the source. `FluentWriterMCP/main.swift:139-228` Test `the_protocol_answers_as_the_swift_server_did`.
- [x] `path` argument must end md/markdown/txt/text ("`path` must be a draft (.md, .markdown, .txt or .text)."); without it, the open draft is used; errors when the open draft isn't saved yet or the app isn't running (exact strings in source). `FluentWriterMCP/main.swift:56-69` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] get_context: app running/not, document path/title/words/lines/remote, selection text+line or caret line, open comments, up to 8 versions, previewed version, last 5 Assist exchanges, how_to_edit; recent 10 documents when no saved draft is open. `FluentWriterMCP/main.swift:238-269` Tests `a_running_app_gets_changes_through_its_inbox`, `speaks_mcp_and_writes_notes`.
- [x] read_document returns the file text; `line_numbers: true` prefixes "%5d  ". `FluentWriterMCP/main.swift:271-276` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] list_comments: open (or all with include_resolved); "No open comments on this draft." / "No comments on this draft." `FluentWriterMCP/main.swift:278-284` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] Comment JSON: id, quote, comment, author, created, status, line (or note "The quoted text is no longer in the draft."), replies. `FluentWriterMCP/main.swift:76-92` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] add_comment / reply_to_comment / resolve_comment / propose_version go through the app's inbox when it's running (waits up to 4 s, else "Queued. Fluent Writer hasn't picked it up yet; it will apply it shortly."), or apply directly with " (Fluent Writer is closed; the author will see this next time they open the draft.)" appended. `FluentWriterMCP/main.swift:116-135` Tests `a_running_app_gets_changes_through_its_inbox`, `speaks_mcp_and_writes_notes`.
- [x] propose_version requires `edits` or `full_text` ("Pass `edits` or `full_text`."). `FluentWriterMCP/main.swift:318` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] Command result messages: "Added comment <id>.", "Resolved comment <id>.", "Replied to comment <id>.", "Created version <id>. The author can preview it in Fluent Writer and choose whether to use it." (+ failures), and errors "That text isn't in the draft.", "The text appears N time(s); occurrence must be 1–N.", "add_comment needs the quoted text and a comment.", "No comment with that id.", "The reply is empty.", "The version is identical to the draft." `WriterCore/DocumentNotes.swift:199-201`, `WriterCore/DocumentNotes.swift:356-405` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] Default version label "<agent> version". `WriterCore/DocumentNotes.swift:396` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] list_versions ("No versions yet."), read_version ("No version with that id. Use list_versions."), get_assist_history (limit 1–500, default 20; "No Assist requests for this draft yet."), list_documents (limit 1–500, default 20; "No recent drafts."). `FluentWriterMCP/main.swift:322-348` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.
- [x] Numeric arguments are rounded and clamped; NaN/∞ ignored. `FluentWriterMCP/main.swift:48-52` Tests `tool_messages_read_as_the_swift_wrote_them`, `speaks_mcp_and_writes_notes`.

---

## 18. Preferences and what persists where

- [x] User defaults `focus` (off|sentence|paragraph|typewriter), default off. `Preferences.swift:7-10` Kept in `preferences.json` under the same keys and defaults; a Mac's existing `UserDefaults` are imported once (test `every_setting_the_swift_app_saved_carries_over`; not run on a Mac).
- [x] `lastDimmingFocus`, default sentence. `Preferences.swift:13-16` Kept in `preferences.json` under the same keys and defaults; a Mac's existing `UserDefaults` are imported once (test `every_setting_the_swift_app_saved_carries_over`; not run on a Mac).
- [x] `textSize` (double), default 19. `Preferences.swift:18-21` Kept in `preferences.json` under the same keys and defaults; a Mac's existing `UserDefaults` are imported once (test `every_setting_the_swift_app_saved_carries_over`; not run on a Mac).
- [x] `appearance` (system|light|dark), default system; applied at launch. `Preferences.swift:23-26`, `main.swift:9` Kept in `preferences.json` under the same keys and defaults; a Mac's existing `UserDefaults` are imported once (test `every_setting_the_swift_app_saved_carries_over`; not run on a Mac).
- [x] `showCounts`, default true. `Preferences.swift:28-31` Kept in `preferences.json` under the same keys and defaults; a Mac's existing `UserDefaults` are imported once (test `every_setting_the_swift_app_saved_carries_over`; not run on a Mac).
- [x] `spellCheck`, default true. `Preferences.swift:33-36` Kept in `preferences.json` under the same keys and defaults; a Mac's existing `UserDefaults` are imported once (test `every_setting_the_swift_app_saved_carries_over`; not run on a Mac).
- [x] `showSidebar`, default false. `Preferences.swift:38-41`
- [x] `sidebarMode` (recent|folder), default recent. `Preferences.swift:43-46`
- [x] `folderLocal` (path) or `folderRemote` (location string), mutually exclusive. `Preferences.swift:48-63`
- [x] `lastSSHLocation`, default empty. `Preferences.swift:65-68`
- [x] `assist.provider` (codex|claude), default codex. `AssistController.swift:138`, `AssistController.swift:670` Kept in `preferences.json` under the same keys and defaults; a Mac's existing `UserDefaults` are imported once (test `every_setting_the_swift_app_saved_carries_over`; not run on a Mac).
- [x] Window frame under `NSWindow Frame FluentWriterMain`. **(AppKit built-in)** `EditorController.swift:146` — *Rust: `windowFrame` and `windowDecoration` in preferences.json.*
- [x] Grammar, autocorrect, smart quotes, smart dashes and text replacement toggles are not saved by the app and reset at launch. `EditorController.swift:164-167` Audit: smart quotes, smart dashes and text replacement were saved between launches; now they reset each launch (fixed this pass, test `substitution_toggles_last_until_the_app_quits`). Grammar and autocorrect don't exist.
- [x] `<support>/state.json`: per-file record (path, title, manual-title flag, selection, last opened, remote location, upload-pending flag) and last opened path. `WriterCore/DraftStore.swift:3-18`, `WriterCore/DraftStore.swift:292-317` writer-core test `reads_state_written_by_swift`.
- [x] `<support>/Recovery/<uuid>.json` recovery snapshots. `WriterCore/DraftStore.swift:218-222` Test `recovery_snapshots_hold_the_draft_and_go_once_it_is_saved`.
- [x] `<support>/Remote/…` remote mirrors. `WriterCore/DraftStore.swift:85`, `WriterCore/DraftStore.swift:350-359`
- [x] `<support>/Notes/<first 12 bytes of SHA-256 of the path, hex>.json` per draft: comments, versions, Assist history (max 200 exchanges); file deleted when empty; `Notes/.lock` for cross-process locking. `WriterCore/DocumentNotes.swift:168-169`, `WriterCore/DocumentNotes.swift:251-284` Tests `notes_keep_200_exchanges_write_iso_dates_with_milliseconds_and_go_when_empty`, `notes_key_matches_swift`, `locked_writers_dont_lose_updates`.
- [x] Notes save 0.6 s after a change, and immediately on document switch, send and quit. `NotesController.swift:82-96` Tests `notes_are_saved_after_a_pause_and_follow_the_first_save`, `quitting_writes_the_notes_and_removes_the_live_state`.
- [x] Notes for a never-saved draft are kept in memory until it gets a file, then filed under that path. `NotesController.swift:41-42`, `NotesController.swift:52-63` Tests `notes_are_saved_after_a_pause_and_follow_the_first_save`, `quitting_writes_the_notes_and_removes_the_live_state`.
- [x] Notes dates are ISO 8601 with milliseconds. `WriterCore/DocumentNotes.swift:224-249` Tests `notes_keep_200_exchanges_write_iso_dates_with_milliseconds_and_go_when_empty`, `notes_key_matches_swift`, `locked_writers_dont_lose_updates`.
- [x] `<support>/Live.json` and `<support>/Inbox/`, `Inbox/Results/`. `WriterCore/DocumentNotes.swift:215-218` writer-core tests `live_state_round_trip`, `inbox_round_trip`.
- [x] Drafts themselves are plain Markdown/text files; nothing else is written into them. `README.md:49`, `README.md:101` Audit: saves write the text and nothing else (`typing_edits_the_draft_and_autosaves_it` reads the file back).

---

## 19. Accessibility labels and announcements

- [x] Editor text: label "Draft", help "The draft text. Use the Format menu or Markdown to style text." `EditorController.swift:180-181` Test `the_editor_and_footer_name_themselves`.
- [x] Title field label "Draft title". `EditorController.swift:215` Test `the_editor_and_footer_name_themselves`.
- [x] Status label "Save status", value = its text. `EditorController.swift:229`, `EditorController.swift:487` Test `the_editor_and_footer_name_themselves`.
- [x] Counts label "Counts", value = its text. `EditorController.swift:230`, `EditorController.swift:516` Test `the_editor_and_footer_name_themselves`.
- [x] Sidebar button "Toggle sidebar"; toolbar "Add comment", "Comments", "Versions", "Assist", each with count value when non-zero. `EditorController.swift:77`, `ChromeControls.swift:71-74`, `ChromeControls.swift:63` Tests `the_editor_and_footer_name_themselves`, `toolbar_buttons_have_their_labels_tooltips_and_actions`.
- [x] Sidebar: "Sidebar view" (segmented), "Files" (list), "Refresh folder" (refresh button), rows "<title>, <subtitle>". `Sidebar.swift:107`, `Sidebar.swift:115`, `Sidebar.swift:147`, `Sidebar.swift:378`
- [x] Open picker: "Find a draft", "Recent drafts", rows "<title>, <when>". `OpenPicker.swift:41`, `OpenPicker.swift:55`, `OpenPicker.swift:159`
- [x] SSH field "SSH location". `RemoteEditing.swift:104`
- [x] Assist: group "Assist"; "Provider"; "Close assist"; "Assist section"; quick actions "<label> with the selected provider"; "Instruction"; "Feedback"; Send/Stop "Send instruction"/"Stop request"; scope label sentences (section 14). `AssistController.swift:182-235`, `AssistController.swift:251`, `AssistController.swift:849` All set; checked through tests and `--tree`, not with a screen reader. Test `the_assist_panel_and_its_cards_name_themselves`; not checked with a screen reader.
- [x] Cards: role button, activatable with VoiceOver press; comment and version card labels (sections 15, 16); icon buttons labelled with their tooltip text. `AssistController.swift:54-56`, `AssistController.swift:526` Test `the_assist_panel_and_its_cards_name_themselves`; not checked with a screen reader.
- [x] Comment composer text area "Comment". `CommentComposer.swift:57` Test `the_composer_and_the_preview_name_themselves`.
- [x] Image previews: role image, label alt text or "Image". `ImageBlocks.swift:101-102`, `ImageBlocks.swift:212` Test `image_lines_say_why_they_show_no_image`.
- [ ] Preview: bar role toolbar "Version preview"; diff "Version preview" with value and help; position "Version <n> of <total>"; chevrons described by their tooltips. `VersionPreview.swift:78`, `VersionPreview.swift:96`, `VersionPreview.swift:121-122`, `VersionPreview.swift:297`, `VersionPreview.swift:302-303` Audit: all set and tested (`the_composer_and_the_preview_name_themselves`) except the diff's value (see section 16).
- [x] Every footer flash is announced (medium priority). `EditorController.swift:495`
- [x] Opening a version preview is announced. `VersionPreview.swift:185-188` — *Rust: `Tx::announce`; not checked with a screen reader.* Test `opening_a_preview_is_announced`; not heard through a screen reader.
- [ ] All animations (chrome fade, scroll, peek, window widen) respect Reduce Motion. `EditorController.swift:604`, `EditorController.swift:634`, `EditorController.swift:683`, `ChromeControls.swift:156`, `ChromeControls.swift:220`, `ChromeControls.swift:240` Audit: chrome fade and peek tested (`reduce_motion_makes_the_chrome_and_the_peek_instant`; the peek still slid under Reduce Motion, fixed this pass); scroll glides are Rune's and stop under Reduce Motion; the window widening is Rune's and wasn't checked under it.
- [ ] VoiceOver text navigation, rotor and keyboard access in standard controls. **(AppKit built-in)** Audit: not run with a screen reader.

---

## 20. Dev variant and command-line flags

- [x] Variant comes from `FWVariant` in Info.plist ("standard" or "dev"); a binary with no bundle id (e.g. `swift run`) counts as dev. `WriterCore/AppVariant.swift:9-13` Test `the_variant_comes_from_the_bundle_or_the_build_folder`: `FWVariant` in a Mac bundle's Info.plist; a build in Cargo's target folder (wherever `target-dir` puts it) is dev; on a Mac an unbundled binary is dev; `FLUENT_WRITER_VARIANT` overrides. Fixed this pass: a target folder not named `target` counted as standard, and Info.plist wasn't read.
- [x] Dev app name "Fluent Writer Dev" (menus, About, live state), drafts `~/Documents/Fluent Writer Dev`, support `~/Library/Application Support/Fluent Writer Dev`. `WriterCore/AppVariant.swift:15-26` Tests `the_variant_comes_from_the_bundle_or_the_build_folder`, `names_and_folders`. The app now gives Rune its variant's name for the app menu (it always said "Fluent Writer").
- [x] Dev uses its own bundle id `com.sunkenintime.fluentwriter.dev`, so its user defaults are separate. `script/build-app.sh:17-21` Audit: `bundle_id` names the old domain, which the one-time import reads; the Rust app's settings live in each variant's own support folder.
- [x] Dev MCP server name `fluent-writer-dev`. `ConnectAgent.swift:14` Test `commands_quote_every_argument`.
- [x] App flag `--open <path>`: opens that file at launch. `main.swift:21-22`
- [x] Env `FLUENT_WRITER_AGENT_FIXTURE=<file>`: Assist replays that file's text word by word (40 ms per word) instead of calling a provider. `WriterAgents/ClaudeAgent.swift:103-136` writer-agents test `fixture_replays_word_by_word_and_stop_silences`.
- [x] Env `FLUENT_WRITER_CLAUDE_PATH`: path to the Claude Code binary for the bridge (set by the app). `bridge/claude/bridge.mjs:54`, `WriterAgents/ClaudeAgent.swift:41` writer-agents test `claude_bridge_protocol_over_a_fake_node`.
- [x] Env `FLUENT_WRITER_SSH_TEST_DIR=host:/path` runs the SSH round-trip test (tests only). `README.md:63` writer-core test `round_trip_over_ssh` (skips unless set).
- [x] MCP flags `--support <dir>` (tilde expanded), `--variant standard|dev`, `--help`/`-h`. `FluentWriterMCP/main.swift:13-24` Test `help_prints_usage_and_exits_0`; tilde expansion in `main.rs`.

---

## 21. Packaging

- [ ] `script/build-app.sh [debug|release] [--dev]`, release by default; any other argument prints "unknown argument: <arg>" and exits 2. `script/build-app.sh:3-16` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [ ] Builds the `FluentWriter` and `fluent-writer-mcp` products. `script/build-app.sh:22-24` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [ ] Output `build/Fluent Writer.app` or `build/Fluent Writer Dev.app`, replaced on each build; path printed at the end. `script/build-app.sh:25-26`, `script/build-app.sh:64` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [ ] Bundle layout: `Contents/MacOS/FluentWriter`, `Contents/MacOS/fluent-writer-mcp`, `Contents/Resources/FluentWriter_FluentWriter.bundle` (fonts). `script/build-app.sh:27-30` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [ ] `bridge/claude` (with its node_modules, minus `node_modules/.cache`) copied to `Contents/Resources/bridge/claude`. `script/build-app.sh:31-34` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [ ] Info.plist: name/display name, bundle id `com.sunkenintime.fluentwriter` (`.dev`), executable FluentWriter, APPL, version 0.2.0, build 2, `FWVariant`, minimum macOS 14.0, high-resolution capable, principal class NSApplication. `script/build-app.sh:35-50` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [ ] Document type "Markdown Draft", role Editor, rank Alternate, for `net.daringfireball.markdown` and `public.plain-text` (so the app appears in Open With). `script/build-app.sh:51-59` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [ ] Ad-hoc code signing; signing failure ignored. `script/build-app.sh:63` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [x] Bundled fonts: IBM Plex Mono Regular, Italic, Bold, BoldItalic, Medium, SemiBold, with the OFL licence. `Resources/Fonts/` Audit: the six faces and `IBM-Plex-OFL.txt` are in `crates/fluent-writer/assets/fonts`, compiled into the binary.
- [ ] The app is a regular Dock app (activation policy regular). `main.swift:54` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).
- [ ] Requires macOS 14+. `Package.swift:7` Audit: no packaging for the Rust app yet; `script/build-app.sh` still builds the Swift app (GAPS).

---

## Oddities

Things in the code that look like bugs, dead code or mismatches. Listed, not fixed. Decide for each whether the rewrite copies it.

1. ⌥⌘J is assigned twice: Edit › Find › Jump to Selection (`MainMenu.swift:70`) and Assist › Show Assist (`MainMenu.swift:139`). Which one fires depends on menu order. **Audit: fixed as promised.** Show Assist keeps ⌥⌘J; Jump to Selection has no shortcut (test `menus_match_the_swift_menu_bar`).
2. The Versions toolbar badge is always set to 0, so it never shows a count, while the tooltip and the tab label do show counts (`ChromeControls.swift:138`). **Audit: kept.** `chrome_button_count` gives Versions 0; the tab label shows the count.
3. Empty `if` block with only a comment in title editing (`EditorController.swift:806-808`). **Audit: nothing to observe.**
4. A footer flash sets the status text but returns before `layoutFooter()` (`EditorController.swift:446-450`), so the status label keeps the width of the previous text (`EditorController.swift:265-270`). A long flash can be cut off, and a flash after an empty status (new draft) can be invisible, until something else re-lays out the footer. The transient path also skips updating the label's accessibility value (`EditorController.swift:487`). *Rust: fixed; the status width comes from the text shown, and flashes are announced.* **Audit: fixed as promised** (test `a_flash_gets_its_own_width_and_returns_to_the_status`); the status label's accessibility value follows flashes too.
5. The status tooltip is not cleared for "Edited", "Recovered unsaved changes" or flash messages, so it keeps the last path or error (`EditorController.swift:475-480`). **Audit: kept** (`refresh_status_tooltip` leaves the tooltip for Edited, Recovered and flashes).
6. Format commands (Bold, Italic, headings, lists, Indent…) always edit the main text view, even while a version preview is showing and the draft is hidden, and even when focus is in the title field or the Assist instruction field (`Commands.swift:86-116`). Image insertion beeps during preview (`ImageBlocks.swift:311`); formatting doesn't. **Audit: kept.** Format commands edit the draft from the title field and from the preview (test `format_commands_edit_the_draft_from_the_title_and_the_preview`); images beep in the preview (test `the_preview_beeps_at_typing_and_at_images`).
7. "Use This Version" is disabled when the version equals the draft (`VersionPreview.swift:301`), but Return in the preview (`VersionPreview.swift:16`) and ⌥⌘↩ (`Commands.swift:190`) still use it. **Audit: kept.** The button is disabled; Return in the diff and ⌥⌘↩ still use it.
8. Closing the window runs the leave check in `windowShouldClose` (`EditorController.swift:841-843`), then `windowWillClose` quits, and `applicationShouldTerminate` runs the leave check again (`main.swift:38-42`). With a failing save this can show the "couldn't be saved" alert twice. *Rust: fixed; a quit that passed the check doesn't ask again.* **Audit: fixed as promised** (test `closing_with_a_failed_save_asks_once`).
9. `application(_:open:)` uses only the first URL and does nothing if the editor hasn't been created yet (`main.swift:34-36`). If macOS delivers the open event before `applicationDidFinishLaunching` (launching the app by opening a file), the file would be ignored and a new or recovered draft shown instead. Not verified at runtime. **Audit: not applicable yet.** Rune passes no file-open events on macOS (section 5).
10. Leaving a remote file with un-uploaded edits runs `ssh` synchronously on the main thread (`RemoteEditing.swift:194-198`); the UI freezes for up to the 10 s connect timeout plus transfer time. Rust: fixed; the upload runs off the UI thread behind a waiting sheet. **Audit: fixed as promised:** the upload runs off the UI thread behind the "Saving to <host>…" sheet.
11. If saving the current draft fails after a remote file has been fetched, the remote open is dropped with no message (`RemoteEditing.swift:163`). Rust: kept as is. **Audit: kept** (`finish_opening_remote`).
12. A remote file's title is set to its file name on open (`WriterCore/DraftStore.swift:376`), but the first edit replaces it with the derived first-line title because the manual-title flag is false (`EditorController.swift:701-705`). Rust: kept as is. **Audit: kept** (`text_did_change` derives the title when it isn't manual).
13. The Open picker lists remote mirror files and opens them with the local `open(url)` (`Commands.swift:19`), which marks them remote from the saved record but never fetches the server copy. The sidebar's Recent list uses `openRemote` instead (`Sidebar.swift:327`). Rust: fixed; the picker opens remote rows over SSH like the sidebar. **Audit: fixed as promised:** a remote row in the picker opens over SSH (`Picker` → `open_remote`).
14. `commentsHeader` in the Comments tab is always set to "" (`AssistController.swift:493`), so it never shows. **Audit: kept** (no header is drawn).
15. Dead code: `Cached.pointSize` is stored but never read (`ImageBlocks.swift:119`); `_ = editor` (`AssistController.swift:502`, `VersionPreview.swift:225`); `totalShift` is computed then discarded (`WriterCore/TextEditing.swift:76-107`); `_ = tc` (`EditorTextView.swift:113`); `MarkdownImages.blockInsertion` is never called, and `insertImages` repeats its logic (`WriterCore/MarkdownImages.swift:83-89`, `ImageBlocks.swift:349-351`); `DraftLibrary.lastOpenedURL` and `hasRecovery` are never used (`WriterCore/DraftStore.swift:121-124`, `WriterCore/DraftStore.swift:228-230`). **Audit: nothing to observe.**
16. `IBMPlexMono-SemiBold.otf` is bundled and registered but never used; `uiFont` maps every non-medium weight to Regular (`Theme.swift:111`). **Audit: kept** (SemiBold is embedded and unused).
17. The composer hint tooltip says only "Shift-Return adds a new line" (`CommentComposer.swift:75`), but Option-Return also does (`CommentComposer.swift:11`). **Audit: kept** (the hint's tooltip is "Shift-Return adds a new line").
18. Right-clicking a commented passage makes that comment active as soon as the menu opens, even if the menu is dismissed without choosing anything (`EditorController.swift:777-779`). **Audit: kept** (`note_context_offset` makes the comment active as the menu opens).
19. The scope label's accessibility label is not updated in the "Write something first" case, so VoiceOver reads the previous sentence (`AssistController.swift:456`). **Audit: kept** (`assist_refresh_scope` leaves the label for "Write something first").
20. Ask Codex/Ask Claude/Show Assist menu items are disabled for an empty draft (`Commands.swift:186`), but the Assist toolbar button opens the panel anyway (`ChromeControls.swift:89-90`, `ChromeControls.swift:159-162`). **Audit: kept** (the toolbar's Assist button opens the panel on an empty draft).
21. Codex is told the client version is "0.1.0" (`WriterAgents/CodexAgent.swift:57`); the app is 0.2.0 (`script/build-app.sh:45`). **Audit: kept** (writer-agents sends 0.1.0).
22. README's connect command uses double quotes (`README.md:98`); the app builds it with single quotes and quotes `--support` too (`ConnectAgent.swift:16-22`). **Audit: kept** (single quotes, `--support` quoted; test `commands_quote_every_argument`).
23. Bigger Text's key equivalent is "+" (`MainMenu.swift:124`); on US layouts that means ⇧⌘= and plain ⌘= does nothing, though the README lists ⌘+. **Audit: fixed as promised:** ⌘= and ⇧⌘= both (test `text_sizes_step_through_the_list_keep_the_selection_and_flash`).
24. Comments added through the inbox are appended without the position sort the app applies to its own comments (`WriterCore/DocumentNotes.swift:204` vs `NotesController.swift:123`), so list order can differ from text order. **Audit: kept** (`DocumentNotes::add_comment` appends).
25. Toggling the sidebar forces the traffic lights to full opacity (`RemoteEditing.swift:58`) even if the rest of the chrome is faded out. **Audit: not applicable:** the app doesn't control the traffic lights' opacity (macOS, section 1).
26. The Info.plist has no app icon key (`script/build-app.sh:35-61`); the app uses the generic icon. **Audit: not applicable yet:** there is no Rust packaging (section 21).
27. Save As uses an app-modal save panel (`EditorController.swift:408`) while every other panel in the app is a window sheet. **Audit: not kept:** Rune has no app-modal save panel, so Save As is a sheet on macOS (GAPS).
28. Typing any key in the editor hides the chrome, including arrow keys and ⌥/⌘ navigation, because the hook is on `keyDown` (`EditorTextView.swift:61-64`). **Audit: kept** (`user_is_typing` runs for every key, arrows included).
29. The Assist scope line refreshes on selection changes, provider changes, opening the panel and a request ending (`AssistController.swift:438-439`, `672`, `386`, `853`), not when a version preview opens or closes from its card, so it can name the draft while a version is what Send will use, until the selection next moves. **Audit: kept** (as noted in section 14).
