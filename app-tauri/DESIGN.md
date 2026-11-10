# Design of the settings window

The window is a floating glass sidebar, a floating glass toolbar and one scrolling page of cards under them. It is
written for macOS 26, where the system draws Liquid Glass under the window and the page lays a veil of its own colour over
it. In a plain browser (`pnpm dev`) the same page paints a wallpaper gradient behind the veil, so that the glass can be seen.

## Rules

1. One accent colour, `--accent`, which is the system's own (`-apple-system-control-accent`) where the web view knows it
   and `#0a84ff` where it does not. State colours (`--good`, `--warn`, `--bad`) say a state and nothing else. No colour,
   gradient or glow belongs to a single page.
2. Glass is for what floats: the sidebar, the toolbar capsules, menus, sheets, the tips. A control that sits on a card has
   the look of glass (a lit rim, a soft drop, a highlight that follows the pointer) and no blur. Content is never glass:
   the cards are a pane a little lighter than the window, with a hairline.
3. Less text. A row is a name and at most one control. What the row would say in words is kept behind the "i" beside its
   name, and also in the page for the search and the reader of the screen (`.row-info`, hidden visually). A state is a
   `Pill` or one muted line (`below`). The small print under a card is one line with an "i" that opens it. A button that
   would only repeat the name of its row is a symbol, and its words are its tip.
4. Everything is on a 4 point grid and aligned to it: names at the left edge of the row, controls at the right edge.
5. Concentric shapes: window 26, panels 18, cards 18, menus 16, fields 10, tiles 9.
6. Motion is spring-eased and only on what is touched; `prefers-reduced-motion` and the system setting turn it off.
7. Text is `--fg` or `--fg-2` (about 4.7 to 1 on a card in the light theme, by calculation). `--fg-3` is for hints,
   placeholders and symbols, and is about 3 to 1: do not put a sentence in it. The system's Increase Contrast and Reduce
   Transparency are honoured (`html.increase-contrast`, `html.reduce-transparency`).

## Tokens

`src/styles/tokens.css`. Light and dark come from `light-dark()`, so a theme is a `color-scheme`; `html.light` and
`html.dark` force one. Tailwind reads the same variables through `@theme inline` (`bg-card`, `text-fg-2`, `border-hair`,
`ease-spring`), so a theme change reaches a utility without a new class.

| Group | Names |
| --- | --- |
| Colour | `--accent`, `--good`, `--warn`, `--bad`, `--fg`, `--fg-2`, `--fg-3`, `--hair`, `--hair-2` |
| Surfaces | `--veil`, `--card`, `--field`, `--glass-bg`, `--pop-bg`, `--panel-bg` |
| Size | `--row-h` 46, `--ctl` 28, `--ctl-sm` 24, `--ctl-lg` 34, `--col` 660 |
| Shape | `--r-window`, `--r-panel`, `--r-card`, `--r-pop`, `--r-field`, `--r-tile` |
| Motion | `--ease-out`, `--spring`, `--motion-fast` 140ms, `--motion` 240ms, `--motion-slow` 380ms |

The older names the style sheets of the pages use (`--text`, `--stroke`, `--tint`, `--group`, `--h-control` ...) are kept as
aliases at the foot of the file.

## Style sheets

Loaded by `src/styles/index.css`, in layers (`theme, base, components, utilities`), so that a utility on an element always
beats the kit. The style sheets of the pages (`src/pages/*/*.css`) are not in a layer and load with their page.

| File | What it holds |
| --- | --- |
| `tokens.css` | the tokens above |
| `base.css` | reset, type, scrollbars, reduced motion, the veil and the preview wallpaper |
| `glass.css` | `.glass` and `.glass-thick` (blur), `.glass-pop` (menus, sheets, tips), `.gl` and `.gl-tint` (the look without blur), `.scroll-edge` |
| `shell.css` | the sidebar, the toolbar, the search, the page, the hero, cards, rows, notes |
| `kit.css` | switch, segmented control, select, slider, fields, pills, keys, hotkey, code, app lists, toast, dialog, sheet |
| `legacy.css` | what a page draws by hand and no kit part covers, each block named for its page |

## The kit

`src/components/macos/`. A page imports from `Controls` (a barrel), `Layout`, `Common`, `Hotkey`, `Apps`, `Dialogs`.

- `Button.tsx`: one recipe (`tailwind-variants`) for every button. Kinds: `glass` (the everyday one), `prominent` (the one
  thing the window wants done), `tinted`, `plain`, `outline`, `danger`, `destructive`, `link`. Sizes `xs sm md lg`; shapes
  `capsule` and `rounded`. `icon`, `iconEnd` and `loading` are props. `IconButton` is a symbol with its words as a tip;
  `ActionIcon` and `ActionButton` also send an action to the backend; `SureButton` asks twice; `ButtonGroup` and
  `ToggleButton` for sets.
- `Switch`, `Segmented`, `Select`, `Slider`: built on react-aria-components; the segmented thumb and the switch knob are
  animated with Motion.
- `Overlay.tsx`: `Tip` (the words of a symbol), `InfoTip` (the "i"; with `text` it is a line that carries the "i"), and
  `MenuButton`.
- `Layout.tsx`: `Card`, `Row` (`desc` goes behind the "i"; `below` is a muted line; `lead` is a picture before the name; `block`
  puts the control on a line of its own), `SectionTitle` (`info` puts an "i" beside it), `Pill`. A `Card` whose last rows only
  explain draws them as one line each under it, unless explaining is all it has.
- `Common.tsx`: `PermissionRow` (a name, a pill that says granted or not, a way to the place that grants it), `AccessRow`,
  `ShortcutRow`, `TryRow`.
- `Icon.tsx`: Phosphor icons. A page names an icon with SF Symbol names (`gearshape`), a short kit name
  (`open`, `plus`, `refresh`) or `pg.<page>` for a page's own. An unknown name draws nothing and warns in development.

## In the AppKit app

The same page runs in the web view of the AppKit app (`crates/macos-sys/src/webview.rs`). What differs from the Tauri window:

- The page talks through `window.fig.send` and `window.__fig.recv` (`src/bridge/appkit.ts`), with the same messages.
- The window has no title bar to drag by, so a press on a `data-tauri-drag-region` asks the app to drag (`window.fig.drag()`),
  and a double click to zoom (`src/lib/drag.ts`).
- The window's glass is the app's own; the page knows it has it from the handler named `glass`, and lays its veil over it as
  in the Tauri window. The panes of glass that the earlier page asked for (`lay_glass`) are not used.
- A window that shows one canvas (the diagram of Network, the palette) is told so by `window.__canvas`, which the app writes
  into the page where `<!--__PRELUDE__-->` is.
- Nothing may be fetched (the content security policy is `default-src 'none'`), so `vite build --mode embed` puts the script,
  the styles, the fonts and the pictures into the one file. `</script` and `<!--` are escaped inside the script, and Vite's
  `__VITE_PRELOAD__` placeholder, which nothing replaces once there are no chunks, becomes `void 0`.
- A page that fails to draw (`PageBoundary`) says so in its place and writes the cause in the app's log
  (`settings page: page <id>: ...`), and the rest of the window goes on.

## The other surfaces

The small windows of the AppKit app and the window of the beta are pages of their own, in `src/surfaces/<name>/index.tsx`,
built one by one by `tools/build-embed.mjs` (see the README). They sit on the system's own glass or on a plain window, so their
style sheet, `src/styles/surface.css`, paints no background: the tokens, the reset and the materials are the settings
window's, the utilities are only those of `src/surfaces` and the button recipe, and a surface imports no part of the kit that
reads the store (the kit's `Button` recipe is in `button-style.ts` for that reason). A surface talks to the app through
`src/surfaces/host.ts`: `window.fig.send` out, `window.__fig.recv` in, and what only the system can draw (`window.__icon`, the SF
Symbols of the permissions as `window.__symbols`) written into the page by the app before it runs
(`crates/app-ui/src/web.rs`).

- The tray, the permissions window, the update popup and the news keep the way of their old pages: the same messages, the same
  words, a state on the system's glass. The colour tiles of the permissions and of the slides of the news are the accent now: no
  colour belongs to one page there either.
- The mail card is as black as the notch it comes from in both themes, and the widget is white on whatever its glass shows.
- The beta for Windows and Linux has its own fonts (Segoe UI, Inter, Cantarell), paints its own background (`html.opaque`), and
  has no glass.

## Adding to it

A page is `src/pages/<id>/index.tsx` with a default export; add it to `src/pages/registry.ts` and `meta.ts`. Use `Card`, `Row`
and the `*Row` helpers; put words in `src/i18n/shell.ts` only if the dictionaries `en.json` and `vi.json` do not have them (they hold
the same keys; a key is in both or in neither).

Preview: `pnpm dev`, then `/?theme=dark&lang=vi`. In development `window.__go('network')` opens a page, and `/?canvas=kit`
shows every button and control.

## Pitfalls met

- Tailwind scans the whole source for class names and turns any that are a utility into a rule. The pages use the class
  names `block` (a row whose control takes a line of its own) and `text-field`. `block` would become `display: block` and
  `text-field` would become a text colour (when a colour called `field` exists in the theme). `index.css` has
  `@source not inline("block")` and the theme has no `--color-field`. `python3 -I` over the built CSS can list the utilities
  that match a class a style sheet also defines; none should.
- A native `<select>` and `<textarea>` are still used in a few places (the diagram's side panel, the snippets, the browser
  page). They take the look of the kit from `kit.css`; they are not kit parts.
- The embedded browser used for previews may keep an old style sheet for a second after an edit: wait before taking a
  screenshot.
