# Design of the settings window

The window is a sidebar and one scrolling page of cards under a small toolbar. Every component is Astryx's
(`@astryxdesign/core`, with the `theme-neutral` theme), set as the theme has it. The app adds no look of its own: no glass, no
second palette, no component kit. The window used to be a Liquid Glass kit of the app's own (`src/components/macos`,
react-aria-components, a hand-written glass material); the kit and its style sheets (`glass.css`, `kit.css`, the old
`shell.css` and `tokens.css`) are gone.

## Rules

1. Colour, type, radius, shadow and motion come from the theme. A page that needs a colour takes an Astryx token
   (`--color-text-secondary`, `--color-border`, `--color-background-card` ...) or one of the names of `tokens.css`, which are
   only aliases of those tokens. No colour, gradient or glow belongs to a single page. Light and dark follow the system
   (`<Theme mode="system">`); `?theme=dark|light` pins one for looking at a page in a browser.
2. Less text. A row is a name and at most one control. What the row would say in words is kept behind the "i" beside its name
   (and written out, hidden, while the search reads the page). A state is a `Pill` (an Astryx `Badge`) or one muted line
   (`below`). A button that would only repeat the name of its row is an icon button with a tooltip.
3. Components are not restyled. If a part does not look right, the fix is its props or the theme, not a rule on its inside.
4. Motion is the theme's, and `prefers-reduced-motion` and the system setting turn it off (`html.reduce-motion`).

## Browsers

Astryx needs the Popover API (Safari 17), `light-dark()` (17.5, which the build lowers in the style sheet) and CSS anchor
positioning (Safari 26) for tooltips, menus and pop-ups. The product says macOS 13. On a WebKit older than 26 an overlay opens
correctly but may not sit beside what opened it; `pnpm exec astryx docs browser-support supporting-older-browsers` lists the
ways to mend that (a polyfill behind `@supports`, a positioning library, or accepting it). None is in yet, and an old WebKit has
not been tried.

## Layers and style sheets

`src/styles/index.css` declares `reset, theme, base, astryx-base, astryx-theme, components, utilities` (the order Astryx
documents), then imports Tailwind's theme and preflight, Astryx's reset and component sheet, the neutral theme, and Tailwind's
utilities, so that a utility on an element beats a component. What the app adds is in the `components` layer. The style sheets
of the pages (`src/pages/*/*.css`) are not in a layer and load with their page.

| File | What it holds |
| --- | --- |
| `fonts.css` | Inter, bundled (the theme's own font comes from Google Fonts, which a page that fetches nothing cannot use) |
| `tokens.css` | the names the style sheets of the pages use (`--fg`, `--hair`, `--card`, `--accent` ...), each an alias of an Astryx token, and the grid and shape the app uses |
| `theme.css` | what Tailwind may name (`text-fg-2`, `bg-card`), read from those names |
| `astryx.css` | what the app tells Astryx that its theme does not know: the font family |
| `base.css` | the window: no page scroll, no text selection but in fields, scroll bars, reduced motion |
| `shell.css` | the sidebar, the toolbar, the search results, the page and its hero |
| `ui.css` | the few classes the helpers of `src/ui` and the pages share |
| `pages.css` | what a page draws by hand and no Astryx part covers (the pad of Scrolling, the tiles of Mole, the style cards ...), each block named for its page |
| `surface.css` | the style sheet of the small surfaces that carry Astryx (see below) |
| `panel.css` | the style sheet of the three panels that carry none |

## `src/ui`

Pages call Astryx for what it has. `src/ui` holds the few parts that join Astryx to the state of the app, and the words the
pages already used for the common ones. Nothing in it draws anything of its own.

- `layout.tsx`: `Row` (`title`, `desc` behind the "i", `below` a muted line, `lead` a picture, `block` the control on a line of
  its own, `when` false greys it), `Card` (an Astryx `Card` of a `List`), `Block` (a line a page fills itself), `SectionTitle`,
  `Section`, `Pill`.
- `rows.tsx`: `ToggleRow`, `SelectRow`, `SegRow`, `SliderRow`: a row that names a setting by its path; what the control says is
  the value in the state, and what it does is `change(path, value)`.
- `controls.tsx`, `button.tsx`, `field.tsx`: `Button`, `IconButton`, `ActionButton`, `ActionIcon`, `SureButton`, `Switch`,
  `Select`, `Segmented`, `Slider`, `MenuButton`, `InfoTip`, `Keycaps`, `Field`. They take the words the pages already use (a
  name for a picture, `primary`, `small`, `disabled`) and hand Astryx its own. `button.tsx` leans on no store, so a surface that
  has none can use it.
- `dialogs.tsx`: `confirmDialog` (an Astryx `AlertDialog`), `Toast` (`useToast`), `Advanced` (the groups a page keeps for later,
  in a `Dialog`).
- `hotkey.tsx`, `apps.tsx`, `common.tsx`, `page.tsx`: the shortcut field and its recorder, the lists of apps, the rows most
  features have (`AccessRow`, `ShortcutRow`, `TryRow`), and the top of a page.
- `Icon.tsx`: Phosphor icons. A page names an icon with an SF Symbol name (`gearshape`), a short name (`open`, `plus`,
  `refresh`) or `pg.<page>` for a page's own. `glyph(name)` and `iconType(name)` give the same picture to an Astryx control.

## In the AppKit app

The same page runs in the web view of the AppKit app (`crates/macos-sys/src/webview.rs`). What differs from the Tauri window:

- The page talks through `window.fig.send` and `window.__fig.recv` (`src/bridge/appkit.ts`), with the same messages.
- The window has no title bar to drag by, so a press on a `data-tauri-drag-region` asks the app to drag (`window.fig.drag()`),
  and a double click to zoom (`src/lib/drag.ts`). The top of the sidebar leaves room for the window's traffic lights.
- The page paints a solid background of its own, so the glass the app puts under the window is not seen through it.
- A window that shows one canvas (the diagram of Network, the palette) is told so by `window.__canvas`, which the app writes
  into the page where `<!--__PRELUDE__-->` is.
- Nothing may be fetched (the content security policy is `default-src 'none'`), so `vite build --mode embed` puts the script,
  the styles, the fonts and the pictures into the one file. `</script` and `<!--` are escaped inside the script, and Vite's
  `__VITE_PRELOAD__` placeholder, which nothing replaces once there are no chunks, becomes `void 0`.
- A page that fails to draw (`PageBoundary`) says so in its place and writes the cause in the app's log
  (`settings page: page <id>: ...`), and the rest of the window goes on.

## The other surfaces

The small windows of the AppKit app and the window of the beta are pages of their own, in `src/surfaces/<name>/index.tsx`,
built one by one by `tools/build-embed.mjs` (see the README). They talk to the app through `src/surfaces/host.ts`:
`window.fig.send` out, `window.__fig.recv` in, and what only the system can draw (`window.__icon`, the SF Symbols of the
permissions as `window.__symbols`) written into the page by the app before it runs (`crates/app-ui/src/web.rs`).

- The permissions window, the update popup, the news of a version and the beta for Windows and Linux use Astryx and are
  mounted under its theme (`src/surfaces/mount.tsx`, `src/styles/surface.css`). A surface imports nothing that reads the
  store of the settings (`src/ui/button.tsx` is the button for that reason).
- The menu bar panel, the mail card and the music widget float on the system's own glass and are drawn with Tailwind alone, on a
  handful of colours (`src/styles/panel.css`). They use no component of Astryx, so they do not carry it: that would add about
  half a megabyte to each for the tokens.
- The beta for Windows and Linux has its own fonts (Segoe UI, Inter, Cantarell) and paints its own background (`html.opaque`).

## Adding to it

A page is `src/pages/<id>/index.tsx` with a default export; add it to `src/pages/registry.ts` and `meta.ts`. Use Astryx's
components, `Card`, `Row` and the `*Row` helpers; put words in `src/i18n/shell.ts` only if the dictionaries `en.json` and
`vi.json` do not have them (they hold the same keys; a key is in both or in neither). Find a component and its props with
`pnpm exec astryx component <Name>` and the rest with `pnpm exec astryx docs <topic>`.

Preview: `pnpm dev`, then `/?theme=dark&lang=vi`. In development `window.__go('network')` opens a page, and `/?canvas=kit`
shows every button and control.

## Pitfalls met

- The theme's `fonts.css` and the generated `astryx-themes.ts` import Google Fonts. Neither may be imported: the build and a Rust
  test refuse an address in a page. The type is the app's own (`astryx.css`).
- Astryx puts its tokens on the root element only once its `Theme` is mounted, so a style sheet that reads them (`tokens.css`)
  has nothing before the first render, and the three panels that mount no theme carry their own colours.
- A selector of the store that makes a new object on every call (`useApp((s) => new Set(...))`) never settles and ends in
  React error 185. Select a string or a number and build the object after.
- Astryx's `Button` and `IconButton` need a `label`; the helpers read it from the children. Astryx's `Text`, `Icon` and the
  rest are not the DOM's `Text` or the app's `Icon`: mind the import.
- The diagram of Network draws with the DOM directly (`src/pages/network/canvas`); its buttons are Astryx's drawn to markup once
  (`button()` in `dom.ts`) with the click put on the element.
- The embedded browser used for previews may keep an old style sheet for a second after an edit: wait before taking a
  screenshot. A snapshot of `settings_preview` can come out blank while the machine is idle, because CSS animations do not
  advance; pass `--eval` with a style that turns them off.
