# Design: macOS forms

How the settings window gets its look, and the rules to follow when adding to it.

## Where it comes from

Until 2.1.17 the window was "the ghost's world": a sky that followed the hour, handwritten titles, a ghost on every page, frosted glass cards, and a hero of its own two colours on each of 38 pages. In October 2026 it was found ugly, hard to use and "made by AI": its 38 pairs of page colours, gradients and glows are what the lists of AI-made interfaces name first, and the glass sat in the content layer, where the HIG says it does not belong (Materials: Liquid Glass is for navigation and controls; Sidebars: icons take the accent; Settings). The window was redrawn after System Settings of macOS 26. The approved drawing is [redesign-macos.html](redesign-macos.html), with the research behind it in its lower half and in [research-ui-references.md](research-ui-references.md).

## The rules

1. One colour. The accent of System Settings (`--tint`) marks the open page, the page icons, the switches and the sliders. A state is said in words in its system colour: green (`--good-text`) for granted or done, orange (`--warn-text`) for still to do, red for what deletes. Nothing else carries colour: no colours of pages, no gradients from one colour to another, no glows around controls. Where the parts of one whole must be told apart (Mole's bars and its disk chart), they are shades of the accent mixed with white, so the pale ones stay bright on the dark theme. A page icon is its SF Symbol, in the system's outline style and never the same as another page's (`PAGE_SYMBOLS`): in the accent in the sidebar, the General list and the search, as Finder draws its sidebar, white on a selected row, and white on a squircle of the accent, lit from above, at the top of its page.
2. One scale of shapes, concentric: window 26, sidebar 18 (26 less the 8 around it), group 12, page icon 9, sidebar item 8; controls are round and 24 points tall; a switch is 38 by 22.
3. One type, SF Pro (Inter only as the fallback): row names 13, descriptions 11, group names 13 semibold, the page's name 15 semibold in the toolbar.
4. Every page opens on the same row (`pageHead`): the page's symbol in white on the accent, its name, the first sentence of `page.<id>.sub`, and the switch of its feature, lifted out of the page so there is only one.
5. A row is a name, at most two lines of description and one control. A longer description folds behind More. Rows at the end of a group that only explain, with nothing to set, are small print under it (`tidy`).
6. Few places to look. The sidebar has 24 pages in six groups. The tabs of the notch have pages of their own (`TAB_PAGES`), opened from the one list of tabs on the Notch page or from General, and the sidebar marks the Notch page while one is open. A group few people need waits behind Advanced… in a sheet: a page of `app.js` wraps it in `advanced()`, a feature names its heading in `ADVANCED`.
7. Only what is touched or live moves, briefly: the controls, the sheet as it comes down, and the live light of Mole. Nothing moves under Reduce motion.
8. For everyone. Text in the window's greys and state colours is at least 4.5:1 in both themes, measured; text on the accent is the system's own pairing. Increase contrast gives the groups, the sidebar and the off switches a firm edge. Every control has a name, focus is always visible, and the sheet closes with Escape.

## Where it is

- `crates/app-ui/web/app.css`: the base part (the system's metrics and the plain controls) and the "macOS forms" layer on top of it, each with its tokens at its top.
- `crates/app-ui/web/app.js`: `NAV`, `TAB_PAGES` and `ADVANCED`; `pageHead`, `tidy`, `featureList`, `meBlock` and `renderNavItems`; the symbols of the pages in `PAGE_SYMBOLS` (each also listed in `SYMBOLS` of `src/settings/page.rs`, which draws them).
- Liquid Glass: on macOS 26 the whole window is the system's glass under a veil of the window's own colour (`VEIL` in `webview.rs`, 0.8 of the way to solid, so what is behind the window only shows faintly and the page stays easy to read), and every `[data-glass]` the page reports (`placeGlass` in `app.js`) is a pane of glass laid over it and under the page (`lay_glass` in `crates/macos-sys/src/webview.rs`): the sidebar and the toolbar's capsules (back and forward, the search). The groups stay solid, so what is read never sits on glass. The page leaves the glass clear; on an older system, in a preview or under Reduce transparency it paints a solid window itself.

## Looking at it

`settings_preview snap <page> out.png [--dark] [--lang vi] [--size 1000x700] [--eval "script"]` draws any page (`--eval "document.querySelector('.page-foot .btn').click()"` opens its sheet), and `settings_preview selftest` runs the page's own checks. Neither draws the glass, which only exists on screen: `settings_window <page> out.png [--dark] [--size 1000x700]` opens the real window over a picture and captures the two.

## Adding a page

Build it with the helpers of `app.js` (`card`, `row`, `toggleRow`, `sliderRow`, `selectRow`, `segRow`, `sectionTitle`). Put its id in `NAV` or `TAB_PAGES`, give it a glyph in `GLYPHS`, and write `nav.<id>`, `page.<id>` and `page.<id>.sub` in both languages; the first sentence of the last is its line at the top. A group that few people need goes in `ADVANCED`.
