# Assumptions

What was run, what was only read, and what nobody has looked at. The tools were run on macOS 27.2 (arm64) with Node 26.13,
pnpm 12.18 and Rust 1.99.

## What was run

- `pnpm typecheck`: TypeScript 7.0.2, strict, no errors. A deliberate mistake in a scratch file was caught.
- `pnpm build`: Vite 8.3.6 with `@vitejs/plugin-react` 6.3.0 and Tailwind 4.3.5, no errors. The page is 2.1 MB in `dist/`, of which
  677 KB is one chunk, the icons: Phosphor ships six weights in every icon and the bundler cannot take the others out. Taking the
  SVG of the one weight used from `@phosphor-icons/core` would cut most of it.
- `cargo check` in `src-tauri`: no errors and no warnings. `cargo test --lib`: 23 of 23 pass (rules, store, messages, native).
  `core-window` from the other workspace resolved. tao is 0.37.2 and Tauri 2.12.5, as locked in `src-tauri/Cargo.lock`.
- `tools/bundle-local.sh`: `pnpm tauri build --bundles app`, then an ad hoc signature whose requirement names the bundle id.
  The release build takes about a minute; the app is 8.7 MB, one arm64 binary of 7.9 MB. `codesign --verify --strict` passes.
  It is not notarized.
- The built app was started on a fresh data folder (`FIGGHOST_TAURI_HOME`). It stays up, uses about 110 MB of memory, opens the
  settings window at launch (1000 by 700 points, found in the window list of the system), and draws the General page in the
  system's WebKit with the real version (2.2.20) and the real Accessibility state. The traffic lights sit over the top of the
  sidebar. That window was captured to a picture and looked at, in the dark theme and in the light one.
- Every page was drawn in a browser (`pnpm dev`, Orca's embedded browser) from the sample state: all 51 in light and English, all 51
  in dark and Vietnamese, and 7 or 8 in each of the other two combinations; the diagram of Network and the palette in light and
  dark; and the kit (`?canvas=kit`). A script walked all 51 pages in both of the full combinations looking for text that leaves its
  column or is cut off; it found the Drag wheel's list, which was fixed. Walking all 51 pages and the kit produced no new
  warning or error in the console.
- Driven through the accessibility tree of the real window (System Events): the sidebar, a switch (the change reached the backend and
  `settings.json`, and the page kept it), a pop-up (Material, on Notch), the button that opens the diagram of Network (a second
  window opened, with its glass header and the traffic lights clear of it), and the close button (the process stayed, with no
  window and the menu bar item).
- Words: every `t('...')` key a page uses was checked against both dictionaries by a script (five prefixes built at run time were
  not); the sample state holds every setting path a page reads (except the operations the backend handles by name).

## What nobody has looked at

- The glass. The window is created with the system's Liquid Glass under a transparent web view (`window-vibrancy` 0.8.1,
  `apply_liquid_glass`), and the page lays a veil of 60 percent of its own colour over it. The call returned no error, and the
  window was seen over a mail window: a dark pane in the dark theme, a frosted white pane in the light one. How much of the
  backdrop comes through on a colourful wallpaper is not known; if it is too little, lower `--veil` in `src/styles/tokens.css`.
- The size of the corners. The window keeps the system's corner for a window without a toolbar (about 12 points); the design draws
  the floating sidebar for a corner of 26, so the two are not exactly concentric. A window gets the larger corner of macOS 26
  from an `NSToolbar`, which Tauri does not offer; an empty one added through `ns_window()` would do it.
- The mouse itself (the clicks above were made through the accessibility tree), dragging the window by the sidebar's empty space
  and the toolbar strip, Command-K and Command-F, the sheets, the keyboard order, VoiceOver, the palette window, and most of the
  51 pages in the real window: 7 were seen there (General, Shortcuts, Notch, Window snapping, Clipboard, Network and About) and
  the diagram of Network. The browser used for the other pictures is Chromium, not WebKit. `light-dark()`, `color-mix()`,
  `backdrop-filter` and `-apple-system-control-accent` are used; the pages seen show that WebKit draws them, and nothing else
  is known.
- Reduce Transparency, Increase Contrast, a display that is not Retina, Vietnamese in the real window.
- Quitting with Command-Q, opening the window again from the menu bar item and from the Dock.
- The Rust code that touches the system (`glass.rs`, `native.rs`, the tray and the windows) has no tests; the 23 tests are of the
  rules and the store.

## Assumptions about behaviour

- Liquid Glass needs macOS 26. On an older system `apply_liquid_glass` returns an error, the window gets no glass, and the page
  paints a solid window. `NSGlassEffectView` is private for the Mac App Store: a build for the store needs another window
  material. The `interactive` option works on macOS 27 and later only, and is not used.
- A transparent window on macOS needs `macOSPrivateApi` and the `macos-private-api` feature; both are set. The same makes the
  app unfit for the Mac App Store.
- The traffic lights are put at (20, 22) points with an overlay title bar, and the pages keep room for them: 84 pixels at the
  left of the diagram's header, the sidebar's top for the settings. The palette uses a title bar of its own instead.
- `data-tauri-drag-region` drags the window from `.dragzone`, the empty space of the sidebar, the diagram's header and the
  palette's header (`deep`, so a press on the text drags and a press on a button or a field does not). A double click zooms, through
  `core:window:allow-internal-toggle-maximize`, which `core:default` includes.
- A window made from a synchronous command on macOS is fine; on Windows it can deadlock (a note in Tauri's source). Make the
  command `async` if this is ever built there.
- The app is an accessory (no Dock icon) until a window is open, then regular, then an accessory again when the last window is
  destroyed, as the current app does; `LSUIElement` in `Info.plist` keeps the Dock icon from flashing at launch. Closing the
  last window is the only thing that sends `ExitRequested` with no code (tauri-runtime-wry 2.12.3), and the backend prevents
  that exit. Command-Q reaches the app as `terminate:`, which tao 0.37.2 turns into the end of the event loop without
  `ExitRequested`, so it quits; Quit in the menu bar item calls `exit(0)`, which has a code and is let through.
- The page is told the language the app runs in (`env.language`) from the setting or, for `auto`, the first language of the
  system read with `defaults read -g AppleLanguages`. The current app does the same, and also needs a restart for its menus.
- Data goes to `~/Library/Application Support/vn.figghost.tauri` (the bundle identifier), apart from the current app's folder.
  The two programs do not share their settings, and a second copy of the diagram's layout, the snippets and the like starts
  from the sample.

## What the page shows that nobody measured

The state starts from the sample of the old page's preview. Until an engine exists, these are samples, not facts: the count of
the clipboard, the shelf, the notch's tabs and screens, the apps of the scroll rules and the ignored apps, the agents and their
accounts, the calendars, the weather, the connections, wires and history of Network, the disk scan of Mole, the sources of Screen
Time. The facts the backend does know are filled in: the version, whether it runs from a bundle, the Accessibility permission,
the language, and whether the window has glass. `app.usage` is empty, so About says it is waiting for a reading.

## What is not wired, and what happens

An action the backend does not carry out answers with a fault, and the page says "That is not available in this version yet".
These work: `open_accessibility`, `check_permissions`, `open_camera_settings`, the four `*_screen_access`, `open_login_settings`,
`network_open_login`, `open_config`, `restart`, `quit`, `network_canvas_open` and `network_canvas_close`, `palette_close`,
`palette_page:<id>`, and `reset_tiling`. The rest do not: the checks and the install of updates, the hooks of the coding agents
and the accounts, the boards and the history of the clipboard, the scans of Mole, the downloads, the browser extension, the
weather, the launcher's pins, Dictation, Jarvis, the reminders and tabs of the palette, the diagram's speed test and tether, the
samples of Islands, and the rest of the 115 action names the pages send.

Also: `set` of `launch_at_login` answers "could not change the login item"; `set` of `jarvis.key` is refused (a secret waits for
the Keychain); `pick_app` is refused and `running` answers with no apps, so the "add app" menus are empty; `icons` answers with
none, so the lists of apps show no icon; a shortcut is checked and kept but not registered with the system.

## Found in the current app

The Breaks page asks for the shortcut of `breaks:now`, and its page reads `breaks.shortcuts.now`, while the state holds it as
`breaks.shortcut`. The shortcut it shows is therefore always empty there, even when one is set. This port reads it from where
the state keeps it (`shortcutOf` and `withShortcut` in `src/lib/shortcuts.ts`, and `slots` in the backend), which is a
difference from the old page, and a fix that the old page could take as well.

## Differences from the current app, on purpose

- The menu bar item is Tauri's tray with three entries (Settings, Command palette, Quit), not the full menu of the current app.
- The settings are the whole state of the sample, kept as a list of changed paths, not the typed `Config` of `app-ui`. The rules
  of the model are written again in `rules.rs` and have to follow it; the layouts of tiling and their default shortcuts are the
  crate's own (`core-window`). The other `core-*` crates are not used: `core-typing` brings the Vietnamese typing engine, which
  is fetched from git, and `core-mouse` depends on `macos-sys`.
- The address `figghost://`, the update, the login item and the global shortcuts wait for the adapter to `macos-sys`.
- `Info.plist` has `LSUIElement` and nothing else. The usage descriptions in `scripts/bundle.sh` (audio capture, Bluetooth,
  microphone, speech, camera, Apple events, calendars, reminders) have to be added with the first feature that asks the system
  for that access, or the system ends the app on the first request.

## To try next

1. Open `Figghost Tauri.app` (`tools/bundle-local.sh` makes it) on a colourful wallpaper, in both themes, and judge the glass;
   change `--veil` if it is too opaque.
2. Click through the pages in the real window; the ones with a native `<select>` or `<textarea>` (the diagram's side panel,
   Snippets, Browser) are the likeliest to differ in WebKit.
3. Close the last window and check that the menu bar item stays; open it again from the item and from the Dock.
4. The first feature that asks the system for the camera, the microphone, the calendar and so on needs its usage description in
   `src-tauri/Info.plist`.
