# Fluent Writer parity checklist

Everything the Swift/AppKit app does, one observable behaviour per item, so the Rust/Rune rewrite can be checked item by item. Sources are `File.swift:line` under `Sources/FluentWriter/` unless a longer path is given. `WriterCore/…` and `WriterAgents/…` are under `Sources/`.

Items marked **(AppKit built-in)** come from AppKit with no app code beyond turning them on. The rewrite has to build them by hand.

Colours are sRGB 0–255 from `Theme.swift`, written `light / dark`. Alpha is given when it isn't 1.

---

## 1. Window and chrome

### Theme colours (each is a light/dark pair that follows the effective appearance)

- [ ] `canvas` 247,247,247 / 26,26,26 — window background, top bar, footer, Assist panel background. `Theme.swift:35`
- [ ] `text` 25,25,25 / 218,218,216 — body text. `Theme.swift:36`
- [ ] `dimmed` 199,196,194 / 78,78,76 — text outside the focused sentence or paragraph. `Theme.swift:37`
- [ ] `markup` 172,170,168 / 112,112,110 — Markdown markers, link URLs, underline colour. `Theme.swift:38`
- [ ] `quiet` 150,148,146 / 120,120,118 — chrome labels, code-block text, secondary text. `Theme.swift:39`
- [ ] `caret` 0,190,255 / 0,175,245 — caret, active toolbar button tint. `Theme.swift:40`
- [ ] `selection` 178,228,250 / 22,78,104 — selected-text background. `Theme.swift:41`
- [ ] `hairline` 228,228,226 / 44,44,44 — 1 px dividers, card borders, image frame. `Theme.swift:42`
- [ ] `sidebar` 240,240,239 / 32,32,32 — sidebar background. `Theme.swift:43`
- [ ] `sidebarSelection` 222,222,220 / 52,52,52 — sidebar selected row; toolbar button plate. `Theme.swift:44`
- [ ] `sidebarCurrent` 230,230,228 / 44,44,44 — sidebar row of the open file. `Theme.swift:45`
- [ ] `codeBackground` 236,236,234 / 38,38,38 — inline code background, image placeholder fill. `Theme.swift:46`
- [ ] `warning` 196,54,36 / 240,110,90 — failed-save and failed-request text. `Theme.swift:47`
- [ ] `inserted` 30,130,70 / 110,200,140 — diff inserted text. `Theme.swift:48`
- [ ] `insertedBackground` 30,160,80 α0.13 / 90,200,120 α0.18. `Theme.swift:49`
- [ ] `deleted` 180,50,40 / 235,120,105 — diff deleted text and its strikethrough. `Theme.swift:50`
- [ ] `deletedBackground` 210,60,40 α0.11 / 230,90,70 α0.17. `Theme.swift:51`
- [ ] `commentHighlight` 255,196,0 α0.20 / 255,196,40 α0.16. `Theme.swift:52`
- [ ] `commentHighlightActive` 255,180,0 α0.42 / 255,190,40 α0.34. `Theme.swift:53`
- [ ] `card` 255,255,255 / 34,34,34. `Theme.swift:54`
- [ ] `cardHover` 252,252,251 / 40,40,40. `Theme.swift:55`
- [ ] `cardSelected` 234,246,253 / 26,54,70. `Theme.swift:56`
- [ ] `icon` 120,118,116 / 172,172,170 — toolbar icons at rest. `Theme.swift:57`
- [ ] `banner` 238,238,236 / 36,36,36 — version preview bar. `Theme.swift:58`

### Window

- [ ] One window per app. Initial content size 1060×760 pt. `EditorController.swift:111-115`
- [ ] Window style: titled, closable, miniaturizable, resizable, full-size content view. `EditorController.swift:113`
- [ ] Title bar is transparent and the window title text is hidden; content draws under the title bar. `EditorController.swift:140-141`
- [ ] Window background colour is `canvas`. `EditorController.swift:142`
- [ ] Minimum window size 420×320 pt. `EditorController.swift:143`
- [ ] Window tabbing is disabled (no tab bar, no "Merge All Windows"). `EditorController.swift:144`
- [ ] Window frame is saved and restored under the autosave name `FluentWriterMain`; with no saved frame the window is centred on screen. **(AppKit built-in)** `EditorController.swift:146-147`
- [ ] Mouse-moved events are delivered to the window (needed for chrome reveal and sidebar peek). `EditorController.swift:149`
- [ ] Window title (used by the Window menu, Mission Control and accessibility) is the draft title. `EditorController.swift:356`, `EditorController.swift:440`
- [ ] Window `representedURL` is the draft's file for local files and nil for remote files and unsaved drafts. **(AppKit built-in: Window-menu/proxy behaviour of representedURL)** `EditorController.swift:355`, `EditorController.swift:441`
- [ ] Closing the window first runs the leave-document check (save, or ask if the save failed); cancelling keeps the window open. `EditorController.swift:841-843`
- [ ] When the window closes, the app quits. `EditorController.swift:845-847`, `main.swift:48`
- [ ] Full screen via View › Enter Full Screen (⌃⌘F); entering and leaving full screen re-lays out the window. **(AppKit built-in: full screen itself)** `MainMenu.swift:131`, `EditorController.swift:838-839`
- [ ] In full screen the traffic lights are hidden by the system. **(AppKit built-in)**
- [ ] Window resize re-lays out every region (sidebar, top bar, footer, editor, Assist panel, preview). `EditorController.swift:826`, `EditorController.swift:238-259`

### Layout regions

- [ ] Top bar is 40 pt tall, filled with `canvas`, spans only the editor column (between sidebar and Assist panel). `EditorController.swift:92`, `EditorController.swift:250`, `EditorController.swift:34-40`
- [ ] Footer is 34 pt tall, filled with `canvas`, at the bottom of the editor column, drawn over the bottom of the text scroll view. `EditorController.swift:93`, `EditorController.swift:251`
- [ ] Editor scroll view starts 40 pt from the top and runs to the bottom of the window (under the footer). `EditorController.swift:252`
- [ ] Assist panel width: 0 when hidden; when shown `min(380, max(260, round(windowWidth × 0.5)))`. `EditorController.swift:84`, `EditorController.swift:241`
- [ ] Pinned sidebar width: `min(260, max(0, windowWidth − assistWidth − 320))` so the editor keeps at least 320 pt when possible. `EditorController.swift:242`
- [ ] While the sidebar peeks it is always 260 pt wide and does not push the editor. `EditorController.swift:244-246`
- [ ] Editor column width = window width − Assist width − pinned sidebar width. `EditorController.swift:243`
- [ ] Assist panel sits at the right edge, full window height. `EditorController.swift:253`

### Chrome fade

- [ ] Chrome = title field, status label, counts label, focus label, sidebar button, the four toolbar buttons, and the traffic lights (traffic lights only when the sidebar is not pinned). The bars themselves do not fade. `EditorController.swift:682`
- [ ] Chrome fades out over 0.6 s and in over 0.25 s; with Reduce Motion both are instant. `EditorController.swift:683-687`
- [ ] Any key press in the editor hides the chrome immediately, unless the save state is failed or the Assist panel is open. `EditorController.swift:650-655`, `EditorTextView.swift:61-64`
- [ ] Chrome does not hide while the title field is being edited. `EditorController.swift:659`
- [ ] Moving the mouse over the window shows the chrome and starts a 3.0 s auto-hide timer. `EditorController.swift:150-153`, `EditorController.swift:664-679`
- [ ] When the 3.0 s timer fires it hides the chrome only if: the window is key, the editor text view is first responder, the save state is not failed, and the pointer is not over the top bar, the footer, or the sidebar panel. `EditorController.swift:672-676`
- [ ] When the window loses key status, the draft is saved and the chrome is shown with no auto-hide. `EditorController.swift:828-831`
- [ ] When the window becomes key, the chrome is shown with auto-hide, and the sidebar list reloads if the sidebar is pinned. `EditorController.swift:833-836`
- [ ] A failed save (local or remote upload) shows the chrome with no auto-hide. `EditorController.swift:469`, `EditorController.swift:485`
- [ ] Every transient footer message shows the chrome with auto-hide. `EditorController.swift:494`

### Toolbar buttons (ChromeButton)

- [ ] Four buttons pinned to the window's top-right corner, right to left: Assist, Versions, Comments, then Add Comment. They do not move when panels open. `ChromeControls.swift:98-109`
- [ ] Button slot is 28×24 pt; the rightmost slot's right edge is 12 pt from the window edge; 2 pt between Assist/Versions/Comments, 10 pt between Comments and Add Comment. `ChromeControls.swift:10`, `ChromeControls.swift:101-108`
- [ ] Buttons are vertically centred on the sidebar button. `ChromeControls.swift:102`, `ChromeControls.swift:106`
- [ ] Buttons always draw above other views (re-raised on every layout). `ChromeControls.swift:109`
- [ ] Add Comment: SF Symbol `plus.bubble`, accessibility label "Add comment", tooltip "Comment on the selection or this line (⌥⌘M)", runs Add Comment. `ChromeControls.swift:71`, `ChromeControls.swift:83-84`
- [ ] Comments: `text.bubble`, label "Comments", tooltip "Comments (⇧⌘M)", toggles the Assist panel on its Comments tab. `ChromeControls.swift:72`, `ChromeControls.swift:85-86`, `ChromeControls.swift:159-166`
- [ ] Versions: `clock.arrow.circlepath`, label "Versions", tooltip "Versions (⌥⌘V)", toggles the Assist panel on its Versions tab. `ChromeControls.swift:73`, `ChromeControls.swift:87-88`
- [ ] Assist: `sparkles`, label "Assist", tooltip "Assist (⌥⌘J)", toggles the Assist panel on its Ask tab. `ChromeControls.swift:74`, `ChromeControls.swift:89-90`
- [ ] Toggle rule: if the panel is open on that tab, close it; otherwise open it on that tab (keeping the current provider). `ChromeControls.swift:159-162`
- [ ] Symbols are drawn at 13 pt, regular weight, image only, no border. `ChromeControls.swift:14-18`
- [ ] Icon tint: `caret` when active, `text` while hovered, `icon` otherwise. `ChromeControls.swift:47`
- [ ] Hover/active plate: rounded rect, corner radius 6, filled with `sidebarSelection` at α0.7 on hover and α0.9 when active; clear otherwise. `ChromeControls.swift:26`, `ChromeControls.swift:49-50`
- [ ] Hover tracking only while the window is key. `ChromeControls.swift:33`
- [ ] Colours refresh when the appearance changes. `ChromeControls.swift:41-44`
- [ ] Count badge: IBM Plex Mono Medium 9 pt, centred, in the button's top-right corner, 11 pt tall, width `max(12, textWidth + 2)`, 1 pt from the right edge. `ChromeControls.swift:19-21`, `ChromeControls.swift:53-57`
- [ ] Badge text is the count, or "99+" above 99; hidden at 0. `ChromeControls.swift:60-66`
- [ ] Badge colour is `caret` when the button is active, `text` otherwise. `ChromeControls.swift:48`
- [ ] Button accessibility value is the count when > 0, none otherwise. `ChromeControls.swift:63`
- [ ] Comments badge shows the number of open comments (detached ones included). `ChromeControls.swift:136-137`
- [ ] Comments button is active while the Assist panel is open on Comments. `ChromeControls.swift:140`
- [ ] Versions button is active while the panel is open on Versions, or while a version preview is showing. `ChromeControls.swift:141`
- [ ] Assist button is active while the panel is open on Ask. `ChromeControls.swift:142`

### Sidebar button

- [ ] SF Symbol `sidebar.left`, label "Toggle sidebar", tooltip "Show or hide the sidebar (⌃⌘S). Point at the left edge to peek." `EditorController.swift:77`
- [ ] Placed 12 pt right of the zoom (green) traffic light, vertically centred on it; in full screen at x 12, y 8. Size 28×24. `ChromeControls.swift:121-128`
- [ ] Always drawn above the sidebar panel. `ChromeControls.swift:129-132`
- [ ] Active (caret tint and plate) while the sidebar is pinned. `ChromeControls.swift:143`
- [ ] Click toggles the sidebar; if the sidebar is peeking, click pins it instead. `RemoteEditing.swift:49-62`, `ChromeControls.swift:249-258`

### Title field position

- [ ] Title field is centred in the top bar, 18 pt tall, at `midY − 9` of the sidebar button. `ChromeControls.swift:116`
- [ ] Title width = `max(0, min(420, topBarWidth − 2 × reserve))`, where reserve = the largest of: the part of the toolbar buttons over the top bar (+8 pt), the sidebar button's right edge + 12 pt inside the top bar, and 24 pt. `ChromeControls.swift:110-115`
- [ ] Title field is hidden when its width is under 60 pt. `ChromeControls.swift:117`

### Opening the Assist panel in a narrow window

- [ ] Opening the Assist panel widens the window to 720 pt (980 pt if the sidebar is pinned) when it is narrower, capped to the screen's visible width, shifted left to stay on screen, animated unless Reduce Motion. Not done in full screen. `ChromeControls.swift:147-157`, `AssistController.swift:380-384`

---

## 2. Editor surface and typography

- [ ] Body font is IBM Plex Mono (bundled OTFs, registered for the process at launch). `Theme.swift:68-77`, `main.swift:8`
- [ ] Font faces: regular `IBMPlexMono`, bold `IBMPlexMono-Bold`, italic `IBMPlexMono-Italic`, bold italic `IBMPlexMono-BoldItalic`. `Theme.swift:94-102`
- [ ] Fallback when the font is missing: the system monospaced font (bold weight for bold; italic trait for italic). `Theme.swift:103-106`
- [ ] UI font is IBM Plex Mono regular, or IBM Plex Mono Medium when weight is medium; any other weight uses regular. Fallback system monospaced at that weight. Default UI size 12 pt. `Theme.swift:109-113`
- [ ] Fonts are looked up in `FluentWriter_FluentWriter.bundle` under `Fonts` or `Contents/Resources/Fonts`, searched in the app's Resources, the bundle root, then beside the executable. `Theme.swift:79-92`
- [ ] Default text size 19 pt. `Theme.swift:60`
- [ ] Allowed text sizes: 14, 15, 16, 17, 18, 19, 20, 22, 24, 26, 28, 32. `Theme.swift:61`
- [ ] Line height = `round(fontSize × 1.55)`, fixed (min = max line height). `Theme.swift:62`, `MarkdownStyler.swift:34`, `MarkdownStyler.swift:41-42`
- [ ] Baseline offset = `max(0, round((lineHeight − naturalLineHeight) / 2))` so text sits centred in the tall line. `MarkdownStyler.swift:35-36`, `MarkdownStyler.swift:56`
- [ ] Line wrapping by word; no hyphenation/line-break strategy; no tightening. `MarkdownStyler.swift:45-47`
- [ ] Character width = width of "0" in the regular font. `Theme.swift:115-117`, `MarkdownStyler.swift:33`
- [ ] Hang width (space left of the prose edge for block markers) = 4 × character width. `MarkdownStyler.swift:17`
- [ ] Every paragraph has first-line and head indent = hang width, so plain prose starts at the prose edge. `MarkdownStyler.swift:43-44`
- [ ] Prose column width = `max(160, min(round(68 × charWidth), visibleWidth − 2 × minPad))`, minPad = `max(36, hangWidth + 12)`. `Theme.swift:63-64`, `EditorController.swift:296-299`
- [ ] Horizontal inset = `max(4, round((visibleWidth − prose) / 2 − hangWidth))`; the column is centred. `EditorController.swift:300`
- [ ] Text container width = prose + hang width; line fragment padding 0. `EditorController.swift:107`, `EditorController.swift:309`
- [ ] Vertical inset 56 pt top and bottom when focus is Off. `EditorController.swift:306`
- [ ] With focus Off, extra scrollable space below the last line = `round(visibleHeight × 0.35)`. `EditorController.swift:307`, `EditorTextView.swift:19-26`
- [ ] With any focus mode on (Sentence, Paragraph, Typewriter) vertical inset = `max(40, round(visibleHeight / 2 − lineHeight / 2))` and no extra bottom space, so the first and last lines can reach the middle. `EditorController.swift:302-304`
- [ ] Text view is at least as tall as the visible area. `EditorController.swift:311`, `EditorTextView.swift:23`
- [ ] The selection is preserved across any relayout. `EditorController.swift:294`, `EditorController.swift:315`
- [ ] Text colour `text`; selection background `selection` (no selected-text colour change). `MarkdownStyler.swift:54`, `EditorController.swift:176`
- [ ] Links get no special link attributes (not clickable as links). `EditorController.swift:177`
- [ ] Editor background is transparent over `canvas`. `EditorController.swift:170`
- [ ] Caret: filled rounded rect (radius 1) in `caret`, width `max(2, round(fontSize / 9))`, height `round(naturalLineHeight × 1.12)`, vertically centred in the line. `EditorController.swift:283-285`, `EditorTextView.swift:71-89`
- [ ] Caret blink-off redraws a rect 1 pt larger than the caret so no trace is left; narrow invalidations are widened by caretWidth + 1. `EditorTextView.swift:86-95`
- [ ] Caret blinks. **(AppKit built-in)**
- [ ] Plain text only: no rich text, no pasted graphics as attachments. `EditorController.swift:158-159`
- [ ] Vertical scroller is an overlay scroller that auto-hides; no border; no scroll view background. `EditorController.swift:191-195`
- [ ] Changing text size keeps the selection, re-centres the caret in focus modes (otherwise scrolls the selection into view), and flashes "Text size N pt". `Commands.swift:129-138`
- [ ] Layout is contiguous (whole document laid out), font leading not used. `EditorController.swift:102-103`
- [ ] Text input: IME/marked text, dead keys, emoji picker, dictation. **(AppKit built-in)**
- [ ] Standard text navigation and selection keys (arrows, ⌥/⌘ arrows, ⇧ extend, Home/End, Page Up/Down, Emacs ⌃A/⌃E/⌃K etc.), double-click word, triple-click paragraph, drag-select, drag-to-move selected text. **(AppKit built-in)**
- [ ] Undo/redo with typing coalescing; undo history cleared when a document loads. **(AppKit built-in for the stack)** `EditorController.swift:160`, `EditorController.swift:328`
- [ ] Undo menu titles name the action ("Undo Typing", "Undo Bold", …) from the action names the app sets. **(AppKit built-in titles)** `EditorController.swift:767`
- [ ] Dragging text in and out of the editor, and dropping text from other apps. **(AppKit built-in)**
- [ ] Continuous spell checking on by default (preference). `EditorController.swift:163`
- [ ] Automatic spelling correction off at launch. `EditorController.swift:164`
- [ ] Smart quotes off at launch. `EditorController.swift:165`
- [ ] Smart dashes off at launch. `EditorController.swift:166`
- [ ] Text replacement (system text shortcuts) on at launch. **(AppKit built-in: the replacement itself)** `EditorController.swift:167`
- [ ] Automatic link detection off. `EditorController.swift:168`
- [ ] Smart insert/delete (automatic spaces around pasted words) off. `EditorController.swift:169`
- [ ] Find bar (not a panel), shown above the text, with incremental search highlighting. **(AppKit built-in)** `EditorController.swift:161-162`, `EditorController.swift:196`
- [ ] Look Up / dictionary on force-click or ⌃⌘D. **(AppKit built-in)**

---

## 3. Markdown styling

- [ ] Restyling covers only the lines touched by an edit, unless the number of code-fence lines changed, in which case the whole document is restyled. `EditorController.swift:527-536`
- [ ] Styling runs after the edit transaction finishes (deferred to the next main-loop turn). `EditorController.swift:538-549`
- [ ] Base attributes for every character: regular font, `text` colour, base paragraph style, baseline offset. `MarkdownStyler.swift:51-58`
- [ ] Headings (`#`–`######` followed by space/tab or end of line): whole line bold. `MarkdownStyler.swift:90-91`, `WriterCore/Markdown.swift:64`
- [ ] Heading marker (`## ` including trailing space) coloured `markup`. `MarkdownStyler.swift:92`
- [ ] Heading marker hangs into the left margin: first-line indent `max(0, hangWidth − markerWidth)`, so heading text starts at the prose edge; wrapped lines indent to the prose edge. `MarkdownStyler.swift:93-94`
- [ ] Bullet items (`-`, `*`, `+` then whitespace), ordered items (1–9 digits then `.` or `)` then whitespace), task items (`- [ ]`, `- [x]`, `- [X]`), blockquotes (one or more `>` with optional space): marker coloured `markup`. `MarkdownStyler.swift:95-96`, `WriterCore/Markdown.swift:65-68`
- [ ] List and quote markers hang: first-line indent `max(0, hangWidth − markerWidth)`; wrapped lines align under the text after the marker (head indent = first-line indent + leading indent width + marker width). `MarkdownStyler.swift:97-100`
- [ ] Code fence lines (``` or ~~~, 3+, up to 3 leading spaces) coloured `markup`; no inline styling. `MarkdownStyler.swift:101-103`, `WriterCore/Markdown.swift:70`
- [ ] A fence closes only on the same fence character it opened with. `WriterCore/Markdown.swift:128-138`
- [ ] Lines inside a fence coloured `quiet`; no inline styling. `MarkdownStyler.swift:104-106`
- [ ] Horizontal rules (3+ of `-`, `*` or `_`, optional spaces, up to 3 leading spaces) coloured `markup`. `MarkdownStyler.swift:101-103`, `WriterCore/Markdown.swift:69`
- [ ] Bold (`**x**`, `__x__`): bold face; delimiters `markup`. `MarkdownStyler.swift:126-127`, `MarkdownStyler.swift:143`
- [ ] Italic (`*x*`, `_x_`): italic face; delimiters `markup`. `MarkdownStyler.swift:128-129`
- [ ] Bold italic (`***x***`): bold italic face (bold and italic combine with any existing trait). `MarkdownStyler.swift:113-121`, `WriterCore/Markdown.swift:235-242`
- [ ] Strikethrough (`~~x~~`): single strikethrough in `quiet` over the content (not the delimiters); delimiters `markup`. `MarkdownStyler.swift:130-132`
- [ ] Inline code (backtick runs): `codeBackground` behind the whole span including backticks; backticks `markup`. Nothing inside code is styled as emphasis. `MarkdownStyler.swift:133-134`, `WriterCore/Markdown.swift:207-213`
- [ ] Links `[text](url)` and images `![alt](url)`: link text underlined (single, `markup` colour); `[`, `](`, `)` and the URL coloured `markup`. `MarkdownStyler.swift:135-138`
- [ ] Link text may carry emphasis. `WriterCore/Markdown.swift:221-222`
- [ ] Autolinks `<https://…>` / `<mailto:…>` and bare URLs (`http://`, `https://`, `www.`): underlined in `markup`; angle brackets `markup`. `MarkdownStyler.swift:139-141`
- [ ] Headings keep the same font size as body text (one size throughout). `MarkdownStyler.swift:4-5`, `MarkdownStyler.swift:91`
- [ ] Image lines are additionally coloured `quiet` (see Images). `ImageBlocks.swift:168-169`

---

## 4. Focus modes and scrolling

- [ ] Four modes: Off, Sentence, Paragraph, Typewriter. `WriterCore/TextUnits.swift:4-12`
- [ ] Sentence and Paragraph dim everything outside the current unit to `dimmed`. Typewriter dims nothing. `WriterCore/TextUnits.swift:10`, `EditorController.swift:587-602`
- [ ] Sentence, Paragraph and Typewriter all keep the caret line at the vertical middle of the window. `WriterCore/TextUnits.swift:11`
- [ ] Paragraph unit = the hard line (text between line breaks) containing the caret. `WriterCore/TextUnits.swift:16-26`
- [ ] Sentence unit = the natural-language sentence inside that line containing the caret, trimmed of trailing whitespace; a caret right after the final punctuation belongs to that sentence. `WriterCore/TextUnits.swift:28-58`
- [ ] With a selection, the bright range covers from the unit at the selection start to the unit at the selection end. `EditorController.swift:577-585`
- [ ] Dimming is a display-only colour (does not change stored attributes) and is recomputed after every edit and on selection change when the range changed. `EditorController.swift:587-602`
- [ ] Image previews outside the bright range fade to α0.28. `ImageBlocks.swift:218-224`
- [ ] Choosing a mode saves it, relays out, keeps the selection, re-centres the caret (animated unless Reduce Motion), sets the footer focus label, and flashes "Focus off" or "<Name> focus" ("Sentence focus", "Paragraph focus", "Typewriter focus"). `EditorController.swift:553-565`
- [ ] Choosing Sentence or Paragraph also records it as the mode Toggle Focus returns to. `EditorController.swift:557`
- [ ] Toggle Focus (⌘D): if Sentence or Paragraph is on, turn focus Off; otherwise switch to the last dimming mode (default Sentence). From Typewriter it switches to the last dimming mode. `Commands.swift:120-122`, `Preferences.swift:13-16`
- [ ] Centring: scroll so the caret's line midpoint is at the middle of the visible area, clamped to the document. `EditorController.swift:607-613`
- [ ] Scroll animation lasts 0.14 s, ease-out; instant with Reduce Motion; no scroll if the target is within 0.5 pt. `EditorController.swift:630-646`
- [ ] In centring modes, typing re-centres with animation after each change. `EditorController.swift:709-713`
- [ ] In centring modes, caret moves caused by key presses (arrows etc.) re-centre with animation; mouse clicks do not re-centre. `EditorController.swift:723-726`
- [ ] With focus Off, after each change the view scrolls only when the caret is too close to an edge: bottom limit is `max(34 + 2 × lineHeight, 22% of visible height)` from the bottom; top limit is one line height from the top. `EditorController.swift:616-628`
- [ ] On document load the selection is scrolled into view, then centred (focus modes) or kept readable (Off), without animation. `EditorController.swift:346-351`
- [ ] Reduce Motion is read from the system accessibility setting. `EditorController.swift:604`

---

## 5. Documents, saving, recovery

### Launch

- [ ] Drafts folder `~/Documents/Fluent Writer` (dev: `~/Documents/Fluent Writer Dev`), created if missing. `WriterCore/AppVariant.swift:23-26`, `WriterCore/DraftStore.swift:89-95`
- [ ] Support folder `~/Library/Application Support/Fluent Writer` (dev: `… Fluent Writer Dev`) with a `Recovery` subfolder, created if missing. `WriterCore/AppVariant.swift:18-21`, `WriterCore/DraftStore.swift:83`, `WriterCore/DraftStore.swift:94-95`
- [ ] Stale temporary files (`.<name>.fw-tmp-<uuid>`) from interrupted saves are deleted at launch in the drafts, support and recovery folders. `WriterCore/DraftStore.swift:96-98`, `WriterCore/AtomicFile.swift:15`, `WriterCore/AtomicFile.swift:61-62`
- [ ] If the folders can't be created, an error alert is shown and the app quits. `main.swift:12-18`
- [ ] Launch document: `--open <path>` if it opens; else the newest recovery snapshot whose file is missing or that never had a file; else a new empty draft. `main.swift:19-27`, `WriterCore/DraftStore.swift:170-172`
- [ ] The app activates and comes to the front at launch. `main.swift:31`
- [ ] Opening files from Finder / Dock opens the first URL in the existing window (same rules as Open). `main.swift:34-36`

### Opening

- [ ] Accepted extensions: md, markdown, txt, text. `WriterCore/DraftStore.swift:87`
- [ ] Opening the file that is already open just focuses the editor. `Commands.swift:41`
- [ ] Opening another file first runs the leave-document check. `Commands.swift:42`
- [ ] Files are read as UTF-8, falling back to ISO Latin 1. `WriterCore/DraftStore.swift:134`
- [ ] Error "Couldn't open “<name>”: <reason>" when the file can't be read; "“<name>” isn't a text file." when it can't be decoded; shown as a sheet alert. `WriterCore/DraftStore.swift:130-136`, `EditorController.swift:850-854`
- [ ] The caret/selection saved for that file is restored on open; with no record, the caret goes to the end of the file. `WriterCore/DraftStore.swift:139-142`
- [ ] A remembered title and manual-title flag are restored; with no record the title is the file name without extension. `WriterCore/DraftStore.swift:137-138`
- [ ] Opened files are added to the system recent-documents list (Dock menu). **(AppKit built-in: the list)** `Commands.swift:45`
- [ ] File › Open Other File… (⇧⌘O): open panel as a sheet, md/markdown/txt/text only, single file, starting in the drafts folder. `Commands.swift:26-38`

### Saving

- [ ] Autosave 0.8 s after the last text change. `EditorController.swift:428-433`, `EditorController.swift:707`
- [ ] Footer shows "Edited" while a save is pending. `EditorController.swift:475-477`, `EditorController.swift:706`
- [ ] Saves are atomic (temp file in the same folder, fsync, rename). `WriterCore/AtomicFile.swift:17-50`
- [ ] An empty, never-saved draft is never written. `WriterCore/DraftStore.swift:193`
- [ ] Nothing is written when the text equals what was last saved (and nothing else is pending). `WriterCore/DraftStore.swift:194`
- [ ] A new draft's first save creates `<title>.md` in the drafts folder; if taken, `<title> 2.md`, `<title> 3.md`, … `WriterCore/DraftStore.swift:197-199`, `WriterCore/DraftStore.swift:278-288`
- [ ] File-name stem: `/` and `:` become `-`, control characters removed, leading/trailing spaces and dots trimmed, "Untitled" if empty, cut to 120 characters if over 200 UTF-8 bytes. `WriterCore/TitleDeriver.swift:29-36`
- [ ] A draft in the drafts folder without a manual title is renamed on save to follow its derived title (unless the current name is already that title, or that title plus " N"). Files elsewhere keep their name. `WriterCore/DraftStore.swift:232-241`
- [ ] File › Save (⌘S) saves now and flashes "Saved"; does nothing for an empty new draft. `Commands.swift:51-54`
- [ ] Save also happens when the window loses focus, before opening another document, before quitting, and before the Open picker appears. `EditorController.swift:828-831`, `EditorController.swift:386-387`, `Commands.swift:15`
- [ ] Caret position is remembered (in `state.json`, not in the file) 1.0 s after the selection stops changing, and on every save. `EditorController.swift:727-731`, `EditorController.swift:368`
- [ ] A successful save in Recent sidebar mode refreshes the Recent list. `EditorController.swift:372-374`
- [ ] A save failure keeps the failure message in the footer as "Not saved — <message>" in `warning`, with the message as tooltip, and keeps the chrome visible. `EditorController.swift:378-381`, `EditorController.swift:481-485`
- [ ] Save failure messages read "Couldn't save “<file>”: <system reason>." `WriterCore/AtomicFile.swift:8-11`
- [ ] While a save has failed, further typing does not change the footer back to "Edited". `EditorController.swift:706`
- [ ] Each failed save writes a recovery snapshot (`Recovery/<draft-uuid>.json`: path, title, manual-title flag, body, caret, time). `WriterCore/DraftStore.swift:212-222`
- [ ] A successful save deletes the draft's recovery snapshot and any other snapshot for the same path. `WriterCore/DraftStore.swift:207-208`

### Leaving a document

- [ ] Before replacing the open draft (New, Open, open over SSH, sidebar click, close, quit) the app saves; if the save fails it shows a warning alert: title "This draft couldn't be saved.", text "<message>\n\nA recovery copy is kept and will be offered next time.", buttons "Keep Editing" (default), "Save As…", "Continue". `EditorController.swift:385-400`
- [ ] Keep Editing cancels; Save As… runs Save As and proceeds only if it succeeds; Continue proceeds. `EditorController.swift:395-399`
- [ ] For remote files with edits not yet uploaded, the leave check also uploads synchronously (see SSH). `EditorController.swift:387`, `RemoteEditing.swift:194-211`
- [ ] Quitting stops any running Assist request, then runs the leave check; cancel aborts the quit. `main.swift:38-42`
- [ ] On quit the open draft's comments/versions are flushed to disk and the live-state file is deleted. `main.swift:44-46`, `NotesController.swift:254-257`

### New draft

- [ ] File › New (⌘N): if the open draft is empty and never saved, just focus the editor; otherwise leave-check then open an empty "Untitled" draft. `Commands.swift:7-11`

### Save As

- [ ] File › Save As… (⇧⌘S): app-modal save panel (not a sheet), `.md` only, name "<title stem>.md", starting in the drafts folder. `EditorController.swift:403-408`
- [ ] Writes the current text atomically to the chosen file, opens that file in place of the current one keeping the current selection, and discards the old draft's recovery snapshot. The old file stays where it was. `EditorController.swift:409-416`
- [ ] Save As errors show as a sheet alert. `EditorController.swift:417-419`

### Recovery

- [ ] Opening a file whose recovery snapshot differs from the file loads the snapshot's text, title and caret; footer shows "Recovered unsaved changes"; the recovered text is saved to the file 0.2 s later. `WriterCore/DraftStore.swift:145-156`, `EditorController.swift:334-336`, `EditorController.swift:478-480`
- [ ] A snapshot identical to the file is deleted on open. `WriterCore/DraftStore.swift:153-155`
- [ ] A recovered never-saved draft is saved as a new file 0.2 s after launch. `WriterCore/DraftStore.swift:162-167`, `EditorController.swift:334-336`

### Show in Finder

- [ ] File › Show in Finder (⌥⌘R) reveals the file in Finder; for an unsaved draft with text it saves first to create the file. `Commands.swift:64-68`
- [ ] Enabled only for local drafts that have a file or have text. `Commands.swift:182`

---

## 6. Title and rename

- [ ] Title field: borderless, no background, no focus ring, centred, IBM Plex Mono 13 pt, single line, truncates at the tail. `EditorController.swift:205-212`
- [ ] Placeholder "Untitled". `EditorController.swift:214`, `WriterCore/TitleDeriver.swift:4`
- [ ] Tooltip "Click to rename"; accessibility label "Draft title". `EditorController.swift:215-216`
- [ ] Field is empty (placeholder shows) when the title is "Untitled" and the draft is empty; otherwise it shows the title. `EditorController.swift:437`
- [ ] Title colour: `text` at α0.75 when the writer named the draft, `quiet` when derived. `EditorController.swift:438`
- [ ] The field isn't overwritten while it is being edited. `EditorController.swift:436`
- [ ] Derived title = first line that isn't blank, a fence or a rule, with its block marker dropped, inline Markdown removed, whitespace collapsed. `WriterCore/TitleDeriver.swift:7-17`
- [ ] Derived titles are cut to 60 characters; if a space falls past the halfway point the cut is at the last space and trailing " ,;:-" are trimmed. `WriterCore/TitleDeriver.swift:19-26`
- [ ] Without a manual title, the title follows the first line as you type. `EditorController.swift:701-705`
- [ ] File › Rename… (⇧⌘R): shows chrome (no auto-hide), focuses the title field and selects all its text. `Commands.swift:58-62`
- [ ] Clicking the title field starts editing it and shows the chrome with no auto-hide. `EditorController.swift:791-793`
- [ ] Ending the edit (Return, Tab, clicking away) with a changed title renames. `EditorController.swift:795-813`
- [ ] Esc in the title field restores the current title and returns focus to the text without renaming. `EditorController.swift:815-822`
- [ ] Rename trims whitespace; an empty title clears the manual title and goes back to the derived one. `WriterCore/DraftStore.swift:243-251`
- [ ] Renaming a local file moves it in the same folder to "<new stem>.<same extension>" (" 2", " 3"… if taken). `WriterCore/DraftStore.swift:252-262`
- [ ] Renaming a remote file changes only the title shown, not the remote file name. `WriterCore/DraftStore.swift:252`
- [ ] Renaming a never-saved draft that has text saves it first (so it gets a file), then renames. `EditorController.swift:800`
- [ ] A rename failure shows "Couldn't rename “<file>”: <reason>" as a sheet alert. `WriterCore/DraftStore.swift:257-258`, `EditorController.swift:803-805`
- [ ] After a rename, comments and versions move with the file. `EditorController.swift:442`, `NotesController.swift:53-63`
- [ ] After the edit ends, focus returns to the text and chrome auto-hide restarts. `EditorController.swift:811-812`

---

## 7. Footer status and counts

- [ ] Three labels in the footer: status (left), focus mode (centre), counts (right); IBM Plex Mono 12 pt, `quiet`, truncate at tail. `EditorController.swift:221-228`
- [ ] Labels are 16 pt tall at y 9 in the 34 pt footer; side padding 20 pt. `EditorController.swift:264-273`
- [ ] Counts width is its text width + 4, capped at 45% of the footer width; right-aligned to 20 pt from the right edge. `EditorController.swift:265-267`
- [ ] Status width is its text width + 4, capped at the space left of the counts minus 16 pt gap. `EditorController.swift:268-270`
- [ ] Focus label is centred and shown only when it fits with at least 12 pt clear of both status and counts. `EditorController.swift:271-274`
- [ ] Focus label text: "Sentence", "Paragraph", "Typewriter", or empty for Off. `EditorController.swift:562`, `EditorController.swift:567-574`
- [ ] Counts text: "<w> word(s)  ·  <c> character(s)" (two spaces each side of the dot), numbers with locale grouping separators. `WriterCore/TextStats.swift:30-40`
- [ ] With a selection: "<sw> of <w> words  ·  <sc> of <c> characters"; singular/plural follows the total. `WriterCore/TextStats.swift:36-38`
- [ ] Counts measure the plain-text export (Markdown delimiters excluded); characters are grapheme clusters. `WriterCore/TextStats.swift:17-28`
- [ ] Counts update at most every 0.15 s after typing or selection change. `EditorController.swift:520-523`
- [ ] View › Show Counts (⌥⌘C) toggles the counts label; checked when shown; persisted. `Commands.swift:156-160`, `Commands.swift:177`
- [ ] Status "Saved" for a saved local file (tooltip: full path). `EditorController.swift:471-474`
- [ ] Status empty for a never-saved draft. `EditorController.swift:472`
- [ ] Status "Edited" while a save is pending. `EditorController.swift:475-477`
- [ ] Status "Recovered unsaved changes". `EditorController.swift:478-480`
- [ ] Status "Not saved — <message>" in `warning`. `EditorController.swift:481-485`
- [ ] Remote, uploaded: "Saved to <host>". `EditorController.swift:457`
- [ ] Remote, saved locally but not uploaded: "Saved locally · not yet on <host>". `EditorController.swift:457`
- [ ] Remote, uploading: "Saving to <host>…". `EditorController.swift:459-461`
- [ ] Remote, server unreachable at open: "Offline · editing the local copy". `EditorController.swift:462-464`
- [ ] Remote, upload failed: "Not on <host> yet — <message>" in `warning`, tooltip the message, chrome kept visible. `EditorController.swift:465-469`
- [ ] Remote status tooltip is the remote location (`host:path` or `ssh://host:port/path`). `EditorController.swift:454`
- [ ] `<host>` in remote statuses is the host without `user@`. `WriterCore/Remote.swift:48`
- [ ] Transient messages replace the status for 2.2 s, then the status returns. `EditorController.swift:491-501`
- [ ] Every transient message is announced to VoiceOver at medium priority. `EditorController.swift:495`

---

## 8. Menus

Every item, its shortcut, its enabled/checked rule and its action. "Always" means always enabled.

### App menu (titled with the app name: "Fluent Writer" or "Fluent Writer Dev")

- [ ] "About <app name>", no shortcut, always; standard About panel (name, version 0.2.0 (2)). **(AppKit built-in panel)** `MainMenu.swift:25`
- [ ] separator `MainMenu.swift:26`
- [ ] "Hide <app name>" ⌘H, always. **(AppKit built-in)** `MainMenu.swift:27`
- [ ] "Hide Others" ⌥⌘H, always. **(AppKit built-in)** `MainMenu.swift:28`
- [ ] "Show All", no shortcut. **(AppKit built-in)** `MainMenu.swift:29`
- [ ] separator `MainMenu.swift:30`
- [ ] "Quit <app name>" ⌘Q; runs the quit flow (stop Assist, leave check). `MainMenu.swift:31`, `main.swift:38-42`
- [ ] No Settings/Preferences item and no Services submenu in the app menu. `MainMenu.swift:24-32`

### File

- [ ] "New" ⌘N, always → New draft. `MainMenu.swift:35`, `Commands.swift:7-11`
- [ ] "Open…" ⌘O, always → Open picker sheet. `MainMenu.swift:36`, `Commands.swift:13-24`
- [ ] "Open Other File…" ⇧⌘O, always → file open panel. `MainMenu.swift:37`
- [ ] "Open Folder…" ⌥⌘O, always → folder panel for the sidebar. `MainMenu.swift:38`
- [ ] "Open over SSH…" ⌃⌘O, always → SSH sheet. `MainMenu.swift:39`
- [ ] separator `MainMenu.swift:40`
- [ ] "Save" ⌘S, always → save now. `MainMenu.swift:41`
- [ ] "Save As…" ⇧⌘S, always → Save As. `MainMenu.swift:42`
- [ ] "Rename…" ⇧⌘R, always → edit title. `MainMenu.swift:43`
- [ ] "Show in Finder" ⌥⌘R, enabled for local drafts with a file or text. `MainMenu.swift:44`, `Commands.swift:182`
- [ ] separator `MainMenu.swift:45`
- [ ] "Copy for Twitter" ⇧⌘C; title becomes "Copy Selection for Twitter" when text is selected; disabled when the draft is empty. `MainMenu.swift:46`, `Commands.swift:183-185`
- [ ] separator `MainMenu.swift:47`
- [ ] "Close" ⌘W → close the window (leave check, then quit). `MainMenu.swift:48`

### Edit

- [ ] "Undo" ⌘Z. **(AppKit built-in, with action names)** `MainMenu.swift:55`
- [ ] "Redo" ⇧⌘Z. **(AppKit built-in)** `MainMenu.swift:56`
- [ ] separator `MainMenu.swift:57`
- [ ] "Cut" ⌘X. **(AppKit built-in)** `MainMenu.swift:58`
- [ ] "Copy" ⌘C. **(AppKit built-in)** `MainMenu.swift:59`
- [ ] "Paste" ⌘V; also enabled when the clipboard holds only an image (see Images). **(AppKit built-in for text)** `MainMenu.swift:60`, `EditorTextView.swift:33-37`
- [ ] "Delete", no shortcut. **(AppKit built-in)** `MainMenu.swift:61`
- [ ] "Select All" ⌘A. **(AppKit built-in)** `MainMenu.swift:62`
- [ ] separator `MainMenu.swift:63`
- [ ] Find › "Find…" ⌘F — shows the find bar. **(AppKit built-in)** `MainMenu.swift:65`
- [ ] Find › "Find and Replace…" ⌥⌘F — find bar with replace. **(AppKit built-in)** `MainMenu.swift:66`
- [ ] Find › "Find Next" ⌘G. **(AppKit built-in)** `MainMenu.swift:67`
- [ ] Find › "Find Previous" ⇧⌘G. **(AppKit built-in)** `MainMenu.swift:68`
- [ ] Find › "Use Selection for Find" ⌘E. **(AppKit built-in)** `MainMenu.swift:69`
- [ ] Find › "Jump to Selection" ⌥⌘J — scrolls the selection to the centre. **(AppKit built-in)** `MainMenu.swift:70`
- [ ] Find bar: Done button, match count, Esc closes, Return/⇧Return next/previous, replace and replace-all, case and contains/starts-with options. **(AppKit built-in)**
- [ ] Spelling and Grammar › "Show Spelling and Grammar" ⌘: (shift-semicolon) — spelling panel. **(AppKit built-in)** `MainMenu.swift:73`
- [ ] Spelling and Grammar › "Check Document Now" ⌘; — jumps to the next misspelling. **(AppKit built-in)** `MainMenu.swift:74`
- [ ] separator `MainMenu.swift:75`
- [ ] Spelling and Grammar › "Check Spelling While Typing", no shortcut, checked per the spell-check preference; toggles and saves it. `MainMenu.swift:76`, `Commands.swift:162-165`, `Commands.swift:178`
- [ ] Spelling and Grammar › "Check Grammar With Spelling", checked state from the text view; toggles grammar checking (not saved by the app). **(AppKit built-in)** `MainMenu.swift:77`
- [ ] Spelling and Grammar › "Correct Spelling Automatically", checked state from the text view; off at each launch. **(AppKit built-in)** `MainMenu.swift:78`, `EditorController.swift:164`
- [ ] Substitutions › "Smart Quotes", checked from the text view; off at each launch. **(AppKit built-in)** `MainMenu.swift:81`, `EditorController.swift:165`
- [ ] Substitutions › "Smart Dashes", checked from the text view; off at each launch. **(AppKit built-in)** `MainMenu.swift:82`, `EditorController.swift:166`
- [ ] Substitutions › "Text Replacement", checked from the text view; on at each launch. **(AppKit built-in)** `MainMenu.swift:83`, `EditorController.swift:167`
- [ ] Red underline under misspelled words and right-click suggestions (Guesses, Ignore Spelling, Learn Spelling). **(AppKit built-in)**
- [ ] macOS adds "Start Dictation…" and "Emoji & Symbols" (⌃⌘Space) to the Edit menu. **(AppKit built-in)**

### Format

- [ ] "Bold" ⌘B → toggle `**`. `MainMenu.swift:88`, `Commands.swift:90`
- [ ] "Italic" ⌘I → toggle `*`. `MainMenu.swift:89`, `Commands.swift:91`
- [ ] "Strikethrough" ⇧⌘U → toggle `~~`. `MainMenu.swift:90`, `Commands.swift:92`
- [ ] "Link" ⌘K → insert link. `MainMenu.swift:91`, `Commands.swift:94-96`
- [ ] "Image…" ⇧⌘I → image open panel. `MainMenu.swift:92`, `ImageBlocks.swift:361-374`
- [ ] separator `MainMenu.swift:93`
- [ ] "Heading 1" ⌘1, "Heading 2" ⌘2, "Heading 3" ⌘3. `MainMenu.swift:94-96`
- [ ] "Body" ⌥⌘0 → remove block prefix. `MainMenu.swift:97`
- [ ] separator `MainMenu.swift:98`
- [ ] "Bulleted List" ⌘L. `MainMenu.swift:99`
- [ ] "Numbered List" ⌥⌘L. `MainMenu.swift:100`
- [ ] "Blockquote" ⌘'. `MainMenu.swift:101`
- [ ] separator `MainMenu.swift:102`
- [ ] "Indent" ⌘]. `MainMenu.swift:103`
- [ ] "Outdent" ⌘[. `MainMenu.swift:104`
- [ ] All Format items are always enabled. `Commands.swift:169-194`

### View

- [ ] "Show Sidebar" ⌃⌘S; title "Hide Sidebar" while the sidebar is pinned. `MainMenu.swift:113`, `Commands.swift:181`
- [ ] "Recent Files" ⇧⌘E → pin the sidebar (if needed) and switch it to Recent. `MainMenu.swift:114`, `RemoteEditing.swift:64-69`
- [ ] separator `MainMenu.swift:115`
- [ ] "Toggle Focus" ⌘D; title "Turn Focus Off" while Sentence or Paragraph is on, else "Turn <Sentence|Paragraph> Focus On" naming the last dimming mode. `MainMenu.swift:116`, `Commands.swift:175`
- [ ] Focus › "Off" ⌃⌘0, checked when Off. `MainMenu.swift:118`, `Commands.swift:171`
- [ ] Focus › "Sentence" ⌃⌘1, checked when Sentence. `MainMenu.swift:119`, `Commands.swift:172`
- [ ] Focus › "Paragraph" ⌃⌘2, checked when Paragraph. `MainMenu.swift:120`, `Commands.swift:173`
- [ ] Focus › "Typewriter" ⌃⌘3, checked when Typewriter. `MainMenu.swift:121`, `Commands.swift:174`
- [ ] separator `MainMenu.swift:123`
- [ ] "Bigger Text" ⌘+ (key equivalent "+"); next larger size in the list; disabled at 32 pt. `MainMenu.swift:124`, `Commands.swift:140-142`, `Commands.swift:179`
- [ ] "Smaller Text" ⌘−; next smaller size; disabled at 14 pt. `MainMenu.swift:125`, `Commands.swift:144-146`, `Commands.swift:180`
- [ ] "Default Text Size" ⌘0 → 19 pt. `MainMenu.swift:126`, `Commands.swift:148`
- [ ] separator `MainMenu.swift:127`
- [ ] Appearance › "Match System", "Light", "Dark"; the current one is checked; choosing one applies and saves it. `MainMenu.swift:107-111`, `MainMenu.swift:128`, `Commands.swift:150-154`, `Commands.swift:176`, `Theme.swift:15-21`
- [ ] "Show Counts" ⌥⌘C, checked when counts are shown. `MainMenu.swift:129`, `Commands.swift:177`
- [ ] separator `MainMenu.swift:130`
- [ ] "Enter Full Screen" ⌃⌘F; the system retitles it "Exit Full Screen" in full screen. **(AppKit built-in)** `MainMenu.swift:131`

### Assist

- [ ] "Ask Codex…" ⌘J → open Assist on Ask with Codex selected; disabled when the draft is empty. `MainMenu.swift:137`, `AssistController.swift:870`, `Commands.swift:186`
- [ ] "Ask Claude…" ⇧⌘J → open Assist on Ask with Claude selected; disabled when the draft is empty. `MainMenu.swift:138`, `AssistController.swift:871`, `Commands.swift:186`
- [ ] "Show Assist" ⌥⌘J → close the panel if open, else open it on its last tab; disabled when the draft is empty. `MainMenu.swift:139`, `AssistController.swift:866-868`, `Commands.swift:186`
- [ ] separator `MainMenu.swift:140`
- [ ] "Add Comment…" ⌥⌘M → comment composer; disabled when the draft is empty. `MainMenu.swift:141`, `Commands.swift:187`
- [ ] "Show Comments" ⇧⌘M → open Assist on Comments; always enabled. `MainMenu.swift:142`, `AssistController.swift:872`
- [ ] separator `MainMenu.swift:143`
- [ ] "Show Versions" ⌥⌘V → open Assist on Versions; always enabled. `MainMenu.swift:144`, `VersionPreview.swift:408-410`
- [ ] "Older Version" ⌥⌘← → step back in the version timeline; enabled when any version exists. `MainMenu.swift:145`, `Commands.swift:188`
- [ ] "Newer Version" ⌥⌘→ → step forward; enabled only while previewing. `MainMenu.swift:146`, `Commands.swift:189`
- [ ] "Use This Version" ⌥⌘↩ → use the previewed version; enabled only while previewing. `MainMenu.swift:147`, `Commands.swift:190`
- [ ] separator `MainMenu.swift:148`
- [ ] "Connect an Outside Agent…", no shortcut, always → connect sheet. `MainMenu.swift:149`

### Window

- [ ] "Minimize" ⌘M. **(AppKit built-in)** `MainMenu.swift:153`
- [ ] "Zoom", no shortcut. **(AppKit built-in)** `MainMenu.swift:154`
- [ ] The menu is registered as the Window menu, so the system lists the open window in it. **(AppKit built-in)** `MainMenu.swift:156`
- [ ] No Help menu. `MainMenu.swift:20-159`

### Editor context menu (right-click in the text)

- [ ] First item "Comment on Selection…" when text is selected, else "Comment on This Line…" → Add Comment. `EditorController.swift:774-776`
- [ ] When the click is on a commented passage: that comment becomes active and a "Show Comment" item opens the Comments tab. `EditorController.swift:777-780`
- [ ] Then a separator, then the standard text-view menu: spelling suggestions, Look Up, Translate, Search With Google, Cut/Copy/Paste, Share, Spelling and Grammar, Substitutions, Transformations, Speech, Services. **(AppKit built-in)** `EditorController.swift:781-786`

---

## 9. Format commands

All format commands apply through the text system as one undo step with the named action, then reset typing attributes to the base style. `EditorController.swift:763-772`

- [ ] Bold/Italic/Strikethrough with a caret inside a word (not at its edge): wraps the whole word. Word characters are letters, digits, `'` and `’`. `WriterCore/TextEditing.swift:133-137`, `WriterCore/TextEditing.swift:192-205`
- [ ] With a caret not in a word: inserts the delimiter pair and puts the caret between them. `WriterCore/TextEditing.swift:147`
- [ ] With a caret directly between an empty pair (`**|**`): removes the pair. `WriterCore/TextEditing.swift:139-146`
- [ ] With a selection that includes the delimiters on both ends: removes them and selects the inner text. `WriterCore/TextEditing.swift:151-157`
- [ ] With a selection directly surrounded by the delimiters: removes them and keeps the text selected. `WriterCore/TextEditing.swift:158-164`
- [ ] A single `*` only counts as a delimiter when it isn't half of `**` (a `***` run counts). `WriterCore/TextEditing.swift:175-190`
- [ ] Otherwise wraps the selection, keeping leading/trailing spaces outside the delimiters, and keeps the wrapped text selected. `WriterCore/TextEditing.swift:165-172`
- [ ] Undo names: "Bold", "Italic", "Strikethrough". `Commands.swift:90-92`
- [ ] Link with no selection: inserts `[]()` with the caret inside `[]`. `WriterCore/TextEditing.swift:210-212`
- [ ] Link with a selection starting `http://`, `https://` or `www.`: `[](<selection>)` with the caret inside `[]`. `WriterCore/TextEditing.swift:213-215`
- [ ] Link with other selected text: `[<selection>]()` with the caret inside `()`. `WriterCore/TextEditing.swift:216-217`
- [ ] Heading 1/2/3, Bulleted List, Numbered List, Blockquote apply to every line the selection touches; each line's existing block marker is replaced. `WriterCore/TextEditing.swift:229-265`
- [ ] If every touched line already has that exact prefix (same heading level, any bullet, any number, any quote), the command removes the prefixes (toggle off). `WriterCore/TextEditing.swift:234-240`, `WriterCore/TextEditing.swift:252-253`
- [ ] Prefixes written: `# `/`## `/`### `, `- `, `1. ` `2. ` … numbered in order, `> `. `WriterCore/TextEditing.swift:255-261`
- [ ] Body removes any block prefix. `WriterCore/TextEditing.swift:252-253`
- [ ] Blank lines inside a multi-line selection are left as they are. `WriterCore/TextEditing.swift:247-250`
- [ ] Caret-only: caret moves by the prefix length change; with a selection, the whole rewritten block is selected. `WriterCore/TextEditing.swift:263`, `WriterCore/TextEditing.swift:267-272`
- [ ] Undo names: "Heading", "Body", "Bulleted List", "Numbered List", "Blockquote". `Commands.swift:102-108`
- [ ] Indent (⌘] or Tab in a list item): adds two spaces before every list item line touched; non-list lines unchanged; nothing happens (Tab inserts a tab) if no touched line is a list item. `Commands.swift:110-112`, `EditorController.swift:743-747`, `WriterCore/TextEditing.swift:68-109`
- [ ] Outdent (⌘[ or Shift-Tab in a list item): removes up to two leading spaces, or one leading tab, from each list item line. `Commands.swift:114-116`, `EditorController.swift:748-752`, `WriterCore/TextEditing.swift:82-89`
- [ ] Undo names "Indent", "Outdent". `Commands.swift:111`, `Commands.swift:115`
- [ ] Return in a bullet, numbered, task or quote line (caret after the marker, no selection) continues it: same indent plus same bullet character; next number with the same `.`/`)`; task as `<bullet> [ ] `; quote with the same `>` marker. `EditorController.swift:738-742`, `WriterCore/TextEditing.swift:24-66`
- [ ] Return on an item with no text removes the marker and indent (ends the list). `WriterCore/TextEditing.swift:38-42`
- [ ] Spaces right before the caret are dropped when continuing a list. `WriterCore/TextEditing.swift:60-62`
- [ ] Undo name "Typing" for list continuation. `EditorController.swift:740`
- [ ] Copy for Twitter (⇧⌘C): copies the selection, or the whole draft, as plain text; flashes "Copied selection as plain text · <n> characters" or "Copied draft as plain text · <n> characters" (number grouped, grapheme count). `Commands.swift:70-82`
- [ ] Plain-text export: headings, quotes and paragraphs lose their markers; bullets and tasks become "• "; numbered items keep "N. "/"N) "; indentation becomes two spaces per level (tab = 4 spaces); fences dropped; code-block lines kept as is; rules become blank lines; runs of blank lines collapse to one; trailing blank lines removed. `WriterCore/PlainTextExporter.swift:9-46`
- [ ] Plain-text inline rules: delimiters removed; images removed; links become "label (url)", or just the url when the label is empty, equals the url, or starts with "http"; backslash escapes removed (not when the line has code). `WriterCore/PlainTextExporter.swift:59-113`

---

## 10. Sidebar, Recent and Folder

### Panel

- [ ] Width 260 pt; background `sidebar`; 1 pt `hairline` on the right edge. `Sidebar.swift:58-66`, `Sidebar.swift:89`
- [ ] Hidden by default; pinned state persisted. `Preferences.swift:38-41`, `EditorController.swift:130`
- [ ] Pinning/unpinning sets the traffic lights fully opaque, shows chrome with auto-hide, and re-lays out. `RemoteEditing.swift:49-62`
- [ ] Pinning reloads the list. `RemoteEditing.swift:54-55`
- [ ] Segmented control "Recent" | "Folder", small size, IBM Plex Mono 11 pt, equal widths, at x 14, y 42, width panel−29, height 22; accessibility label "Sidebar view". `Sidebar.swift:73`, `Sidebar.swift:101-108`, `Sidebar.swift:160`
- [ ] Mode is persisted; default Recent. `Preferences.swift:43-46`, `Sidebar.swift:299-304`
- [ ] List rows: 42 pt with a subtitle, 28 pt without; no header; no grid; transparent background; no focus ring; accessibility label "Files". `Sidebar.swift:134-147`, `Sidebar.swift:341-343`
- [ ] List starts at y 74 (y 96 when the folder header shows) and fills the rest; overlay auto-hiding scroller. `Sidebar.swift:161-168`, `Sidebar.swift:148-153`
- [ ] Row of the open file (when not selected): `sidebarCurrent` rounded rect inset 8 pt horizontally, 1 pt vertically, radius 6. `Sidebar.swift:35-39`
- [ ] Selected row: `sidebarSelection` with the same shape. `Sidebar.swift:41-44`
- [ ] Row icon (11 pt, `quiet`): `arrow.turn.left.up` for "..", `folder` for folders, `network` for remote files, `doc.text` for local files. `Sidebar.swift:349-357`
- [ ] Row title IBM Plex Mono 13 pt, `text` for the open file, `text` α0.8 otherwise, truncates at tail. `Sidebar.swift:358-361`
- [ ] Two-line row: icon 14×14 at (18, 22), title at (38, 21), subtitle IBM Plex Mono 11 pt `quiet` at (38, 4), truncating in the middle. `Sidebar.swift:363-371`
- [ ] One-line row: icon at (18, 7), title at (38, 5). `Sidebar.swift:372-375`
- [ ] Row accessibility label "<title>, <subtitle>". `Sidebar.swift:378`
- [ ] Type-to-select by title. `Sidebar.swift:382-384`
- [ ] Single click on a row activates it. `Sidebar.swift:144-145`, `Sidebar.swift:310-314`
- [ ] Return or keypad Enter activates the selected row. `Sidebar.swift:49-56`, `Sidebar.swift:146`
- [ ] Activating a file deselects the row and opens it (remote files over SSH). `Sidebar.swift:325-327`
- [ ] After every list load the selection is cleared and the open file is re-marked. `Sidebar.swift:284-289`
- [ ] Empty-state message: IBM Plex Mono 12 pt, `quiet`, centred, wrapping, at listY + 24, 20 pt side margins, 60 pt tall. `Sidebar.swift:129-132`, `Sidebar.swift:170`

### Recent

- [ ] Up to 100 files: every draft-extension file in the drafts folder plus every remembered file that still exists, newest first by max(modified, last opened). `Sidebar.swift:192-197`, `WriterCore/DraftStore.swift:321-345`
- [ ] Row title is the remembered title (or remote name, or file name); subtitle is the remote location, or the parent folder with `~` for home. `Sidebar.swift:194-196`, `WriterCore/DraftStore.swift:340`
- [ ] Empty message "Files you open show up here." `Sidebar.swift:198`
- [ ] The Recent list refreshes after saves and document changes. `Sidebar.swift:187-190`, `EditorController.swift:372-374`, `EditorController.swift:354`

### Folder

- [ ] With no folder chosen: message "Open a folder to see its files here." and a centred small rounded "Open Folder…" button at listY + 90. `Sidebar.swift:204-211`, `Sidebar.swift:171-173`
- [ ] Header (only in Folder mode with a folder): IBM Plex Mono 11 pt `quiet`, truncating in the middle, at (16, 74), width panel−53; text is the folder name, or "root › sub › sub" when navigated into subfolders; tooltip is the full path (`~` abbreviated) or remote location. `Sidebar.swift:110-113`, `Sidebar.swift:161-166`, `Sidebar.swift:214-215`, `Sidebar.swift:15-20`
- [ ] Remote folder name is the last path component, or "<host> ~" for the home folder. `Sidebar.swift:8-13`
- [ ] Refresh button `arrow.clockwise`, borderless, `quiet` tint, 20×20 at (panel−33, 72), tooltip "Refresh", accessibility description "Refresh folder"; reloads the list. `Sidebar.swift:115-121`, `Sidebar.swift:166`, `Sidebar.swift:307`
- [ ] Lists folders and files ending md/markdown/txt/text (case-insensitive); hidden files skipped. `Sidebar.swift:238-243`, `Sidebar.swift:251-252`
- [ ] Sort: folders first, then Finder-style name order. `Sidebar.swift:253`
- [ ] When inside a subfolder, the first row is ".." (no subtitle); clicking it goes up one level. `Sidebar.swift:269-271`, `Sidebar.swift:318-320`
- [ ] Clicking a folder row goes into it. `Sidebar.swift:321-324`
- [ ] Empty folder message "No Markdown or text files here." `Sidebar.swift:263`
- [ ] Listing error: message is the error text, ".." row kept. `Sidebar.swift:247-249`
- [ ] Remote folder: shows "Loading <host>…" while listing in the background; a stale listing result (after navigating or reloading) is ignored. `Sidebar.swift:219-235`
- [ ] Only the root folder is persisted (local path or remote location); subfolder navigation is not. `Preferences.swift:48-63`, `Sidebar.swift:96`, `Sidebar.swift:293-297`
- [ ] File › Open Folder… (⌥⌘O): folder-only open panel as a sheet, button "Open Folder", message "Choose a folder to list its Markdown and text files in the sidebar.", starting in the current local root; on OK pins the sidebar and shows the folder. `RemoteEditing.swift:78-92`
- [ ] The "Open Folder…" button in the sidebar runs the same command. `Sidebar.swift:308`

### Peek

- [ ] With the sidebar not pinned, holding the pointer within 4 pt of the window's left edge (no mouse button down, window key) for 0.12 s, and still within 10 pt then, slides the sidebar in over the text. `ChromeControls.swift:171-199`
- [ ] Peek slides in from x −260 to 0 over 0.18 s ease-out (instant with Reduce Motion), with a black shadow, opacity 0.22, radius 18, offset (4, 0); list reloads; chrome shown with no auto-hide. `ChromeControls.swift:201-226`
- [ ] Peek closes 0.25 s after the pointer moves more than 296 pt (260 + 36) from the left edge; moving back cancels the timer. `ChromeControls.swift:174-186`
- [ ] Clicking in the text closes the peek. `EditorController.swift:183-186`
- [ ] Peek slides out over 0.16 s ease-in (instant with Reduce Motion), then chrome auto-hide restarts. `ChromeControls.swift:228-246`
- [ ] Clicking the sidebar button (or ⌃⌘S) while peeking pins the sidebar and saves it as shown. `ChromeControls.swift:249-258`, `RemoteEditing.swift:50`

---

## 11. Open picker (⌘O)

- [ ] Saves the current draft first, then shows a titled sheet, 520×420 pt. `Commands.swift:13-16`, `OpenPicker.swift:18-24`, `OpenPicker.swift:36`
- [ ] Search field at top (16 pt margins), IBM Plex Mono 14 pt, placeholder "Find a draft", no focus ring, accessibility label "Find a draft"; focused when the sheet appears. `OpenPicker.swift:37-41`, `OpenPicker.swift:76-79`, `OpenPicker.swift:95-98`
- [ ] List 10 pt under the search, 8 pt side margins, 12 pt above the buttons; rows 50 pt; accessibility label "Recent drafts". `OpenPicker.swift:44-55`, `OpenPicker.swift:80-83`
- [ ] Lists up to 200 drafts (drafts folder + remembered files), newest first. `OpenPicker.swift:91`, `WriterCore/DraftStore.swift:321`
- [ ] Row: title IBM Plex Mono Medium 13 pt; below, IBM Plex Mono 11 pt secondary-label colour, "<relative date> — <preview>", 3 pt apart, 10 pt side insets, vertically centred; both truncate at tail. `OpenPicker.swift:135-158`
- [ ] Relative date uses the short style ("2 hr. ago"); omitted when unknown. `OpenPicker.swift:141-143`
- [ ] Preview = plain text of the first 600 bytes without the first line, joined with spaces, up to 140 characters. `WriterCore/DraftStore.swift:392-399`
- [ ] Row accessibility label "<title>, <relative date>". `OpenPicker.swift:159`
- [ ] Filtering as you type: case-insensitive substring of title or preview, query trimmed of spaces. `OpenPicker.swift:100-103`, `OpenPicker.swift:114`
- [ ] Initial selection: the second row when the open draft is first, there is more than one row and the query is empty; otherwise the first row. `OpenPicker.swift:106-111`
- [ ] Empty states centred over the list, IBM Plex Mono 13 pt `quiet`: "No drafts yet" or "No matching drafts". `OpenPicker.swift:62-64`, `OpenPicker.swift:104-105`
- [ ] ↓/↑ in the search field move the selection (clamped). `OpenPicker.swift:116-131`
- [ ] Return opens the selected draft; Esc cancels. `OpenPicker.swift:120-121`
- [ ] Double-click a row opens it. `OpenPicker.swift:54`
- [ ] Buttons along the bottom (16 pt margins, 14 pt from bottom): "Other File…", "New Draft", flexible space, "Cancel" (Esc), "Open" (Return, default). `OpenPicker.swift:66-73`, `OpenPicker.swift:86-88`
- [ ] Other File… closes the sheet and shows the file open panel; New Draft closes and runs New. `OpenPicker.swift:175-176`, `Commands.swift:19-22`
- [ ] The chosen action runs after the sheet has closed. `OpenPicker.swift:163-168`

---

## 12. SSH and remote editing

- [ ] File › Open over SSH… (⌃⌘O): sheet alert, title "Open over SSH", text "Enter a file to edit or a folder to browse, like me@server:~/notes or ssh://me@server:2222/srv/notes/draft.md. Your SSH config and keys are used; password prompts aren't supported." `RemoteEditing.swift:96-100`
- [ ] Text field 340×24, placeholder "user@host:~/notes/draft.md", prefilled with the last location, initially focused, accessibility label "SSH location"; buttons "Open", "Cancel". `RemoteEditing.swift:101-108`
- [ ] Accepted forms: `host:path`, `user@host:path`, `ssh://[user@]host[:port]/path`; empty path means `~`; `ssh://host/~/x` means `~/x`. `WriterCore/Remote.swift:17-33`
- [ ] Rejected: empty host, host starting with `-`, host containing whitespace or any of `/'"`$;|&<>()\`, path containing newline or NUL. `WriterCore/Remote.swift:35-40`
- [ ] Invalid input: second sheet alert "That doesn't look like an SSH location." / "Use host:path, user@host:path, or ssh://user@host:port/path." `RemoteEditing.swift:112-118`
- [ ] A valid location is remembered for next time. `RemoteEditing.swift:119`, `Preferences.swift:65-68`
- [ ] Flashes "Connecting to <host>…" and checks in the background whether the path is a folder, file or missing. `RemoteEditing.swift:124-128`, `WriterCore/Remote.swift:113-121`
- [ ] Folder → pin the sidebar and show it in Folder mode. File → open it. Missing → alert "Nothing at <location>." Error → alert with the ssh error. `RemoteEditing.swift:131-141`
- [ ] Runs `/usr/bin/ssh -o BatchMode=yes -o ConnectTimeout=10 -o ServerAliveInterval=5 -o ServerAliveCountMax=2 [-p port] <host> <command>`, so `~/.ssh/config`, keys and agent apply and passwords are never prompted. `WriterCore/Remote.swift:86-100`
- [ ] Remote paths starting `~` stay unquoted so the remote shell expands them; everything else is single-quoted. `WriterCore/Remote.swift:61-70`
- [ ] SSH error "…Permission denied…" → "<host> refused the login. Fluent Writer can't type passwords, so use an SSH key or ssh-agent (ssh-copy-id <host>)." `WriterCore/Remote.swift:176-178`
- [ ] SSH error "Host key verification failed" → "<host> isn't a known host yet. Run `ssh <host>` in Terminal once to trust it." `WriterCore/Remote.swift:179-181`
- [ ] No stderr → "ssh to <host> failed (exit <status>)."; otherwise "<host>: <last stderr line>" (ignoring "Warning: Permanently added" lines). `WriterCore/Remote.swift:173-184`
- [ ] ssh can't start → "Couldn't start ssh: <reason>". `WriterCore/Remote.swift:148-150`
- [ ] Opening a remote file that is already open just focuses the editor. `RemoteEditing.swift:147`
- [ ] Opening a remote file runs the leave check, flashes "Opening <name> from <host>…" and reads it in the background; only the latest open request is applied. `RemoteEditing.swift:146-160`
- [ ] Remote text is kept in a local mirror under `<support>/Remote/<host>[-<port>]/home|root/<path components>`. `WriterCore/DraftStore.swift:350-359`
- [ ] On a successful read the mirror is overwritten with the server's text, unless the mirror has saved edits that never reached the server; those are kept and the app flashes "Kept your edits that hadn't reached <host>" and uploads them 0.2 s later. `WriterCore/DraftStore.swift:366-379`, `RemoteEditing.swift:166-169`, `EditorController.swift:339`
- [ ] Remote file title is the remote file name unless the writer named it. `WriterCore/DraftStore.swift:376`
- [ ] If the server can't be reached and this file was opened before (record and mirror exist), the mirror opens and the footer shows "Offline · editing the local copy"; otherwise an error alert. `RemoteEditing.swift:170-176`
- [ ] Each save writes the mirror first, then uploads in the background. `EditorController.swift:369-371`, `WriterCore/DraftStore.swift:201-204`
- [ ] Upload writes `<path>.fwtmp-<8 hex>` beside the target, then `mv -f` over it; on failure the temp file is removed. `WriterCore/Remote.swift:106-111`
- [ ] One upload at a time; while one runs, only the newest pending text is queued. `RemoteEditing.swift:9-35`
- [ ] The "not yet uploaded" flag clears only if the uploaded text is still the last saved text; the flag is persisted per file. `WriterCore/DraftStore.swift:382-390`
- [ ] Leaving a remote file with an un-uploaded save uploads it synchronously first; on failure a warning alert: "Your latest edits aren't on <host> yet." / "<error>\n\nThey're saved on this Mac and will be sent the next time you open this file.", buttons "Keep Editing" (stay), "Continue" (leave). `RemoteEditing.swift:194-211`
- [ ] Show in Finder is disabled for remote files. `Commands.swift:182`
- [ ] Remote files show a `network` icon in the sidebar. `Sidebar.swift:353`
- [ ] Remote folder listing runs `cd -- <path> && ls -1Ap`; entries starting with `.` are skipped; trailing `/` marks folders. `WriterCore/Remote.swift:123-137`

---

## 13. Images

### Inline previews

- [ ] A paragraph line that holds only an image (`![alt](src)` or `![alt](<src>)`, optional title, up to 3 leading spaces) gets a framed preview below it. Lines in code blocks don't. `WriterCore/MarkdownImages.swift:11-31`
- [ ] The Markdown line stays editable above the preview and is coloured `quiet`. `ImageBlocks.swift:168-169`
- [ ] Space for the preview is made with paragraph spacing after the line = preview height + 2 × gap; gap = `round(lineHeight × 0.45)`. `ImageBlocks.swift:135`, `ImageBlocks.swift:161-167`
- [ ] Preview position: x = text origin + hang width (prose edge), y = bottom of the line's last fragment + gap. `ImageBlocks.swift:201-206`
- [ ] Preview width = min(column width, natural width); height by aspect ratio; if taller than max height, height = max height and width scaled. Column width = `max(80, containerWidth − hangWidth)`; max height = `max(160, min(560, visibleHeight × 0.7))`. `ImageBlocks.swift:137-146`, `ImageBlocks.swift:179-185`
- [ ] Image with unknown size uses column width × 0.6 column width. `ImageBlocks.swift:180`
- [ ] Image is drawn clipped to a rounded rect (radius 6) with a 1 pt `hairline` border, high-quality interpolation. `ImageBlocks.swift:23-34`
- [ ] Placeholder: min(column, 420) × 40 pt, `codeBackground` fill, radius 6, SF Symbol 12 pt + message in IBM Plex Mono 11 pt, both in `text` α0.62, 12 pt left inset, 8 pt gap, truncated. `ImageBlocks.swift:35-51`, `ImageBlocks.swift:177-178`
- [ ] Remote (SSH) draft: placeholder `photo` "Images aren't shown for files opened over SSH". `ImageBlocks.swift:229`
- [ ] Unresolvable source: `questionmark.square.dashed` "Can't read image address “<source>”". `ImageBlocks.swift:230`
- [ ] Missing local file: `photo.badge.exclamationmark` "Image not found · <source>". `ImageBlocks.swift:233`
- [ ] Unreadable local file: `photo.badge.exclamationmark` "Can't open <filename>". `ImageBlocks.swift:236-238`
- [ ] Web image loading: `photo` "Loading <host>…". `ImageBlocks.swift:244-245`
- [ ] Web image failed: `wifi.exclamationmark` "Couldn't load image from <host>" (network error) or "Not an image · <host>" (data isn't an image). Failures are remembered for the session. `ImageBlocks.swift:243`, `ImageBlocks.swift:283`
- [ ] Sources resolve as: http/https/file URLs as is; other single-letter-or-longer schemes rejected; `/…` absolute; `~…` home; otherwise relative to the draft's folder (drafts folder for unsaved drafts); percent-encoding decoded. `WriterCore/MarkdownImages.swift:34-44`, `ImageBlocks.swift:152`
- [ ] Local images are downsampled to the column width × the screen's backing scale (pixels), honouring EXIF orientation; size in points uses the image DPI (72 if absent), so 2× screenshots show at on-screen size. `ImageBlocks.swift:234`, `ImageBlocks.swift:249-266`
- [ ] SVG and other formats ImageIO can't read are loaded with the generic image loader. `ImageBlocks.swift:267-269`
- [ ] Local images are cached and reused while the file's modification date is unchanged and the cached size is at least 90% of what's needed. `ImageBlocks.swift:235`
- [ ] Web images are fetched once per URL per session and the previews refresh when they arrive. `ImageBlocks.swift:272-288`
- [ ] Previews reflow as text is edited, resized or restyled. `ImageBlocks.swift:149-173`, `EditorController.swift:314`
- [ ] Previews outside the focused sentence/paragraph fade to α0.28. `ImageBlocks.swift:218-224`
- [ ] Clicking a preview puts the caret at the end of its Markdown line and focuses the editor (also on first click when the window isn't key). `ImageBlocks.swift:21`, `ImageBlocks.swift:54-59`
- [ ] Double-clicking opens the image in the default app, only for http/https links or existing local files whose type is an image. `ImageBlocks.swift:55`, `ImageBlocks.swift:82-90`
- [ ] Right-click menu: "Open Image" (when openable), "Show in Finder" (local files), "Copy Image" (when the image loaded; copies the full-resolution file for local images). No menu when the source didn't resolve. `ImageBlocks.swift:61-80`, `ImageBlocks.swift:94-99`
- [ ] Preview tooltip: local path with `~`, or the URL. `ImageBlocks.swift:211`
- [ ] Preview accessibility role image, label = alt text or "Image". `ImageBlocks.swift:101-102`, `ImageBlocks.swift:212`

### Adding images

- [ ] Paste (⌘V) with image files on the clipboard, or image data (PNG/TIFF) and no text, inserts images; if there's any text on the clipboard the text is pasted instead. `EditorTextView.swift:28-31`, `ImageBlocks.swift:300-310`
- [ ] Dragging image files or PNG/TIFF data onto the text shows a copy cursor and inserts at the drop point. `EditorTextView.swift:39-59`
- [ ] Image file extensions: png, jpg, jpeg, gif, heic, heif, webp, tiff, tif, bmp, svg. `WriterCore/MarkdownImages.swift:15`
- [ ] Format › Image… (⇧⌘I): open panel as a sheet, those image types, multiple selection, message "Images are copied into an images folder next to this draft." `ImageBlocks.swift:361-374`
- [ ] While a version preview is showing, adding an image beeps and does nothing. `ImageBlocks.swift:311`
- [ ] Remote drafts: flashes "Images can't be added to files opened over SSH yet". `ImageBlocks.swift:312-315`
- [ ] A file already inside the draft's folder (any depth) is linked by relative path, not copied. `ImageBlocks.swift:323-326`, `WriterCore/MarkdownImages.swift:47-52`
- [ ] Other files are copied into `images/` beside the draft (created if needed), renamed "name 2.ext", "name 3.ext" … if taken. `ImageBlocks.swift:327-331`, `WriterCore/MarkdownImages.swift:70-80`
- [ ] Pasted image data is saved as `images/Pasted image yyyy-MM-dd at HH.mm.ss.png` (TIFF converted to PNG), with empty alt text. `ImageBlocks.swift:333-341`
- [ ] Alt text from a file name: extension dropped, runs of `-`/`_` become a space, trimmed. `WriterCore/MarkdownImages.swift:64-67`
- [ ] Markdown inserted is `![alt](path)` with the path percent-encoded (spaces and parentheses escaped) and `[`/`]`/newlines removed from alt. `WriterCore/MarkdownImages.swift:55-61`
- [ ] Several images are separated by a blank line; a blank line is added before/after when the insertion point isn't already at a line start/end; the caret goes after the inserted text. `ImageBlocks.swift:347-352`
- [ ] Undo name "Insert Image" or "Insert Images". `ImageBlocks.swift:353-354`
- [ ] Flash "Image added · copied to the images folder beside this draft" / "<n> images added · …", or "… · linked where it already is" when nothing was copied. `ImageBlocks.swift:356-357`
- [ ] Copy failure flashes "Couldn't save the image: <reason>"; images already handled are still inserted. `ImageBlocks.swift:342-345`

---

## 14. Assist panel

### Panel

- [ ] Panel background `canvas`, 1 pt `hairline` on its left edge; accessibility group "Assist". `AssistController.swift:6-14`, `AssistController.swift:182-183`
- [ ] Created on first use. `AssistController.swift:857-864`
- [ ] Showing it adds it at the right edge (widening the window if narrow), selects the requested tab, refreshes the scope line and lists, and focuses the instruction field when on Ask. `AssistController.swift:377-391`
- [ ] Ask Codex / Ask Claude switch the provider and open on Ask. `AssistController.swift:385`, `AssistController.swift:870-871`
- [ ] Closing stops any running request, removes the panel and focuses the preview text (if previewing) or the editor. `AssistController.swift:393-400`
- [ ] Esc in the editor: clears the active comment if any; otherwise closes the Assist panel if open. `EditorController.swift:753-755`
- [ ] Chrome does not auto-hide on typing while the panel is open. `EditorController.swift:653`
- [ ] Panel padding 22 pt. `AssistController.swift:294`
- [ ] Provider pop-up at (18, 40), 120×24, borderless, IBM Plex Mono 12 pt, items "Codex", "Claude"; accessibility label "Provider". `AssistController.swift:185-191`, `AssistController.swift:296`
- [ ] Provider choice persisted under `assist.provider`; default Codex. `AssistController.swift:138`, `AssistController.swift:668-674`
- [ ] Close button: text "Close", borderless, IBM Plex Mono 12 pt, `quiet`, 50×20 at right edge − 22, y 42; accessibility label "Close assist". `AssistController.swift:193-199`, `AssistController.swift:297`
- [ ] Tabs: segmented "Ask" | "Comments" | "Versions", small, IBM Plex Mono 11 pt, equal widths, at (22, 72), full inner width, 22 tall; accessibility label "Assist section". `AssistController.swift:109`, `AssistController.swift:201-207`, `AssistController.swift:298`
- [ ] Tab labels show counts: "Comments <open count>" and "Versions <count>" when non-zero. `AssistController.swift:472-477`
- [ ] Tab content starts at y 106. `AssistController.swift:299`
- [ ] Switching tabs refreshes lists and toolbar button states. `AssistController.swift:423-427`

### Ask tab

- [ ] Scope line (IBM Plex Mono 12 pt, `text`, 18 pt tall, truncating): "Full draft · <n> word(s) → <Provider>". `AssistController.swift:212-213`, `AssistController.swift:465`
- [ ] With non-empty selected words: "Draft · focus on <n> selected word(s) → <Provider>". `AssistController.swift:460-463`
- [ ] While previewing a version: "Version “<label>” · <n> words → <Provider>". `AssistController.swift:449-453`
- [ ] Empty draft: "Write something first". `AssistController.swift:456`
- [ ] Scope accessibility labels: "Will send the full draft, <n> words, to <Provider>"; "Will send the draft, <n> words, focused on <m> selected words, to <Provider>"; "Will send the previewed version, <n> words, to <Provider>". `AssistController.swift:452`, `AssistController.swift:463`, `AssistController.swift:466`
- [ ] Scope updates on selection change (not while a request runs) and on provider change. `AssistController.swift:438-439`, `AssistController.swift:672`
- [ ] Privacy note (IBM Plex Mono 11 pt, `quiet`, wraps): "Nothing is sent until you press Send. The agent gets this draft, your selection and comments, with no files, commands or web access." `AssistController.swift:214-215`, `AssistController.swift:318-320`
- [ ] Quick-action buttons "Critique", "Clarify", "Shorten", "Stronger opening": recessed style, IBM Plex Mono 12 pt, 22 pt tall, fitted width + 8, 6 pt apart, wrapping to new rows 28 pt apart; accessibility label "<label> with the selected provider". `AssistController.swift:216-224`, `AssistController.swift:321-328`, `WriterAgents/AgentModels.swift:25-36`
- [ ] Quick actions and the provider pop-up are disabled while a request runs. `AssistController.swift:850-851`
- [ ] Instruction field: 44 pt tall, width inner − 74, rounded bezel, IBM Plex Mono 13 pt, wraps, placeholder "Ask anything, or say what to change or add", no focus ring; accessibility label "Instruction". `AssistController.swift:225-235`, `AssistController.swift:330`
- [ ] Return in the instruction field sends it (ignored when blank); Esc stops a running request, otherwise closes the panel. `AssistController.swift:693-703`, `AssistController.swift:684-687`
- [ ] Option-Return in the instruction field inserts a line break. **(AppKit built-in field-editor behaviour)**
- [ ] Send button 66×28, rounded, IBM Plex Mono 12 pt, no key equivalent; "Send" (accessibility "Send instruction") or "Stop" while running (accessibility "Stop request"); Stop cancels the request. `AssistController.swift:236-240`, `AssistController.swift:331`, `AssistController.swift:689-691`, `AssistController.swift:848-849`
- [ ] Status line (IBM Plex Mono 12 pt, wraps) under the field. `AssistController.swift:241`, `AssistController.swift:333-335`
- [ ] "Preview Version" small button appears under the status when the last reply's version still exists; it opens the preview. `AssistController.swift:257`, `AssistController.swift:336-342`, `AssistController.swift:656-658`
- [ ] "Feedback" header (IBM Plex Mono 11 pt, `quiet`) shown while running or when there is feedback; feedback below it, read-only selectable, IBM Plex Mono 13 pt, `text`, line height ×1.35, filling to 20 pt above the panel bottom; accessibility label "Feedback". `AssistController.swift:242-256`, `AssistController.swift:343-345`, `AssistController.swift:839-845`

### Sending

- [ ] The request uses the previewed version's text when previewing, otherwise the draft; nothing is sent if that text is blank. `AssistController.swift:705-709`
- [ ] Sending stops any earlier request, clears the result area, and flushes notes to disk. `AssistController.swift:710-712`
- [ ] The selection is sent as the passage to focus on only when not previewing and it isn't blank. `AssistController.swift:713-716`
- [ ] Ask (custom) and Send Comments include all open, still-attached comments with their replies; quick actions send none. `AssistController.swift:717-722`
- [ ] The last 4 Assist exchanges for this draft are sent as history. `AssistController.swift:724`
- [ ] Status while waiting: "<Provider> is reading the draft…", "… the version…", or "… <n> comment(s)…". `AssistController.swift:736-738`
- [ ] Send Comments switches to the Ask tab. `AssistController.swift:735`
- [ ] Feedback streams in as the reply arrives; status changes to "Drafting edits…" once the reply contains `<edit`. `AssistController.swift:751-753`, `WriterAgents/AgentModels.swift:213-228`
- [ ] Failure: feedback cleared, status shows the error in `warning`. `AssistController.swift:758-764`
- [ ] Stop (button, Esc, closing the panel, opening another document, quitting): "Stopped. Nothing was changed." `AssistController.swift:402-412`, `AssistController.swift:431-436`, `main.swift:40`
- [ ] A reply from a stopped/older request is ignored. `AssistController.swift:743-746`
- [ ] Replies to comments are added to those comments with the provider name as author. `AssistController.swift:774-776`
- [ ] Edits are placed on the text as it is when the reply arrives (typing during the wait is kept); duplicate matches pick the occurrence nearest the selection or the edit's comment. `AssistController.swift:777-783`, `WriterCore/EditPlacement.swift:69-90`
- [ ] If the edits change the text, a new version is created (draft untouched): label is the instruction (one line, 48 characters max: 47 + "…"), "Resolve <n> comment(s)", or the action label; "(from “<base label>”)" appended when based on a version; source is the provider; feedback attached; resolves the comments its edits named. `AssistController.swift:784-800`, `AssistController.swift:828-831`
- [ ] Feedback shown: the reply's notes; "No feedback." when there are no notes, no version and no replies. `AssistController.swift:802-803`
- [ ] Appended: "Answered <n> comment(s) — see Comments." when the agent replied to comments. `AssistController.swift:804-806`
- [ ] Appended: "<n> edit(s) couldn't be placed:" then for each "• <reason>\n  “<first 300 chars of new text>”". Reasons: "The edit didn't say what text to change.", "Couldn't find the text this edit changes. It may have been edited since.", "This edit overlaps another one." `AssistController.swift:807-812`, `WriterCore/EditPlacement.swift:80-89`
- [ ] Every completed request is added to the draft's Assist history (provider, action, instruction, feedback, version, comments). `AssistController.swift:814-815`
- [ ] With a version: status "<n> edit(s) ready as a new version, shown in the editor. ← → switch versions · ↩ use it · esc back to your draft." and the preview opens; otherwise the status clears. `AssistController.swift:816-823`
- [ ] Send Comments that got only replies (no version) switches to the Comments tab. `AssistController.swift:824`
- [ ] Opening a different document stops the request and clears the result. `AssistController.swift:431-436`, `EditorController.swift:353`

### Providers

- [ ] Codex runs `codex app-server` (found on PATH, Homebrew, /usr/local, ~/.local/bin, ~/.npm-global/bin, ~/.bun/bin, ~/.volta/bin, ~/.claude/local, ~/.codex/bin, or a login-shell lookup) in an empty temporary folder, ephemeral thread, read-only sandbox, approval "never". `WriterAgents/CodexAgent.swift:25-60`, `WriterAgents/CodexAgent.swift:121-143`, `WriterAgents/LineProcess.swift:92-122`
- [ ] Codex declines every command, file-change, permission and elicitation request. `WriterAgents/CodexAgent.swift:185-198`
- [ ] Codex errors: "Codex isn't installed. Install it with `npm install -g @openai/codex`, then sign in with `codex login`."; sign-in errors → "Codex isn't signed in. Run `codex login` in Terminal, then try again."; "Codex stopped unexpectedly: <last stderr line>"; "Couldn't start Codex: <reason>"; "Codex didn't start a conversation."; interrupted → "Stopped." `WriterAgents/CodexAgent.swift:34`, `WriterAgents/CodexAgent.swift:47`, `WriterAgents/CodexAgent.swift:50`, `WriterAgents/CodexAgent.swift:133`, `WriterAgents/CodexAgent.swift:172`, `WriterAgents/CodexAgent.swift:200-206`
- [ ] Claude runs the bundled Node bridge (`Resources/bridge/claude/bridge.mjs`, or the repo copy) with `node`, in an empty temporary folder, passing the `claude` binary's path in `FLUENT_WRITER_CLAUDE_PATH`. `WriterAgents/ClaudeAgent.swift:13-56`
- [ ] Claude errors: "Claude needs Node.js. Install it from nodejs.org or with `brew install node`."; "The Claude bridge is missing from this build."; "Claude stopped unexpectedly: <detail>"; "Couldn't start Claude: <reason>"; "Claude reported an error." `WriterAgents/ClaudeAgent.swift:29`, `WriterAgents/ClaudeAgent.swift:33`, `WriterAgents/ClaudeAgent.swift:47`, `WriterAgents/ClaudeAgent.swift:50`, `WriterAgents/ClaudeAgent.swift:69`
- [ ] Bridge: one turn, no tools, no MCP servers, no setting sources, every tool request denied ("Writing assistance has no tools."), system prompt "You are a careful writing editor. You only read the text you are given and reply in the requested format.", streams text deltas. `bridge/claude/bridge.mjs:37-73`
- [ ] Bridge errors: "The Claude bridge dependencies are missing. Run `npm install` in bridge/claude."; sign-in → "Claude isn't signed in. Run `claude` in Terminal and sign in, then try again."; missing binary → "Claude Code isn't installed. Install it with `npm install -g @anthropic-ai/claude-code`."; aborted → "Stopped." `bridge/claude/bridge.mjs:18-35`, `bridge/claude/bridge.mjs:77`, `bridge/claude/bridge.mjs:86-88`
- [ ] Bridge cancel on SIGTERM, `{"type":"cancel"}` or stdin close; exits 50–100 ms after finishing. `bridge/claude/bridge.mjs:11-16`, `bridge/claude/bridge.mjs:91-106`
- [ ] Temporary agent folders are deleted after each request; child processes are terminated (SIGKILL after 2 s if still running). `WriterAgents/ClaudeAgent.swift:92-100`, `WriterAgents/CodexAgent.swift:217-225`, `WriterAgents/LineProcess.swift:59-74`
- [ ] Prompt format (feedback tag, edit/old/new tags, `at="end"`, `for="ID"`, reply tags, history, selection block) is built by `AgentPrompt.build`; Critique asks for no edits. `WriterAgents/AgentModels.swift:130-209`

---

## 15. Comments

- [x] Add Comment (⌥⌘M, toolbar, Comments tab button, context menu): target is the selection, or the current line when nothing is selected, trimmed of surrounding whitespace; beeps if empty. `CommentComposer.swift:118-126`, `CommentComposer.swift:138-140` Built in `comment_composer.rs` and `notes.rs` (`App::add_comment_command`); the toolbar, Comments-tab and context-menu entry points call it from their own modules. Rune has no system beep: beeps are counted in `Notes::beeps`.
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
- [x] Open, attached comments are highlighted with `commentHighlight` behind the text; the active one with `commentHighlightActive`. Display-only colour. `NotesController.swift:162-171` Drawn with `Span::bg`, which shifts with IME preedit (`El::text_highlight` does not). Rune draws span backgrounds as rounded boxes around the glyphs, not AppKit's square full-line-height fill.
- [x] Comments follow their text as it is edited; a comment whose text is deleted becomes detached (no highlight). `NotesController.swift:65-75` Rune reports no edited range, so `notes::edit_between` works it out from the text before and after, the selection before the edit and the caret after; `App::apply` passes its exact range.
- [x] A detached open comment re-attaches when its quoted text reappears (e.g. after undo). `NotesController.swift:98-107`
- [x] Comments re-anchor by their quote when a draft is opened. `NotesController.swift:43-44`
- [ ] Comments tab: "Add Comment" small button top-left (tooltip "Comment on the selection or the current line (⌥⌘M)"); "Show resolved" small checkbox top-right, IBM Plex Mono 11 pt, hidden when there are no resolved comments. `AssistController.swift:263-269`, `AssistController.swift:351-355`, `AssistController.swift:492`
- [ ] List shows open comments, plus resolved ones when "Show resolved" is ticked. `AssistController.swift:486`, `AssistController.swift:648-651`
- [ ] Empty text (centred, IBM Plex Mono 12 pt, `quiet`): "Select some text, or put the caret on a line, and press ⌥⌘M to leave a comment.\n\nWrite as many as you like, then send them all at once. The agent's fixes come back as a version you can preview." `AssistController.swift:273-275`
- [ ] Send button pinned 52 pt from the bottom, full width, 30 pt tall, IBM Plex Mono 12 pt: "Send <n> Comment(s) to <Provider>", or "Send Comments" when none; enabled only with attached open comments and no request running. `AssistController.swift:270-272`, `AssistController.swift:359-361`, `AssistController.swift:488-490`
- [ ] Cards: rounded rect radius 8, 8 pt apart, fill `card`, `cardHover` on hover, `cardSelected` when active; 1 pt border `hairline`, or `caret` α0.5 when active. `AssistController.swift:21-57`, `AssistController.swift:92`
- [ ] Resolved cards at α0.6. `AssistController.swift:24`, `AssistController.swift:533`
- [ ] Card quote “<text>” (newlines as spaces), IBM Plex Mono Italic 11 pt `quiet`, max 2 lines; detached adds " — no longer in the draft"; quote width leaves 72 pt for buttons. `AssistController.swift:535-536`, `AssistController.swift:561-562`
- [ ] Card body IBM Plex Mono 12 pt `text`; each reply "<author>: <text>" 11 pt `quiet`. 12 pt padding, 10 pt top, 4 pt after the quote, 6 pt after others. `AssistController.swift:537-543`, `AssistController.swift:554-574`
- [ ] Card buttons 18×18, top-right, 24 pt apart, `quiet`: resolve `checkmark.circle` "Resolve" (or `arrow.uturn.backward.circle` "Reopen" when resolved), `pencil` "Edit comment", `trash` "Delete comment". `AssistController.swift:544-547`, `AssistController.swift:568-572`
- [x] Clicking a card selects its text in the editor, scrolls to it (centres in focus modes), makes it active and closes any preview. `AssistController.swift:549`, `CommentComposer.swift:168-175` `App::reveal_comment`; the card is the Assist panel's.
- [ ] Card accessibility: role button, press = click, label "Comment on “<first 60 chars of quote>”: <body>". `AssistController.swift:54-56`, `AssistController.swift:550`
- [x] Edit opens the composer pre-filled, button "Save", no placeholder; not available for detached comments. `CommentComposer.swift:157-165` `App::edit_comment`; the card's pencil button is the Assist panel's.
- [x] Resolve/Reopen toggles the resolved state. `AssistController.swift:623-627` `App::set_comment_resolved`; the button is the Assist panel's.
- [x] Delete removes the comment immediately (no confirmation). `AssistController.swift:634-638` `App::delete_comment`; the button is the Assist panel's.
- [x] With the Comments tab open, moving the caret into a commented passage makes that comment active (shortest span wins when nested). `AssistController.swift:440-443`, `NotesController.swift:156-160`
- [x] Using a version resolves the comments that version addresses. `NotesController.swift:199-206`

---

## 16. Versions and the inline diff preview

### Versions tab

- [ ] Header text (IBM Plex Mono 11 pt, `quiet`): "Each agent reply becomes a version. Click one to preview it as a diff, then use it or go back. Using one keeps your current draft here too." `AssistController.swift:280-281`
- [ ] First card "Your draft" (Medium 12 pt) with "What you're editing now", or "Back to editing (→ or esc)" while previewing; selected when not previewing; click closes the preview. `AssistController.swift:576-587`
- [ ] Then one card per version, newest first: label (Medium 12 pt, 2 lines max, 20 pt narrower for the delete button); detail "<Saved draft | source> · <relative time>[ · used]"; feedback up to 3 lines, 11 pt `quiet`. `AssistController.swift:588-595`, `AssistController.swift:605-621`
- [x] Relative time: "just now" under 30 s, otherwise the short relative style. `VersionPreview.swift:330-335` `notes::relative_time`, in US English ("2 min. ago", "3 days ago").
- [ ] Version card delete button `xmark` 14×14 "Delete version"; deleting the previewed version closes the preview. `AssistController.swift:596-598`, `AssistController.swift:640-646`
- [ ] Clicking a version card previews it; the previewed card is selected. `AssistController.swift:579`, `AssistController.swift:600`
- [ ] Version card accessibility label "<label>, <detail parts joined by ', '>". `AssistController.swift:601`
- [ ] At most 40 versions per draft; past that the oldest unused version is dropped. `WriterCore/DocumentNotes.swift:168`, `WriterCore/DocumentNotes.swift:177-182`

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
- [ ] Under 620 pt wide, "Back to Draft" is hidden; under 460 pt "Use This Version" becomes "Use". `VersionPreview.swift:132-137` **Not ticked:** Built; not checked at those widths.
- [ ] Title and detail hidden when their width is under 48 pt; when under 140 pt the bar's tooltip shows title and detail. `VersionPreview.swift:150-152` **Not ticked:** Built; not checked at those widths.
- [x] Diff text uses the same column and horizontal inset as the editor, 40 pt top/bottom inset, 35% extra bottom space, no typewriter centring. `VersionPreview.swift:154-161`
- [x] Diff is word-level between the current draft and the version, with Markdown styling applied to the combined text. `VersionPreview.swift:242-270`
- [x] Inserted text: `inserted` colour on `insertedBackground`. `VersionPreview.swift:276-277`
- [x] Deleted text: `deleted` colour on `deletedBackground`, struck through in `deleted`. `VersionPreview.swift:278-282`
- [x] Line breaks inside changes are not painted. `VersionPreview.swift:306-320`
- [x] A space is added between a deletion and the insertion that follows when neither side has whitespace at the join. `VersionPreview.swift:251-254`
- [x] Word counts for +/− use the plain-text word count of each changed segment. `VersionPreview.swift:262-263`
- [ ] Opening a preview scrolls to the first change (or top when none) and focuses the diff. `VersionPreview.swift:180-181`, `VersionPreview.swift:322-328` **Not ticked:** Built (`preview_poll`) and focus checked in tests; the scroll to a change below the fold was not checked on a long draft.
- [x] Diff text is selectable and copyable but not editable. `VersionPreview.swift:69-71` A read-only `text_input`: typing, paste and cut beep and change nothing.
- [x] Keys in the preview: ← older, → newer, Esc back to the draft, Return/keypad Enter use; other keys behave normally. `VersionPreview.swift:6-20`
- [x] Timeline: versions oldest → newest, then your draft. ← from the draft opens the newest version; ← at the oldest beeps; → at the newest returns to the draft; → while not previewing beeps. `VersionPreview.swift:166-226`
- [x] Older/Newer/Use are also ⌥⌘←/⌥⌘→/⌥⌘↩ from anywhere. `MainMenu.swift:145-147`
- [ ] Opening a preview announces "Previewing <label>. Left and right arrows switch versions, Return uses it, Escape goes back." `VersionPreview.swift:185-188` **Not ticked:** Built as a polite live region (Rune has no announcement request); not checked with a screen reader.
- [ ] Diff accessibility: label "Version preview", value = the version's full text, help "Inserted text is green, removed text is struck through." `VersionPreview.swift:78`, `VersionPreview.swift:302-303` **Not ticked:** Set (`label`, `a11y_value`, `description`); not checked with a screen reader.
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

- [ ] Assist › Connect an Outside Agent… with no `fluent-writer-mcp` executable beside the app binary: sheet "The agent connector is missing from this build." / "Build the app with script/build-app.sh, which bundles fluent-writer-mcp." `ConnectAgent.swift:8-12`, `ConnectAgent.swift:33-39`
- [ ] Otherwise a sheet titled "Connect an outside agent" with text "Claude Code or Codex can then see the draft open here, your selection, comments, versions and Assist history. They can leave comments and propose versions, which you preview and choose here, the same as Assist's. Run one of these in Terminal, or click a button to do it now." `ConnectAgent.swift:43-45`
- [ ] Accessory 460×150: "Claude Code" heading (system 11 pt medium) and its command; "Codex" heading and its command; commands in IBM Plex Mono 10 pt, secondary colour, wrapping, selectable. `ConnectAgent.swift:46-62`
- [ ] Claude command: `claude mcp add --scope user <server name> -- '<server path>' '--support' '<support folder>'` (each argument single-quoted). `ConnectAgent.swift:16-22`
- [ ] Codex command: `codex mcp add <server name> -- '<server path>' '--support' '<support folder>'`. `ConnectAgent.swift:24-26`
- [ ] Server name `fluent-writer`, or `fluent-writer-dev` in the dev build. `ConnectAgent.swift:14`
- [ ] Buttons "Add to Claude Code", "Add to Codex", "Copy Commands", "Done". `ConnectAgent.swift:63-66`
- [ ] Add to …: runs the CLI with those arguments; success flashes "Added to <Claude Code|Codex>. Start a new <name> session to use it." `ConnectAgent.swift:69-72`, `ConnectAgent.swift:83-101`
- [ ] CLI not found: flash "<name> isn't installed". CLI can't start: flash "Couldn't run <tool>: <reason>". `ConnectAgent.swift:84-87`, `ConnectAgent.swift:110`
- [ ] CLI non-zero exit: sheet "<name> didn't add the connector." with the last 600 characters of its output, or "It exited with status <n>." `ConnectAgent.swift:102-107`
- [ ] Copy Commands copies "# Claude Code\n<claude cmd>\n\n# Codex\n<codex cmd>\n" and flashes "Copied the connect commands". `ConnectAgent.swift:73-77`

### What the app publishes and accepts

- [x] Live state file `<support>/Live.json` (pid, app name, path, remote location, title, selection, selected text, previewed version id), written 0.4 s after selection changes, comment adds, preview changes and document loads. `NotesController.swift:237-252`
- [x] Live state is deleted on quit; readers treat it as absent when the pid isn't running. `NotesController.swift:254-257`, `WriterCore/DocumentNotes.swift:308-312` Deleted when the app's state is dropped (window closed); a killed process leaves it, and readers ignore it because the pid is gone.
- [x] The app watches `<support>/Inbox/` and applies each command file (oldest first), deleting it and writing `Inbox/Results/<id>.json`. `NotesController.swift:261-308`, `WriterCore/DocumentNotes.swift:316-336` Watched with the `notify` crate; reading, deleting and answering run on the notes worker thread.
- [x] Commands for the open draft apply to the live text and notes; a new version flashes "<agent> proposed a version · ⌥⌘V to see it"; a new comment flashes "<agent> added a comment". `NotesController.swift:279-293`
- [x] Commands for other files apply to that file's notes under the notes lock; if the file can't be read the result is "Couldn't read <path>." `NotesController.swift:294-305`
- [x] The inbox is also processed whenever a document loads. `NotesController.swift:49`

### fluent-writer-mcp server

- [ ] Usage `fluent-writer-mcp [--support <dir>] [--variant standard|dev]`; `--help`/`-h` prints usage and exits 0; default support folder is the standard app's. `FluentWriterMCP/main.swift:13-24`
- [ ] Speaks JSON-RPC 2.0, one message per line on stdio; accepts batches; ignores blank or unparsable lines. `FluentWriterMCP/main.swift:403-406`
- [ ] `initialize`: echoes the client's protocol version (default "2025-06-18"), tools capability, server info name "fluent-writer", title "Fluent Writer", version "0.2.0", and the instructions text. `FluentWriterMCP/main.swift:371-382`, `FluentWriterMCP/main.swift:357-363`
- [ ] Agent name for comments/versions: "Claude Code" if the client name/title contains "claude", "Codex CLI" if "codex", else the client's title or name, else "Outside agent". `FluentWriterMCP/main.swift:27`, `FluentWriterMCP/main.swift:372-376`
- [ ] `ping` → empty result; `notifications/*` ignored; unknown methods → error -32601 "Method not found: <method>". `FluentWriterMCP/main.swift:383-400`
- [ ] Tool errors return `isError: true` with the message text. `FluentWriterMCP/main.swift:387-396`
- [ ] Tools: get_context, read_document, list_comments, add_comment, reply_to_comment, resolve_comment, propose_version, list_versions, read_version, get_assist_history, list_documents, with the descriptions and input schemas in the source. `FluentWriterMCP/main.swift:139-228`
- [ ] `path` argument must end md/markdown/txt/text ("`path` must be a draft (.md, .markdown, .txt or .text)."); without it, the open draft is used; errors when the open draft isn't saved yet or the app isn't running (exact strings in source). `FluentWriterMCP/main.swift:56-69`
- [ ] get_context: app running/not, document path/title/words/lines/remote, selection text+line or caret line, open comments, up to 8 versions, previewed version, last 5 Assist exchanges, how_to_edit; recent 10 documents when no saved draft is open. `FluentWriterMCP/main.swift:238-269`
- [ ] read_document returns the file text; `line_numbers: true` prefixes "%5d  ". `FluentWriterMCP/main.swift:271-276`
- [ ] list_comments: open (or all with include_resolved); "No open comments on this draft." / "No comments on this draft." `FluentWriterMCP/main.swift:278-284`
- [ ] Comment JSON: id, quote, comment, author, created, status, line (or note "The quoted text is no longer in the draft."), replies. `FluentWriterMCP/main.swift:76-92`
- [ ] add_comment / reply_to_comment / resolve_comment / propose_version go through the app's inbox when it's running (waits up to 4 s, else "Queued. Fluent Writer hasn't picked it up yet; it will apply it shortly."), or apply directly with " (Fluent Writer is closed; the author will see this next time they open the draft.)" appended. `FluentWriterMCP/main.swift:116-135`
- [ ] propose_version requires `edits` or `full_text` ("Pass `edits` or `full_text`."). `FluentWriterMCP/main.swift:318`
- [ ] Command result messages: "Added comment <id>.", "Resolved comment <id>.", "Replied to comment <id>.", "Created version <id>. The author can preview it in Fluent Writer and choose whether to use it." (+ failures), and errors "That text isn't in the draft.", "The text appears N time(s); occurrence must be 1–N.", "add_comment needs the quoted text and a comment.", "No comment with that id.", "The reply is empty.", "The version is identical to the draft." `WriterCore/DocumentNotes.swift:199-201`, `WriterCore/DocumentNotes.swift:356-405`
- [ ] Default version label "<agent> version". `WriterCore/DocumentNotes.swift:396`
- [ ] list_versions ("No versions yet."), read_version ("No version with that id. Use list_versions."), get_assist_history (limit 1–500, default 20; "No Assist requests for this draft yet."), list_documents (limit 1–500, default 20; "No recent drafts."). `FluentWriterMCP/main.swift:322-348`
- [ ] Numeric arguments are rounded and clamped; NaN/∞ ignored. `FluentWriterMCP/main.swift:48-52`

---

## 18. Preferences and what persists where

- [ ] User defaults `focus` (off|sentence|paragraph|typewriter), default off. `Preferences.swift:7-10`
- [ ] `lastDimmingFocus`, default sentence. `Preferences.swift:13-16`
- [ ] `textSize` (double), default 19. `Preferences.swift:18-21`
- [ ] `appearance` (system|light|dark), default system; applied at launch. `Preferences.swift:23-26`, `main.swift:9`
- [ ] `showCounts`, default true. `Preferences.swift:28-31`
- [ ] `spellCheck`, default true. `Preferences.swift:33-36`
- [ ] `showSidebar`, default false. `Preferences.swift:38-41`
- [ ] `sidebarMode` (recent|folder), default recent. `Preferences.swift:43-46`
- [ ] `folderLocal` (path) or `folderRemote` (location string), mutually exclusive. `Preferences.swift:48-63`
- [ ] `lastSSHLocation`, default empty. `Preferences.swift:65-68`
- [ ] `assist.provider` (codex|claude), default codex. `AssistController.swift:138`, `AssistController.swift:670`
- [ ] Window frame under `NSWindow Frame FluentWriterMain`. **(AppKit built-in)** `EditorController.swift:146`
- [ ] Grammar, autocorrect, smart quotes, smart dashes and text replacement toggles are not saved by the app and reset at launch. `EditorController.swift:164-167`
- [ ] `<support>/state.json`: per-file record (path, title, manual-title flag, selection, last opened, remote location, upload-pending flag) and last opened path. `WriterCore/DraftStore.swift:3-18`, `WriterCore/DraftStore.swift:292-317`
- [ ] `<support>/Recovery/<uuid>.json` recovery snapshots. `WriterCore/DraftStore.swift:218-222`
- [ ] `<support>/Remote/…` remote mirrors. `WriterCore/DraftStore.swift:85`, `WriterCore/DraftStore.swift:350-359`
- [ ] `<support>/Notes/<first 12 bytes of SHA-256 of the path, hex>.json` per draft: comments, versions, Assist history (max 200 exchanges); file deleted when empty; `Notes/.lock` for cross-process locking. `WriterCore/DocumentNotes.swift:168-169`, `WriterCore/DocumentNotes.swift:251-284`
- [ ] Notes save 0.6 s after a change, and immediately on document switch, send and quit. `NotesController.swift:82-96`
- [ ] Notes for a never-saved draft are kept in memory until it gets a file, then filed under that path. `NotesController.swift:41-42`, `NotesController.swift:52-63`
- [ ] Notes dates are ISO 8601 with milliseconds. `WriterCore/DocumentNotes.swift:224-249`
- [ ] `<support>/Live.json` and `<support>/Inbox/`, `Inbox/Results/`. `WriterCore/DocumentNotes.swift:215-218`
- [ ] Drafts themselves are plain Markdown/text files; nothing else is written into them. `README.md:49`, `README.md:101`

---

## 19. Accessibility labels and announcements

- [ ] Editor text: label "Draft", help "The draft text. Use the Format menu or Markdown to style text." `EditorController.swift:180-181`
- [ ] Title field label "Draft title". `EditorController.swift:215`
- [ ] Status label "Save status", value = its text. `EditorController.swift:229`, `EditorController.swift:487`
- [ ] Counts label "Counts", value = its text. `EditorController.swift:230`, `EditorController.swift:516`
- [ ] Sidebar button "Toggle sidebar"; toolbar "Add comment", "Comments", "Versions", "Assist", each with count value when non-zero. `EditorController.swift:77`, `ChromeControls.swift:71-74`, `ChromeControls.swift:63`
- [ ] Sidebar: "Sidebar view" (segmented), "Files" (list), "Refresh folder" (refresh button), rows "<title>, <subtitle>". `Sidebar.swift:107`, `Sidebar.swift:115`, `Sidebar.swift:147`, `Sidebar.swift:378`
- [ ] Open picker: "Find a draft", "Recent drafts", rows "<title>, <when>". `OpenPicker.swift:41`, `OpenPicker.swift:55`, `OpenPicker.swift:159`
- [ ] SSH field "SSH location". `RemoteEditing.swift:104`
- [ ] Assist: group "Assist"; "Provider"; "Close assist"; "Assist section"; quick actions "<label> with the selected provider"; "Instruction"; "Feedback"; Send/Stop "Send instruction"/"Stop request"; scope label sentences (section 14). `AssistController.swift:182-235`, `AssistController.swift:251`, `AssistController.swift:849`
- [ ] Cards: role button, activatable with VoiceOver press; comment and version card labels (sections 15, 16); icon buttons labelled with their tooltip text. `AssistController.swift:54-56`, `AssistController.swift:526`
- [ ] Comment composer text area "Comment". `CommentComposer.swift:57`
- [ ] Image previews: role image, label alt text or "Image". `ImageBlocks.swift:101-102`, `ImageBlocks.swift:212`
- [ ] Preview: bar role toolbar "Version preview"; diff "Version preview" with value and help; position "Version <n> of <total>"; chevrons described by their tooltips. `VersionPreview.swift:78`, `VersionPreview.swift:96`, `VersionPreview.swift:121-122`, `VersionPreview.swift:297`, `VersionPreview.swift:302-303`
- [ ] Every footer flash is announced (medium priority). `EditorController.swift:495`
- [ ] Opening a version preview is announced. `VersionPreview.swift:185-188`
- [ ] All animations (chrome fade, scroll, peek, window widen) respect Reduce Motion. `EditorController.swift:604`, `EditorController.swift:634`, `EditorController.swift:683`, `ChromeControls.swift:156`, `ChromeControls.swift:220`, `ChromeControls.swift:240`
- [ ] VoiceOver text navigation, rotor and keyboard access in standard controls. **(AppKit built-in)**

---

## 20. Dev variant and command-line flags

- [ ] Variant comes from `FWVariant` in Info.plist ("standard" or "dev"); a binary with no bundle id (e.g. `swift run`) counts as dev. `WriterCore/AppVariant.swift:9-13`
- [ ] Dev app name "Fluent Writer Dev" (menus, About, live state), drafts `~/Documents/Fluent Writer Dev`, support `~/Library/Application Support/Fluent Writer Dev`. `WriterCore/AppVariant.swift:15-26`
- [ ] Dev uses its own bundle id `com.sunkenintime.fluentwriter.dev`, so its user defaults are separate. `script/build-app.sh:17-21`
- [ ] Dev MCP server name `fluent-writer-dev`. `ConnectAgent.swift:14`
- [ ] App flag `--open <path>`: opens that file at launch. `main.swift:21-22`
- [ ] Env `FLUENT_WRITER_AGENT_FIXTURE=<file>`: Assist replays that file's text word by word (40 ms per word) instead of calling a provider. `WriterAgents/ClaudeAgent.swift:103-136`
- [ ] Env `FLUENT_WRITER_CLAUDE_PATH`: path to the Claude Code binary for the bridge (set by the app). `bridge/claude/bridge.mjs:54`, `WriterAgents/ClaudeAgent.swift:41`
- [ ] Env `FLUENT_WRITER_SSH_TEST_DIR=host:/path` runs the SSH round-trip test (tests only). `README.md:63`
- [ ] MCP flags `--support <dir>` (tilde expanded), `--variant standard|dev`, `--help`/`-h`. `FluentWriterMCP/main.swift:13-24`

---

## 21. Packaging

- [ ] `script/build-app.sh [debug|release] [--dev]`, release by default; any other argument prints "unknown argument: <arg>" and exits 2. `script/build-app.sh:3-16`
- [ ] Builds the `FluentWriter` and `fluent-writer-mcp` products. `script/build-app.sh:22-24`
- [ ] Output `build/Fluent Writer.app` or `build/Fluent Writer Dev.app`, replaced on each build; path printed at the end. `script/build-app.sh:25-26`, `script/build-app.sh:64`
- [ ] Bundle layout: `Contents/MacOS/FluentWriter`, `Contents/MacOS/fluent-writer-mcp`, `Contents/Resources/FluentWriter_FluentWriter.bundle` (fonts). `script/build-app.sh:27-30`
- [ ] `bridge/claude` (with its node_modules, minus `node_modules/.cache`) copied to `Contents/Resources/bridge/claude`. `script/build-app.sh:31-34`
- [ ] Info.plist: name/display name, bundle id `com.sunkenintime.fluentwriter` (`.dev`), executable FluentWriter, APPL, version 0.2.0, build 2, `FWVariant`, minimum macOS 14.0, high-resolution capable, principal class NSApplication. `script/build-app.sh:35-50`
- [ ] Document type "Markdown Draft", role Editor, rank Alternate, for `net.daringfireball.markdown` and `public.plain-text` (so the app appears in Open With). `script/build-app.sh:51-59`
- [ ] Ad-hoc code signing; signing failure ignored. `script/build-app.sh:63`
- [ ] Bundled fonts: IBM Plex Mono Regular, Italic, Bold, BoldItalic, Medium, SemiBold, with the OFL licence. `Resources/Fonts/`
- [ ] The app is a regular Dock app (activation policy regular). `main.swift:54`
- [ ] Requires macOS 14+. `Package.swift:7`

---

## Oddities

Things in the code that look like bugs, dead code or mismatches. Listed, not fixed. Decide for each whether the rewrite copies it.

1. ⌥⌘J is assigned twice: Edit › Find › Jump to Selection (`MainMenu.swift:70`) and Assist › Show Assist (`MainMenu.swift:139`). Which one fires depends on menu order.
2. The Versions toolbar badge is always set to 0, so it never shows a count, while the tooltip and the tab label do show counts (`ChromeControls.swift:138`).
3. Empty `if` block with only a comment in title editing (`EditorController.swift:806-808`).
4. A footer flash sets the status text but returns before `layoutFooter()` (`EditorController.swift:446-450`), so the status label keeps the width of the previous text (`EditorController.swift:265-270`). A long flash can be cut off, and a flash after an empty status (new draft) can be invisible, until something else re-lays out the footer. The transient path also skips updating the label's accessibility value (`EditorController.swift:487`).
5. The status tooltip is not cleared for "Edited", "Recovered unsaved changes" or flash messages, so it keeps the last path or error (`EditorController.swift:475-480`).
6. Format commands (Bold, Italic, headings, lists, Indent…) always edit the main text view, even while a version preview is showing and the draft is hidden, and even when focus is in the title field or the Assist instruction field (`Commands.swift:86-116`). Image insertion beeps during preview (`ImageBlocks.swift:311`); formatting doesn't.
7. "Use This Version" is disabled when the version equals the draft (`VersionPreview.swift:301`), but Return in the preview (`VersionPreview.swift:16`) and ⌥⌘↩ (`Commands.swift:190`) still use it.
8. Closing the window runs the leave check in `windowShouldClose` (`EditorController.swift:841-843`), then `windowWillClose` quits, and `applicationShouldTerminate` runs the leave check again (`main.swift:38-42`). With a failing save this can show the "couldn't be saved" alert twice.
9. `application(_:open:)` uses only the first URL and does nothing if the editor hasn't been created yet (`main.swift:34-36`). If macOS delivers the open event before `applicationDidFinishLaunching` (launching the app by opening a file), the file would be ignored and a new or recovered draft shown instead. Not verified at runtime.
10. Leaving a remote file with un-uploaded edits runs `ssh` synchronously on the main thread (`RemoteEditing.swift:194-198`); the UI freezes for up to the 10 s connect timeout plus transfer time.
11. If saving the current draft fails after a remote file has been fetched, the remote open is dropped with no message (`RemoteEditing.swift:163`).
12. A remote file's title is set to its file name on open (`WriterCore/DraftStore.swift:376`), but the first edit replaces it with the derived first-line title because the manual-title flag is false (`EditorController.swift:701-705`).
13. The Open picker lists remote mirror files and opens them with the local `open(url)` (`Commands.swift:19`), which marks them remote from the saved record but never fetches the server copy. The sidebar's Recent list uses `openRemote` instead (`Sidebar.swift:327`).
14. `commentsHeader` in the Comments tab is always set to "" (`AssistController.swift:493`), so it never shows.
15. Dead code: `Cached.pointSize` is stored but never read (`ImageBlocks.swift:119`); `_ = editor` (`AssistController.swift:502`, `VersionPreview.swift:225`); `totalShift` is computed then discarded (`WriterCore/TextEditing.swift:76-107`); `_ = tc` (`EditorTextView.swift:113`); `MarkdownImages.blockInsertion` is never called, and `insertImages` repeats its logic (`WriterCore/MarkdownImages.swift:83-89`, `ImageBlocks.swift:349-351`); `DraftLibrary.lastOpenedURL` and `hasRecovery` are never used (`WriterCore/DraftStore.swift:121-124`, `WriterCore/DraftStore.swift:228-230`).
16. `IBMPlexMono-SemiBold.otf` is bundled and registered but never used; `uiFont` maps every non-medium weight to Regular (`Theme.swift:111`).
17. The composer hint tooltip says only "Shift-Return adds a new line" (`CommentComposer.swift:75`), but Option-Return also does (`CommentComposer.swift:11`).
18. Right-clicking a commented passage makes that comment active as soon as the menu opens, even if the menu is dismissed without choosing anything (`EditorController.swift:777-779`).
19. The scope label's accessibility label is not updated in the "Write something first" case, so VoiceOver reads the previous sentence (`AssistController.swift:456`).
20. Ask Codex/Ask Claude/Show Assist menu items are disabled for an empty draft (`Commands.swift:186`), but the Assist toolbar button opens the panel anyway (`ChromeControls.swift:89-90`, `ChromeControls.swift:159-162`).
21. Codex is told the client version is "0.1.0" (`WriterAgents/CodexAgent.swift:57`); the app is 0.2.0 (`script/build-app.sh:45`).
22. README's connect command uses double quotes (`README.md:98`); the app builds it with single quotes and quotes `--support` too (`ConnectAgent.swift:16-22`).
23. Bigger Text's key equivalent is "+" (`MainMenu.swift:124`); on US layouts that means ⇧⌘= and plain ⌘= does nothing, though the README lists ⌘+.
24. Comments added through the inbox are appended without the position sort the app applies to its own comments (`WriterCore/DocumentNotes.swift:204` vs `NotesController.swift:123`), so list order can differ from text order.
25. Toggling the sidebar forces the traffic lights to full opacity (`RemoteEditing.swift:58`) even if the rest of the chrome is faded out.
26. The Info.plist has no app icon key (`script/build-app.sh:35-61`); the app uses the generic icon.
27. Save As uses an app-modal save panel (`EditorController.swift:408`) while every other panel in the app is a window sheet.
28. Typing any key in the editor hides the chrome, including arrow keys and ⌥/⌘ navigation, because the hook is on `keyDown` (`EditorTextView.swift:61-64`).
