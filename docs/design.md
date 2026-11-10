# Design

Where the look of the windows comes from, and how the AppKit app gets its pages.

## Where it comes from

Until 2.1.17 the window was "the ghost's world": a sky that followed the hour, handwritten titles, a ghost on every page, frosted glass cards, and a hero of its own two colours on each of 38 pages. In October 2026 it was found ugly, hard to use and "made by AI": its 38 pairs of page colours, gradients and glows are what the lists of AI-made interfaces name first, and the glass sat in the content layer, where the HIG says it does not belong (Materials: Liquid Glass is for navigation and controls; Sidebars: icons take the accent; Settings). The window was redrawn after System Settings of macOS 26. The research behind the redesign is in [research-ui-references.md](research-ui-references.md); the page that came of it was written again as `app-tauri` in 2.2, with the rules of its own look in `app-tauri/DESIGN.md`.

## Where it is

The look of every page is written in `app-tauri` (React, Tailwind) and its rules are in `app-tauri/DESIGN.md`: one accent
colour, glass only for what floats, a row that is a name and one control with the explanation behind an "i", a 4 point grid,
concentric shapes, text that reaches 4.5 to 1. This file is how the AppKit app gets those pages and how to look at them.

- `app-tauri/src/pages/<id>` is a page of the settings. `app-tauri/src/surfaces/<name>` is one of the other windows: the menu bar
  panel, the permissions window, the update popup, the news of a version, the card of a mail, the music widget, and the beta for
  Windows and Linux.
- `pnpm build:embed` in `app-tauri` writes each as one HTML file in `crates/app-ui/web` (and `crates/desktop/web`), and the
  program includes it with `include_str!`. `crates/app-ui/src/web.rs` writes what only the system can draw into a page before it
  runs. The file is committed; a change to `app-tauri/src` is not in the app until the file is built again.
- Liquid Glass: on macOS 26 the window is the system's glass under a veil of the window's own colour (`VEIL` in
  `crates/macos-sys/src/webview.rs`); a page that finds the handler named `glass` lays its own veil over it. The panels
  (`TrayPanel`, `DropPanel`, `WidgetPanel`) are the system's glass behind a transparent web view.

## Looking at it

No window is needed, and the glass is not drawn (it only exists on screen):

- `settings_preview snap <page> out.png [--dark] [--lang vi] [--size 1000x700] [--eval "script"]` draws a page of the settings
  with the sample state of `app-tauri/src/data/state.sample.json` (`--eval "document.querySelector('.page-foot .btn').click()"`
  opens its sheet).
- `tray_preview`, `update_preview snap <state|news>` and `widget_preview <style>` draw the others with what the app would send.
- Build the example again after `pnpm build:embed`: the page is compiled into it.
- `settings_window <page> out.png [--dark] [--size 1000x700]` opens the real window over a picture and checks that the window laid
  glass under the page. It is on screen, so leave the machine alone while it runs.
- In a browser, `pnpm dev` in `app-tauri` shows the settings with the sample state (`/?theme=dark&lang=vi`).

## Adding a page

A page of the settings is a folder `app-tauri/src/pages/<id>` with its component as the default export of `index.tsx`; its place
in the sidebar or the tabs is in `src/pages/meta.ts`, its words (`nav.<id>`, `page.<id>` and `page.<id>.sub`, the first sentence
of the last being its line at the top) are in `src/i18n/en.json` and `vi.json`, and its part of the state is in the model
(`crates/app-ui/src/settings/model.rs` and the feature's `settings.rs`) and in `app-tauri/src/data/state.sample.json`, which a
test of the feature keeps in step with the real state. Build the page with `pnpm build:embed`.
