# Arranging displays and presenting from the notch

Two requests of 27 October 2026: a way to arrange several displays from the notch, as System Settings does, with a
canvas to drag them on; and a way to present, choosing an area, a display or an app to show and the display to show
it on, with the keys a presenter needs and a zoom that follows the pointer. This note says what the platform and the
usual tools offer, what was chosen, and what was left for later.

## Arranging displays

What System Settings offers under Displays, with more than one display: Arrange (the displays drawn where they are,
dragged into place; the white bar of the menu bar dragged onto another display to make it the main one), and for each
display "Use as": Main display, Extended display, or Mirror for one of the others. Resolution, refresh rate, rotation
and HDR are settings of one display, not of the arrangement.

The public API is Core Graphics' display configuration: `CGBeginDisplayConfiguration`, then
`CGConfigureDisplayOrigin` for each display (points of the desktop, y downwards from the top-left of the main display)
and `CGConfigureDisplayMirrorOfDisplay` (0 ends a mirror), then `CGCompleteDisplayConfiguration` with
`kCGConfigurePermanently`, which is what System Settings keeps. The main display is the one at (0, 0), so making a
display the main one moves every display by the same amount. Where the displays are is read with
`CGGetOnlineDisplayList` (the active list leaves out the displays of a mirror set), `CGDisplayBounds`,
`CGDisplayIsMain`, `CGDisplayIsBuiltin` and `CGDisplayMirrorsDisplay`. A display that mirrors another has no `NSScreen`,
so its name is the one it had while it had one.

What was built (`macos-sys/src/display_layout.rs`, `macos-sys/src/displays_ui.rs`, `app-ui/src/features/displays`):
a second view of the Displays tab, Arrange, shown when two or more displays are connected. Displays that mirror
another are drawn as a stack under it; a click chooses a display (a second click on a stack chooses the next display
of it); a drag moves one, and where it is let go it goes flush against the nearest side of another display, sharing at
least 40 points of it, overlapping none, and lined up with the other's edges or middle when it is within 8 points of
the canvas (`arrange.rs`, unit tested); the main display stays at the origin, so moving it moves the others. The white
bar is dragged onto another display to make it the main one. A menu chooses what the chosen display is used as.
Everything is read again after a change and once more after the system has settled. It is on by default once the tab
is on, and can be turned off on the Displays page.

Left out: resolution, refresh rate and rotation (settings of one display, and a wrong one needs the 15 second
"keep this?" of System Settings), and naming a display that has never had a screen of its own since the app started.

## Presenting

### What the usual tools do

- Video calls (Zoom, Meet, Teams) share a whole screen, one window, or a portion of the screen, and draw a green frame
  around what is shared on the presenter's own screen.
- ScreenCaptureKit gives each of these: `SCContentFilter` of a display excluding apps (the presenter's own windows),
  of a display including only some apps (one app, all its windows, nothing else), of one desktop-independent window
  (the window even while covered, wherever it moves), and `sourceRect` for an area. `SCContentSharingPicker` (macOS 14
  and later) is the system's own picker of a display, window or app; it offers no area, and the app already has a
  frozen-screen picker for screenshots, so that is used instead.
- Screen recorders that zoom by themselves (Screen Studio and the like) zoom in where the mouse is clicked, follow the
  pointer smoothly while zoomed, and zoom back out after a quiet moment.
- ZoomIt (Sysinternals, also in PowerToys): Ctrl+1 zoom, Ctrl+2 draw, Ctrl+3 break timer, Ctrl+4 live zoom; while
  zoomed the mouse wheel or the arrows zoom in and out; in draw mode R, G, B, O, Y and P choose a pen, E erases, W and
  K give a white or black board, T types text.
- The zoom of macOS (Accessibility): Option-Command-8 turns it on and off, Option-Command-= zooms in,
  Option-Command-minus zooms out, Option-Command-backslash smooths images, and Control with a scroll gesture zooms.
- Presenter helpers (Presentify, Mouseposé): a ring or a spotlight around the pointer, a ripple for each click, the
  keys pressed shown on screen, drawing over the screen.

The keys of slide shows, while a show is in front:

| Action | PowerPoint | Keynote | Google Slides |
| --- | --- | --- | --- |
| Black screen | B or period | B | B |
| White screen | W or comma | W | W |
| Laser pointer | Ctrl+L | | L |
| Show or hide the pointer | Ctrl+H hides, Ctrl+A shows | C | |
| Zoom | plus and minus, arrows pan | | |
| Pen and eraser | Ctrl+P, Ctrl+E, E clears | | |
| End | Esc | Esc | Esc |

### The keys chosen

A presentation of another app has that app in front, so single letters cannot be used; every key has a modifier.
Control with Option is the family of quick global keys on the Mac, and the letters follow the slide shows where they
are free:

| Key | Action | Why that letter |
| --- | --- | --- |
| Control-Option-P | Start or stop (any app, while the feature is on) | Present |
| Control-Option-B | Black screen, again to come back | B of every slide show |
| Control-Option-W | White screen | W of every slide show |
| Control-Option-H | Hold the picture, again to go live | Hold |
| Control-Option-Z | Zoom in one step around the pointer | Z of ZoomIt and of the macOS zoom |
| Control-Option-X | Zoom out one step | next to Z |
| Control-Option-0 | Back to the whole picture | zero zoom |
| Control-Option-A | Automatic zoom on or off | Automatic |
| Control-Option-L | Spotlight | L of the laser in Google Slides |
| Control-Option-M | Ring around the pointer | Mouse |

None is a default of macOS (Control-Option-Space is the next input source and is avoided), of the window snapping of
the app (Control-Option with the arrows, Return, C, D, F, G, U, I, J, K) or of Smart Select (Control-Option-S), and E, T,
F, D and G, which Rectangle and Magnet use, are avoided too; a unit test keeps every default of the app different.
Only the key that starts is held while no presentation runs, so the others take nothing from other apps the rest of
the time. Every key can be changed or turned off on the Present page. VoiceOver uses Control-Option as its own
modifier; a person who presents with VoiceOver on should move these keys.

### What was built

- `core-capture/src/camera.rs`: the camera, plain Rust with unit tests. A click zooms to the chosen level (1.5 to 4
  times) centred on it; while zoomed the view follows the pointer only once it leaves the middle half of the view;
  after the chosen seconds without a click it goes back to the whole picture; keys step through 1, 1.5, 2, 2.5, 3, 4,
  6 and 8 times around the pointer, and a zoom chosen with keys is not undone by the quiet. The zoom closes in on a
  log scale (about 0.22 s) and the pan more quickly (about 0.14 s), the same at any frame rate; with Reduce motion it
  is where it is going at once. The view never leaves the picture.
- `macos-sys/src/presenter.rs`: the frames of an `SCStream` (BGRA, at most 4096 pixels on the long side, 30 or 60 a
  second) go into a `CALayer` as IOSurfaces, without a copy and without GPU code; the zoom is the frame of that layer
  inside a clipping layer of the picture's shape, letterboxed on the stage. Over it: a spotlight (a shape layer with an
  even-odd hole), a ring, a ripple for clicks, and a black or white cover; holding the picture drops new frames. The
  stage is a borderless window over the whole display the audience sees, above its menu bar and Dock, or a titled
  window that a call can share. A green frame on the presenter's screen marks an area or display that is shown.
- `macos-sys/src/present_ui.rs` and `app-ui/src/features/present`: the Present tab (what, where, start; then the time,
  what is shown where, stop, the switches, the zoom and the line of keys), the time in the closed notch (green while
  live, orange while held or covered), an entry in the menu bar panel, and the Present page of the settings.
- The overlay of screenshots gained a mode that picks a whole display with a click, for presenting a display when
  there are more than two.

Found while checking (`macos-sys/examples/present_check.rs`): ScreenCaptureKit lists only the apps that have windows
when the content is asked for, so a process whose stage did not exist yet was not left out of its own picture and the
stage showed itself inside itself. The stage and the frame are now made before the content is asked for (off-screen
windows included), and the app's windows are left out by app or, failing that, one by one.

### Left for later

- Drawing over the screen (ZoomIt's draw mode, a pen as in PowerPoint): it needs a window over the presenter's own
  screen that takes the mouse while drawing.
- The keys pressed shown to the audience: the key display of recordings already exists, but it needs Input
  Monitoring, which presenting does not ask for today.
- Zoom with Control and the scroll wheel: watching the wheel is free, but stopping the scroll from also reaching the
  app below needs an event tap and Accessibility.
- Typing keeping an automatic zoom open, for the same reason: keys are only seen with Input Monitoring.
