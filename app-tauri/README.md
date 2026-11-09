# Figghost on Tauri

The settings window of Figghost, rebuilt as a Tauri 2 app: a React 19 and TypeScript page, Tailwind 4 for the theme,
react-aria-components for the behaviour of the controls and Motion for the few things that move, and a Rust backend that holds
the state and carries out the changes. It is a second program beside the current one. Nothing outside this folder was
changed: the repository's workspace, its `Cargo.lock` and the crates keep building as they did.

Status: it builds, type-checks, passes its Rust tests and runs as an app that opens its settings window; every page has been drawn
in a browser in both themes. How the glass looks on a colourful wallpaper, and the pages other than General in the real window,
have not been looked at (`ASSUMPTIONS.md` has the list). The design is in `DESIGN.md`.

## Why Tauri when the main README chose AppKit

The README of the repository chose AppKit on purpose, for Vietnamese typing in `NSSearchField`, for panels that do not take
focus, and for memory. None of that moves here. The notch, the panels and the event tap stay in `crates/macos-sys` and are
meant to be reached through an adapter later. What changes host is the settings window, which already was a web page in a
`WKWebView`: it becomes a Tauri window, and its code becomes typed components instead of one script per feature.

## What is in it

- 51 pages of the settings (the sidebar's, the tabs of the notch, About) and two windows of their own, the diagram of Network
  and the command palette, drawn by the same page told which canvas to show (`?canvas=network`).
- The protocol of the current page, unchanged: JSON messages with a `t` field in, the whole state out. One backend can serve
  this page and the old one, and the extracted dictionaries (2353 keys, English and Vietnamese) are the old ones. The styles
  are new.
- A backend that keeps the settings in a file, checks every change against the rules of the current app (ranges, words,
  shortcuts and their conflicts, snippets, the wires of Network), and, while a window is open, looks at the state every two
  seconds and tells the page when it has changed.
- A new look, written for macOS 26 (`DESIGN.md`): Liquid Glass under the window (`window-vibrancy` 0.8) with the page laying a
  veil of its own colour over it; a floating glass sidebar and toolbar; one accent colour; buttons in eight kinds and four
  sizes; Phosphor icons; and as little text as can be, with what a row would say kept behind an "i".

## What is not in it

The engines. The page shows the sample state of `crates/app-ui/web/fixture.json` where a number or a name can only come from
an engine (the connections of Network, the accounts of the agents, the calendars, the clipboard count), and the actions that
need one answer with a fault ("not available in this version yet"). The list is at the end of `ASSUMPTIONS.md`. Also missing:
global shortcuts (a shortcut is checked and kept, not registered), the login item, the updater, the `figghost://` address,
choosing an app from the disk, and the usage descriptions of `scripts/bundle.sh`, which the bundle needs the day a feature
asks the system for the microphone, the camera, the calendar and so on.

## Layout

```
package.json  vite.config.ts  tsconfig.json  index.html
src/
  main.tsx                the entry
  app/                    the shell: App (and canvas mode), Sidebar, Toolbar, Search, PageHost
  bridge/                 the only place that knows how the page talks to the backend
    tauri.ts              invoke('fig_send') and listen('fig:recv'): the only use of invoke
    fixture.ts            a stand-in in a plain browser, with the sample state
    messages.ts           the messages, typed
  stores/                 the zustand store of the state pushed by the backend, and its hooks
  components/macos/       the kit: Button, Controls (a barrel), Switch, Segmented, Select, Slider, Overlay (tips and menus),
                          Layout, Page, Common, Hotkey, Apps, Dialogs, Icon, Keys
  pages/<id>/             one folder per page; Canvas.tsx beside index.tsx for a window of its own
  i18n/                   en.json and vi.json (extracted), shell.ts (words this shell adds), plugins.json (which features
                          add a window or a place on another page; extracted, not read at run time)
  data/state.sample.json  the sample state (extracted)
  styles/                 tokens, base, glass, shell, kit, legacy (see DESIGN.md)
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
tools/                    extract-i18n.mjs, extract-state.mjs, make-tray-icon.mjs
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
pnpm typecheck
pnpm build            # type-check, then the page into dist/
cargo test --manifest-path src-tauri/Cargo.toml
tools/bundle-local.sh             # Figghost Tauri.app under src-tauri/target/release/bundle/macos, signed ad hoc, not notarized
```

`FIGGHOST_TAURI_HOME=/some/folder pnpm tauri dev` keeps the data of a run apart. If the Rust build complains that `../dist`
does not exist, run `pnpm build` once. Regenerating what was extracted from the old page:
`pnpm extract:i18n`, `pnpm extract:state`, `node tools/make-tray-icon.mjs`.

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
