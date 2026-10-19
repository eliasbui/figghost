# What the notch of OneNotch is made of

OneNotch 2.0.5 (build 67, `com.huyt.OneNotch`, arm64, macOS 15 and later) was read from the copy installed in
`/Applications` on 19 October 2026 to guide the redesign of figghost's notch. Everything below is split into what was
observed in the shipped artifact, what is inferred from it, and what is unknown. Nothing was copied: no code, no
shader source, no image or video was taken, only the shape of the behaviour and numbers a user could also measure.

## How it was read

- REA `inspect_macho` (no disassembler needed): the 6131 imported symbols, which say which SwiftUI and Core Animation
  calls the app makes. `open_binary` also bound the Hopper provider, which crashed with `EXC_BAD_ACCESS` on this
  binary twice (macOS 27.0.1), and the machine has neither Ghidra nor IDA, so function-level REA tools were not
  available.
- `nm`, `dyld_info -fixups`, `otool -tvV`: the symbol table (79 000 local functions are redacted, the Objective-C
  methods, instance variables and 10 000 exported symbols are not), the chained-fixup imports and the disassembly.
  The call sites of the SwiftUI animation constructors were found by following each stub to its import and reading
  the floating point arguments loaded before the call.
- `strings` over the executable, the string tables of the `en` localisation, `default.metallib` (function names only:
  there is no Metal toolchain here) and the user defaults of the installed copy.
- Frames of the app's own onboarding videos (`onboarding-pill`, `-glance`, `-clipboard`, `-tools`, 30 frames per
  second): the look of the closed and open states.
- A live run was tried and gave nothing: the screen of this machine was locked, so OneNotch drew only its lock-screen
  mark, `observe_native_ui` could not select its windows (`window-mismatch`) and screen capture returned black. It was
  quit again. The numbers that need a running app are marked unknown.

## Observed

Windows. Three windows of the running app: the closed notch is its own window of 255 by 30 points on a 1920 by 1080
screen (a screen without a notch), at a private window level far above the menu bar; the open notch is a second window
of 700 by 745 points centred at the top, at level `mainMenu + 3` (27); settings is an ordinary window. The binary
creates a private Space (`CGSSpaceCreate`, `CGSSpaceSetAbsoluteLevel`) to keep the notch above full screen apps and the
lock screen.

Motion. The SwiftUI constructors it imports are `interpolatingSpring(mass:stiffness:damping:initialVelocity:)`,
`spring(response:dampingFraction:blendDuration:)`, `interactiveSpring`, `Spring(duration:bounce:)`, `timingCurve`,
`easeIn`, `easeOut`, `easeInOut`, `linear`, `delay`, `keyframeAnimator` with `CubicKeyframe`, `matchedGeometryEffect`,
`TimelineView` with an animation schedule, `contentTransition` (numeric text, opacity, symbol effects) and the SF
Symbol bounce, replace, rotate and draw-on effects. Arguments at the 32 `interpolatingSpring` call sites:

| Mass | Stiffness | Damping | Initial velocity | Sites | Damping ratio | Period |
|---|---|---|---|---|---|---|
| 0.96 | 235 | 24 | 0.04 | 25 | 0.80 | 0.40 s |
| 1.0 | 235 | 20 | 0 | 3 | 0.65 | 0.41 s |
| 1.0 | 170 | 34 and 38 | 0 | 2 | 1.30 and 1.45 | 0.48 s |
| 1.02 | 108 | 19 | 0.82 | 1, next to `NotchMotionEnvironment` | 0.90 | 0.61 s |
| 1.0 | 520, 680, 760 | 34, 30, 28 | 0 | 3 | 0.74, 0.57, 0.51 | 0.28 to 0.24 s |

The first row is used throughout the app, the notch's page code included, so it is the default movement. The most
common `spring(response:dampingFraction:)` pairs are 0.195 with 1.0, 0.3 with 1.0, 0.25 with 1.0, 0.4375 with 1.0 (no
overshoot), and 0.3 with 0.7, 0.5 with 0.75, 0.4375 with 0.78, 0.42 with 0.78 (one visible overshoot). The cubic
curves are mostly `(0.77, 0, 0.175, 1)` over 0.26 s (an ease in and out), `(0.16, 1, 0.3, 1)` over 0.7 s (a sharp ease
out) and `(0.22, 0.94, 0.24, 1)` over 0.42 s. The ease-out durations of fades are 0.12, 0.14, 0.15, 0.16 and 0.18 s.
`NotchMotionEnvironment` holds `durationScale`, `refreshRate`, `prefersLightweightEffects` and `trackedScreen`, so
motion is scaled to the display and lightened on a slow machine.

Shape and glass. Type names: `ClipboardDripPanelShape`, `LiquidBottomArcMask`, `ExactNotchPathCache` (the notch path
is computed once per size and cached), `NotchLiquidGlassBackground` (SwiftUI `glassEffect` with a `variant` and a
`tint`), `NotchRefractionBackdropView` (a backdrop with a signed-distance-field layer, `sdfBleed`, `sampleScale`,
`strength`, `groupName`), `BackdropBlurLayer` (`radius`, `saturation`). Private filter keys in the strings:
`inputInnerRefractionAmount`, `inputInnerRefractionHeight`, `inputOuterRefractionAmount`,
`inputOuterRefractionHeight`, `inputRefractionDistance0`, `inputRefractionDistance1`, `inputRefractionOpacity`. Layout
properties: `closedTopCornerInset`, `closedActivityTopCornerRadius`, `bottomCornerRadius`, `baseOpenWidth`,
`_openLayoutCompression`, `extendedHoverPadding`, `_closedNotchSize`, `_effectiveClosedNotchHeight`. Settings text:
"Corner radius scaling", "Mirror shape", "Custom notch height", "Expands below the notch", "Semi Liquid Glass (Beta)"
with a compatibility renderer where Liquid Glass is missing, "Peek on hover", "Hover delay", "Haptic feedback",
"Collapse again after", "Extend hover area", "Enlarges the notch 2x for screen recordings".

The drip. `default.metallib` (24 KB) exports `clipboardDripWarp` and `clipboardGooeyThreshold` next to `captureRipple`.
The parameters named in the strings are `progress`, `collapsedHeight`, `pinchX`, `pinchAnchor`, `pinchScale`,
`pinchStartHeight`, `amplitude`, `decay`, `frequency`, `speed`, `time`. The app imports `distortionEffect` and
`layerEffect`, which are what a SwiftUI shader is attached with. The view model keeps `_openContentMounted`,
`_openContentRevealProgress`, `_hasSettledOpen`, `_liquidCloseTransitionTask`, `_keepLiquidVisualDuringClose` and
`_liquidVisualBlend`: the page content is mounted when the shape opens and revealed by a progress value, and the
liquid look is kept alive while the shape closes.

Behaviour, from the names of its state: hover on the cutout, on either ear (a different target each: music, timer,
agents, Bluetooth) and on the edge opens it; `_frozenHoverZone*` keeps the hover area still while the shape moves;
`_ignoreHoverExitUntil`, `_suppressNotchOpenUntilAfterTimerPopupClose` and `_manualOpenUntil` stop it flickering;
`NotchFileDragApproachMonitor` and `_openedForFileDrop` open it as a file drag approaches; `_sneakPeek`,
`_peekTarget`, `_nowPlayingSneakPeek*` show the playing track for a moment when it changes; `NotchToastCenter`,
`liveActivityAlertStyle` (default `detailed`) and `batteryAppearDuration` show a toast for AirPods, battery and
network. User defaults of the installed copy: `notchOpenBehavior = activity`, `nonNotchAppearanceStyle = notch`,
`nonNotchHeight = 24`, `notchHeight = 44`, `minimumHoverDuration = 0`.

Look, from the onboarding videos. The closed notch on a screen without a cutout is a black capsule or a notch-shaped
tab; with a live activity it widens to a capsule with an icon on the left and a value on the right (an artwork and
bars for music, an AirPods icon, a name and a battery ring for a connected device), and its content cross-fades while
the width changes. The open shape hangs from the top edge with concave fillets where it meets the menu bar band, a
large bottom radius, solid black at the top turning into dark translucent glass lower down, and it changes size with
the page (a wide, short strip for the clipboard, a taller panel for the calendar). The header is one row: a dark
capsule with three dots on the left and a dark capsule holding the page icons on the right. The clipboard is a row of
cards that scrolls sideways under a row of category chips and a search field; a card shows a picture or the first lines
of text, the source app and the age. The toolbar and the clipboard chips use 11 to 13 point type; the calendar title
uses about 28 point bold. Other pages shown: calendar with a week strip and an agenda, a focus timer with a ruler
dial, a Claude Code approval card with Allow, Always, Deny, and a row of file actions that opens while a file is held
at the notch.

## Inferred

- The default open movement is the 0.96, 235, 24 spring: one overshoot of under 2 per cent that is hardly seen, over
  about 0.5 s. figghost's existing spring (0.42 s, bounce 0.22: 14.96 rad/s, damping ratio 0.78) is within 5 per cent
  of it, so the movement needs a new shape, not a new spring.
- The drip is a warp of the panel's rendering driven by `progress`: a neck at the top that is pinched at the start
  and a bulb below it that grows, wobbling with a damped sine (`amplitude`, `frequency`, `decay`), and the gooey
  threshold merges the two once they overlap. The first row of springs with initial velocity 0.82 is probably the
  settling of that wobble.
- Each page asks for its own open size and the shape moves to it when the page changes.
- Closed hover areas are frozen while a movement runs, so the pointer crossing the moving edge cannot close it.

## Unknown

- The numeric values of `bottomCornerRadius`, `closedTopCornerInset`, `baseOpenWidth`, the corner flare radius and
  the header heights: their getters are redacted functions and no live run was possible. The values used in figghost
  are measured from the onboarding videos, scaled so the closed cutout of a notched MacBook (32 points) matches.
- The shader source and its constants.
- Exactly which spring belongs to which transition: the 25 default call sites sit in the notch view code but the
  functions are unnamed.
- Anything that needs a running app: window frames while open, hover areas, real frame timings.

## What figghost took from it

- The outline of the open shape: concave fillets of 20 points where it meets the top edge, bottom corners of 28, and a
  neck that is pinched while the shape grows out of the closed notch and widens into the body later than the body
  grows (0.52 s against the spring's 0.42 s), so what comes out is a drop that fills out. Going back in, the neck
  draws in a little ahead of the body after a short pause. `silhouette.rs` holds the geometry and its tests.
- The glass and the black. The black drop is the only thing visible until the neck has filled out; Liquid Glass does
  not follow its view's opacity, so it stays hidden until then and the black fades off it. The pages appear with the
  glass. A band of black at the top of the glass fades out over the 46 points below the cutout, on every screen
  and not only on a hardware notch.
- A header of one row: the capsule of three dots at the left (it shows the menu of the menu bar item) and the capsule
  of page icons at the right, riding the edges of the shape while it grows. Pages slide in from the side they sit on.
- A size for every page. The shape is as large as the page's root plus the header, moves to it when the page changes,
  and follows the root when the page resizes itself (the Media page is short, and takes the full height only while it
  shows lyrics). Clipboard is 660 wide with the search field in the header row, a row of chips and a row of cards
  that scrolls sideways; Shelf is a wide row of tiles; Media is the cover beside the track, the bar and the buttons.
- The closed notch: ears that change what they show with a short fade, two lines of text under them (a track that
  has just started is shown for 3.6 s), the shape growing down to hold them.

## How it was checked

- `anim_check` on the real window server (outside every screen): the spring peaks 1.8 per cent past the open height
  and settles, and Liquid Glass has the shape's size on all 294 to 299 samples. It failed once for the sampler's
  timing and passed on the next three runs.
- `notch_shots` with `FIGGHOST_SLOW_MOTION=10` on the real screen, which showed the drop growing out of the notch
  and then, once the neck had filled out, the glass coming up under the fading black with the page content. It also
  showed the first version's fault: the glass rectangle showed at the corners beside the neck, because Liquid Glass
  ignores its view's opacity. It is hidden until revealed now.
- `render_drip` (the outline at every 30 ms), `render_closed` (rest, ears, two lines, with and without a hardware
  notch), `render_notch` (every page at its size), `app-ui/lyrics_preview` (the Media page with lyrics, full height),
  `layout_check`, `page_check` (the shape follows the page: strip, short Media page, Media page with lyrics, back, closed),
  `cargo test --workspace` and `cargo clippy --workspace --all-targets`.

Not checked: hover and click on the real pointer (the screen was locked for most of the work, and another session was
using the machine when it was not), the menu of the three dots, the sideways wheel on the strip, a Mac with a hardware
notch, and Reduce Motion. The toast has no source in the app
yet besides the track that has just started.
