# Figghost on Tauri

The pages of Figghost, written once as React 19 and TypeScript, with Tailwind 4 for the theme, react-aria-components for the
behaviour of the controls and Motion for the few things that move. They come in two forms:

- A Tauri 2 app of its own, a second program beside the AppKit one: the settings window with a Rust backend that holds the
  state and carries out the changes.
- The pages that the programs of this repository carry inside them. `pnpm build:embed` builds each into one HTML file, with
  its script, styles, fonts and pictures inline, beside the Rust code that includes it. The files are committed, so that a
  build of the apps needs no Node. They are build output: change `src` here and run `pnpm build:embed`.

| Surface | Source | File it builds | Opened by |
| --- | --- | --- | --- |
| settings, and its canvases | `src/main.tsx`, `src/pages` | `crates/app-ui/web/settings.html` | the settings, Network and palette windows of the AppKit app |
| tray | `src/surfaces/tray` | `crates/app-ui/web/tray.html` | the panel under the menu bar item |
| permissions | `src/surfaces/permissions` | `crates/app-ui/web/permissions.html` | the permissions window |
| update | `src/surfaces/update` | `crates/app-ui/web/update.html` | the popup that asks whether to update |
| whatsnew | `src/surfaces/whatsnew` | `crates/app-ui/web/whatsnew.html` | what is new in the version that just ran |
| outlook | `src/surfaces/outlook` | `crates/app-ui/web/outlook.html` | the card of a mail that fell from the notch |
| widget | `src/surfaces/widget` | `crates/app-ui/web/widget.html` | the music widget |
| desktop | `src/surfaces/desktop` | `crates/desktop/web/index.html` | the beta for Windows and Linux |

Nothing in this repository is hand-written HTML, JavaScript or CSS outside this folder, except the service worker of the
browser extension (`crates/app-ui/assets/browser-extension/background.js`), which a browser only takes as a script.

Status: it builds, type-checks, passes its tests, and every surface has been drawn headless in WebKit, the engine of the
AppKit app, in both themes and both languages. The settings window has also been run in the AppKit app. How the glass looks on
a colourful wallpaper, the other surfaces in their real windows, and the beta on Windows and Linux, have not been looked at
(`ASSUMPTIONS.md` has the list). The design is in `DESIGN.md`.

The settings window needs a WebKit that draws what Tailwind 4 writes: Safari 16.4, which is macOS 13.3. The product says
macOS 13; on 13.0 to 13.2 the pages have not been tried and may not look right. `light-dark()` is lowered by the build, so
the macOS 14.5 that it once seemed to need is not needed.

## What is in it

- 51 pages of the settings (the sidebar's, the tabs of the notch, About) and two windows of their own, the diagram of Network
  and the command palette, drawn by the same page told which canvas to show (`?canvas=network`).
- The protocol the settings page has always had: JSON messages with a `t` field in, the whole state out. The AppKit app and
  the Tauri backend both speak it. The dictionaries (2353 keys, English and Vietnamese) are in `src/i18n`.
- A backend that keeps the settings in a file, checks every change against the rules of the current app (ranges, words,
  shortcuts and their conflicts, snippets, the wires of Network), and, while a window is open, looks at the state every two
  seconds and tells the page when it has changed.
- A new look, written for macOS 26 (`DESIGN.md`): Liquid Glass under the window (`window-vibrancy` 0.8) with the page laying a
  veil of its own colour over it; a floating glass sidebar and toolbar; one accent colour; buttons in eight kinds and four
  sizes; Phosphor icons; and as little text as can be, with what a row would say kept behind an "i".

## What is not in it

The engines. The page shows the sample state of `src/data/state.sample.json` where a number or a name can only come from
an engine (the connections of Network, the accounts of the agents, the calendars, the clipboard count), and the actions that
need one answer with a fault ("not available in this version yet"). The list is at the end of `ASSUMPTIONS.md`. Also missing:
global shortcuts (a shortcut is checked and kept, not registered), the login item, the updater, the `figghost://` address,
choosing an app from the disk, and the usage descriptions of `scripts/bundle.sh`, which the bundle needs the day a feature
asks the system for the microphone, the camera, the calendar and so on.

## Layout

```
package.json  vite.config.ts  tsconfig.json  index.html
src/
  main.tsx                the entry of the settings
  app/                    the shell: App (and canvas mode), Sidebar, Toolbar, Search, PageHost
  surfaces/               the other pages: tray, permissions, update, whatsnew, outlook, widget, desktop; each folder has
                          an index.tsx (its entry) and, where it decides anything, a logic.ts that imports nothing, so that
                          tests/ can run it as it is; host.ts is the line to the AppKit app that they share
  bridge/                 the only place that knows how the page talks to the backend
    tauri.ts              invoke('fig_send') and listen('fig:recv'): the only use of invoke
    appkit.ts             window.fig.send and window.__fig.recv of the AppKit app's web view
    fixture.ts            a stand-in in a plain browser, with the sample state
    messages.ts           the messages, typed
  stores/                 the zustand store of the state pushed by the backend, and its hooks
  components/macos/       the kit: Button, Controls (a barrel), Switch, Segmented, Select, Slider, Overlay (tips and menus),
                          Layout, Page, Common, Hotkey, Apps, Dialogs, Icon, Keys
  pages/<id>/             one folder per page; Canvas.tsx beside index.tsx for a window of its own
  i18n/                   en.json and vi.json, shell.ts (words this shell adds), plugins.json (which features add a window or
                          a place on another page; not read at run time)
  data/state.sample.json  the sample state; crates/app-ui checks it against the real state
  styles/                 tokens, base, glass, shell, kit, legacy (see DESIGN.md); surface.css for the small surfaces
  lib/                    small helpers
src-tauri/
  Cargo.toml              its own workspace; depends on core-window by path
  tauri.conf.json  capabilities/default.json  Info.plist  icons/
  src/
    lib.rs                Backend, the command fig_send, the event fig:recv, the menu bar item, run()
    messages.rs           the messages from the page (the Rust side of bridge/messages.ts)
    store.rs              the state, the file, and what each message does to them
    rules.rs              the rules of the settings, with their tests
    windows.rs            the three windows, the theme, the Dock icon
    actions.rs            the actions the backend carries out
    glass.rs  native.rs   the only code that touches the system (Liquid Glass, Accessibility, open)
tools/                    build-embed.mjs (the pages for the apps), make-tray-icon.mjs, bundle-local.sh
tests/                    node:test files for the logic of the surfaces
```

The rules of the architecture it follows: `invoke` appears in one file; native code appears in two files of the backend; at
run time only the store and the shell use the bridge (a page imports its types, and the kit and the store); the levels of
glass are the window (system glass and the veil), the navigation (sidebar and toolbar capsules, menus and tips) and the
controls, and content is never glass.

TanStack Query, Router and Virtual, and React Hook Form with Zod are not used: the state is one object pushed by the backend,
the pages are kept in a trail in memory, and the lists are short. Add them where a page needs them.

## Running it

```
cd app-tauri
pnpm install
pnpm tauri dev        # the window, with the backend; the data goes to ~/Library/Application Support/vn.figghost.tauri
pnpm dev              # the page alone in a browser with the sample state; /?theme=dark&lang=vi, ?canvas=network, ?canvas=kit
pnpm typecheck        # the app, then the tests
pnpm test             # the logic of the surfaces, with Node's own test runner
pnpm build            # type-check, then the page into dist/
pnpm build:embed      # every page the apps carry, each as one HTML file; `node tools/build-embed.mjs tray` builds one
pnpm check:embed      # builds them again and fails if a committed file is not what the source makes (the build is deterministic)
cargo test --manifest-path src-tauri/Cargo.toml
tools/bundle-local.sh             # Figghost Tauri.app under src-tauri/target/release/bundle/macos, signed ad hoc, not notarized
```

`FIGGHOST_TAURI_HOME=/some/folder pnpm tauri dev` keeps the data of a run apart. If the Rust build complains that `../dist`
does not exist, run `pnpm build` once. The icon of the Tauri menu bar item is made by `node tools/make-tray-icon.mjs`.

To see a surface without opening a window, the programs in `crates/app-ui/examples` draw one in WebKit to a picture
(`tray_preview`, `update_preview`, `widget_preview`, `settings_preview`; build the example after `pnpm build:embed`, since the
page is compiled into it). The beta's page and the permissions and mail cards are drawn the same way by loading the built file
with a stand-in for `window.fig` and posting the messages the app would.

## The protocol

The page calls one command, `fig_send`, with a message, and listens to one event, `fig:recv`.

From the page: `ready`, `set {path, value}`, `app_rule`, `app_add`, `app_remove`, `ignored_add`, `ignored_remove`,
`hotkey {target, value}`, `recording {on}`, `action {name}`, `pick_app`, `running`, `icons`, `page`, `js_error`.

To the page: `state {state}`, `fault {code, detail}`, `running`, `icons`, `update`, `go {page}`, `chord`, `palette_result`.

After every change the backend answers with the whole state, so a refused change is undone on the page. A `set` of a list that
has messages of its own (apps, snippets, wires) is refused; `set` with a path such as `snippets.add` is an operation, not a
setting. The shortcuts are found by the names the page uses: `clipboard`, `keys:pin`, `tiling:left`, `smart`, `capture:area`,
`breaks:now` and so on (`src-tauri/src/rules.rs`, `slots`).

## Where the data is

`settings.json` in the data folder holds only what was changed, as `{ "version": 1, "overrides": { "<path>": <value> } }`. It is
read through the same rules as a change from the page, so a hand-edited value outside its range is brought in and an unknown
path is dropped. A file that cannot be read is moved to `settings.json.bad` and the app starts from the sample. This is a
folder of its own: the configuration of the current app is neither read nor written.
