<p align="center">
  <img src="docs/images/icon.png" width="112" alt="">
</p>

<h1 align="center">figghost</h1>

<p align="center">
  <img src="docs/images/banner.png" width="100%" alt="A black notch pill above frosted glass cards: a clipboard list, a volume slider, window layouts, a media bar and a calendar tile">
</p>

An all-in-one macOS app written in Rust, with a pure AppKit shell (the `objc2` crates) and a settings window that is a web page on a Liquid Glass window: it combines a scrolling tool, a clipboard history, a notch panel and per-app sound, and the rest of what a notch toolbox does.

- Scrolling (like Mos): smooth mouse-wheel scrolling, a reverse direction for external mice only, modifier keys (sideways, faster, zoom) and per-app rules.
- Clipboard history (like Maccy): text, links, code, images and files, instant search, pinning with a paste letter, Copy Stack, pinned boards, sensitive data kept out or masked, and a popup at the pointer.
- Notch panel (like OneNotch): a black shape that grows out of the top edge of the screen with seven tabs (clipboard, shelf, media, focus, AI agents, calendar, sound), window snapping, an optional volume indicator, lyrics, and an AI assistant (MCP) server for the clipboard history. The agents tab answers Claude Code's and Codex's permission requests from the notch.
- Sound (like SoundSource and Boom 3D): volume to 400 %, mute, output device and equalizer for each app, effects on everything played, and volume keys for speakers the system cannot turn down.
- Off until switched on in the settings: Smart Select (units, money, times and translation for the selected text), screenshots, scrolling capture and screen recordings with an editor (like CleanShot X), a colour picker, a calendar with meeting alerts, a clipboard mascot, dragging a window onto the notch to snap it, and hiding menu bar icons.

Opening the app opens its settings window; closing the window sends the app back to the menu bar. Settings live in `~/Library/Application Support/Figghost/config.json`. The interface is available in English and Vietnamese.

## Screenshots

Every picture below is drawn by the app itself, with no screen: `app-ui/settings_preview snap`, `macos-sys/render_notch`, `render_snap` and `render_smart`.

**Settings** — Liquid Glass, both themes, two languages.

![The settings window: a sidebar with a search field over the app's pages, and grouped cards for launching at login, the menu bar icon, the settings window's appearance and the language](docs/images/settings-general.png)

**The notch** — seven tabs. The Clipboard tab, and the Shelf tab, where dropped files are kept as references to where they live: right-click one to ZIP it, convert it, make it smaller, remove its background or copy the text in the picture.

![The Clipboard tab of the notch with rows of copied text and their times](docs/images/notch-clipboard.png)

![The Shelf tab of the notch with files and folders, and the hint about dropping, opening, dragging out and removing](docs/images/notch-shelf.png)

**Window snapping** — the layouts that hang under the notch while a window is dragged by its title bar.

![The twelve drop zones, each a small screen with the part it takes, one of them chosen](docs/images/snap-zones.png)

**Smart Select** — select text anywhere and press the shortcut; a small window next to the pointer says what it is.

![Smart Select answering 5 km with 3.107 mi and 16,404 ft](docs/images/smart-select.png)

## Install

An Apple silicon Mac with macOS 13 or later. With Homebrew:

    brew tap eliasbui/figghost
    brew install --cask eliasbui/figghost/figghost

Without Homebrew:

    curl -fsSL https://raw.githubusercontent.com/eliasbui/figghost/main/scripts/install.sh | sh

Writing the cask out in full installs it from the tap and trusts only that cask, so no `brew trust` is needed. Both ways put `Figghost.app` in `/Applications` (the script takes `DEST=~/Applications` for somewhere else). The app is signed ad hoc and not notarized, so both remove the quarantine flag after installing it, as other open source notch apps do; build it yourself (below) if you would rather not. `brew uninstall --cask figghost` removes the app, and `--zap` its data in `~/Library/Application Support/Figghost` too. Figghost looks for a new version once a day and asks before it installs one, with what is new in it; with *Update automatically* on in the About page it installs without asking, and *Check now* there does it at once. An update replaces the app where it is, deletes the old copy and the download, and takes effect when Figghost is restarted (the popup has a button for that). It needs to be able to write in the folder the app is in, which an administrator can for `/Applications`; where it cannot, the About page links to the release instead, and running the install command again works as before. Grant Accessibility again after an update, because macOS ties the grant to the code signature.

## Build and run

Rust is installed with rustup into `~/.cargo`. Keep the build directory outside the project:

    . "$HOME/.cargo/env"
    export CARGO_TARGET_DIR="$HOME/Library/Caches/figghost-target"
    cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    scripts/bundle.sh            # creates dist/Figghost.app
    scripts/release.sh 0.2.1     # publishes a release and points the Homebrew cask at it (DRY_RUN=1 to check first)

For development, create a persistent local Code Signing certificate in Keychain Access (for example, `Figghost Local`) and install the watcher once with `cargo install cargo-watch`. Then `SIGN_IDENTITY="Figghost Local" scripts/dev.sh` rebuilds and restarts the local app whenever source files change. The stable identity keeps the Accessibility permission across rebuilds; stop the watcher with Control-C.

Accessibility access is tied to the code signature of the app. The default ad-hoc signature changes on every build, so access has to be granted again. To keep it, create a local code-signing certificate in Keychain and run `SIGN_IDENTITY="Certificate name" scripts/bundle.sh`.

Accessibility is needed for smooth scrolling, automatic paste, window snapping and the volume indicator. Without it the app still runs: the popup and the notch work, items are copied for a manual paste, and the scroll engine starts by itself once access is granted. Music and Spotify ask once for permission to be read by the media tab.

The app icon is drawn by code: `cargo run -p macos-sys --example make_icon -- X.iconset`, then `iconutil -c icns X.iconset -o crates/app-ui/assets/Figghost.icns`.

## Design

The settings window is a macOS window, not a web page that looks like one. The sidebar is a full-height pane with the window's three buttons on it and a field over the list that finds a page by name; the content is grouped sections of a label and its control. Every size and colour is the system's own, measured from AppKit rather than guessed: a 54 by 24 switch, 24 tall buttons, pop-up buttons, segmented controls and fields (20 for the small control size), 13 pt labels with 11 pt small print, and NSColor's ink, accent and separators. Under it the page follows Apple's Liquid Glass guidance: navigation and controls are glass, everything that is read sits on denser surfaces, colour is for state, and only what the user touches moves, briefly for feedback and with springs for things that travel. Contrast was measured, not guessed: text is at least 4.5:1 and the edge of an off control at least 3:1 on the worst backdrops, in both themes. Every effect has a fallback for Reduce transparency, Increase contrast and Reduce motion, focus is always visible, every control has a name, and the whole window works from the keyboard. All tokens are CSS variables at the top of `crates/app-ui/web/app.css`.

## Configuration

`~/Library/Application Support/Figghost/config.json`, written by the settings window. Missing values use defaults, out-of-range values are clamped, and an unreadable file is moved to `config.json.bad` instead of being overwritten. Files written by older versions load unchanged. Every key, every accepted value, and the environment variables and arguments the test instances use are in [docs/configuration.md](docs/configuration.md).

## Data and privacy

History lives in `~/Library/Application Support/Figghost` (`history.sqlite`, an `images` folder, `shelf.json` and the files of the features you turn on), with directory mode 0700 and no encryption. Screenshots and recordings are saved in the folder of macOS's own screenshots (else the Desktop) unless you choose another; the key display of a recording shows shortcuts and keys such as Return, never plain typing. Items flagged by the source app and password managers are skipped, but a password copied from another source can still be stored: use the ignored apps list or clear the history. The settings page is loaded from a string and cannot reach the network. Everything else stays on this Mac.

Apart from the daily look for a new version (one request to api.github.com that carries nothing about you, and the download from github.com when you update), nothing is sent over the network unless you turn it on, and then only what the settings text says: Smart Select downloads a table of exchange rates (nothing about you) and, to translate, sends the selected text (at most 450 characters) to api.mymemory.translated.net; lyrics send the artist, track and album names and the track length to lrclib.net. Both go through the system's curl over https. The refresh button of the usage limits on the Agents tab (Command+R there) runs the Codex program installed on this Mac to ask for its current limits; Codex contacts OpenAI itself, as it does whenever you use it, and Figghost sends nothing and never reads the Codex login. The permissions asked for are Accessibility (shortcuts that act on other apps, the volume indicator, Copy Stack, Smart Select, drag to the notch), Screen Recording (screenshots, recordings and the colour picker, only when you use them), System Audio Recording (Sound, the first time an app is changed, a recording with the computer's audio, and the bars of the closed notch when they follow the sound), Microphone and Camera (a recording, when turned on in its options), Calendars and Reminders (only when you turn the calendar on) and Automation of Music and Spotify (the Media tab).

## Technology choices

The shell uses AppKit through `objc2`, not GPUI or Tauri. The search field must accept Telex and VNI typing, which `NSSearchField` handles natively. Non-activating panels, mouse tracking and the app lifecycle are proven in this repository, and idle memory stays low. The settings window is a WKWebView because a settings screen with live graphs, glass effects and two languages is far quicker to build and refine as a page, and the page is a thin client: every rule lives in tested Rust. SQLite is the system library (with FTS5 and the trigram tokenizer), not a bundled build.

## Next steps

1. Try the unverified items in [docs/verification.md](docs/verification.md) on real hardware, starting with the ones that need real input or a real screen: Smart Select and screen capture from their shortcuts, the colour picker, Copy Stack, dragging a window to the notch, the menu bar icons, a calendar alert, lyrics with a playing track, physical volume keys, Telex and VNI typing, a notched display, launch at login.
2. From the OneNotch list, what is left: object removal, the system live activities (charging, Bluetooth, Focus), lock screen presence, gestures on the notch and a notch on every display.
3. A Developer ID signature and notarization, which also settles the permissions that have to be granted again after each ad-hoc build.
4. What to take from Raycast and PowerToys, and which of it is possible on macOS from an ad-hoc signed app, is in [docs/research-raycast-powertoys.md](docs/research-raycast-powertoys.md).

## Development

The repository layout, the programs in `crates/*/examples` that check the app, and how to add a language are in [docs/development.md](docs/development.md). What has been verified and what has not is in [docs/verification.md](docs/verification.md).

## License

MIT, see [LICENSE](LICENSE).

The settings window is set in SF Pro, the system's own font, so it reads as the rest of macOS does. Inter is bundled as the fallback for a machine without it, under the SIL Open Font License 1.1: see
[crates/app-ui/assets/Inter-OFL.txt](crates/app-ui/assets/Inter-OFL.txt).
