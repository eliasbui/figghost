# Design: the ghost's world

How the settings window gets its look, and the rules to follow when adding to it.

## Where it comes from

The 2026 Apple Design Awards (developer.apple.com/design/awards, and the developer interviews on developer.apple.com/news) have no single style: grug draws every letter of its own type and uses two flat colours, Tide Guide sets its palette to the colour of the sky through the day, Moonlitt is a quiet night that came alive the first time its makers tried Liquid Glass, and Is This Seat Taken? makes its interface a part of its story. What they share is what the jury says of the Visuals and Graphics category: "a distinctive and cohesive theme", and what the makers of grug said of their own work: if the handwriting had been used for one quote only, "grug would feel like a quote app with a cute font. Instead, it feels like its own little world." The same makers: "build with the platform, not against it ... then add your own point of view." The makers of Tide Guide: "the more time you spend with it, the more you can dig into different layers." The makers of Is This Seat Taken?: "Is this necessary? Does this add value? If the answer was no, we removed it."

Figghost takes those, not the look of any one app, and nothing is copied from an app: the ghost, the sky and the handwriting are drawn or chosen for this one.

## The rules

1. One place. The place is the ghost's night. The sky behind the page follows the hour (`html[data-sky]` is `dawn`, `day`, `dusk` or `night`, set by `applySky` in `app.js`; a state may name one in `env.sky`, which previews use). The dark theme at dusk and at night has stars.
2. The platform inside it. Controls are the system's: they take the accent colour, a state its own system colour, and a colour never means two things (green for on or granted, orange for needs something, red for danger). The glass of the window, the sidebar and the cards is the glass theme below this block and is left as it is.
3. Handwriting for names and for speech, and for nothing else. Patrick Hand has one weight, so it is never bold and nothing is faked (`font-synthesis: none`). It is used for the title of a page, the names of sections, the names of the tiles of General, the title of the hero, what the ghost says, the label in the foot of the sidebar and the line of an empty list. Rows, descriptions, numbers and controls stay in SF Pro.
4. The ghost. It is the guide of every page: a bubble at the top right says the first sentence of the page's description, and a click makes it hop and say something else for a moment. It is the sign of whether the app has the access it needs, in the foot of the sidebar: awake, or asleep with closed eyes and a z, and the words beside it say the same, so the ghost is never the only sign. It is the character of an empty list (`.empty`), and the big one in the General hero looks at the pointer.
5. Small and brief. A page rises into place only when a person chose it (`go.rise`); the ghost hops when it is clicked and blinks now and then, from a timer in `app.js`, and between those nothing runs: an endless animation would repaint the window sixty times a second for as long as it is open, and the app is proud of costing nothing. Under Reduce motion every animation ends at once and the ghost does not blink. Nothing moves to get attention.
6. Nothing extraneous. A description keeps the first sentence in the bubble and the rest where it was.
7. For everyone. The outline of the bubble turns to the text colour under Increase contrast, the canvas is solid under Reduce transparency, every control has a name, the guide is a button with a name, and decoration is hidden from VoiceOver.

## Where it is

- `crates/app-ui/web/app.css`, the last block, "Ghost world". It sits on top of the glass theme and can be removed as one block. Tokens: `--hand`, `--ghost-fill`, `--ghost-line`, `--ghost-cheek`, `--bubble-fill`, `--bubble-line`, and the sky's `--aurora-1` to `--aurora-3`.
- `crates/app-ui/web/app.js`: `GHOST_SVG`, `SPIRIT_SVG` and `WOBBLE_SVG` (the drawing, and the filter that wobbles its outline like a pen), `ghost()`, `accessPill()`, `guideLine()`, `poke()`, `metricsStrip()`, `applySky()`.
- `crates/app-ui/src/settings/page.rs` bundles the font, `crates/app-ui/assets/PatrickHand-*.woff2` (Latin and Vietnamese subsets), under the SIL Open Font License 1.1 (`PatrickHand-OFL.txt`).

## Looking at it

`settings_preview snap <page> out.png [--dark] [--lang vi] [--size 820x560] [--patch '{"env":{"sky":"night"}}']` draws any page with any sky. An animation must never decide whether something is visible: the hidden stage of the preview does not advance animation clocks, and a page that rose from nothing came out blank in every picture, which is why the rise is only ever added by a click.

## Adding a page

Use the helpers of `app.js` (`card`, `row`, `toggleRow`, `sliderRow`, `sectionTitle`, `pill`) and the empty list `h('div', { class: 'empty', text })`, and the page has the world already. Write the strings of both languages, and the first sentence of `page.<id>.sub`: it is what the ghost says.
