# What to take from Raycast and PowerToys

Which of the two most complete toolboxes on the market hold something figghost does not have yet, and
which of those are possible on macOS for an ad-hoc signed, unnotarized app.

Sources: the Raycast manual and developer documentation, Microsoft Learn's PowerToys utility list and
the PowerToys release notes, and Apple's framework documentation. The figghost column was read from the
working tree on 16 October 2026 (the uncommitted restyle included), not from the README, which is stale
in one place (see the end).

## Already covered, so not a gap

| Feature | Where it already is |
|---|---|
| Text Extractor (read the text of a screen area) | `macos-sys/src/ocr.rs`: `VNRecognizeTextRequest` and `VNDetectBarcodesRequest`, called after an area capture from `app-ui/src/features/capture`. `macos-sys/examples/ocr_check.rs` draws Vietnamese and English text into a picture and reads it back. |
| Color Picker | The `vision` feature: `app-ui/src/features/vision`, `macos-sys/src/picker`. |
| FancyZones | `core-window/src/snap.rs` offers twelve layouts, `macos-sys/src/snap_ui.rs` hangs them from the notch while a window is dragged, `tiling.shortcuts` places a window from the keyboard, and `snap.zones` chooses which layouts appear. |
| Image Resizer | The shelf menu writes a PNG, JPEG or HEIC next to the original, and *Resize to half* and *Resize to a quarter* write a smaller copy in the same format next to it: `app-ui/src/shelf_actions.rs`. |
| Text Extractor for an image already held | *Copy text from the image* in the shelf menu reads a PNG, JPEG, HEIC or BMP on the shelf and copies its lines and any codes: `macos-sys/src/screen_capture.rs::Shot::from_file`, `app-ui/src/shelf.rs`. |
| Object removal | `macos-sys/src/vision.rs::remove_background`, macOS 14 and later, offered from the shelf menu as *Remove background*. |
| SoundSource, Boom 3D | `macos-sys/src/audio_tap.rs`, `mixer_ui.rs`, `core-audio-fx`. |
| CleanShot X (capture, scrolling capture, editor, recording) | `app-ui/src/features/capture`, `macos-sys/src/editor_ui.rs`. |

## Small: the plumbing is already there

| Feature | From | Entry point | What is missing |
|---|---|---|---|
| Paste transforms: plain text, a URL without its tracking parameters, JSON to YAML or CSV, base64 | PowerToys Advanced Paste | the clipboard popup, `core-clipboard` | Everything. These are pure functions, so the tests are cheap. Raycast has no equivalent. |
| Keep the Mac awake for a while | PowerToys Awake | new | Nothing in the tree: no `IOPMAssertion`, no `caffeinate`. A menu item and a timer. |
| Cursor helpers: shake to find the pointer, jump it to the next display, keep it on one display | PowerToys Mouse Utilities, CursorWrap | `core-mouse` | The event tap and the smoother are there, and `CGWarpMouseCursorPosition` is already used in `screen_capture.rs`. |
| Mute the microphone from anywhere | PowerToys Video Conference Mute | the mixer | CoreAudio is driven per app for output only; the input side is untouched. |
| System commands and a process list: sleep, lock, eject, empty the trash, list apps by memory and CPU and quit them | Raycast | new | Nothing uses `libproc` today. |
| `figghost://` links | Raycast Deeplinks and Quicklinks | `scripts/bundle.sh`, new | No `CFBundleURLTypes` today. |

## Medium

| Feature | From | Entry point | What is missing |
|---|---|---|---|
| Snippets: a keyword expands to text, with dates and the clipboard as placeholders | Raycast Snippets | `core-clipboard`, the event tap | Nothing today. The word "snippets" appears only as the name of a board in a test. This is the Raycast feature people use most. |
| A launcher in the notch: type once to open an app, find a file, find a clipboard item, expand a snippet, run a command, and do arithmetic or a conversion inline | Raycast root search, PowerToys Run and Command Palette | the notch window | Nothing today: nothing in the tree lists or opens apps, and nothing searches files. It would reuse the clipboard search, the conversions in `core-smart` and the notch itself, which is why it belongs here and not in a window of its own. |
| Zone grids of the user's own shape, one set per display, and an override per app | PowerToys FancyZones | `core-window/src/snap.rs` | `OFFERED` is a fixed list of twelve layouts and `snap.zones` is a single list for every screen. A grid editor, a set per display and a rule per app are the work. |
| Crop and Lock: a live copy of part of a window, small and always on top | PowerToys Crop and Lock | `macos-sys/src/screen_capture.rs` | The plumbing is already used: `initWithDesktopIndependentWindow` for a single window and `setSourceRect` for a crop. What is missing is a panel that keeps the stream alive and draws it. |
| Dictation: speak and have the text inserted where the cursor is | Raycast Dictation | new | `SFSpeechRecognizer` with on-device recognition (macOS 13 and later; `SpeechAnalyzer` from macOS 26). Needs the Speech Recognition permission. |
| Workspaces: save a set of windows and bring it back | PowerToys Workspaces | `macos-sys/src/ax.rs`, `workspace.rs` | Accessibility already reads and places windows. Nothing saves a set. |
| Notes | Raycast Notes | `core-clipboard` | SQLite with FTS5 is already there. |
| More MCP tools: a screenshot, the notch, a focus session, a snippet | Raycast MCP | `app-ui/src/mcp.rs` | Only `clipboard_search` and `clipboard_get` exist. The socket and the wire format are shared with the agents tab, so a third tool is cheap. |
| Sync settings and snippets through a folder the user chooses | Raycast Cloud | `app-ui/src/config.rs` | An iCloud container needs an entitlement this app does not carry; a folder the user picks does not. |
| A switcher for windows, with previews | PowerToys Window Hopper (0.101) | `macos-sys/src/switcher.rs`, `ax.rs` | macOS switches apps, not windows. Check what Window Hopper does exactly before planning this. |

## Do not do these

| Feature | Why not |
|---|---|
| Finder context menus: New+, PowerRename, the Quick Look add-ons, Image Resizer in Finder | A Finder Sync or Quick Look extension has to be Developer ID signed and notarized, and an ad-hoc signed app cannot load one. The shelf menu already plays this part; the actions belong there. |
| Always on top for a window of another app | No public API. Afloat and Floaty call the private SkyLight window server. Crop and Lock gives a live copy instead. |
| Blocking websites during a focus session | `NEFilterProvider` needs a paid team and a provisioning profile. `FamilyControls` and `DeviceActivity` need entitlements Apple grants case by case. Quitting the apps that distract is possible; blocking websites is not, for this app. |
| Muting the camera | No public API stops another app's camera. |
| Peek, Quick Accent, Grab And Move, Light Switch, Mouse Without Borders | macOS already has these: Quick Look, holding a key, Control-Command-drag, automatic appearance, Universal Control. |
| Environment Variables, Hosts File Editor, Registry Preview, Command Not Found | Windows only. |

## Order

1. The small ones that reuse plumbing: paste transforms, awake, the cursor helpers. Each is one file and
   one setting.
2. Snippets, then zone grids, dictation, MCP tools, Crop and Lock. Snippets and zone grids are the two
   things macOS users miss most from Raycast and FancyZones.
3. The launcher in the notch, then script commands as the way to extend it. The launcher is the largest
   piece and the one that meets Raycast directly, so it should come after the rest is stable.

## To check before planning

1. Checked on 16 October 2026, with `macos-sys/examples/ocr_check`: this system's Vision lists both
   languages the app asks for, `vi-VT` and `en-US`, so no wanted language is dropped here; the first
   recognition after a cold start took over twenty seconds while the model was loaded, and the ones after
   it 50 to 110 ms. What is still open is the same list on the oldest supported Mac.
2. Which of these need macOS 14 or 14.2. Process taps already need 14.2 and background removal 14, and
   the app runs from macOS 13, so every new feature has to say which release it needs.
3. What PowerToys Window Hopper (0.101) and Power Display (0.99) do. The sources found were thin.
4. How Raycast Focus blocks websites, if it does. A different answer would change the table above.
5. The README still lists object removal among what is left; the shelf has had it since the vision
   feature. One of the two is stale.
