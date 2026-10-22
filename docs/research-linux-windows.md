# Porting figghost to Windows and Linux

A study of what it takes to run figghost on Windows and Linux: what the code already allows, what each
feature needs on each system, where the systems cannot do what macOS does, and an order of work.
The figures about the code come from the 2.0.1 tree; the statements about the other systems are from their public
documentation.

## Status: the beta

A first beta exists as its own program, `crates/desktop` (`figghost`), described in the README under "Windows and
Linux (beta)". It did not take the `sys` alias route below: it is a small separate shell (tao, wry, tray-icon,
global-hotkey, arboard) over the shared crates, with the tray, the clipboard history, notes, the agents and the MCP
server. To get there, leaks 1 to 3 were fixed for those crates: `core-clipboard` and `core-agents` build on Windows
(file modes behind `cfg(unix)`, the hooks socket through `uds_windows`), the MCP server moved from `app-ui` to
`core-clipboard`, and the notes moved to a `core-notes` crate. `core-mouse` and the rest of the list are unchanged.
`scripts/check-desktop-linux.sh` runs the tests on Linux, starts the app on a virtual screen and builds the Windows
program with MinGW; the Windows program has not been run on Windows yet.

## Where the code stands

| Part | Lines | Platform ties |
|---|---|---|
| `macos-sys` | about 41,000 in 87 files | All of it: AppKit, Core Graphics, ScreenCaptureKit, Vision, EventKit, WebKit, Core Audio taps |
| `app-ui` | about 55,700 | 80 `use macos_sys` sites, `MainThreadMarker` in 33 files, `std::os::unix` in 2 files, macOS command line tools |
| `core-*` (10 crates) | about 22,000 | Mostly pure Rust, with the leaks below |
| `nowplaying-adapter` | 747 | macOS only (a dylib loaded by `/usr/bin/perl` to reach MediaRemote), has no port |
| `app-ui/web` | settings page | Calls `window.webkit.messageHandlers`, otherwise portable |

The design already helps: the rules live in `core-*` (snap state machine, smoother curve, clipboard store,
search, sensitive data, Smart Select parsers, Telex and VNI plan, audio DSP, calendar model, lyrics) and the
settings window is a web page. Those are reused as they are.

Leaks to fix before any port, all small:

1. `core-mouse` depends on `macos-sys` for `synth::SELF_TAG`, `synth::post_zoom` and the event types
   (`crates/core-mouse/src/lib.rs:7`, `:11`, `:250`). The smoother itself is pure; the posting belongs in
   the platform crate.
2. `std::os::unix` in `core-clipboard/src/store.rs` (directory and file modes), `core-agents` (`server.rs`
   and `cli.rs` use a Unix socket, `accounts.rs`, `install.rs`, `tokens.rs`), `app-ui/src/agents.rs` and
   `app-ui/src/features/mod.rs` (`CommandExt`). File modes become `cfg(unix)`; the agents socket needs a
   Windows transport (AF_UNIX works on Windows 10 1803 and later, but `std` has no `UnixListener` there, so
   the `uds_windows` crate or a named pipe).
3. Data paths are written as `Library/Application Support/...` (`app-ui/src/config.rs:547`,
   `core-agents/src/accounts.rs:68` for Orca). Use `%APPDATA%\Figghost` and `$XDG_DATA_HOME/figghost`
   (the `dirs` crate, or three `cfg` lines).
4. macOS tools run as processes: `sips`, `screencapture`, `osascript`, `osacompile`, `afplay`,
   `launchctl`, `open`, `pgrep`. Each needs a replacement (the `image` crate for `sips`, `open::that` or
   `ShellExecuteW` and `xdg-open` for `open`, and so on).
5. The settings page posts to `window.webkit.messageHandlers.fig`, `.drag` and `.zoom`
   (`macos-sys/src/webview.rs:48`). `wry` gives `window.ipc.postMessage`; one shim in the injected script
   keeps `app.js` unchanged.
6. No `cfg(target_os)` exists anywhere in the tree today, so the workspace does not build for another
   target. `cargo check --target x86_64-pc-windows-msvc` could not be run here (the target is not
   installed); it is the first thing to try.

## How to split the platform layer

Recommended: one crate per system with the same module names and function signatures as `macos-sys`
(`win-sys`, `linux-sys`), and in `app-ui` a single alias:

```rust
#[cfg(target_os = "macos")]   use macos_sys as sys;
#[cfg(target_os = "windows")] use win_sys as sys;
#[cfg(target_os = "linux")]   use linux_sys as sys;
```

No trait per feature: the 80 import sites change from `macos_sys::` to `sys::` and the compiler checks that
each port offers the same surface. A module that a system cannot offer exports a stub that reports
"unavailable", and the settings page hides the feature the same way it hides one that is turned off.
`MainThreadMarker` becomes a type of each crate (on Windows and Linux an empty marker made on the UI thread).

The biggest cost is not the system calls but the drawing: `macos-sys` draws the notch, the wheel, the
HUD, the snap overlay and some thirty panels with AppKit and Core Animation (`notch.rs` 3,663 lines,
`wheel_ui.rs` 2,115, `ui.rs` 1,692). There are three ways to draw them elsewhere:

| Option | For | Against |
|---|---|---|
| Write each panel again with Win32 or GTK | Native look | Two more copies of some 15,000 lines of UI |
| Draw the panels as web pages in transparent `wry` windows | One copy, the settings page already does it, `wry` uses WebView2 and WebKitGTK | Every open panel is a web view: tens of MB each, slower first show, the 25 MB figure is lost |
| One Rust renderer for both (`tiny-skia` or `vello` into `softbuffer` or `wgpu`, windows from `winit`) | One copy, light, the existing headless renders (`render_notch` and others) could share it | The drawing code is new; text needs `cosmic-text` or `parley` |

Recommended: `wry` for the settings window on both systems (it is already a web page), and a shared Rust
renderer for the overlays. A single shared crate (`overlay-ui`, say) costs less than two native copies and
keeps the memory low.

## Feature by feature

Columns: the Windows way, the Linux way, and how far each gets. "X11" and "Wayland" differ so much on Linux
that they are named apart; GNOME, KDE and wlroots compositors (Sway, Hyprland) differ again under Wayland.

| Feature | Windows | Linux | Reach |
|---|---|---|---|
| App shell, tray icon, menu | Win32 message loop, `Shell_NotifyIcon` (`tray-icon`, `muda`) | StatusNotifierItem over D-Bus (`tray-icon` or `ksni`) | Windows: full. Linux: KDE full; GNOME needs the AppIndicator extension |
| Settings window | `wry` on WebView2 (on Windows 11, shipped; on 10, the runtime may need installing) | `wry` on WebKitGTK 4.1 | Full, after the JS bridge shim (leak 5). Liquid Glass becomes Mica or Acrylic (`DwmSetWindowAttribute`) on Windows 11; on Linux only KDE has blur |
| Launch at login | `HKCU\...\Run` or a Startup shortcut | `~/.config/autostart/*.desktop` | Full |
| Notch panel | No notch: a pill at the top centre in a topmost `WS_EX_TOOLWINDOW` / `WS_EX_NOACTIVATE` layered window | X11: override-redirect window. Wayland: `wlr-layer-shell` (KDE, wlroots). GNOME has no layer shell; only a GNOME Shell extension can place it | Windows and X11: full. KDE and wlroots: full. GNOME: needs an extension written in JS |
| Smooth scrolling | `WH_MOUSE_LL` hook, swallow the wheel event, post smaller ones with `SendInput` | X11: XInput2 raw events and XTest. Wayland: no client can see or change another client's input; only an `evdev` reader plus a `uinput` device (needs the `input` group or root) | Windows: full. Linux: hard, and on Wayland it is a privileged daemon. libinput already smooths touchpads |
| Global shortcuts | `RegisterHotKey` (`global-hotkey`) | X11: `XGrabKey`. Wayland: the GlobalShortcuts portal (KDE, GNOME 48 and later, Hyprland); the user approves each one | Windows: full. Linux: works where the portal exists |
| Clipboard history | `AddClipboardFormatListener`, formats `CF_UNICODETEXT`, `CF_HTML`, `CF_DIB`, `CF_HDROP`; `ExcludeClipboardContentFromMonitorProcessing` marks secrets | X11: XFixes selection events. Wayland: `wlr-data-control` or `ext-data-control` (KDE, wlroots). GNOME offers neither: a client sees the clipboard only while focused | Windows: full. Linux: full except GNOME on Wayland |
| Paste into the front app | `SendInput` Ctrl+V | X11: XTest. Wayland: the RemoteDesktop portal (asks the user) or `uinput` | Windows: full. Wayland: with a prompt |
| Window snapping | `SetWinEventHook` (`EVENT_SYSTEM_MOVESIZESTART`/`END`) to see a title bar drag, `SetWindowPos` / `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` to place it. Windows 11 Snap Layouts overlap the idea | X11: EWMH (`_NET_MOVERESIZE_WINDOW`). Wayland: a client may not move another's window: only a KWin script, a GNOME extension, or Hyprland and Sway IPC | Windows: full. X11: full. Wayland: one plugin per compositor |
| Window list, switcher, pin on top | `EnumWindows`, `SetWindowPos(HWND_TOPMOST)` | X11: EWMH. Wayland: `wlr-foreign-toplevel` lists (wlroots, KDE); always-on-top only through the compositor | Same pattern as snapping |
| Accessibility (selected text, title bar hit test) | UI Automation (`IUIAutomation`, `TextPattern`) | AT-SPI over D-Bus | Windows: good. Linux: patchy, many apps (Electron, Qt without the bridge) expose little |
| Smart Select | Read the selection by UIA, or copy with Ctrl+C and restore the clipboard; the parsers are in `core-smart` | AT-SPI, or the PRIMARY selection on X11 and wlroots | Good on both, the logic is shared |
| Screenshots, recording | `Windows.Graphics.Capture`, Media Foundation for MP4 | XDG ScreenCast portal (PipeWire stream) and the Screenshot portal; GStreamer or `ffmpeg` for encoding | Full on both; on Wayland the portal asks the user to choose a screen |
| Text in images (OCR) | `Windows.Media.Ocr` (needs the language pack) | Tesseract (a dependency of the package) | Full; Tesseract quality is lower for Vietnamese without the trained data |
| Remove background | No system API: an ONNX model through `ort` | Same | The same code for both, adds a model file of tens of MB |
| Now Playing, media keys | `GlobalSystemMediaTransportControlsSessionManager` (WinRT) | MPRIS over D-Bus (`mpris` crate) | Full on both, simpler than the macOS adapter |
| Per-app volume | `IAudioSessionManager2` and `ISimpleAudioVolume` per session | PipeWire or PulseAudio sink inputs (`pipewire` crate, or `pactl`) | Full on both |
| Per-app EQ and effects (`core-audio-fx`) | Process loopback capture (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`, Windows 10 2004 and later) can read an app but cannot change what plays; changing it needs an APO driver or a virtual device | A PipeWire filter node linked between the app stream and the output, the DSP from `core-audio-fx` inside it | Linux: easier than macOS. Windows: needs a virtual audio driver, which is a separate signed product |
| Volume and brightness HUD | Hide the system flyout is not possible; draw our own after `WM_APPCOMMAND` or a hook | Desktop dependent | Partial on both |
| External display brightness (DDC) | Monitor Configuration API (`SetVCPFeature`) | `i2c-dev` (`ddc-hi` crate), needs the `i2c` group | Full |
| Calendar | `Windows.ApplicationModel.Appointments` is meant for packaged apps and sees little of Outlook; Microsoft Graph or CalDAV and ICS are surer | Evolution Data Server over D-Bus (GNOME), Akonadi (KDE), or CalDAV and ICS | Partial; CalDAV and ICS are one code path for both |
| Secrets (keychain) | Credential Manager | Secret Service (GNOME Keyring, KWallet) | Full with the `keyring` crate |
| Vietnamese typing (`core-typing`) | A low-level keyboard hook and `SendInput` (the way UniKey and EVKey work), or a TSF text service | An engine for Fcitx5 or IBus | Windows: full. Linux: works, but as an input method engine, which is a separate shared library loaded by the IME framework |
| System tab | PDH counters, `GlobalMemoryStatusEx`, `GetSystemPowerStatus`; temperature only through WMI and often missing | `/proc`, `/sys/class/hwmon`, `/sys/class/power_supply` | Good; `sysinfo` covers most of both |
| Bluetooth device battery | WinRT `Bluetooth` and the GATT battery service | BlueZ over D-Bus (`Battery1`) | Good |
| AI agents, MCP server, PRs, weather, lyrics, notes, pomodoro, launcher | Plain Rust and files | Same | Full after the socket and path fixes |
| Keep awake | `SetThreadExecutionState` | `systemd-inhibit` or the Inhibit portal | Full |
| Update | Replace the exe and restart, or an MSIX | AppImage update, or leave it to the package manager | Different per channel |

## What the systems cannot do

1. Wayland forbids, on purpose, the things figghost does most: watching the pointer and keys of other apps,
   changing scroll events, moving other windows, reading the clipboard while in the background (GNOME), and
   placing a panel at the top of the screen (GNOME). Each needs a portal (with a prompt), a privileged
   daemon, or a compositor plugin. Most Linux desktops now default to Wayland (Ubuntu since 22.04, Fedora,
   KDE Plasma 6).
2. Per-app audio effects on Windows need a driver. Without one, per-app volume and mute are the most that
   can be offered.
3. There is no notch. The pill at the top centre keeps the idea; the "grows out of the camera" look does not.
4. Liquid Glass has no match: Mica and Acrylic on Windows 11, blur on KDE, a solid canvas elsewhere.

## Recommended order

1. Prepare the tree on macOS, with no behaviour change: the leaks above, the `sys` alias in `app-ui`,
   a `cfg` gate around `nowplaying-adapter`, and CI jobs that run `cargo check` for
   `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu` on the `core-*` crates. Running the `core-*`
   tests on all three systems is a cheap first win.
2. Windows first. One API surface, every feature reachable except per-app effects, and users who already
   pay for this kind of tool (PowerToys covers part of it for free: FancyZones, Text Extractor, Color Picker,
   which sets the bar). A first version: tray, settings window, clipboard history, smooth scrolling,
   shortcuts, window snapping, Smart Select, the top pill with Media and Clipboard tabs, per-app volume,
   Vietnamese typing.
3. Linux after, in this order of support: X11 and KDE Plasma on Wayland first (both have data-control,
   layer shell and scripting), wlroots compositors next, GNOME last and only with a companion Shell
   extension. Ship as a Flatpak or AppImage plus a `.deb`; the privileged parts (scroll smoothing through
   `uinput`) cannot live in a Flatpak sandbox.

## Rough size

Order of magnitude only, for one developer who knows the code:

| Step | Estimate |
|---|---|
| Prepare the tree | 1 to 2 weeks |
| Shared overlay renderer and the top pill | 4 to 6 weeks |
| Windows first version (list above) | 8 to 12 weeks after the renderer |
| Windows parity without per-app effects | 4 to 6 more months |
| Linux X11 and KDE, first version | 8 to 12 weeks, the renderer and logic shared with Windows |
| GNOME extension, scroll daemon, PipeWire effects | 1 to 2 months each |

## Rust crates worth using

`windows` (Win32 and WinRT), `wry` and `tao` or `winit`, `tray-icon`, `muda`, `global-hotkey`, `keyring`,
`zbus` (D-Bus: MPRIS, StatusNotifierItem, BlueZ, AT-SPI), `ashpd` (XDG portals), `x11rb`,
`smithay-client-toolkit` (layer shell, data control), `pipewire`, `ddc-hi`, `sysinfo`, `tiny-skia` or
`vello`, `softbuffer`, `cosmic-text`, `ort`, `image`, `dirs`.
