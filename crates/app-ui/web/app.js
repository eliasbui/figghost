'use strict';
// Figghost settings page. The app talks to it through window.fig.send(message) and window.__fig.recv(message).

// ---------------------------------------------------------------- utilities

const $ = (selector, root = document) => root.querySelector(selector);
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs || {})) {
    if (value === false || value == null) continue;
    if (key === 'class') el.className = value;
    else if (key === 'text') el.textContent = value;
    else if (key === 'html') el.innerHTML = value;
    else if (key.startsWith('on')) el.addEventListener(key.slice(2), value);
    else el.setAttribute(key, value === true ? '' : value);
  }
  for (const kid of kids.flat()) {
    if (kid == null || kid === false) continue;
    el.append(kid.nodeType ? kid : document.createTextNode(String(kid)));
  }
  return el;
}

// Glyphs are SF Symbols drawn by the system (window.__symbols, see settings/page.rs), used as a mask so they
// take the text colour.
const SYMBOL_ALIASES = { close: 'xmark', refresh: 'arrow.clockwise' };
const missingSymbols = new Set();
function icon(name, cls) {
  const src = (window.__symbols || {})[SYMBOL_ALIASES[name] || name];
  if (!src) missingSymbols.add(name);
  return h('span', { class: 'glyph' + (cls ? ' ' + cls : ''), 'aria-hidden': 'true', style: src ? `--glyph:url("${src}")` : 'visibility:hidden' });
}
/** The app's own icon (window.__appIcon, drawn from Figghost.icns). */
function appIcon(cls) { return h('img', { class: cls, src: window.__appIcon, alt: '' }); }

// The ghost, drawn by hand: the guide of every page and the sign of whether the app has the access it needs. Its
// colours and its movement are in the "Ghost world" part of app.css; `asleep` on its wrapper closes its eyes.
const GHOST_BODY = 'M13 63 C12 52 11 40 11.5 31 C12.5 16 21 7 32.5 7 C44 7 52.5 16 53 31 C53.5 41 52.5 52 51 63 C49 62 46.5 59.5 44 61 C41.5 62.5 40 66.5 37 65.5 C34.5 64.6 34 60.5 31.5 60.5 C29 60.5 28.5 65 25.5 65.7 C22.5 66.2 21.5 62 19 61 C16.5 60 14.5 62.5 13 63 Z';
const GHOST_SVG =
  '<svg class="ghost" viewBox="0 0 64 72" focusable="false"><path class="g-body" d="' + GHOST_BODY + '"/>' +
  '<g class="eyes-open"><ellipse class="g-ink" cx="25" cy="31" rx="3.1" ry="4.6"/><ellipse class="g-ink" cx="40" cy="31" rx="3.1" ry="4.6"/></g>' +
  '<g class="eyes-shut"><path class="g-line" d="M21.5 32 Q25 35 28.5 32"/><path class="g-line" d="M36.5 32 Q40 35 43.5 32"/></g>' +
  '<ellipse class="g-cheek" cx="19.5" cy="39.5" rx="3.6" ry="2.2"/><ellipse class="g-cheek" cx="45.5" cy="39.5" rx="3.6" ry="2.2"/>' +
  '<path class="g-line" d="M29.5 40 Q32.5 43.6 35.5 40"/><text class="z g-ink" x="48" y="14" font-size="13">z</text></svg>';
// The big ghost of the hero: white eyes with pupils that look where the pointer is (--px and --py, in pixels).
const SPIRIT_SVG =
  '<svg class="spirit" viewBox="0 0 64 72" focusable="false"><path class="g-body" d="' + GHOST_BODY + '"/>' +
  '<ellipse class="eye-white" cx="25" cy="30" rx="5.6" ry="6.6"/><ellipse class="eye-white" cx="41" cy="30" rx="5.6" ry="6.6"/>' +
  '<circle class="pupil" cx="25" cy="31" r="2.7"/><circle class="pupil" cx="41" cy="31" r="2.7"/>' +
  '<ellipse class="g-cheek" cx="17.5" cy="40.5" rx="3.6" ry="2.2"/><ellipse class="g-cheek" cx="48.5" cy="40.5" rx="3.6" ry="2.2"/>' +
  '<path class="g-line" d="M28.5 41.5 Q33 46.5 37.5 41.5"/></svg>';
// The filter that wobbles the outline of the ghost and nothing else. It is defined once, in the shell.
const WOBBLE_SVG = '<svg width="0" height="0" focusable="false"><filter id="wobble" x="-5%" y="-5%" width="110%" height="110%"><feTurbulence type="fractalNoise" baseFrequency="0.03" numOctaves="2" seed="4" result="n"/><feDisplacementMap in="SourceGraphic" in2="n" scale="1.6" xChannelSelector="R" yChannelSelector="G"/></filter></svg>';
function ghost(cls) { return h('span', { class: 'ghost-wrap' + (cls ? ' ' + cls : ''), 'aria-hidden': 'true', html: GHOST_SVG }); }

// The ghosts blink now and then. A timer rather than an endless animation: between blinks nothing runs at all.
function blinkSoon() {
  setTimeout(() => {
    if (document.visibilityState === 'visible' && !document.documentElement.classList.contains('reduce-motion')) {
      const eyes = document.querySelectorAll('.ghost .eyes-open');
      for (const e of eyes) e.classList.add('blink');
      setTimeout(() => { for (const e of eyes) e.classList.remove('blink'); }, 150);
    }
    blinkSoon();
  }, 3500 + Math.random() * 5000);
}
blinkSoon();

function throttleBy(key, ms, fn) {
  const slots = (throttleBy.slots = throttleBy.slots || new Map());
  const slot = slots.get(key) || { last: 0, timer: 0, args: null };
  slots.set(key, slot);
  slot.args = [fn];
  const flush = () => { slot.timer = 0; slot.last = performance.now(); const [f] = slot.args; f(); };
  const wait = slot.last + ms - performance.now();
  if (wait <= 0) { clearTimeout(slot.timer); flush(); } else if (!slot.timer) slot.timer = setTimeout(flush, wait);
}

// ---------------------------------------------------------------- language

const I18N = {
  en: {
    'side.search': 'Search settings', 'side.clear': 'Clear search', 'side.none': 'No settings found',
    'guide.label': 'The ghost', 'guide.q1': 'Boo.', 'guide.q2': 'I live in your notch.', 'guide.q3': 'Nothing extraneous here.', 'guide.q4': 'Ask me again later.',
    'metric.features': 'Features on', 'metric.tabs': 'Notch tabs', 'metric.clips': 'Copied items', 'metric.access': 'Accessibility',
    'cat.general': 'General', 'cat.keys': 'Shortcuts', 'cat.scroll': 'Mouse & Cursor', 'cat.notch': 'Notch & Island', 'cat.agents': 'AI Agents & PRs', 'cat.calendar': 'Schedule & Focus', 'cat.tabs': 'Notch Widgets', 'cat.system': 'System & Devices', 'cat.clipboard': 'Clipboard & Typing', 'cat.smart': 'Smart & Translate', 'cat.capture': 'Capture & Media', 'cat.tiling': 'Windows & Pin',
    'nav.sec.overview': 'Overview', 'nav.sec.notch': 'Notch & Island', 'nav.sec.mouse': 'Mouse & Windows', 'nav.sec.tools': 'Clipboard & Tools',
    'nav.general': 'General', 'nav.keys': 'Shortcuts', 'nav.scroll': 'Scrolling', 'nav.clipboard': 'Clipboard', 'nav.notch': 'Notch', 'nav.agents': 'AI agents', 'nav.tiling': 'Window snapping', 'nav.about': 'About',
    'page.general': 'General', 'page.general.sub': 'Startup, language and permissions.',
    'page.keys': 'Shortcuts', 'page.keys.sub': 'The keys Figghost listens to, in one place. Click a shortcut, then press the new keys.',
    'keys.sec.panel': 'In the notch and the clipboard popup', 'keys.board.title': 'Put in the next board', 'keys.board.desc': 'Boards group pinned items above the clipboard list.',
    'keys.pastealt.desc': 'Keeps the formatting when pasting without it is on, and the other way round.',
    'keys.delete.title': 'Delete, deny or dismiss', 'keys.delete.desc': 'Deletes the selected item, refuses what an agent asks and dismisses a calendar alert.',
    'keys.sec.fixed': 'Standard keys', 'keys.fixed.desc': 'They work as they do everywhere on the Mac and stay as they are.',
    'keys.fixed.enter': 'Paste, allow or join', 'keys.fixed.move': 'Move the selection', 'keys.fixed.tab': 'Switch tab or paste a row',
    'keys.sec.global': 'Anywhere on the Mac', 'keys.elsewhere': 'Several shortcuts, set on its own page.', 'keys.open': 'Open', 'keys.change': 'Change keys',
    'target.keys:pin': 'the pin key', 'target.keys:board': 'the board key', 'target.keys:paste_other': 'the key that pastes the other way', 'target.keys:delete': 'the delete key',
    'fault.reserved': 'Command with one key already means a tab, a pinned item or Delete in the panels. Add Option, Control or Shift.',
    'page.scroll': 'Scrolling', 'page.scroll.sub': 'Smooth, reversible mouse-wheel scrolling. Trackpads and the Magic Mouse are never touched.',
    'page.clipboard': 'Clipboard', 'page.clipboard.sub': 'A history of everything you copy, with instant search, pinning and keyboard shortcuts.',
    'page.notch': 'Notch', 'page.notch.sub': 'A panel that grows out of the top edge of your screen: clipboard, file shelf, media, your coding agents and your calendar.',
    'page.agents': 'AI agents', 'page.agents.sub': 'Follow Claude Code and Codex from the notch: answer their permission requests, see what they are doing and how much of your limits is left.',
    'sec.agents.follow': 'Following', 'agents.enable.title': 'Follow coding agents', 'agents.enable.desc': 'Listen for what Claude Code and Codex report and show it on the Agents tab of the notch.',
    'sec.agents.connect': 'Connect', 'agents.claude.title': 'Claude Code', 'agents.claude.desc': 'Adds hooks to ~/.claude/settings.json. Your other hooks stay as they are.',
    'agents.codex.title': 'Codex', 'agents.codex.desc': 'Adds hooks to ~/.codex/hooks.json. Codex asks you to review new hooks the first time it runs.',
    'agents.agy.title': 'Antigravity', 'agents.agy.desc': 'Adds a hook named figghost to ~/.gemini/config/hooks.json, next to your own. The agy command line tool then shows its sessions on the notch, working, finished or failed. It asks for permissions in its own terminal as before.',
    'agents.pi.title': 'Pi', 'agents.pi.desc': 'Adds an extension to ~/.pi/agent/extensions/figghost.ts. Pi loads it the next time it starts; its questions can then be answered on the notch as well as in its terminal.',
    'agents.install': 'Install', 'agents.remove': 'Remove', 'agents.repair': 'Repair',
    'agents.status.on': 'Connected', 'agents.status.off': 'Not connected', 'agents.status.partial': 'Incomplete ({n} of {m})', 'agents.status.missing': 'Not found', 'agents.status.error': 'Cannot read its file',
    'sec.agents.answer': 'Answering', 'agents.wait.title': 'Wait for an answer', 'agents.wait.desc': 'A command waits this long on the notch. Without an answer the agent asks in its own terminal, as usual.', 'agents.wait.n': '{n} seconds',
    'agents.sound.title': 'Play a sound', 'agents.sound.desc': 'When a command needs an answer or an agent finishes.',
    'agents.peek.title': 'Show the notch when an agent finishes', 'agents.peek.desc': 'The notch opens for a few seconds and closes by itself, unless the pointer is on it.',
    'sec.agents.limits': 'Usage limits', 'agents.usage.title': 'Claude Code limits', 'agents.usage.desc': 'Claude Code reports how much of its five-hour and weekly limits is used to its status line. Turning this on runs Figghost there first and then your own status line, so it looks the same. Codex limits are read from its session logs and need nothing. The refresh button on the Agents tab also asks the Codex program installed on this Mac for its current limits: Codex contacts OpenAI itself, as it does whenever you use it, and Figghost sends nothing and never reads the Codex login.',
    'agents.usage.on': 'Turn on', 'agents.usage.off': 'Turn off', 'agents.usage.waiting': 'Waiting for the next message from Claude Code.', 'agents.usage.ok': 'On', 'agents.usage.none': 'Off',
    'sec.agents.note': 'Good to know', 'agents.note.title': 'Other apps that answer permission requests', 'agents.note.body': 'If another app, such as OneNotch, also answers these requests, both ask and the first answer wins. Turn one of them off.',
    'tab.agents': 'Agents', 'tab.agents.desc': 'Claude Code and Codex: the command waiting for you, the sessions that are running and what is left of their limits.',
    'tab.calendar': 'Calendar', 'tab.calendar.desc': 'Today and tomorrow, the reminders that are due, and an alert with a Join button before a meeting. Off until you turn it on in the Calendar page.',
    'fault.agents_failed': 'Could not change the agent files.',
    'page.tiling': 'Window snapping', 'page.tiling.sub': 'Place the focused window on a part of the screen with a shortcut.',
    'page.about': 'About', 'page.about.sub': 'One light app for the small tools of a Mac.',

    'hero.tagline': 'Smooth scrolling, clipboard history and a notch toolbox in one quiet menu bar app.',
    'chip.version': 'Version {v}', 'chip.access.ok': 'Accessibility on', 'chip.access.no': 'Accessibility needed',
    'sec.startup': 'Startup', 'sec.language': 'Language', 'sec.permissions': 'Permissions', 'sec.data': 'Data',
    'login.title': 'Launch at login', 'login.desc': 'Start Figghost when you log in. The window stays closed; use the menu bar icon.',
    'login.approval': 'Allow Figghost in System Settings, under Login Items, to finish.', 'login.unavailable': 'Available when Figghost runs as an app from a folder such as Applications.',
    'login.open': 'Open Login Items',
    'showlaunch.title': 'Open this window at start', 'showlaunch.desc': 'Show settings each time you open the app yourself. Never at login.',
    'hello.title': 'Say hello at start', 'hello.desc': 'Each time you open Figghost yourself, its ghost comes out of the notch and blinks. Never at login or in the background, and only while the notch is on.',
    'lang.title': 'Language', 'lang.desc': 'This window changes at once. Menus and the notch change after a restart.',
    'lang.auto': 'Automatic', 'lang.en': 'English', 'lang.vi': 'Tiếng Việt',
    'lang.restart': 'Restart Figghost to use the new language in menus and the notch.', 'lang.restart.btn': 'Restart now',
    'access.title': 'Accessibility', 'access.desc': 'Needed for smooth scrolling, pasting into other apps, window snapping and the volume indicator.',
    'access.granted': 'Granted', 'access.missing': 'Not granted', 'access.open': 'Open System Settings',
    'perm.all_title': 'All permissions', 'perm.all_desc': 'Check and grant all system permissions Figghost uses.', 'perm.all_btn': 'Check all permissions…',
    'data.title': 'Settings folder', 'data.desc': 'Where your settings, clipboard history and shelf are stored. Everything stays on this Mac.', 'data.open': 'Open folder',

    'sec.wheel': 'Mouse wheel', 'sec.feel': 'Feel', 'sec.try': 'Try it', 'sec.mods': 'Modifier keys', 'sec.apps': 'Apps',
    'smooth.title': 'Smooth scrolling', 'smooth.desc': 'Turns each wheel notch into a gliding motion.',
    'reverse.title': 'Reverse wheel direction', 'reverse.desc': 'Only mouse wheels are reversed. The trackpad keeps its natural direction.',
    'step.title': 'Step', 'step.desc': 'Distance of one wheel notch', 'speed.title': 'Speed', 'speed.desc': 'Multiplier for every scroll', 'duration.title': 'Glide', 'duration.desc': 'How long a glide takes to settle',
    'preset.precise': 'Precise', 'preset.balanced': 'Balanced', 'preset.fluid': 'Fluid',
    'graph.distance': 'Distance', 'graph.velocity': 'Speed', 'graph.per': 'One notch moves {px} px; 98% of it is done in {ms} ms.',
    'pad.hint': 'Scroll here with your mouse wheel to feel the settings.', 'pad.idle': 'Scroll to measure', 'pad.speed': 'Speed', 'pad.peak': 'Peak', 'pad.frames': 'Frames', 'pad.jump': 'Largest jump', 'pad.line': 'line {n}',
    'shift.title': 'Shift scrolls sideways', 'shift.desc': 'Hold Shift and turn the wheel to scroll horizontally, smoothly.',
    'option.title': 'Option scrolls faster', 'option.desc': 'Hold Option to scroll several times as far. Good for long documents.',
    'boost.title': 'Boost', 'cmd.title': 'Command zooms', 'cmd.desc': 'Hold Command and turn the wheel to zoom in or out. Sends Command + and Command - in browsers, editors and viewers.',
    'apps.desc': 'With Smooth scrolling on, set an app to Off to leave it alone (a block list). With it off, set an app to On and only those apps glide (an allow list).',
    'apps.rules': 'Per-app rules', 'apps.empty': 'No app rules yet.', 'apps.add': 'Add app', 'apps.smooth': 'Smooth', 'apps.reverse': 'Reverse', 'rule.default': 'Default', 'rule.on': 'On', 'rule.off': 'Off', 'apps.remove': 'Remove',
    'menu.running': 'Running apps', 'menu.disk': 'Choose from disk...', 'menu.none': 'No other apps are open',

    'sec.shortcut': 'Shortcut', 'sec.pasting': 'Pasting', 'sec.history': 'History', 'sec.privacy': 'Privacy', 'sec.keys': 'Keyboard',
    'clip.hotkey.title': 'Open clipboard history', 'clip.hotkey.desc': 'Works everywhere. The popup appears next to the pointer.',
    'paste.auto.title': 'Paste automatically', 'paste.auto.desc': 'Press Enter on an item to paste it into the app you were using. Off copies only.',
    'paste.plain.title': 'Paste without formatting', 'paste.plain.desc': 'Text goes in as plain text. {key} does the opposite for one paste.',
    'limit.title': 'Items to keep', 'limit.desc': 'Pinned items are never removed.', 'count.title': 'In the history now',
    'clear.title': 'Clear history', 'clear.desc': 'Removes every item that is not pinned.', 'clear.btn': 'Clear',
    'clear.confirm.title': 'Clear the clipboard history?', 'clear.confirm.body': 'Every item that is not pinned is removed. This cannot be undone.', 'cancel': 'Cancel',
    'ignored.title': 'Never record copies from', 'ignored.desc': 'Password managers and items marked as concealed are always skipped.', 'ignored.empty': 'No apps added.', 'ignored.add': 'Add app',
    'key.paste': 'Paste', 'key.pastealt': 'Paste the other way', 'key.pin': 'Pin or unpin', 'key.delete': 'Delete', 'key.close': 'Close', 'key.pinkey': 'Paste a pinned item', 'key.filters': 'Filters',
    'key.filters.val': '@image @link @code @file @pin',

    'notch.panel.title': 'Notch panel', 'notch.panel.desc': 'Move the pointer to the top edge of the screen to open it. Off removes it entirely.',
    'notch.preview': 'Hover the top edge', 'sec.tabs': 'Tabs', 'tab.waiting': 'Not in the notch yet: its tab appears when its feature is turned on, under Notch tabs.', 'tab.clipboard': 'Clipboard', 'tab.clipboard.desc': 'The same list as the popup, with search.',
    'tab.shelf': 'Shelf', 'tab.shelf.desc': 'Drop files to keep them handy and drag them out again. Right-click a file to compress it or convert an image. Only references are stored.', 'tab.media': 'Media', 'tab.media.desc': 'Previous, play and next for whatever is playing; track details for Music and Spotify.',
    'sec.volume': 'Volume', 'vol.title': 'Volume indicator in the notch', 'vol.desc': 'Replaces the system volume display. Needs Accessibility. Shift + Option moves in finer steps.',
    'vol.style.title': 'Indicator style', 'vol.style.desc': 'How the volume panel looks while the keys are pressed.',
    'vol.style.notch': 'Notch', 'vol.style.glass': 'Liquid Glass', 'vol.style.corner': 'Corner capsule', 'vol.style.bar': 'Level bar', 'vol.style.island': 'Floating pill', 'vol.style.side': 'Side slider', 'vol.style.tile': 'Large tile', 'vol.style.ring': 'Glass dial', 'vol.style.equalizer': 'Equalizer', 'vol.style.wide': 'Wide slab',
    'vol.style.notch.desc': 'Black, hangs from the notch.', 'vol.style.glass.desc': 'The same hang, in Liquid Glass.', 'vol.style.corner.desc': 'A small glass capsule.', 'vol.style.bar.desc': 'The level fills the notch shape.', 'vol.style.island.desc': 'A glass pill that floats.', 'vol.style.side.desc': 'A tall slim glass slider.', 'vol.style.tile.desc': 'A big speaker and segments.', 'vol.style.ring.desc': 'A round glass dial with a ring.', 'vol.style.equalizer.desc': 'Bars that light up as it grows.', 'vol.style.wide.desc': 'A long thin glass capsule.',
    'vol.place.title': 'Where it shows', 'vol.place.desc': 'For the floating styles; the ones that hang stay under the notch.', 'vol.place.auto': 'Automatic',
    'vol.place.top-left': 'Top left', 'vol.place.top': 'Top', 'vol.place.top-right': 'Top right', 'vol.place.left': 'Left', 'vol.place.center': 'Center', 'vol.place.right': 'Right', 'vol.place.bottom-left': 'Bottom left', 'vol.place.bottom': 'Bottom', 'vol.place.bottom-right': 'Bottom right',
    'vol.preview': 'Preview', 'vol.preview.title': 'Try it', 'vol.preview.desc': 'Shows the indicator now with the style and place you chose.',
    'shelf.count': 'Files on the shelf', 'shelf.clear': 'Clear shelf', 'shelf.clear.desc': 'Removes the references only. Your files are not touched.',
    'sec.shelf': 'Shelf',

    'tiling.enable.title': 'Window snapping', 'tiling.enable.desc': 'Move and resize the focused window with a shortcut. Command+1–9 are global and may replace shortcuts in other apps. Needs Accessibility.',
    'tiling.reset': 'Reset shortcuts', 'tiling.none': 'None', 'tiling.clear': 'Clear shortcut',
    'layout.left': 'Left half', 'layout.right': 'Right half', 'layout.top': 'Top half', 'layout.bottom': 'Bottom half', 'layout.top_left': 'Top left', 'layout.top_right': 'Top right', 'layout.bottom_left': 'Bottom left', 'layout.bottom_right': 'Bottom right',
    'layout.maximize': 'Maximize', 'layout.center': 'Center', 'layout.first_third': 'First third', 'layout.center_third': 'Center third', 'layout.last_third': 'Last third',
    'layout.grid_1': 'Cell 1 · top left', 'layout.grid_2': 'Cell 2 · top middle', 'layout.grid_3': 'Cell 3 · top right',
    'layout.grid_4': 'Cell 4 · middle left', 'layout.grid_5': 'Cell 5 · center', 'layout.grid_6': 'Cell 6 · middle right',
    'layout.grid_7': 'Cell 7 · bottom left', 'layout.grid_8': 'Cell 8 · bottom middle', 'layout.grid_9': 'Cell 9 · bottom right',

    'about.line': 'A light native toolbox for the menu bar and the notch of your Mac.',
    'about.usage.title': 'What Figghost is using', 'about.usage.desc': 'Read every two seconds while this window is open. The network counts the tools Figghost runs, such as curl and gh, since the app opens no connection of its own. Frames are those its animations draw: none while nothing moves.', 'about.usage.wait': 'Reading...',
    'about.usage.cpu': 'CPU', 'about.usage.gpu': 'GPU', 'about.usage.memory': 'Memory', 'about.usage.net': 'Network', 'about.usage.disk': 'Disk', 'about.usage.fps': 'Frames', 'about.usage.fps.n': '{n} fps', 'about.usage.disk.n': 'read {r}/s, write {w}/s',
    'about.version': 'Version', 'about.privacy': 'Privacy', 'about.privacy.val': 'Everything stays on this Mac. The only thing Figghost asks the network by itself is whether a new version exists, once a day.', 'about.quit': 'Quit Figghost',
    'about.quit.desc': 'Stops scrolling, clipboard and notch features until you open the app again.',
    'about.update': 'Check for updates', 'about.update.desc': 'Looks for a new version every day. Press Check now to look at once; a new version is downloaded and installed for you.', 'about.update.check': 'Check now',
    'about.update.checking': 'Checking GitHub…', 'about.update.current': 'This is the latest version ({v}).',
    'about.update.available': 'Version {v} is available.', 'about.update.install': 'Update',
    'about.update.downloading': 'Downloading version {v}… {p}%', 'about.update.installing': 'Installing version {v}…',
    'about.update.ready': 'Version {v} is installed. Restart Figghost to use it.', 'about.update.restart': 'Restart',
    'about.update.blocked': 'Version {v} is out, but it could not be installed here: {detail}', 'about.update.open': 'Download',
    'about.update.failed': 'Could not check: {detail}',
    'update.news.title': "What's new after an update", 'update.news.desc': 'Opens a short tour of the new features the first time a new version runs. Many changes show as slides.', 'update.news.show': "Show what's new",
    'update.auto.title': 'Update automatically', 'update.auto.desc': 'Install a new version as soon as it is found, without asking. Otherwise Figghost asks once a day. Either way, restart Figghost to use it.',

    'rec.press': 'Press shortcut', 'rec.esc': 'Esc to cancel',
    'fault.invalid_value': 'That value is not valid.', 'fault.unknown_setting': 'Unknown setting.', 'fault.invalid_app_id': 'That app cannot be used.', 'fault.needs_modifier': 'Add Control, Option, Shift or Command.',
    'fault.invalid_shortcut': 'That key cannot be used.', 'fault.shortcut_required': 'This shortcut cannot be removed.', 'fault.conflict': 'Already used by {name}.', 'fault.taken': 'Another app already uses this shortcut.',
    'fault.accessibility_needed': 'Accessibility access is needed. Allow Figghost in System Settings.', 'fault.login_failed': 'Could not change the login item: {detail}', 'fault.unknown_action': 'Unknown action.', 'fault.unsupported_key': 'That key is not supported.',
    'target.clipboard': 'the clipboard shortcut',
    'sec.ai': 'AI assistants', 'mcp.title': 'Let AI assistants search the history', 'mcp.desc': 'Starts a local server (MCP) that Claude and other assistants can use to find what you copied. Anything in the history can be read while this is on.',
    'mcp.how': 'Add this to the assistant\'s MCP settings:', 'mcp.copy': 'Copy configuration', 'mcp.copied': 'Copied to the clipboard.',
  },
  vi: {
    'side.search': 'Tìm chức năng', 'side.clear': 'Xóa tìm kiếm', 'side.none': 'Không có mục nào',
    'guide.label': 'Con ma', 'guide.q1': 'Hú.', 'guide.q2': 'Tôi sống trong notch của bạn.', 'guide.q3': 'Ở đây không có gì thừa.', 'guide.q4': 'Lát nữa hỏi lại nhé.',
    'metric.features': 'Đang bật', 'metric.tabs': 'Tab của notch', 'metric.clips': 'Mục đã chép', 'metric.access': 'Trợ năng',
    'cat.general': 'Cài đặt chung', 'cat.keys': 'Phím tắt', 'cat.scroll': 'Chuột & Con trỏ', 'cat.notch': 'Notch & Đảo động', 'cat.agents': 'Tác tử AI & PR', 'cat.calendar': 'Lịch & Tập trung', 'cat.tabs': 'Tiện ích Notch', 'cat.system': 'Hệ thống & Thiết bị', 'cat.clipboard': 'Bảng tạm & Gõ chữ', 'cat.smart': 'Chọn nhanh & Dịch', 'cat.capture': 'Chụp & Tải media', 'cat.tiling': 'Cửa sổ & Ghim',
    'nav.sec.overview': 'Tổng quan', 'nav.sec.notch': 'Notch & Đảo động', 'nav.sec.mouse': 'Chuột & Cửa sổ', 'nav.sec.tools': 'Bảng tạm & Công cụ',
    'nav.general': 'Chung', 'nav.keys': 'Phím tắt', 'nav.scroll': 'Cuộn chuột', 'nav.clipboard': 'Clipboard', 'nav.notch': 'Notch', 'nav.agents': 'Tác tử AI', 'nav.tiling': 'Chia cửa sổ', 'nav.about': 'Giới thiệu',
    'page.general': 'Chung', 'page.general.sub': 'Khởi động, ngôn ngữ và quyền truy cập.',
    'page.keys': 'Phím tắt', 'page.keys.sub': 'Mọi phím Figghost dùng, gom về một chỗ. Nhấn vào một phím tắt rồi bấm tổ hợp phím mới.',
    'keys.sec.panel': 'Trong notch và cửa sổ clipboard', 'keys.board.title': 'Đưa vào bảng kế tiếp', 'keys.board.desc': 'Bảng gom các mục đã ghim phía trên danh sách clipboard.',
    'keys.pastealt.desc': 'Giữ định dạng khi đang bật Dán không định dạng, và ngược lại.',
    'keys.delete.title': 'Xóa, từ chối hoặc bỏ qua', 'keys.delete.desc': 'Xóa mục đang chọn, từ chối yêu cầu của tác tử và bỏ qua nhắc lịch.',
    'keys.sec.fixed': 'Phím chuẩn', 'keys.fixed.desc': 'Dùng như ở mọi nơi trên máy Mac và giữ nguyên.',
    'keys.fixed.enter': 'Dán, cho phép hoặc tham gia', 'keys.fixed.move': 'Di chuyển lựa chọn', 'keys.fixed.tab': 'Chuyển tab hoặc dán một dòng',
    'keys.sec.global': 'Ở mọi nơi trên máy Mac', 'keys.elsewhere': 'Nhiều phím tắt, đặt ở trang riêng.', 'keys.open': 'Mở', 'keys.change': 'Đổi phím',
    'target.keys:pin': 'phím ghim', 'target.keys:board': 'phím đưa vào bảng', 'target.keys:paste_other': 'phím dán theo kiểu ngược lại', 'target.keys:delete': 'phím xóa',
    'fault.reserved': 'Command với một phím đã dùng cho tab, mục đã ghim hoặc Xóa trong panel. Thêm Option, Control hoặc Shift.',
    'page.scroll': 'Cuộn chuột', 'page.scroll.sub': 'Cuộn mượt và đảo chiều cho con lăn chuột. Trackpad và Magic Mouse không bị can thiệp.',
    'page.clipboard': 'Clipboard', 'page.clipboard.sub': 'Lịch sử mọi thứ bạn sao chép, tìm kiếm tức thì, ghim và phím tắt.',
    'page.notch': 'Notch', 'page.notch.sub': 'Bảng mọc ra từ mép trên màn hình: clipboard, khay tệp, media, các tác tử lập trình và lịch của bạn.',
    'page.agents': 'Tác tử AI', 'page.agents.sub': 'Theo dõi Claude Code và Codex ngay trên notch: trả lời yêu cầu cấp quyền, xem chúng đang làm gì và còn bao nhiêu hạn mức.',
    'sec.agents.follow': 'Theo dõi', 'agents.enable.title': 'Theo dõi các tác tử lập trình', 'agents.enable.desc': 'Lắng nghe những gì Claude Code và Codex báo về và hiện trên tab Tác tử của notch.',
    'sec.agents.connect': 'Kết nối', 'agents.claude.title': 'Claude Code', 'agents.claude.desc': 'Thêm hook vào ~/.claude/settings.json. Các hook khác của bạn được giữ nguyên.',
    'agents.codex.title': 'Codex', 'agents.codex.desc': 'Thêm hook vào ~/.codex/hooks.json. Codex sẽ hỏi bạn xem lại hook mới ở lần chạy đầu tiên.',
    'agents.agy.title': 'Antigravity', 'agents.agy.desc': 'Thêm một hook tên figghost vào ~/.gemini/config/hooks.json, bên cạnh hook của bạn. Công cụ dòng lệnh agy khi đó hiện các phiên của nó trên notch: đang chạy, đã xong hay bị lỗi. Nó vẫn hỏi quyền trong terminal của nó như trước.',
    'agents.pi.title': 'Pi', 'agents.pi.desc': 'Thêm một extension vào ~/.pi/agent/extensions/figghost.ts. Pi nạp nó ở lần khởi động tiếp theo; sau đó câu hỏi của Pi trả lời được trên notch cũng như trong terminal của nó.',
    'agents.install': 'Cài', 'agents.remove': 'Gỡ', 'agents.repair': 'Sửa',
    'agents.status.on': 'Đã kết nối', 'agents.status.off': 'Chưa kết nối', 'agents.status.partial': 'Chưa đủ ({n}/{m})', 'agents.status.missing': 'Không tìm thấy', 'agents.status.error': 'Không đọc được tệp của nó',
    'sec.agents.answer': 'Trả lời', 'agents.wait.title': 'Thời gian chờ câu trả lời', 'agents.wait.desc': 'Một lệnh chờ chừng này trên notch. Không có câu trả lời thì tác tử hỏi trong terminal của nó như bình thường.', 'agents.wait.n': '{n} giây',
    'agents.sound.title': 'Phát âm thanh', 'agents.sound.desc': 'Khi một lệnh cần câu trả lời hoặc tác tử làm xong.',
    'agents.peek.title': 'Hiện notch khi tác tử làm xong', 'agents.peek.desc': 'Notch mở vài giây rồi tự đóng, trừ khi con trỏ đang ở trên nó.',
    'sec.agents.limits': 'Hạn mức sử dụng', 'agents.usage.title': 'Hạn mức Claude Code', 'agents.usage.desc': 'Claude Code báo mức đã dùng của hạn mức năm giờ và hạn mức tuần vào dòng trạng thái của nó. Bật mục này thì Figghost chạy ở đó trước rồi mới đến dòng trạng thái của bạn, nên giao diện vẫn như cũ. Hạn mức Codex đọc từ nhật ký phiên của nó, không cần cài gì. Nút làm mới trên tab Tác tử còn nhờ chương trình Codex cài trên máy này báo hạn mức hiện tại: Codex tự liên hệ OpenAI như mỗi lần bạn dùng nó, còn Figghost không gửi gì và không đọc thông tin đăng nhập của Codex.',
    'agents.usage.on': 'Bật', 'agents.usage.off': 'Tắt', 'agents.usage.waiting': 'Đang chờ tin nhắn tiếp theo từ Claude Code.', 'agents.usage.ok': 'Đang bật', 'agents.usage.none': 'Đang tắt',
    'sec.agents.note': 'Cần biết', 'agents.note.title': 'Ứng dụng khác cũng trả lời yêu cầu cấp quyền', 'agents.note.body': 'Nếu một ứng dụng khác, như OneNotch, cũng trả lời các yêu cầu này thì cả hai cùng hỏi và câu trả lời đầu tiên được dùng. Hãy tắt một trong hai.',
    'tab.agents': 'Tác tử', 'tab.agents.desc': 'Claude Code và Codex: lệnh đang chờ bạn, các phiên đang chạy và hạn mức còn lại.',
    'tab.calendar': 'Lịch', 'tab.calendar.desc': 'Hôm nay và ngày mai, các lời nhắc đến hạn và cảnh báo có nút Tham gia trước cuộc họp. Tắt cho đến khi bạn bật ở trang Lịch.',
    'fault.agents_failed': 'Không sửa được tệp của tác tử.',
    'page.tiling': 'Chia cửa sổ', 'page.tiling.sub': 'Đặt cửa sổ đang dùng vào một vùng của màn hình bằng phím tắt.',
    'page.about': 'Giới thiệu', 'page.about.sub': 'Một ứng dụng nhẹ cho các công cụ nhỏ của máy Mac.',

    'hero.tagline': 'Cuộn mượt, lịch sử clipboard và hộp công cụ notch trong một ứng dụng thanh menu gọn nhẹ.',
    'chip.version': 'Phiên bản {v}', 'chip.access.ok': 'Đã có quyền Trợ năng', 'chip.access.no': 'Cần quyền Trợ năng',
    'sec.startup': 'Khởi động', 'sec.language': 'Ngôn ngữ', 'sec.permissions': 'Quyền truy cập', 'sec.data': 'Dữ liệu',
    'login.title': 'Mở khi đăng nhập', 'login.desc': 'Chạy Figghost khi bạn đăng nhập. Cửa sổ này không tự mở; dùng biểu tượng trên thanh menu.',
    'login.approval': 'Hãy cho phép Figghost trong Cài đặt hệ thống, mục Mục đăng nhập, để hoàn tất.', 'login.unavailable': 'Chỉ dùng được khi Figghost chạy như một ứng dụng đặt trong thư mục như Applications.',
    'login.open': 'Mở Mục đăng nhập',
    'showlaunch.title': 'Mở cửa sổ này khi khởi động', 'showlaunch.desc': 'Hiện Cài đặt mỗi khi bạn tự mở ứng dụng. Không bao giờ khi đăng nhập.',
    'hello.title': 'Chào khi mở ứng dụng', 'hello.desc': 'Mỗi khi bạn tự mở Figghost, chú ma chui ra từ notch rồi nháy mắt. Không chạy khi đăng nhập hay khi chạy nền, và chỉ khi notch đang bật.',
    'lang.title': 'Ngôn ngữ', 'lang.desc': 'Cửa sổ này đổi ngay. Menu và notch đổi sau khi khởi động lại.',
    'lang.auto': 'Tự động', 'lang.en': 'English', 'lang.vi': 'Tiếng Việt',
    'lang.restart': 'Hãy khởi động lại Figghost để dùng ngôn ngữ mới trong menu và notch.', 'lang.restart.btn': 'Khởi động lại',
    'access.title': 'Trợ năng', 'access.desc': 'Cần cho cuộn mượt, dán vào ứng dụng khác, chia cửa sổ và chỉ báo âm lượng.',
    'access.granted': 'Đã cấp', 'access.missing': 'Chưa cấp', 'access.open': 'Mở Cài đặt hệ thống',
    'perm.all_title': 'Tất cả quyền', 'perm.all_desc': 'Kiểm tra và cấp tất cả quyền hệ thống mà Figghost sử dụng.', 'perm.all_btn': 'Kiểm tra tất cả quyền…',
    'data.title': 'Thư mục dữ liệu', 'data.desc': 'Nơi lưu cài đặt, lịch sử clipboard và khay tệp. Mọi thứ nằm trên máy Mac này.', 'data.open': 'Mở thư mục',

    'sec.wheel': 'Con lăn chuột', 'sec.feel': 'Cảm giác cuộn', 'sec.try': 'Thử ngay', 'sec.mods': 'Phím bổ trợ', 'sec.apps': 'Ứng dụng',
    'smooth.title': 'Cuộn mượt', 'smooth.desc': 'Biến mỗi nấc lăn thành chuyển động trượt êm.',
    'reverse.title': 'Đảo chiều con lăn', 'reverse.desc': 'Chỉ đảo con lăn của chuột. Trackpad giữ chiều tự nhiên.',
    'step.title': 'Bước cuộn', 'step.desc': 'Quãng đường của một nấc lăn', 'speed.title': 'Tốc độ', 'speed.desc': 'Hệ số nhân cho mọi lần cuộn', 'duration.title': 'Độ trượt', 'duration.desc': 'Thời gian để chuyển động dừng hẳn',
    'preset.precise': 'Chính xác', 'preset.balanced': 'Cân bằng', 'preset.fluid': 'Mượt',
    'graph.distance': 'Quãng đường', 'graph.velocity': 'Tốc độ', 'graph.per': 'Một nấc lăn đi {px} px; 98% hoàn thành sau {ms} ms.',
    'pad.hint': 'Lăn chuột tại đây để cảm nhận các thiết lập.', 'pad.idle': 'Cuộn để đo', 'pad.speed': 'Tốc độ', 'pad.peak': 'Cao nhất', 'pad.frames': 'Số khung', 'pad.jump': 'Bước nhảy lớn nhất', 'pad.line': 'dòng {n}',
    'shift.title': 'Shift để cuộn ngang', 'shift.desc': 'Giữ Shift và lăn chuột để cuộn ngang, vẫn mượt.',
    'option.title': 'Option để cuộn nhanh', 'option.desc': 'Giữ Option để cuộn xa gấp nhiều lần. Hợp với tài liệu dài.',
    'boost.title': 'Hệ số', 'cmd.title': 'Command để phóng to', 'cmd.desc': 'Giữ Command và lăn chuột để phóng to hoặc thu nhỏ. Gửi Command + và Command - trong trình duyệt, trình soạn thảo và trình xem ảnh.',
    'apps.desc': 'Khi Cuộn mượt đang bật, đặt một ứng dụng sang Tắt để bỏ qua nó (danh sách chặn). Khi đang tắt, đặt ứng dụng sang Bật thì chỉ các ứng dụng đó được làm mượt (danh sách cho phép).',
    'apps.rules': 'Quy tắc theo ứng dụng', 'apps.empty': 'Chưa có quy tắc nào.', 'apps.add': 'Thêm ứng dụng', 'apps.smooth': 'Cuộn mượt', 'apps.reverse': 'Đảo chiều', 'rule.default': 'Mặc định', 'rule.on': 'Bật', 'rule.off': 'Tắt', 'apps.remove': 'Xóa',
    'menu.running': 'Ứng dụng đang chạy', 'menu.disk': 'Chọn từ ổ đĩa...', 'menu.none': 'Không có ứng dụng nào khác đang mở',

    'sec.shortcut': 'Phím tắt', 'sec.pasting': 'Dán', 'sec.history': 'Lịch sử', 'sec.privacy': 'Riêng tư', 'sec.keys': 'Bàn phím',
    'clip.hotkey.title': 'Mở lịch sử clipboard', 'clip.hotkey.desc': 'Dùng ở mọi nơi. Cửa sổ nhỏ hiện cạnh con trỏ.',
    'paste.auto.title': 'Dán tự động', 'paste.auto.desc': 'Nhấn Enter ở một mục để dán vào ứng dụng bạn đang dùng. Tắt thì chỉ sao chép.',
    'paste.plain.title': 'Dán không định dạng', 'paste.plain.desc': 'Văn bản vào ở dạng thuần. {key} làm ngược lại cho một lần dán.',
    'limit.title': 'Số mục giữ lại', 'limit.desc': 'Mục đã ghim không bao giờ bị xóa.', 'count.title': 'Hiện có trong lịch sử',
    'clear.title': 'Xóa lịch sử', 'clear.desc': 'Xóa mọi mục chưa ghim.', 'clear.btn': 'Xóa',
    'clear.confirm.title': 'Xóa lịch sử clipboard?', 'clear.confirm.body': 'Mọi mục chưa ghim sẽ bị xóa. Không thể hoàn tác.', 'cancel': 'Hủy',
    'ignored.title': 'Không ghi lại nội dung sao chép từ', 'ignored.desc': 'Trình quản lý mật khẩu và mục được đánh dấu ẩn luôn bị bỏ qua.', 'ignored.empty': 'Chưa thêm ứng dụng nào.', 'ignored.add': 'Thêm ứng dụng',
    'key.paste': 'Dán', 'key.pastealt': 'Dán theo kiểu ngược lại', 'key.pin': 'Ghim hoặc bỏ ghim', 'key.delete': 'Xóa', 'key.close': 'Đóng', 'key.pinkey': 'Dán một mục đã ghim', 'key.filters': 'Bộ lọc',
    'key.filters.val': '@image @link @code @file @pin',

    'notch.panel.title': 'Bảng notch', 'notch.panel.desc': 'Đưa con trỏ lên mép trên màn hình để mở. Tắt thì bảng biến mất hoàn toàn.',
    'notch.preview': 'Rê chuột lên mép trên', 'sec.tabs': 'Các tab', 'tab.waiting': 'Chưa có trong notch: tab hiện ra khi bật tính năng của nó, ở mục Tab của notch.', 'tab.clipboard': 'Clipboard', 'tab.clipboard.desc': 'Cùng danh sách như cửa sổ nhỏ, có tìm kiếm.',
    'tab.shelf': 'Khay', 'tab.shelf.desc': 'Thả tệp vào để giữ tạm và kéo ra lại. Nhấp chuột phải vào tệp để nén hoặc đổi định dạng ảnh. Chỉ lưu đường dẫn tham chiếu.', 'tab.media': 'Media', 'tab.media.desc': 'Trước, phát và sau cho ứng dụng đang phát; thông tin bài hát cho Music và Spotify.',
    'sec.volume': 'Âm lượng', 'vol.title': 'Chỉ báo âm lượng trong notch', 'vol.desc': 'Thay màn hình âm lượng của hệ thống. Cần quyền Trợ năng. Shift + Option để chỉnh từng bước nhỏ.',
    'vol.style.title': 'Kiểu chỉ báo', 'vol.style.desc': 'Bảng âm lượng trông thế nào trong lúc nhấn phím.',
    'vol.style.notch': 'Notch', 'vol.style.glass': 'Liquid Glass', 'vol.style.corner': 'Viên góc phải', 'vol.style.bar': 'Thanh mức', 'vol.style.island': 'Viên nổi giữa', 'vol.style.side': 'Thanh dọc bên phải', 'vol.style.tile': 'Ô lớn giữa màn hình', 'vol.style.ring': 'Vòng kính tròn', 'vol.style.equalizer': 'Thanh cân bằng', 'vol.style.wide': 'Thanh dài',
    'vol.style.notch.desc': 'Màu đen, treo dưới notch.', 'vol.style.glass.desc': 'Cùng dáng treo, bằng Liquid Glass.', 'vol.style.corner.desc': 'Viên kính nhỏ gọn.', 'vol.style.bar.desc': 'Mức âm lượng lấp đầy hình notch.', 'vol.style.island.desc': 'Viên kính nổi.', 'vol.style.side.desc': 'Thanh trượt kính mảnh, dựng đứng.', 'vol.style.tile.desc': 'Loa lớn và các vạch mức.', 'vol.style.ring.desc': 'Vòng kính tròn có vành tiến độ.', 'vol.style.equalizer.desc': 'Các cột sáng dần khi to lên.', 'vol.style.wide.desc': 'Viên kính dài và mảnh.',
    'vol.place.title': 'Vị trí hiển thị', 'vol.place.desc': 'Dùng cho các kiểu nổi; kiểu treo vẫn nằm dưới notch.', 'vol.place.auto': 'Tự động',
    'vol.place.top-left': 'Trên trái', 'vol.place.top': 'Trên giữa', 'vol.place.top-right': 'Trên phải', 'vol.place.left': 'Giữa trái', 'vol.place.center': 'Chính giữa', 'vol.place.right': 'Giữa phải', 'vol.place.bottom-left': 'Dưới trái', 'vol.place.bottom': 'Dưới giữa', 'vol.place.bottom-right': 'Dưới phải',
    'vol.preview': 'Xem thử', 'vol.preview.title': 'Thử ngay', 'vol.preview.desc': 'Hiện chỉ báo ngay với kiểu và vị trí đã chọn.',
    'shelf.count': 'Tệp trên khay', 'shelf.clear': 'Dọn khay', 'shelf.clear.desc': 'Chỉ xóa phần tham chiếu. Tệp của bạn không bị động tới.',
    'sec.shelf': 'Khay tệp',

    'tiling.enable.title': 'Chia cửa sổ', 'tiling.enable.desc': 'Di chuyển và đổi cỡ cửa sổ đang dùng bằng phím tắt. Command+1–9 là phím toàn hệ thống, có thể thay phím tắt của ứng dụng khác. Cần quyền Trợ năng.',
    'tiling.reset': 'Đặt lại phím tắt', 'tiling.none': 'Chưa có', 'tiling.clear': 'Xóa phím tắt',
    'layout.left': 'Nửa trái', 'layout.right': 'Nửa phải', 'layout.top': 'Nửa trên', 'layout.bottom': 'Nửa dưới', 'layout.top_left': 'Góc trên trái', 'layout.top_right': 'Góc trên phải', 'layout.bottom_left': 'Góc dưới trái', 'layout.bottom_right': 'Góc dưới phải',
    'layout.maximize': 'Phóng to tối đa', 'layout.center': 'Giữa màn hình', 'layout.first_third': 'Một phần ba đầu', 'layout.center_third': 'Một phần ba giữa', 'layout.last_third': 'Một phần ba cuối',
    'layout.grid_1': 'Ô 1 · trên trái', 'layout.grid_2': 'Ô 2 · trên giữa', 'layout.grid_3': 'Ô 3 · trên phải',
    'layout.grid_4': 'Ô 4 · giữa trái', 'layout.grid_5': 'Ô 5 · chính giữa', 'layout.grid_6': 'Ô 6 · giữa phải',
    'layout.grid_7': 'Ô 7 · dưới trái', 'layout.grid_8': 'Ô 8 · dưới giữa', 'layout.grid_9': 'Ô 9 · dưới phải',

    'about.line': 'Hộp công cụ gọn nhẹ, bản địa, cho thanh menu và notch của máy Mac.',
    'about.usage.title': 'Figghost đang dùng', 'about.usage.desc': 'Đọc mỗi hai giây khi cửa sổ này mở. Mạng tính cả các công cụ Figghost chạy như curl và gh, vì bản thân app không tự mở kết nối nào. Khung hình là của các hiệu ứng app đang vẽ: bằng 0 khi không có gì chuyển động.', 'about.usage.wait': 'Đang đọc...',
    'about.usage.cpu': 'CPU', 'about.usage.gpu': 'GPU', 'about.usage.memory': 'Bộ nhớ', 'about.usage.net': 'Mạng', 'about.usage.disk': 'Ổ đĩa', 'about.usage.fps': 'Khung hình', 'about.usage.fps.n': '{n} fps', 'about.usage.disk.n': 'đọc {r}/s, ghi {w}/s',
    'about.version': 'Phiên bản', 'about.privacy': 'Riêng tư', 'about.privacy.val': 'Mọi thứ nằm trên máy Mac này. Thứ duy nhất Figghost tự hỏi qua mạng là đã có bản mới hay chưa, mỗi ngày một lần.', 'about.quit': 'Thoát Figghost',
    'about.quit.desc': 'Dừng cuộn mượt, clipboard và notch cho đến khi bạn mở lại ứng dụng.',
    'about.update': 'Kiểm tra bản mới', 'about.update.desc': 'Mỗi ngày tự hỏi GitHub xem có bản mới chưa. Bấm Kiểm tra để hỏi ngay; bản mới sẽ được tải và cài sẵn cho bạn.', 'about.update.check': 'Kiểm tra',
    'about.update.checking': 'Đang hỏi GitHub…', 'about.update.current': 'Bạn đang dùng bản mới nhất ({v}).',
    'about.update.available': 'Đã có bản {v}.', 'about.update.install': 'Cập nhật',
    'about.update.downloading': 'Đang tải bản {v}… {p}%', 'about.update.installing': 'Đang cài bản {v}…',
    'about.update.ready': 'Đã cài bản {v}. Khởi động lại Figghost để dùng.', 'about.update.restart': 'Khởi động lại',
    'about.update.blocked': 'Đã có bản {v} nhưng không cài được ở đây: {detail}', 'about.update.open': 'Tải về',
    'about.update.failed': 'Không kiểm tra được: {detail}',
    'update.news.title': 'Có gì mới sau khi cập nhật', 'update.news.desc': 'Mở bản giới thiệu ngắn các tính năng mới ngay lần đầu chạy bản mới. Nhiều thay đổi sẽ hiện dạng trang trượt.', 'update.news.show': 'Xem có gì mới',
    'update.auto.title': 'Tự động cập nhật', 'update.auto.desc': 'Cài bản mới ngay khi tìm thấy, không hỏi trước. Nếu tắt, Figghost hỏi bạn mỗi ngày một lần. Dù cách nào, bạn cũng cần khởi động lại Figghost để dùng bản mới.',

    'rec.press': 'Nhấn phím tắt', 'rec.esc': 'Esc để hủy',
    'fault.invalid_value': 'Giá trị không hợp lệ.', 'fault.unknown_setting': 'Thiết lập không tồn tại.', 'fault.invalid_app_id': 'Không dùng được ứng dụng này.', 'fault.needs_modifier': 'Thêm Control, Option, Shift hoặc Command.',
    'fault.invalid_shortcut': 'Không dùng được phím này.', 'fault.shortcut_required': 'Không thể xóa phím tắt này.', 'fault.conflict': 'Đã được dùng cho {name}.', 'fault.taken': 'Ứng dụng khác đang dùng phím tắt này.',
    'fault.accessibility_needed': 'Cần quyền Trợ năng. Hãy cho phép Figghost trong Cài đặt hệ thống.', 'fault.login_failed': 'Không đổi được mục đăng nhập: {detail}', 'fault.unknown_action': 'Hành động không tồn tại.', 'fault.unsupported_key': 'Phím này chưa được hỗ trợ.',
    'target.clipboard': 'phím tắt clipboard',
    'sec.ai': 'Trợ lý AI', 'mcp.title': 'Cho trợ lý AI tìm trong lịch sử', 'mcp.desc': 'Chạy một máy chủ cục bộ (MCP) để Claude và các trợ lý khác tìm nội dung bạn đã sao chép. Mọi thứ trong lịch sử đều đọc được khi bật mục này.',
    'mcp.how': 'Thêm đoạn này vào cài đặt MCP của trợ lý:', 'mcp.copy': 'Sao chép cấu hình', 'mcp.copied': 'Đã chép vào clipboard.',
  },
};

// The notch, general and agents settings that came with the notch's live activities. Kept apart from the table
// above so each block reads as one piece; they are merged into it before anything is drawn.
Object.assign(I18N.en, {
  'page.notch.sub': 'A panel that grows out of the top edge of your screen. Closed, it shows what is playing and what your coding agents do; under the pointer it opens into your tabs.',
  'sec.notch.behavior': 'Behaviour',
  'notch.hover.title': 'Open on hover', 'notch.hover.desc': 'Move the pointer onto the notch to open it. Off: a click opens it.',
  'notch.delay.title': 'Hover delay', 'notch.delay.desc': 'How long the pointer rests on the notch before it opens. A longer delay keeps it shut when the pointer only passes by.',
  'notch.delay.none': 'Immediately', 'notch.delay.ms': '{n} ms',
  'notch.width.title': 'Width', 'notch.width.desc': 'How wide the open notch is. Automatic fits each tab; a wider notch keeps the tab in the middle.',
  'notch.width.auto': 'Automatic', 'notch.width.pt': '{n} pt',
  'sec.notch.style': 'Size', 'notch.style.compact': 'Compact', 'notch.style.compact.desc': 'A narrower closed pill', 'notch.style.standard': 'Standard', 'notch.style.standard.desc': 'Fits each tab',
  'notch.style.wide': 'Wide', 'notch.style.wide.desc': 'A big open notch', 'notch.style.custom': 'Custom', 'notch.style.custom.desc': 'Set each size',
  'notch.style.reset': 'Reset', 'notch.style.reset.title': 'Back to standard', 'notch.style.reset.desc': 'Every size returns to automatic.',
  'notch.height.title': 'Height', 'notch.height.desc': 'How tall the open notch is. Automatic fits each tab; a taller notch leaves room below it.',
  'notch.restwidth.title': 'Closed width', 'notch.restwidth.desc': 'How wide the closed notch is on a screen without a hardware notch. A real notch keeps its own size.',
  'notch.restheight.title': 'Closed height', 'notch.restheight.desc': 'How tall the closed notch is on a screen without a hardware notch.',
  'notch.openpage.title': 'Opens on', 'notch.openpage.desc': 'The tab shown when the notch opens.',
  'notch.openpage.activity': 'What is live', 'notch.openpage.last': 'The last tab used',
  'notch.haptics.title': 'Haptic feedback', 'notch.haptics.desc': 'A light tap on a Force Touch trackpad when the notch opens under the pointer.',
  'notch.shortcut.title': 'Open or close the notch', 'notch.shortcut.desc': 'A shortcut that works everywhere. Press Delete while recording to remove it.',
  'sec.notch.look': 'Appearance',
  'notch.material.title': 'Material', 'notch.material.desc': 'Liquid Glass lets the desktop show through and bends it at the edges. Closed, the notch stays black, like a hardware notch.',
  'notch.material.glass': 'Liquid Glass', 'notch.material.clear': 'Clear glass', 'notch.material.ink': 'Ink glass', 'notch.material.accent': 'Accent glass', 'notch.material.black': 'Black',
  'notch.motion.title': 'Motion', 'notch.motion.desc': 'How the notch moves as it opens, closes and widens. Reduce Motion in System Settings turns movement off.',
  'notch.motion.spring': 'Spring', 'notch.motion.smooth': 'Smooth', 'notch.motion.none': 'None',
  'notch.shadow.title': 'Shadow', 'notch.shadow.desc': 'A soft shadow under the open notch.',
  'sec.notch.display': 'Display',
  'notch.display.title': 'Show on', 'notch.display.desc': 'The screen the notch lives on.',
  'notch.display.auto': 'The screen with a notch', 'notch.display.main': 'The main screen', 'notch.display.pointer': 'Where the pointer is',
  'notch.rest.title': 'On a screen without a notch', 'notch.rest.desc': 'A black tab where a notch would be, or nothing until the pointer reaches the top edge.',
  'notch.rest.hardware': 'This screen has a notch of its own.',
  'notch.rest.notch': 'Notch', 'notch.rest.hidden': 'Hidden',
  'notch.capture.title': 'Hide in screenshots and recordings', 'notch.capture.desc': 'The notch stays out of every screenshot and screen recording, including this app\'s own.',
  'notch.fullscreen.title': 'Show over full-screen apps', 'notch.fullscreen.desc': 'Off hides the notch completely while the app in front is full screen on its screen, and brings it back when that app leaves full screen.',
  'tabs.desc': 'Turn tabs on or off and change their order. Command + 1 to 9 follow this order.',
  'tab.sound': 'Sound', 'tab.sound.desc': 'Volume, output and equalizer for each app that plays.',
  'tab.media.desc': 'Previous, play and next for whatever plays, browsers included, with its cover, the app it comes from and the time.',
  'tab.up': 'Move up', 'tab.down': 'Move down',
  'sec.notch.live': 'Live activities',
  'notch.livemedia.title': 'Music in the closed notch', 'notch.livemedia.desc': 'While something plays, in any app or browser, the closed notch shows its cover and moving bars. Point at it to open the player.',
  'notch.waveform.title': 'Bars', 'notch.waveform.desc': 'Follow the sound the Mac plays (macOS asks once for System Audio Recording; nothing is recorded), or only move.',
  'notch.waveform.live': 'Follow the sound', 'notch.waveform.animated': 'Move only',
  'notch.liveagents.title': 'Coding agents in the closed notch', 'notch.liveagents.desc': 'While Claude Code or Codex works, the closed notch shows it with the share of the five-hour limit used. A question waits there too.',
  'target.notch': 'the notch shortcut',

  'sec.menuicon': 'Menu bar', 'menuicon.title': 'Show the icon in the menu bar', 'menuicon.desc': 'Without it, open Figghost again from Finder or Spotlight to reach these settings.',
  'sec.appearance': 'Appearance', 'theme.title': 'Settings window', 'theme.desc': 'Light, dark, or as the system is.',
  'general.theme.auto': 'Automatic', 'general.theme.light': 'Light', 'general.theme.dark': 'Dark',

  'agents.ask.title': 'Open the notch for a question', 'agents.ask.desc': 'Off: a question waits in the closed notch, breathing, until you point at it. On: the notch opens by itself.',
  'agents.peek.title': 'Say so in the notch when an agent finishes', 'agents.peek.desc': 'The closed notch shows it for a few seconds; it does not open.',
  'sec.agents.tokens': 'Tokens and cost',
  'agents.tokens.title': 'Count tokens and cost', 'agents.tokens.desc': 'Read from the transcripts Claude Code and Codex keep on this Mac; nothing is sent anywhere. The cost is what the same work would cost at Anthropic\'s API list prices. A subscription is billed a flat fee, so it is not a charge.',
  'agents.spend.claude': 'Claude Code', 'agents.spend.codex': 'Codex',
  'agents.spend.today': 'Today {tokens} tokens', 'agents.spend.week': 'Last 7 days {tokens} tokens',
  'agents.spend.none': 'Nothing counted yet. The numbers appear once Claude Code or Codex has run on this Mac.',
});
Object.assign(I18N.vi, {
  'page.notch.sub': 'Bảng mọc ra từ mép trên màn hình. Khi đóng, nó cho thấy bài đang phát và việc các tác tử lập trình đang làm; rê chuột lên thì mở ra các tab.',
  'sec.notch.behavior': 'Hành vi',
  'notch.hover.title': 'Mở khi rê chuột', 'notch.hover.desc': 'Đưa con trỏ lên notch để mở. Tắt thì cần nhấp chuột.',
  'notch.delay.title': 'Độ trễ khi rê chuột', 'notch.delay.desc': 'Thời gian con trỏ dừng trên notch trước khi notch mở. Trễ lâu hơn thì notch không mở nhầm khi chuột chỉ lướt qua.',
  'notch.delay.none': 'Ngay lập tức', 'notch.delay.ms': '{n} ms',
  'notch.width.title': 'Độ rộng', 'notch.width.desc': 'Độ rộng của notch khi mở. Tự động là vừa với từng tab; notch rộng hơn vẫn giữ tab ở giữa.',
  'notch.width.auto': 'Tự động', 'notch.width.pt': '{n} pt',
  'sec.notch.style': 'Kích thước', 'notch.style.compact': 'Gọn', 'notch.style.compact.desc': 'Viên đóng hẹp hơn', 'notch.style.standard': 'Chuẩn', 'notch.style.standard.desc': 'Vừa với từng tab',
  'notch.style.wide': 'Rộng', 'notch.style.wide.desc': 'Notch mở lớn', 'notch.style.custom': 'Tùy chỉnh', 'notch.style.custom.desc': 'Tự đặt từng kích thước',
  'notch.style.reset': 'Đặt lại', 'notch.style.reset.title': 'Về mặc định', 'notch.style.reset.desc': 'Mọi kích thước trở về tự động.',
  'notch.height.title': 'Chiều cao', 'notch.height.desc': 'Chiều cao của notch khi mở. Tự động là vừa với từng tab; notch cao hơn thì chừa thêm chỗ bên dưới.',
  'notch.restwidth.title': 'Độ rộng khi đóng', 'notch.restwidth.desc': 'Độ rộng của notch khi đóng trên màn hình không có tai thỏ. Tai thỏ thật giữ nguyên kích thước của nó.',
  'notch.restheight.title': 'Chiều cao khi đóng', 'notch.restheight.desc': 'Chiều cao của notch khi đóng trên màn hình không có tai thỏ.',
  'notch.openpage.title': 'Mở vào', 'notch.openpage.desc': 'Tab hiện ra khi notch mở.',
  'notch.openpage.activity': 'Theo hoạt động', 'notch.openpage.last': 'Tab dùng gần nhất',
  'notch.haptics.title': 'Phản hồi xúc giác', 'notch.haptics.desc': 'Một cú chạm nhẹ trên trackpad Force Touch khi notch mở dưới con trỏ.',
  'notch.shortcut.title': 'Mở hoặc đóng notch', 'notch.shortcut.desc': 'Phím tắt dùng ở mọi nơi. Nhấn Delete khi đang ghi để xóa.',
  'sec.notch.look': 'Giao diện',
  'notch.material.title': 'Chất liệu', 'notch.material.desc': 'Liquid Glass để màn hình phía sau hiện qua và khúc xạ ở mép. Khi đóng, notch vẫn màu đen như notch phần cứng.',
  'notch.material.glass': 'Liquid Glass', 'notch.material.clear': 'Kính trong', 'notch.material.ink': 'Kính mực', 'notch.material.accent': 'Kính màu nhấn', 'notch.material.black': 'Đen',
  'notch.motion.title': 'Chuyển động', 'notch.motion.desc': 'Cách notch chuyển động khi mở, đóng và nới rộng. Giảm chuyển động trong Cài đặt hệ thống sẽ tắt chuyển động.',
  'notch.motion.spring': 'Lò xo', 'notch.motion.smooth': 'Mượt', 'notch.motion.none': 'Không',
  'notch.shadow.title': 'Đổ bóng', 'notch.shadow.desc': 'Một lớp bóng mềm dưới notch khi mở.',
  'sec.notch.display': 'Màn hình',
  'notch.display.title': 'Hiển thị trên', 'notch.display.desc': 'Màn hình mà notch nằm trên đó.',
  'notch.display.auto': 'Màn hình có notch', 'notch.display.main': 'Màn hình chính', 'notch.display.pointer': 'Màn hình có con trỏ',
  'notch.rest.title': 'Trên màn hình không có notch', 'notch.rest.desc': 'Hiện một khối đen ở chỗ notch, hoặc không hiện gì cho đến khi con trỏ chạm mép trên.',
  'notch.rest.hardware': 'Màn hình này có notch riêng.',
  'notch.rest.notch': 'Notch', 'notch.rest.hidden': 'Ẩn',
  'notch.capture.title': 'Ẩn khi chụp và quay màn hình', 'notch.capture.desc': 'Notch không xuất hiện trong ảnh chụp và video quay màn hình, kể cả của chính ứng dụng này.',
  'notch.fullscreen.title': 'Hiện khi app toàn màn hình', 'notch.fullscreen.desc': 'Khi tắt, notch ẩn hẳn trong lúc app phía trước đang toàn màn hình trên màn hình của notch, và hiện lại khi app đó thoát toàn màn hình.',
  'tabs.desc': 'Bật, tắt và sắp xếp các tab. Command + 1 đến 9 theo đúng thứ tự này.',
  'tab.sound': 'Âm thanh', 'tab.sound.desc': 'Âm lượng, thiết bị phát và bộ chỉnh âm cho từng ứng dụng đang phát.',
  'tab.media.desc': 'Trước, phát và sau cho mọi thứ đang phát, kể cả trình duyệt, kèm ảnh bìa, ứng dụng nguồn và thời gian.',
  'tab.up': 'Đưa lên', 'tab.down': 'Đưa xuống',
  'sec.notch.live': 'Hoạt động trực tiếp',
  'notch.livemedia.title': 'Nhạc trên notch đang đóng', 'notch.livemedia.desc': 'Khi có nội dung đang phát, ở bất kỳ ứng dụng hay trình duyệt nào, notch đang đóng hiện ảnh bìa và sóng nhạc. Đưa con trỏ lên để mở trình phát.',
  'notch.waveform.title': 'Sóng nhạc', 'notch.waveform.desc': 'Chạy theo âm thanh máy đang phát (macOS hỏi quyền Ghi âm hệ thống một lần; không ghi lại gì), hoặc chỉ chuyển động.',
  'notch.waveform.live': 'Theo âm thanh', 'notch.waveform.animated': 'Chỉ chuyển động',
  'notch.liveagents.title': 'Tác tử lập trình trên notch đang đóng', 'notch.liveagents.desc': 'Khi Claude Code hoặc Codex đang làm việc, notch đang đóng cho thấy điều đó cùng mức đã dùng của hạn mức năm giờ. Câu hỏi cấp quyền cũng chờ ở đó.',
  'target.notch': 'phím tắt notch',

  'sec.menuicon': 'Thanh menu', 'menuicon.title': 'Hiện biểu tượng trên thanh menu', 'menuicon.desc': 'Khi tắt, mở lại Figghost từ Finder hoặc Spotlight để vào phần cài đặt này.',
  'sec.appearance': 'Giao diện', 'theme.title': 'Cửa sổ cài đặt', 'theme.desc': 'Sáng, tối hoặc theo hệ thống.',
  'general.theme.auto': 'Tự động', 'general.theme.light': 'Sáng', 'general.theme.dark': 'Tối',

  'agents.ask.title': 'Mở notch khi có câu hỏi', 'agents.ask.desc': 'Tắt: câu hỏi chờ trên notch đang đóng, nhấp nháy nhẹ, cho đến khi bạn đưa con trỏ lên. Bật: notch tự mở ra.',
  'agents.peek.title': 'Báo trên notch khi tác tử làm xong', 'agents.peek.desc': 'Notch đang đóng báo xong trong vài giây, không tự mở ra.',
  'sec.agents.tokens': 'Token và chi phí',
  'agents.tokens.title': 'Đếm token và chi phí', 'agents.tokens.desc': 'Đọc từ các bản ghi hội thoại Claude Code và Codex lưu trên máy Mac này; không gửi đi đâu. Chi phí là số tiền nếu làm cùng việc đó qua API theo bảng giá niêm yết của Anthropic. Gói đăng ký tính phí cố định nên đây không phải khoản bị trừ.',
  'agents.spend.claude': 'Claude Code', 'agents.spend.codex': 'Codex',
  'agents.spend.today': 'Hôm nay {tokens} token', 'agents.spend.week': '7 ngày qua {tokens} token',
  'agents.spend.none': 'Chưa có số liệu. Con số hiện ra sau khi Claude Code hoặc Codex chạy trên máy này.',
});

let lang = 'en';
const missingKeys = new Set();
function t(key, vars) {
  if (!I18N.en[key]) missingKeys.add(key);
  let text = (I18N[lang] && I18N[lang][key]) || I18N.en[key] || key;
  for (const [name, value] of Object.entries(vars || {})) text = text.replaceAll('{' + name + '}', value);
  return text;
}

function resolveLang(setting) {
  if (setting === 'en' || setting === 'vi') return setting;
  for (const tag of navigator.languages || [navigator.language || 'en']) {
    const primary = String(tag).slice(0, 2).toLowerCase();
    if (I18N[primary]) return primary;
  }
  return 'en';
}

// ---------------------------------------------------------------- state and bridge

let S = null;               // the latest state from the app
const syncs = [];           // functions that bring the visible controls in line with S
const iconCache = {};
const pendingIcons = new Set();
let iconTimer = 0;

function send(message) { window.fig.send(message); }
function get(path) { return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), S); }
function setLocal(path, value) {
  const keys = path.split('.');
  const last = keys.pop();
  keys.reduce((o, k) => o[k], S)[last] = value;
}
function reg(fn) { syncs.push(fn); fn(); return fn; }
function runSyncs() { for (const fn of [...syncs]) fn(); }
/** Runs `build` and returns the sync functions it registered, so a list can drop them when it redraws. */
function scoped(build) {
  const before = syncs.length;
  build();
  return syncs.slice(before);
}
function dropSyncs(list) {
  for (const fn of list) {
    const i = syncs.indexOf(fn);
    if (i >= 0) syncs.splice(i, 1);
  }
}

function change(path, value) {
  setLocal(path, value);
  runSyncs();
  throttleBy(path, 40, () => send({ t: 'set', path, value: get(path) }));
}

function requestIcons() {
  clearTimeout(iconTimer);
  iconTimer = setTimeout(() => {
    const ids = [...pendingIcons];
    pendingIcons.clear();
    if (ids.length) send({ t: 'icons', ids });
  }, 30);
}
function onIcons(map) {
  Object.assign(iconCache, map);
  for (const img of document.querySelectorAll('img[data-id]')) {
    const url = iconCache[img.dataset.id];
    if (url && img.getAttribute('src') !== url) img.setAttribute('src', url);
  }
}
function iconImg(id) {
  const img = h('img', { class: 'app-icon', alt: '', 'data-id': id });
  if (iconCache[id]) img.setAttribute('src', iconCache[id]);
  else { pendingIcons.add(id); requestIcons(); }
  return img;
}

// ---------------------------------------------------------------- shortcuts

const MOD_ORDER = ['ctrl', 'opt', 'shift', 'cmd'];
const MOD_ALIASES = { command: 'cmd', control: 'ctrl', option: 'opt', alt: 'opt' };
const KEY_ALIASES = { enter: 'return', escape: 'esc' };
const SYMBOLS = { ctrl: '⌃', opt: '⌥', shift: '⇧', cmd: '⌘', left: '←', right: '→', up: '↑', down: '↓', return: '↩', space: 'Space', tab: '⇥', esc: '⎋', delete: '⌫' };
const MODIFIER_CODES = new Set(['ShiftLeft', 'ShiftRight', 'ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'MetaLeft', 'MetaRight', 'CapsLock', 'Fn']);

/** The key name the app understands for a KeyboardEvent.code, or null when it does not support it. */
function keyName(code) {
  let m;
  if ((m = /^Key([A-Z])$/.exec(code))) return m[1].toLowerCase();
  if ((m = /^Digit(\d)$/.exec(code))) return m[1];
  if ((m = /^F(\d{1,2})$/.exec(code)) && +m[1] >= 1 && +m[1] <= 20) return 'f' + m[1];
  return { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down', Enter: 'return', NumpadEnter: 'return', Space: 'space', Tab: 'tab', Backspace: 'delete' }[code] || null;
}

/** "cmd+shift+c" as the parts to draw: modifiers in the order macOS writes them, then the key. */
function shortcutParts(shortcut) {
  const parts = String(shortcut || '').split('+').map((p) => p.trim().toLowerCase()).filter(Boolean)
    .map((p) => MOD_ALIASES[p] || KEY_ALIASES[p] || p);
  return [...MOD_ORDER.filter((m) => parts.includes(m)), ...parts.filter((p) => !MOD_ORDER.includes(p))];
}
function keyLabel(part) { return SYMBOLS[part] || part.toUpperCase(); }
/** A shortcut as macOS writes it in a sentence: "opt+shift+return" is "⌥⇧↩". */
function keysText(shortcut) { return shortcutParts(shortcut).map(keyLabel).join(''); }
function keycapsEl(shortcut) {
  return h('span', { class: 'keys' }, shortcutParts(shortcut).map((p) => h('kbd', { text: keyLabel(p) })));
}

function shortcutOf(target) {
  if (target === 'clipboard') return S.clipboard.hotkey;
  // A feature's shortcut: target "<feature>" reads S.<feature>.shortcut, "<feature>:<what>" reads S.<feature>.shortcuts.<what>.
  const [head, what] = target.split(':');
  const own = S[head];
  if (own && head !== 'tiling') return (what === undefined ? own.shortcut : (own.shortcuts || {})[what]) || '';
  const key = target.split(':')[1];
  return (S.tiling.layouts.find((l) => l.key === key) || {}).shortcut || '';
}

let rec = null; // the shortcut button that is capturing a key combination

function stopRecording() {
  if (!rec) return;
  const { el, render } = rec;
  rec = null;
  el.classList.remove('recording');
  window.removeEventListener('keydown', onRecordKey, true);
  window.removeEventListener('mousedown', onRecordAway, true);
  send({ t: 'recording', on: false });
  render();
}
function startRecording(el, target, allowClear, render) {
  if (rec) stopRecording();
  rec = { el, target, allowClear, render };
  el.classList.add('recording');
  send({ t: 'recording', on: true });
  render();
  window.addEventListener('keydown', onRecordKey, true);
  window.addEventListener('mousedown', onRecordAway, true);
}
function onRecordAway(e) { if (rec && !rec.el.contains(e.target)) stopRecording(); }
/** The app took a shortcut the system would have kept from the page (Control+Space), and tells it here. */
function onChord(m) {
  if (!rec) return;
  if (!m.value) return toast(t('fault.unsupported_key'));
  send({ t: 'hotkey', target: rec.target, value: m.value });
  stopRecording();
}
function onRecordKey(e) {
  e.preventDefault();
  e.stopPropagation();
  if (MODIFIER_CODES.has(e.code)) return;
  const noMods = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
  if (e.code === 'Escape') return stopRecording();
  if ((e.code === 'Backspace' || e.code === 'Delete') && noMods && rec.allowClear) {
    send({ t: 'hotkey', target: rec.target, value: '' });
    return stopRecording();
  }
  const key = keyName(e.code);
  if (!key) return toast(t('fault.unsupported_key'));
  const mods = [];
  if (e.ctrlKey) mods.push('ctrl');
  if (e.altKey) mods.push('opt');
  if (e.shiftKey) mods.push('shift');
  if (e.metaKey) mods.push('cmd');
  if (!mods.length) return toast(t('fault.needs_modifier'));
  send({ t: 'hotkey', target: rec.target, value: [...mods, key].join('+') });
  stopRecording();
}

// ---------------------------------------------------------------- feedback

let toastTimer = 0;
function toast(text) {
  const el = $('#toast');
  el.replaceChildren(h('div', { class: 'glass', role: 'status', text }));
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 3600);
}

function onFault(m) {
  let text;
  if (m.code === 'conflict') text = t('fault.conflict', { name: targetName(m.detail) });
  else if (m.code === 'login_failed') text = t('fault.login_failed', { detail: m.detail });
  else text = t('fault.' + m.code);
  toast(text);
}
function targetName(target) {
  if (target === 'clipboard') return t('target.clipboard');
  if (I18N.en['target.' + target]) return t('target.' + target);
  return t('layout.' + String(target).replace('tiling:', ''));
}

function closeModal() {
  $('#modal').classList.remove('show');
  window.removeEventListener('keydown', modalKey, true);
}
function modalKey(e) { if (e.key === 'Escape') { e.stopPropagation(); closeModal(); } }
function confirmDialog({ title, body, confirm }, onYes) {
  const cancel = h('button', { class: 'btn', text: t('cancel'), onclick: closeModal });
  const yes = h('button', { class: 'btn primary', text: confirm, onclick: () => { closeModal(); onYes(); } });
  $('#modal').replaceChildren(h('div', { class: 'dialog glass', role: 'dialog', 'aria-modal': 'true', 'aria-label': title },
    h('h3', { text: title }), h('p', { text: body }), h('div', { class: 'actions' }, cancel, yes)));
  $('#modal').classList.add('show');
  window.addEventListener('keydown', modalKey, true);
  cancel.focus();
}

// ---------------------------------------------------------------- components

function switchEl(path, label) {
  const el = h('button', { class: 'switch', role: 'switch', 'aria-checked': 'false', 'aria-label': label || '', onclick: () => change(path, !get(path)) });
  reg(() => el.setAttribute('aria-checked', String(!!get(path))));
  return el;
}

function row(title, desc, control, opts = {}) {
  const el = h('div', { class: 'row' + (opts.block ? ' block' : '') },
    h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, title), desc && h('div', { class: 'row-desc', text: desc })),
    control && h('div', { class: 'row-control' }, control));
  if (opts.when) reg(() => el.classList.toggle('disabled', !opts.when()));
  return el;
}
function toggleRow(path, title, desc, opts) { return row(title, desc, switchEl(path, title), opts); }
function card(...rows) { return h('section', { class: 'card' }, rows); }
function sectionTitle(text) { return h('div', { class: 'section-title', text }); }

function segmented({ options, value, onPick, small, label }) {
  const thumb = h('span', { class: 'seg-thumb' });
  const buttons = options.map((o) => h('button', { role: 'radio', 'aria-checked': 'false', text: o.label, onclick: () => onPick(o.value) }));
  const el = h('div', { class: 'seg' + (small ? ' small' : ''), role: 'radiogroup', 'aria-label': label || '', style: `--n:${options.length}` }, thumb, buttons);
  reg(() => {
    const current = value();
    // A value that is none of the options (the drag wheel's own key) chooses none, rather than the first.
    const i = options.findIndex((o) => o.value === current);
    thumb.hidden = i < 0;
    el.style.setProperty('--i', Math.max(0, i));
    buttons.forEach((b, k) => b.setAttribute('aria-checked', String(k === i)));
  });
  return el;
}

function sliderRow({ path, title, desc, min, max, step, format, onInput }) {
  const input = h('input', { type: 'range', min, max, step, 'aria-label': title });
  const out = h('output', { 'aria-live': 'off' });
  let dragging = false;
  input.addEventListener('pointerdown', () => { dragging = true; });
  window.addEventListener('pointerup', () => { dragging = false; });
  const paint = () => {
    const v = +input.value;
    input.style.setProperty('--p', ((v - min) / (max - min)) * 100 + '%');
    out.textContent = format(v);
  };
  input.addEventListener('input', () => { paint(); change(path, +input.value); if (onInput) onInput(); });
  reg(() => { if (!dragging) input.value = get(path); paint(); });
  return row(title, desc, h('div', { class: 'slider' }, input, out));
}

function button(label, onclick, opts = {}) {
  return h('button', { class: 'btn' + (opts.primary ? ' primary' : '') + (opts.danger ? ' danger' : '') + (opts.small ? ' small' : ''), onclick, 'aria-label': opts.aria },
    opts.icon && icon(opts.icon), label);
}

function hotkeyButton(target, opts = {}) {
  const el = h('button', { class: 'hotkey' });
  const render = () => {
    if (rec && rec.el === el) { el.replaceChildren(h('span', { class: 'hint', text: t('rec.press') })); return; }
    const shortcut = shortcutOf(target);
    el.replaceChildren(shortcut ? keycapsEl(shortcut) : h('span', { class: 'none', text: t('tiling.none') }));
    el.setAttribute('aria-label', (opts.label || '') + ' ' + (shortcut ? shortcutParts(shortcut).map(keyLabel).join(' ') : t('tiling.none')));
  };
  el.addEventListener('click', () => startRecording(el, target, !!opts.allowClear, render));
  reg(render);
  return el;
}

// App pickers -------------------------------------------------------

let menuEl = null;
let pendingMenu = null;
function closeMenu() {
  if (menuEl) { menuEl.remove(); menuEl = null; }
  window.removeEventListener('mousedown', menuAway, true);
  window.removeEventListener('keydown', menuKey, true);
}
function menuAway(e) { if (menuEl && !menuEl.contains(e.target)) closeMenu(); }
function menuKey(e) { if (e.key === 'Escape') { e.stopPropagation(); closeMenu(); } }

function addAppButton(purpose, label) {
  const btn = h('button', { class: 'btn small', onclick: () => { pendingMenu = { anchor: btn, purpose }; send({ t: 'running', purpose }); } }, icon('plus'), label);
  return btn;
}
function onRunning(m) {
  if (!pendingMenu || pendingMenu.purpose !== m.purpose) return;
  const { anchor, purpose } = pendingMenu;
  pendingMenu = null;
  const listed = new Set((purpose === 'scroll' ? S.mouse.apps : purpose === 'typing' ? S.typing.apps : S.clipboard.ignored).map((a) => a.id));
  const free = m.apps.filter((a) => !listed.has(a.id));
  closeMenu();
  // Vietnamese typing keeps its list itself, through an action that carries the app.
  const addMessage = (id) => (purpose === 'typing' ? { t: 'action', name: 'typing_app_add:' + id } : { t: purpose === 'scroll' ? 'app_add' : 'ignored_add', id });
  menuEl = h('div', { class: 'menu glass', role: 'menu' },
    h('div', { class: 'section-title', style: 'margin:8px 10px 6px', text: t('menu.running') }),
    free.length
      ? free.map((a) => h('button', { role: 'menuitem', onclick: () => { closeMenu(); send(addMessage(a.id)); } }, iconImg(a.id), a.name || a.id))
      : h('div', { class: 'empty', text: t('menu.none') }),
    h('hr'),
    h('button', { role: 'menuitem', onclick: () => { closeMenu(); send({ t: 'pick_app', purpose }); } }, icon('folder', 'menu-icon'), t('menu.disk')));
  document.body.append(menuEl);
  const rect = anchor.getBoundingClientRect();
  const width = menuEl.offsetWidth;
  const height = menuEl.offsetHeight;
  menuEl.style.left = clamp(rect.right - width, 8, innerWidth - width - 8) + 'px';
  const below = rect.bottom + 6;
  menuEl.style.top = (below + height > innerHeight - 8 ? Math.max(8, rect.top - height - 6) : below) + 'px';
  window.addEventListener('mousedown', menuAway, true);
  window.addEventListener('keydown', menuKey, true);
}

function appRow(app, controls) {
  return h('div', { class: 'app-row' }, iconImg(app.id),
    h('div', { class: 'app-text' }, h('div', { class: 'app-name', text: app.name }), h('div', { class: 'app-id', text: app.id })),
    controls);
}

/** A list that redraws only when its data changes, dropping the controls of the rows it replaces. */
function dynamicList(read, build, emptyText) {
  const box = h('div', { class: 'apps' });
  let last = null;
  let mine = [];
  reg(() => {
    const data = read();
    const key = JSON.stringify(data) + lang;
    if (key === last) return;
    last = key;
    dropSyncs(mine);
    mine = scoped(() => {
      box.replaceChildren(...(data.length ? data.map(build) : [h('div', { class: 'empty', text: emptyText })]));
    });
  });
  return box;
}

function ruleRow(rule) {
  const current = (field) => (S.mouse.apps.find((a) => a.id === rule.id) || {})[field] ?? null;
  const choice = (field, label) => {
    const el = h('select', { class: 'pick small', 'aria-label': label + ': ' + rule.name },
      h('option', { value: 'default', text: t('rule.default') }), h('option', { value: 'on', text: t('rule.on') }), h('option', { value: 'off', text: t('rule.off') }));
    el.addEventListener('change', () => {
      const value = el.value === 'on' ? true : el.value === 'off' ? false : null;
      const entry = S.mouse.apps.find((a) => a.id === rule.id);
      if (entry) entry[field] = value;
      runSyncs();
      send({ t: 'app_rule', id: rule.id, field, value });
    });
    reg(() => { const v = current(field); el.value = v === true ? 'on' : v === false ? 'off' : 'default'; });
    return el;
  };
  return appRow(rule, h('div', { class: 'app-opts' },
    h('label', {}, t('apps.smooth'), choice('smooth', t('apps.smooth'))),
    h('label', {}, t('apps.reverse'), choice('reverse', t('apps.reverse'))),
    h('button', { class: 'btn icon', 'aria-label': t('apps.remove') + ': ' + rule.name, title: t('apps.remove'), onclick: () => send({ t: 'app_remove', id: rule.id }) }, icon('close'))));
}
function ignoredRow(app) {
  return appRow(app, h('button', { class: 'btn icon', 'aria-label': t('apps.remove') + ': ' + app.name, title: t('apps.remove'), onclick: () => send({ t: 'ignored_remove', id: app.id }) }, icon('close')));
}

// ---------------------------------------------------------------- pages

function pill(textFn, goodFn) {
  const dot = h('span', { class: 'dot' });
  const label = h('span');
  const el = h('span', { class: 'chip' }, dot, label);
  reg(() => { label.textContent = textFn(); dot.classList.toggle('good', goodFn()); });
  return el;
}

/** The figures under the hero of the General page. Every one is read from the state the page already has. */
function metricsStrip() {
  const features = () => {
    const flags = ['mouse.smooth', 'notch.enabled', 'agents.enabled', 'tiling.enabled'].map((path) => !!get(path));
    for (const p of PLUGINS) if (p.build && S[p.id] && typeof S[p.id].enabled === 'boolean') flags.push(S[p.id].enabled);
    return [flags.filter(Boolean).length, flags.length];
  };
  const tabs = () => {
    const pages = (S.notch && S.notch.pages) || [];
    return [pages.filter((x) => x.on && !x.waiting).length, pages.length];
  };
  const tile = (glyph, labelKey, fill) => {
    const value = h('div', { class: 'metric-val' });
    const el = h('section', { class: 'card metric' },
      h('div', { class: 'metric-top' }, icon(glyph), h('span', { class: 'metric-label', text: t(labelKey) })),
      value);
    reg(() => fill(value, el));
    return el;
  };
  const ratio = (pair) => (value) => value.replaceChildren(String(pair()[0]), h('small', { text: '/ ' + pair()[1] }));
  return h('div', { class: 'metrics' },
    tile('sparkles', 'metric.features', ratio(features)),
    tile('rectangle.topthird.inset.filled', 'metric.tabs', ratio(tabs)),
    tile('doc.on.clipboard.fill', 'metric.clips', (value) => { value.textContent = String((S.clipboard && S.clipboard.count) || 0); }),
    tile('checkmark.shield', 'metric.access', (value, el) => {
      value.classList.add('word');
      value.textContent = S.env.accessibility ? t('access.granted') : t('access.missing');
      el.classList.toggle('warn', !S.env.accessibility);
    }));
}

function pageGeneral() {
  const spirit = h('span', { class: 'hero-spirit', 'aria-hidden': 'true', html: SPIRIT_SVG });
  const look = (e) => {
    if (!spirit.isConnected) { window.removeEventListener('pointermove', look); return; }
    const box = spirit.getBoundingClientRect();
    const dx = e.clientX - (box.left + box.width / 2);
    const dy = e.clientY - (box.top + box.height * 0.45);
    const reach = Math.min(2.6, Math.hypot(dx, dy) / 60);
    const angle = Math.atan2(dy, dx);
    spirit.style.setProperty('--px', (Math.cos(angle) * reach).toFixed(2));
    spirit.style.setProperty('--py', (Math.sin(angle) * reach).toFixed(2));
  };
  window.addEventListener('pointermove', look);
  const hero = h('section', { class: 'hero hero-ghost card' },
    appIcon('hero-mark'),
    h('div', { class: 'hero-body' },
      h('h2', { text: 'Figghost' }),
      h('p', { text: t('hero.tagline') }),
      h('div', { class: 'chips' },
        h('span', { class: 'chip', text: t('chip.version', { v: S.app.version }) }),
        pill(() => (S.env.accessibility ? t('chip.access.ok') : t('chip.access.no')), () => S.env.accessibility))),
    spirit);

  // Launch at login depends on how the app was installed, so its row has more states than a plain switch.
  const loginOn = () => S.env.login === 'enabled' || S.env.login === 'needs_approval';
  const loginSwitch = h('button', {
    class: 'switch', role: 'switch', 'aria-label': t('login.title'),
    onclick: () => send({ t: 'set', path: 'launch_at_login', value: !loginOn() }),
  });
  const loginOpen = button(t('login.open'), () => send({ t: 'action', name: 'open_login_settings' }), { small: true });
  const loginRow = row(t('login.title'), t('login.desc'), [loginOpen, loginSwitch]);
  const loginDesc = $('.row-desc', loginRow);
  reg(() => {
    loginSwitch.setAttribute('aria-checked', String(loginOn()));
    loginOpen.style.display = S.env.login === 'needs_approval' ? '' : 'none';
    loginDesc.textContent = S.env.login === 'needs_approval' ? t('login.approval')
      : S.env.login === 'unavailable' ? t('login.unavailable') : t('login.desc');
    loginSwitch.disabled = S.env.login === 'unavailable';
    loginRow.classList.toggle('disabled', S.env.login === 'unavailable');
  });

  const restart = h('div', { class: 'banner' }, h('div', { class: 'grow', text: t('lang.restart') }),
    button(t('lang.restart.btn'), () => send({ t: 'action', name: 'restart' }), { primary: true, small: true }));
  reg(() => { restart.style.display = resolveLang(S.general.language) !== S.env.language ? '' : 'none'; });

  const access = row(t('access.title'), t('access.desc'), [
    pill(() => (S.env.accessibility ? t('access.granted') : t('access.missing')), () => S.env.accessibility),
    button(t('access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }),
  ]);

  const featureTile = (page, path) => {
    const el = h('section', { class: 'card feat', 'data-page': page },
      h('div', { class: 'feat-top' }, h('span', { class: 'feat-badge' }, icon(PAGES.find((p) => p.id === page).icon)), switchEl(path, t('nav.' + page))),
      h('div', { class: 'feat-name', text: t('nav.' + page) }));
    reg(() => el.classList.toggle('off', !get(path)));
    return el;
  };

  return [
    hero,
    metricsStrip(),
    h('div', { class: 'tiles-grid' },
      featureTile('scroll', 'mouse.smooth'), featureTile('notch', 'notch.enabled'),
      featureTile('agents', 'agents.enabled'), featureTile('tiling', 'tiling.enabled')),
    sectionTitle(t('sec.startup')),
    card(
      loginRow,
      toggleRow('general.show_on_launch', t('showlaunch.title'), t('showlaunch.desc')),
      toggleRow('general.launch_effect', t('hello.title'), t('hello.desc'), { when: () => S.notch.enabled }),
      toggleRow('general.menu_icon', t('menuicon.title'), t('menuicon.desc'))),
    sectionTitle(t('sec.appearance')),
    card(
      segRow('general.theme', t('theme.title'), t('theme.desc'), ['auto', 'light', 'dark']),
      row(t('lang.title'), t('lang.desc'), segmented({
        label: t('lang.title'),
        options: [{ value: 'auto', label: t('lang.auto') }, { value: 'en', label: t('lang.en') }, { value: 'vi', label: t('lang.vi') }],
        value: () => S.general.language,
        onPick: (v) => { change('general.language', v); applyLanguage(); },
      }))),
    restart,
    sectionTitle(t('sec.permissions')),
    card(
      access,
      row(t('perm.all_title'), t('perm.all_desc'), button(t('perm.all_btn'), () => send({ t: 'action', name: 'check_permissions' }), { small: true })),
      row(t('data.title'), t('data.desc'), button(t('data.open'), () => send({ t: 'action', name: 'open_config' }), { icon: 'folder', small: true }))),
  ];
}

// -- scrolling ------------------------------------------------------

let curve = { duration: 0, points: [] }; // from the app: [[ms, share covered, relative speed], ...]
const graphs = new Set();
function requestCurve() { throttleBy('curve', 30, () => send({ t: 'curve', duration_ms: S.mouse.duration_ms })); }
function onCurve(m) { curve = { duration: m.duration_ms, points: m.points }; for (const draw of graphs) draw(); }

function cssVar(name) { return getComputedStyle(document.documentElement).getPropertyValue(name).trim(); }

function drawCurve(canvas) {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth, hgt = canvas.clientHeight;
  if (!w || !hgt) return;
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(hgt * dpr);
  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, hgt);
  const pts = curve.points;
  if (!pts.length) return;

  const m = { l: 46, r: 12, t: 12, b: 26 };
  const pw = w - m.l - m.r, ph = hgt - m.t - m.b;
  const total = S.mouse.step_px * S.mouse.speed;
  const tMax = pts[pts.length - 1][0];
  const vMax = pts[0][2];
  const x = (ms) => m.l + (ms / tMax) * pw;
  const yDist = (share) => m.t + ph - share * ph;
  const accent = cssVar('--accent'), accent2 = cssVar('--accent-2'), text3 = cssVar('--text-3'), stroke = cssVar('--stroke');

  ctx.font = '11px ' + cssVar('--mono');
  ctx.textBaseline = 'middle';
  ctx.lineWidth = 1;
  for (let i = 0; i <= 4; i++) {
    const share = i / 4, y = yDist(share);
    ctx.strokeStyle = stroke;
    ctx.beginPath(); ctx.moveTo(m.l, y); ctx.lineTo(w - m.r, y); ctx.stroke();
    ctx.fillStyle = text3; ctx.textAlign = 'right';
    ctx.fillText(Math.round(share * total) + '', m.l - 8, y);
  }
  ctx.textAlign = 'center'; ctx.textBaseline = 'top';
  const ticks = 5;
  for (let i = 0; i <= ticks; i++) {
    const ms = (tMax / ticks) * i;
    ctx.fillStyle = text3;
    ctx.fillText(Math.round(ms) + ' ms', clamp(x(ms), m.l + 14, w - m.r - 14), hgt - m.b + 8);
  }

  // Share of the distance covered so far.
  const fill = ctx.createLinearGradient(0, m.t, 0, m.t + ph);
  fill.addColorStop(0, accent + '55');
  fill.addColorStop(1, accent + '04');
  ctx.beginPath();
  pts.forEach(([ms, share], i) => (i ? ctx.lineTo(x(ms), yDist(share)) : ctx.moveTo(x(ms), yDist(share))));
  ctx.lineTo(x(tMax), yDist(0)); ctx.lineTo(x(0), yDist(0)); ctx.closePath();
  ctx.fillStyle = fill; ctx.fill();
  ctx.beginPath();
  pts.forEach(([ms, share], i) => (i ? ctx.lineTo(x(ms), yDist(share)) : ctx.moveTo(x(ms), yDist(share))));
  ctx.strokeStyle = accent; ctx.lineWidth = 2.5; ctx.lineJoin = 'round'; ctx.stroke();

  // Speed, as a dashed line scaled to the same height.
  ctx.beginPath();
  pts.forEach(([ms, , speed], i) => {
    const y = m.t + ph - (speed / vMax) * ph * 0.92;
    i ? ctx.lineTo(x(ms), y) : ctx.moveTo(x(ms), y);
  });
  ctx.setLineDash([5, 4]); ctx.strokeStyle = accent2; ctx.lineWidth = 2; ctx.stroke(); ctx.setLineDash([]);

  // Where the glide is 98% done.
  const done = pts.find(([, share]) => share >= 0.98);
  if (done) {
    ctx.strokeStyle = text3; ctx.setLineDash([2, 4]);
    ctx.beginPath(); ctx.moveTo(x(done[0]), m.t); ctx.lineTo(x(done[0]), m.t + ph); ctx.stroke(); ctx.setLineDash([]);
    ctx.fillStyle = cssVar('--text-2'); ctx.textAlign = 'left'; ctx.textBaseline = 'bottom';
    ctx.fillText('98%', x(done[0]) + 6, m.t + ph - 6);
  }
}

function curveCard() {
  const canvas = h('canvas', { role: 'img', 'aria-label': t('graph.distance') + ' / ' + t('graph.velocity') });
  const summary = h('span');
  const legend = h('div', { class: 'graph-legend' },
    h('span', {}, h('b', { style: 'background:var(--accent)' }), t('graph.distance')),
    h('span', {}, h('b', { style: 'background:var(--accent-2)' }), t('graph.velocity')),
    h('span', { style: 'margin-left:auto' }, summary));
  const draw = () => drawCurve(canvas);
  graphs.add(draw);
  new ResizeObserver(draw).observe(canvas);
  reg(() => {
    if (Math.abs(curve.duration - S.mouse.duration_ms) > 0.5) requestCurve();
    const done = curve.points.find(([, share]) => share >= 0.98);
    summary.textContent = t('graph.per', { px: Math.round(S.mouse.step_px * S.mouse.speed), ms: done ? Math.round(done[0]) : Math.round(S.mouse.duration_ms * 1.0) });
    draw();
  });
  return h('section', { class: 'card graph' }, canvas, legend);
}

const PRESETS = {
  precise: { step_px: 28, speed: 0.9, duration_ms: 220 },
  balanced: { step_px: 36, speed: 1, duration_ms: 360 },
  fluid: { step_px: 48, speed: 1.15, duration_ms: 560 },
};
function applyPreset(values) { for (const [key, value] of Object.entries(values)) change('mouse.' + key, value); }
function presetsRow() {
  const near = (a, b) => Math.abs(a - b) < 0.001;
  const matches = (p) => near(S.mouse.step_px, p.step_px) && near(S.mouse.speed, p.speed) && near(S.mouse.duration_ms, p.duration_ms);
  const buttons = Object.entries(PRESETS).map(([name, values]) => {
    const el = h('button', { class: 'btn small', text: t('preset.' + name), onclick: () => applyPreset(values) });
    reg(() => el.classList.toggle('primary', matches(values)));
    return el;
  });
  return h('div', { class: 'row' }, h('div', { class: 'row-text' }, h('div', { class: 'presets' }, buttons)));
}

function scrollPad() {
  const lines = Array.from({ length: 150 }, (_, i) => h('div', { class: 'pad-line', text: t('pad.line', { n: i + 1 }) }));
  const pad = h('div', { class: 'pad', tabindex: '0', role: 'region', 'aria-label': t('pad.hint') }, lines);
  const meter = h('canvas', { role: 'img', 'aria-label': t('pad.speed') });
  const values = { speed: h('b', { text: '0' }), peak: h('b', { text: '0' }), frames: h('b', { text: '0' }), jump: h('b', { text: '0' }) };
  const stat = (key, label) => h('div', { class: 'stat' }, h('span', { text: label }), values[key]);

  let last = { top: 0, at: 0 }, running = false, idleSince = 0, deltas = [];
  const stats = { peak: 0, frames: 0, jump: 0 };
  const paint = () => {
    const dpr = window.devicePixelRatio || 1, w = meter.clientWidth, hgt = meter.clientHeight;
    if (!w) return;
    meter.width = Math.round(w * dpr); meter.height = Math.round(hgt * dpr);
    const ctx = meter.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, hgt);
    if (!deltas.length) {
      // Waiting for a scroll: a dashed baseline and a short hint.
      ctx.strokeStyle = cssVar('--text-3'); ctx.setLineDash([4, 4]); ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(10, hgt - 14); ctx.lineTo(w - 10, hgt - 14); ctx.stroke(); ctx.setLineDash([]);
      ctx.fillStyle = cssVar('--text-2'); ctx.font = '12px ' + cssVar('--font'); ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.fillText(t('pad.idle'), w / 2, hgt / 2 - 6);
      return;
    }
    const peak = Math.max(20, ...deltas);
    const bar = w / 60;
    deltas.forEach((d, i) => {
      const bh = (d / peak) * (hgt - 8);
      ctx.fillStyle = cssVar('--accent');
      ctx.globalAlpha = 0.35 + 0.65 * (i / deltas.length);
      ctx.fillRect(i * bar + 1, hgt - 4 - bh, Math.max(1, bar - 2), bh);
    });
    ctx.globalAlpha = 1;
  };
  const frame = (now) => {
    const dt = Math.max(1, now - last.at) / 1000;
    const delta = Math.abs(pad.scrollTop - last.top);
    last = { top: pad.scrollTop, at: now };
    if (delta > 0) {
      idleSince = now;
      deltas.push(delta); if (deltas.length > 60) deltas.shift();
      stats.frames++; stats.jump = Math.max(stats.jump, delta);
      const speed = delta / dt; stats.peak = Math.max(stats.peak, speed);
      values.speed.textContent = Math.round(speed) + ' px/s';
    }
    values.peak.textContent = Math.round(stats.peak) + ' px/s';
    values.frames.textContent = String(stats.frames);
    values.jump.textContent = Math.round(stats.jump) + ' px';
    paint();
    if (now - idleSince < 500) requestAnimationFrame(frame); else running = false;
  };
  pad.addEventListener('scroll', () => {
    if (running) return;
    // A new gesture starts after a pause: begin the statistics again.
    deltas = []; stats.peak = 0; stats.frames = 0; stats.jump = 0;
    running = true; idleSince = performance.now(); last = { top: pad.scrollTop, at: performance.now() };
    requestAnimationFrame(frame);
  });
  new ResizeObserver(paint).observe(meter);
  return h('section', { class: 'card' },
    h('div', { class: 'pad-wrap' }, pad,
      h('div', { class: 'meter' }, meter, stat('speed', t('pad.speed')), stat('peak', t('pad.peak')), stat('frames', t('pad.frames')), stat('jump', t('pad.jump')),
        h('div', { class: 'row-desc', text: t('pad.hint') }))));
}

function modTitle(symbol, text) {
  return h('span', {}, h('kbd', { text: symbol, style: 'margin-right:9px' }), text);
}

function boostSelect() {
  const values = [2, 3, 5, 8, 12];
  const el = h('select', { class: 'pick', 'aria-label': t('boost.title') }, values.map((v) => h('option', { value: String(v), text: '× ' + v })));
  el.addEventListener('change', () => change('mouse.boost', +el.value));
  reg(() => {
    const current = String(S.mouse.boost);
    if (![...el.options].some((o) => o.value === current)) el.append(h('option', { value: current, text: '× ' + S.mouse.boost }));
    el.value = current;
    el.disabled = !S.mouse.option_boost;
  });
  return el;
}

function pageScroll() {
  return [
    sectionTitle(t('sec.wheel')),
    card(
      toggleRow('mouse.smooth', t('smooth.title'), t('smooth.desc')),
      toggleRow('mouse.reverse', t('reverse.title'), t('reverse.desc'))),
    sectionTitle(t('sec.feel')),
    curveCard(),
    card(
      sliderRow({ path: 'mouse.step_px', title: t('step.title'), desc: t('step.desc'), min: 8, max: 120, step: 1, format: (v) => Math.round(v) + ' px' }),
      sliderRow({ path: 'mouse.speed', title: t('speed.title'), desc: t('speed.desc'), min: 0.3, max: 3, step: 0.05, format: (v) => '× ' + v.toFixed(2) }),
      sliderRow({ path: 'mouse.duration_ms', title: t('duration.title'), desc: t('duration.desc'), min: 80, max: 1000, step: 10, format: (v) => Math.round(v) + ' ms' }),
      presetsRow()),
    sectionTitle(t('sec.try')),
    scrollPad(),
    sectionTitle(t('sec.mods')),
    card(
      toggleRow('mouse.shift_horizontal', modTitle('⇧', t('shift.title')), t('shift.desc')),
      row(modTitle('⌥', t('option.title')), t('option.desc'), [boostSelect(), switchEl('mouse.option_boost', t('option.title'))]),
      toggleRow('mouse.command_zoom', modTitle('⌘', t('cmd.title')), t('cmd.desc'))),
    sectionTitle(t('sec.apps')),
    card(
      row(t('apps.rules'), t('apps.desc'), addAppButton('scroll', t('apps.add'))),
      dynamicList(() => S.mouse.apps, ruleRow, t('apps.empty'))),
  ];
}

// -- clipboard ------------------------------------------------------

function limitSelect() {
  const values = [100, 250, 500, 1000, 5000, 20000];
  const el = h('select', { class: 'pick', 'aria-label': t('limit.title') }, values.map((v) => h('option', { value: String(v), text: v.toLocaleString() })));
  el.addEventListener('change', () => change('clipboard.history_limit', +el.value));
  reg(() => {
    const current = String(S.clipboard.history_limit);
    if (![...el.options].some((o) => o.value === current)) el.append(h('option', { value: current, text: (+current).toLocaleString() }));
    el.value = current;
  });
  return el;
}

function mcpSnippet() {
  return JSON.stringify({ mcpServers: { figghost: { command: S.mcp.command, args: ['--mcp'] } } }, null, 2);
}
function mcpCard() {
  const code = h('pre', { class: 'code', tabindex: '0', 'aria-label': t('mcp.how') });
  const detail = h('div', { class: 'row block' }, h('div', { class: 'row-desc', text: t('mcp.how') }), code,
    h('div', {}, button(t('mcp.copy'), () => { send({ t: 'action', name: 'copy_mcp_config' }); toast(t('mcp.copied')); }, { small: true })));
  reg(() => { code.textContent = mcpSnippet(); detail.classList.toggle('dim', !S.mcp.enabled); });
  return card(toggleRow('mcp.enabled', t('mcp.title'), t('mcp.desc')), detail);
}

function cheat(keys, text) {
  return h('div', { class: 'cheat' }, h('span', { text }), h('span', { class: 'keys' }, keys.map((k) => h('kbd', { text: k }))));
}

function pageClipboard() {
  const count = h('span', { class: 'row-title' });
  reg(() => { count.textContent = S.clipboard.count.toLocaleString(); });
  return [
    sectionTitle(t('sec.pasting')),
    card(
      row(t('clip.hotkey.title'), t('clip.hotkey.desc'), hotkeyButton('clipboard', { label: t('clip.hotkey.title') })),
      toggleRow('clipboard.auto_paste', t('paste.auto.title'), t('paste.auto.desc')),
      toggleRow('clipboard.paste_plain', t('paste.plain.title'), t('paste.plain.desc', { key: keysText(S.keys.shortcuts.paste_other) }))),
    sectionTitle(t('sec.history')),
    card(
      row(t('limit.title'), t('limit.desc'), limitSelect()),
      row(t('count.title'), null, count),
      row(t('clear.title'), t('clear.desc'), button(t('clear.btn'), () => confirmDialog({ title: t('clear.confirm.title'), body: t('clear.confirm.body'), confirm: t('clear.btn') }, () => send({ t: 'action', name: 'clear_history' })), { icon: 'trash', small: true, danger: true }))),
    sectionTitle(t('sec.privacy')),
    card(
      row(t('ignored.title'), t('ignored.desc'), addAppButton('ignore', t('ignored.add'))),
      dynamicList(() => S.clipboard.ignored, ignoredRow, t('ignored.empty'))),
    sectionTitle(t('sec.ai')),
    mcpCard(),
    sectionTitle(t('sec.keys')),
    h('section', { class: 'card' }, h('div', { class: 'cheats' },
      cheat(['↩'], t('key.paste')),
      cheat(shortcutParts(S.keys.shortcuts.paste_other).map(keyLabel), t('key.pastealt')),
      cheat(shortcutParts(S.keys.shortcuts.pin).map(keyLabel), t('key.pin')),
      cheat(shortcutParts(S.keys.shortcuts.delete).map(keyLabel), t('key.delete')),
      cheat(['⌘', 'B…'], t('key.pinkey')),
      cheat(['Esc'], t('key.close')),
      h('div', { class: 'cheat' }, h('span', { text: t('key.filters') }), h('span', { class: 'row-title', style: 'font-family:var(--mono);font-size:12px', text: t('key.filters.val') })))),
    h('div', {}, button(t('keys.change'), () => go('keys'), { icon: 'keyboard', small: true })),
  ];
}

// -- shortcuts --------------------------------------------------------

function pageKeys() {
  const key = (action, title, desc) => row(title, desc, hotkeyButton('keys:' + action, { label: title }));
  // The shortcuts that work anywhere, each also on its own page; a feature that is not built in is left out.
  const global = [['clipboard', t('clip.hotkey.title')], ['notch', t('notch.shortcut.title'), true]]
    .concat(['smart', 'translate', 'vision', 'pin'].filter((id) => S[id] && PAGES.some((p) => p.id === id)).map((id) => [id, t('nav.' + id), id !== 'smart' && id !== 'translate']))
    .map(([target, title, allowClear]) => row(title, null, hotkeyButton(target, { label: title, allowClear })));
  const elsewhere = ['capture', 'tiling'].filter((id) => PAGES.some((p) => p.id === id))
    .map((id) => row(t('nav.' + id), t('keys.elsewhere'), button(t('keys.open'), () => go(id), { small: true })));
  return [
    sectionTitle(t('keys.sec.panel')),
    card(
      key('pin', t('key.pin'), null),
      key('board', t('keys.board.title'), t('keys.board.desc')),
      key('paste_other', t('key.pastealt'), t('keys.pastealt.desc')),
      key('delete', t('keys.delete.title'), t('keys.delete.desc'))),
    sectionTitle(t('keys.sec.fixed')),
    h('section', { class: 'card' }, h('div', { class: 'cheats' },
      cheat(['↩'], t('keys.fixed.enter')),
      cheat(['↑', '↓'], t('keys.fixed.move')),
      cheat(['⌘', '1-9'], t('keys.fixed.tab')),
      cheat(['⌘', 'A-Z'], t('key.pinkey')),
      cheat(['⌘', '⌫'], t('key.delete')),
      cheat(['Esc'], t('key.close'))),
      h('div', { class: 'row-desc', style: 'padding:0 14px 12px', text: t('keys.fixed.desc') })),
    sectionTitle(t('keys.sec.global')),
    card(...global, ...elsewhere),
  ];
}

// -- notch ----------------------------------------------------------

function tabTitle(page) { return I18N.en['tab.' + page.id] ? t('tab.' + page.id) : page.title; }

function notchStage() {
  const tabs = h('div', { class: 'notch-tabs' });
  const notch = h('div', { class: 'notch' },
    h('div', { class: 'notch-body' }, tabs, h('div', { class: 'notch-line' }), h('div', { class: 'notch-line' }), h('div', { class: 'notch-line' })));
  const stage = h('div', { class: 'notch-stage', 'aria-hidden': 'true' }, h('div', { class: 'menubar' }), notch, h('div', { class: 'notch-note', text: t('notch.preview') }));
  reg(() => {
    notch.className = 'notch mat-' + S.notch.material;
    stage.className = 'notch-stage mo-' + S.notch.motion;
    tabs.replaceChildren(...S.notch.pages.filter((p) => p.on && !p.waiting).slice(0, 6).map((p) => h('span', { text: tabTitle(p) })));
  });
  return stage;
}

function infoRow(title, desc) { return row(title, desc, null); }

/** A pop-up of fixed choices for a setting; numbers stay numbers. */
function selectRow({ path, title, desc, options, when }) {
  const el = h('select', { class: 'pick', 'aria-label': title });
  const fill = () => {
    const list = options();
    const key = JSON.stringify(list) + lang;
    if (el.dataset.key === key) return;
    el.dataset.key = key;
    el.replaceChildren(...list.map((o) => h('option', { value: String(o.value), text: o.label })));
  };
  el.addEventListener('change', () => change(path, typeof get(path) === 'number' ? +el.value : el.value));
  reg(() => { fill(); el.value = String(get(path)); });
  return row(title, desc, el, { when });
}

function segRow(path, title, desc, values, opts = {}) {
  return row(title, desc, segmented({
    label: title,
    options: values.map((v) => ({ value: v, label: t(path + '.' + v) })),
    value: () => get(path),
    onPick: (v) => change(path, v),
  }), opts);
}

/** Writes a new list of tabs: the page keeps every tab in order, the app keeps the ids of the ones that show. */
function sendPages(pages) {
  S.notch.pages = pages;
  runSyncs();
  send({ t: 'set', path: 'notch.pages', value: pages.filter((p) => p.on).map((p) => p.id) });
}

function tabRow(page) {
  const pages = () => S.notch.pages;
  const at = () => pages().findIndex((p) => p.id === page.id);
  const title = tabTitle(page);
  const move = (by) => {
    const list = pages().slice();
    const i = at(), j = i + by;
    if (i < 0 || j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    sendPages(list);
  };
  const up = h('button', { class: 'btn icon', 'aria-label': t('tab.up') + ': ' + title, title: t('tab.up'), onclick: () => move(-1) }, icon('chevron.up'));
  const down = h('button', { class: 'btn icon', 'aria-label': t('tab.down') + ': ' + title, title: t('tab.down'), onclick: () => move(1) }, icon('chevron.down'));
  const toggle = h('button', {
    class: 'switch', role: 'switch', 'aria-label': title,
    onclick: () => {
      const list = pages().map((p) => (p.id === page.id ? { ...p, on: !p.on } : p));
      if (list.some((p) => p.on)) sendPages(list);
    },
  });
  reg(() => {
    const now = pages()[at()];
    const on = !!(now && now.on);
    toggle.setAttribute('aria-checked', String(on));
    // The last tab that shows cannot go: a notch with no tab could not be used. A tab that waits for its feature does not show.
    const shows = (p) => p.on && !p.waiting;
    toggle.disabled = !!(now && shows(now) && pages().filter(shows).length === 1);
    up.disabled = at() <= 0;
    down.disabled = at() >= pages().length - 1;
  });
  const desc = page.waiting ? t('tab.waiting') : (I18N.en['tab.' + page.id + '.desc'] ? t('tab.' + page.id + '.desc') : null);
  return row(title, desc, [h('span', { class: 'tab-order' }, up, down), toggle]);
}

// The sizes of the notch as ready-made styles. 0 is automatic: what each tab needs, or the default pill.
const NOTCH_SIZES = ['width', 'height', 'rest_width', 'rest_height'];
const NOTCH_STYLES = {
  compact: { width: 0, height: 0, rest_width: 150, rest_height: 0 },
  standard: { width: 0, height: 0, rest_width: 0, rest_height: 0 },
  wide: { width: 800, height: 520, rest_width: 260, rest_height: 32 },
};
const notchStyleOf = () => Object.keys(NOTCH_STYLES).find((k) => NOTCH_SIZES.every((f) => NOTCH_STYLES[k][f] === S.notch[f])) || 'custom';

/** A card with a small drawing of the closed pill and the open shape; `sizes` are the values it stands for. */
function styleCard(id, sizes, pressed, onPick) {
  const pill = h('span', { class: 'sc-pill' });
  const open = h('span', { class: 'sc-open' });
  const el = h('button', { class: 'style-card', type: 'button', 'aria-pressed': 'false', onclick: onPick },
    h('span', { class: 'sc-art', 'aria-hidden': 'true' }, pill, open),
    h('span', { class: 'sc-name', text: t('notch.style.' + id) }),
    h('span', { class: 'sc-desc', text: t('notch.style.' + id + '.desc') }));
  reg(() => {
    const v = sizes();
    pill.style.width = ((v.rest_width || 200) * 0.2) + 'px';
    pill.style.height = Math.max(4, (v.rest_height || 24) * 0.2) + 'px';
    open.style.width = ((v.width || 560) * 0.11) + 'px';
    open.style.height = ((v.height || 440) * 0.09) + 'px';
    el.setAttribute('aria-pressed', String(pressed()));
    $('.sc-name', el).textContent = t('notch.style.' + id);
    $('.sc-desc', el).textContent = t('notch.style.' + id + '.desc');
  });
  return el;
}

function notchStyleSection(on) {
  let customOpen = false;
  const cards = h('div', { class: 'style-cards', role: 'group', 'aria-label': t('sec.notch.style') });
  for (const id of Object.keys(NOTCH_STYLES)) {
    cards.append(styleCard(id, () => NOTCH_STYLES[id], () => !customOpen && notchStyleOf() === id, () => {
      customOpen = false;
      NOTCH_SIZES.forEach((f) => change('notch.' + f, NOTCH_STYLES[id][f]));
    }));
  }
  cards.append(styleCard('custom', () => S.notch, () => customOpen || notchStyleOf() === 'custom', () => { customOpen = true; runSyncs(); }));
  const slider = (field, min, max, step, key) => sliderRow({
    path: 'notch.' + field, title: t(key + '.title'), desc: t(key + '.desc'), min, max, step,
    format: (v) => (S.notch[field] === 0 ? t('notch.width.auto') : t('notch.width.pt', { n: Math.round(v) })),
  });
  const closed = () => on() && !S.notch.hardware;
  const reset = button(t('notch.style.reset'), () => { customOpen = false; NOTCH_SIZES.forEach((f) => change('notch.' + f, 0)); }, { small: true });
  const custom = card(
    slider('width', 480, 1000, 10, 'notch.width'),
    slider('height', 260, 900, 10, 'notch.height'),
    row(t('notch.restwidth.title'), t('notch.restwidth.desc'), null, { when: closed }),
    row(t('notch.restheight.title'), t('notch.restheight.desc'), null, { when: closed }),
    row(t('notch.style.reset.title'), t('notch.style.reset.desc'), reset));
  // The two closed sliders replace the plain rows above, so the closed ones are built the same way.
  custom.replaceChildren(
    custom.children[0], custom.children[1],
    ...[slider('rest_width', 120, 400, 5, 'notch.restwidth'), slider('rest_height', 20, 48, 1, 'notch.restheight')]
      .map((r) => { reg(() => r.classList.toggle('disabled', !closed())); return r; }),
    custom.children[4]);
  reg(() => { custom.style.display = customOpen || notchStyleOf() === 'custom' ? '' : 'none'; });
  const wrap = h('div', { class: 'style-wrap' }, cards, custom);
  reg(() => wrap.classList.toggle('disabled', !on()));
  return [sectionTitle(t('sec.notch.style')), wrap];
}

// The look and the place of the volume indicator: a card for each style and a small screen to pick the place on.
const HUD_STYLES = ['notch', 'glass', 'corner', 'bar', 'island', 'side', 'tile', 'ring', 'equalizer', 'wide'];
const HUD_PLACES = ['top-left', 'top', 'top-right', 'left', 'center', 'right', 'bottom-left', 'bottom', 'bottom-right'];
// The styles that hang from the notch stay under it, so a place means nothing for them.
const HUD_HANGING = ['notch', 'glass', 'bar'];

function hudRows() {
  const floating = () => S.notch.volume_hud && !HUD_HANGING.includes(S.notch.hud);
  const grid = h('div', { class: 'hud-places', role: 'group', 'aria-label': t('vol.place.title') });
  const places = HUD_PLACES.map((p) => {
    const b = h('button', { class: 'hud-place', type: 'button', 'aria-pressed': 'false', onclick: () => change('notch.hud_position', p) });
    grid.append(b);
    return [p, b];
  });
  const auto = button(t('vol.place.auto'), () => change('notch.hud_position', 'auto'), { small: true });
  reg(() => {
    for (const [p, b] of places) {
      b.setAttribute('aria-pressed', String(S.notch.hud_position === p));
      b.setAttribute('aria-label', t('vol.place.' + p));
      b.title = t('vol.place.' + p);
    }
    auto.textContent = t('vol.place.auto');
    auto.classList.toggle('on', S.notch.hud_position === 'auto');
  });
  const test = button(t('vol.preview'), () => send({ t: 'action', name: 'hud_preview' }), { small: true });
  reg(() => { test.textContent = t('vol.preview'); });
  return [
    row(t('vol.place.title'), t('vol.place.desc'), h('div', { class: 'hud-place-box' }, grid, auto), { when: floating }),
    row(t('vol.preview.title'), t('vol.preview.desc'), test, { when: () => S.notch.volume_hud }),
  ];
}

function hudStyleSection() {
  const cards = h('div', { class: 'style-cards hud-cards', role: 'group', 'aria-label': t('vol.style.title') });
  for (const id of HUD_STYLES) {
    const name = h('span', { class: 'sc-name' });
    const desc = h('span', { class: 'sc-desc' });
    const el = h('button', { class: 'style-card', type: 'button', 'aria-pressed': 'false', onclick: () => change('notch.hud', id) },
      h('span', { class: 'sc-art hud-art hud-' + id, 'aria-hidden': 'true' }, h('i')), name, desc);
    reg(() => {
      el.setAttribute('aria-pressed', String(S.notch.hud === id));
      name.textContent = t('vol.style.' + id);
      desc.textContent = t('vol.style.' + id + '.desc');
    });
    cards.append(el);
  }
  const wrap = h('div', { class: 'style-wrap' }, cards);
  reg(() => wrap.classList.toggle('disabled', !S.notch.volume_hud));
  return [sectionTitle(t('vol.style.title')), wrap];
}

function pageNotch() {
  const shelfCount = h('span', { class: 'row-title' });
  reg(() => { shelfCount.textContent = String(S.notch.shelf_count); });
  const on = () => S.notch.enabled;
  const delays = [0, 100, 200, 300, 500, 800];
  const rest = segRow('notch.rest', t('notch.rest.title'), t('notch.rest.desc'), ['notch', 'hidden'], { when: () => on() && !S.notch.hardware });
  const restDesc = $('.row-desc', rest);
  reg(() => { restDesc.textContent = S.notch.hardware ? t('notch.rest.hardware') : t('notch.rest.desc'); });
  return [
    notchStage(),
    card(toggleRow('notch.enabled', t('notch.panel.title'), t('notch.panel.desc'))),
    ...notchStyleSection(on),
    sectionTitle(t('sec.notch.behavior')),
    card(
      toggleRow('notch.open_on_hover', t('notch.hover.title'), t('notch.hover.desc'), { when: on }),
      selectRow({
        path: 'notch.hover_delay_ms', title: t('notch.delay.title'), desc: t('notch.delay.desc'),
        when: () => on() && S.notch.open_on_hover,
        options: () => {
          const list = delays.map((n) => ({ value: n, label: n ? t('notch.delay.ms', { n }) : t('notch.delay.none') }));
          if (!delays.includes(S.notch.hover_delay_ms)) list.push({ value: S.notch.hover_delay_ms, label: t('notch.delay.ms', { n: S.notch.hover_delay_ms }) });
          return list;
        },
      }),
      selectRow({
        path: 'notch.open_page', title: t('notch.openpage.title'), desc: t('notch.openpage.desc'), when: on,
        options: () => [{ value: 'activity', label: t('notch.openpage.activity') }, { value: 'last', label: t('notch.openpage.last') }]
          .concat(S.notch.pages.filter((p) => p.on && !p.waiting).map((p) => ({ value: p.id, label: tabTitle(p) }))),
      }),
      toggleRow('notch.haptics', t('notch.haptics.title'), t('notch.haptics.desc'), { when: on }),
      row(t('notch.shortcut.title'), t('notch.shortcut.desc'), hotkeyButton('notch', { allowClear: true, label: t('notch.shortcut.title') }), { when: on })),
    sectionTitle(t('sec.notch.look')),
    card(
      selectRow({
        path: 'notch.material', title: t('notch.material.title'), desc: t('notch.material.desc'), when: on,
        options: () => ['glass', 'clear', 'ink', 'accent', 'black'].map((v) => ({ value: v, label: t('notch.material.' + v) })),
      }),
      segRow('notch.motion', t('notch.motion.title'), t('notch.motion.desc'), ['spring', 'smooth', 'none'], { when: on }),      toggleRow('notch.shadow', t('notch.shadow.title'), t('notch.shadow.desc'), { when: on })),
    sectionTitle(t('sec.notch.display')),
    card(
      selectRow({
        path: 'notch.display', title: t('notch.display.title'), desc: t('notch.display.desc'), when: on,
        options: () => ['auto', 'main', 'pointer'].map((v) => ({ value: v, label: t('notch.display.' + v) }))
          .concat((S.notch.screens || []).map((name) => ({ value: 'screen:' + name, label: name }))),
      }),
      rest,
      toggleRow('notch.show_in_fullscreen', t('notch.fullscreen.title'), t('notch.fullscreen.desc'), { when: on }),
      toggleRow('notch.hide_in_capture', t('notch.capture.title'), t('notch.capture.desc'), { when: on })),
    sectionTitle(t('sec.notch.live')),
    card(
      toggleRow('notch.live_media', t('notch.livemedia.title'), t('notch.livemedia.desc'), { when: on }),
      segRow('notch.waveform', t('notch.waveform.title'), t('notch.waveform.desc'), ['live', 'animated'], { when: () => on() && S.notch.live_media }),
      toggleRow('notch.live_agents', t('notch.liveagents.title'), t('notch.liveagents.desc'), { when: on })),
    sectionTitle(t('sec.tabs')),
    card(infoRow(t('tabs.desc'), null), dynamicList(() => S.notch.pages, tabRow, '')),
    sectionTitle(t('sec.shelf')),
    card(
      row(t('shelf.count'), null, shelfCount),
      row(t('shelf.clear'), t('shelf.clear.desc'), button(t('shelf.clear'), () => send({ t: 'action', name: 'clear_shelf' }), { icon: 'trash', small: true }))),
    sectionTitle(t('sec.volume')),
    card(
      toggleRow('notch.volume_hud', t('vol.title'), t('vol.desc'))),
    ...hudStyleSection(),
    card(...hudRows()),
  ];
}

// -- agents ----------------------------------------------------------

function agentLine(which) {
  const a = () => S.agents[which];
  const complete = () => a().installed > 0 && a().installed === a().wanted && !a().error;
  const state = pill(() => {
    if (a().error) return t('agents.status.error');
    if (!a().found && a().installed === 0) return t('agents.status.missing');
    if (a().installed === 0) return t('agents.status.off');
    if (a().installed < a().wanted) return t('agents.status.partial', { n: a().installed, m: a().wanted });
    return t('agents.status.on');
  }, complete);
  const btn = h('button', { class: 'btn small' });
  reg(() => {
    btn.textContent = complete() ? t('agents.remove') : (a().installed > 0 ? t('agents.repair') : t('agents.install'));
    btn.classList.toggle('danger', complete());
    btn.disabled = !a().found && a().installed === 0;
    btn.setAttribute('aria-label', btn.textContent + ' ' + t('agents.' + which + '.title'));
  });
  btn.addEventListener('click', () => send({ t: 'action', name: 'agents_' + (complete() ? 'remove_' : 'install_') + which }));
  return row(t('agents.' + which + '.title'), t('agents.' + which + '.desc'), [state, btn]);
}

function waitSelect() {
  const values = [10, 20, 30, 60, 100];
  const el = h('select', { class: 'pick', 'aria-label': t('agents.wait.title') }, values.map((v) => h('option', { value: String(v), text: t('agents.wait.n', { n: v }) })));
  el.addEventListener('change', () => change('agents.wait', +el.value));
  reg(() => {
    const now = String(S.agents.wait);
    if (![...el.options].some((o) => o.value === now)) el.append(h('option', { value: now, text: t('agents.wait.n', { n: now }) }));
    el.value = now;
  });
  return el;
}

function usageRow() {
  const state = pill(() => (S.agents.usage_tap ? t('agents.usage.ok') : t('agents.usage.none')), () => S.agents.usage_tap);
  const btn = h('button', { class: 'btn small' });
  reg(() => {
    btn.textContent = S.agents.usage_tap ? t('agents.usage.off') : t('agents.usage.on');
    btn.disabled = !S.agents.claude.found && !S.agents.usage_tap;
    btn.setAttribute('aria-label', btn.textContent + ' ' + t('agents.usage.title'));
  });
  btn.addEventListener('click', () => send({ t: 'action', name: S.agents.usage_tap ? 'agents_usage_off' : 'agents_usage_on' }));
  const waiting = h('div', { class: 'row-desc', role: 'status' });
  reg(() => {
    waiting.textContent = S.agents.usage_tap && !S.agents.usage_known ? t('agents.usage.waiting') : '';
    waiting.style.display = waiting.textContent ? '' : 'none';
  });
  const line = row(t('agents.usage.title'), t('agents.usage.desc'), [state, btn]);
  line.querySelector('.row-text').append(waiting);
  return line;
}

function compact(n) {
  return new Intl.NumberFormat(lang === 'vi' ? 'vi-VN' : 'en-US', { notation: 'compact', maximumFractionDigits: n < 1e4 ? 1 : 2 }).format(n);
}
function usd(amount) {
  return new Intl.NumberFormat(lang === 'vi' ? 'vi-VN' : 'en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: amount >= 1000 ? 0 : 2 }).format(amount);
}

/** What one agent used today and in the week, with the cost when there is a price. */
function spendRow(spend) {
  const line = (key, tokens, cost) => t(key, { tokens: compact(tokens) }) + (cost == null ? '' : ' · ' + usd(cost));
  return row(t('agents.spend.' + spend.agent), line('agents.spend.today', spend.today, spend.today_cost),
    h('span', { class: 'row-title', text: line('agents.spend.week', spend.week, spend.week_cost) }));
}

// The Accounts section: the Claude Code and Codex accounts Orca keeps, their limits, and switching the account of
// the plain claude and codex commands (see crates/app-ui/src/accounts.rs). Its words are kept with it.
Object.assign(I18N.en, {
  'sec.agents.accounts': 'Accounts',
  'accounts.intro.title': 'Accounts in Orca',
  'accounts.intro.body': 'The Claude Code and Codex accounts you keep in Orca. Switching to one copies its sign-in to the plain claude (or codex) command, after keeping a private copy of the account the command used, which Restore puts back.',
  'accounts.cli': 'The claude command uses {email}.',
  'accounts.cli.outside': 'The claude command uses {email}, an account Orca does not keep. A switch keeps a copy of it, so Restore brings it back.',
  'accounts.badge.cli.claude': 'Used by claude', 'accounts.badge.cli.codex': 'Used by codex', 'accounts.badge.orca': 'Used by Orca',
  'accounts.switch': 'Switch',
  'accounts.switch.title': 'Use {email} for the {cmd} command?',
  'accounts.switch.body.claude': 'Figghost keeps a private copy of the account the command uses now, so Restore can bring it back. New Claude Code sessions use the new account; running ones may need a restart. The account then shares one sign-in with Orca: when one of them renews it, the other may have to sign in again.',
  'accounts.switch.body.codex': 'Figghost keeps a private copy of ~/.codex/auth.json, so Restore can bring it back. New Codex sessions use the new account; running ones may need a restart.',
  'accounts.refresh.title': 'Limits of each account',
  'accounts.limits.desc': 'Refresh asks Anthropic for the limits of each Claude account with its own sign-in, read from the Keychain; macOS asks you first whether Figghost may read it. After that they are asked again every five minutes while this page is open. A sign-in that has run out is never renewed here, because that would sign Orca out; its numbers come back once Orca or Claude Code has renewed it.',
  'accounts.refresh': 'Refresh', 'accounts.asking': 'Asking...', 'accounts.updated': 'Updated {when}',
  'accounts.nokeychain': 'A private instance does not use the Keychain.',
  'accounts.note.none': 'Press Refresh to see the limits.',
  'accounts.note.expired': 'The sign-in has run out. The numbers come back once Orca or Claude Code renews it.',
  'accounts.note.no_sign_in': 'No sign-in was found for this account. Sign in to it again in Orca.',
  'accounts.note.refused': 'The Keychain did not let Figghost read the sign-in. Press Refresh to be asked again.',
  'accounts.note.failed': 'Anthropic did not give the limits. Try again later.',
  'accounts.note.codex': 'The limits of Codex accounts are not shown here.',
  'accounts.empty': 'Orca keeps no Claude Code or Codex accounts on this Mac.',
  'accounts.restore.claude': 'Restore the previous Claude Code account', 'accounts.restore.codex': 'Restore the previous Codex account',
  'accounts.restore.desc': 'Puts back the sign-in the command had before the last switch.',
  'accounts.restore': 'Restore', 'accounts.restore.confirm': 'Restore the previous account?',
  'accounts.done.switched': 'Switched. New sessions use the new account.',
  'accounts.done.restored': 'The previous account is back.',
  'accounts.done.failed': 'That did not work: {detail}',
  'fault.accounts_failed': 'Could not change the account.',
});
Object.assign(I18N.vi, {
  'sec.agents.accounts': 'Tài khoản',
  'accounts.intro.title': 'Tài khoản trong Orca',
  'accounts.intro.body': 'Các tài khoản Claude Code và Codex bạn lưu trong Orca. Chuyển sang một tài khoản sẽ chép thông tin đăng nhập của nó cho lệnh claude (hoặc codex), sau khi giữ riêng một bản của tài khoản lệnh đang dùng để nút Khôi phục đặt lại.',
  'accounts.cli': 'Lệnh claude đang dùng {email}.',
  'accounts.cli.outside': 'Lệnh claude đang dùng {email}, một tài khoản Orca không lưu. Khi chuyển, Figghost giữ một bản của nó để nút Khôi phục đặt lại.',
  'accounts.badge.cli.claude': 'claude đang dùng', 'accounts.badge.cli.codex': 'codex đang dùng', 'accounts.badge.orca': 'Orca đang dùng',
  'accounts.switch': 'Chuyển',
  'accounts.switch.title': 'Dùng {email} cho lệnh {cmd}?',
  'accounts.switch.body.claude': 'Figghost giữ riêng một bản của tài khoản lệnh đang dùng để nút Khôi phục đặt lại. Phiên Claude Code mới dùng tài khoản mới; phiên đang chạy có thể cần khởi động lại. Sau đó tài khoản này dùng chung một lần đăng nhập với Orca: khi một bên gia hạn, bên kia có thể phải đăng nhập lại.',
  'accounts.switch.body.codex': 'Figghost giữ riêng một bản của ~/.codex/auth.json để nút Khôi phục đặt lại. Phiên Codex mới dùng tài khoản mới; phiên đang chạy có thể cần khởi động lại.',
  'accounts.refresh.title': 'Hạn mức của từng tài khoản',
  'accounts.limits.desc': 'Nút Làm mới hỏi Anthropic hạn mức của từng tài khoản Claude bằng chính thông tin đăng nhập của nó, đọc từ Keychain; macOS hỏi bạn trước khi cho Figghost đọc. Sau đó hạn mức được hỏi lại mỗi năm phút khi trang này đang mở. Figghost không bao giờ tự gia hạn thông tin đăng nhập đã hết hạn, vì làm vậy sẽ đăng xuất Orca; con số hiện lại khi Orca hoặc Claude Code đã gia hạn.',
  'accounts.refresh': 'Làm mới', 'accounts.asking': 'Đang hỏi...', 'accounts.updated': 'Cập nhật {when}',
  'accounts.nokeychain': 'Bản chạy riêng không dùng Keychain.',
  'accounts.note.none': 'Bấm Làm mới để xem hạn mức.',
  'accounts.note.expired': 'Thông tin đăng nhập đã hết hạn. Con số hiện lại khi Orca hoặc Claude Code gia hạn.',
  'accounts.note.no_sign_in': 'Không tìm thấy thông tin đăng nhập của tài khoản này. Hãy đăng nhập lại trong Orca.',
  'accounts.note.refused': 'Keychain không cho Figghost đọc thông tin đăng nhập. Bấm Làm mới để được hỏi lại.',
  'accounts.note.failed': 'Anthropic không trả về hạn mức. Hãy thử lại sau.',
  'accounts.note.codex': 'Hạn mức của tài khoản Codex không hiện ở đây.',
  'accounts.empty': 'Orca không lưu tài khoản Claude Code hay Codex nào trên máy này.',
  'accounts.restore.claude': 'Khôi phục tài khoản Claude Code trước đó', 'accounts.restore.codex': 'Khôi phục tài khoản Codex trước đó',
  'accounts.restore.desc': 'Đặt lại thông tin đăng nhập mà lệnh dùng trước lần chuyển gần nhất.',
  'accounts.restore': 'Khôi phục', 'accounts.restore.confirm': 'Khôi phục tài khoản trước đó?',
  'accounts.done.switched': 'Đã chuyển. Phiên mới dùng tài khoản mới.',
  'accounts.done.restored': 'Đã đặt lại tài khoản trước đó.',
  'accounts.done.failed': 'Không làm được: {detail}',
  'fault.accounts_failed': 'Không đổi được tài khoản.',
});

function accountsState() { return S.agents.accounts || { list: [], restore: {}, keychain: false }; }

/** One window of an account's limits, drawn like the bars of the notch. */
function accountLimit(limit) {
  const used = limit.fraction == null ? 0 : Math.min(1, Math.max(0, limit.fraction));
  const tone = used >= 0.9 ? ' bad' : used >= 0.7 ? ' warn' : '';
  return h('div', { class: 'acct-limit' },
    h('span', { text: limit.label }),
    h('span', { class: 'acct-bar' + tone, role: 'img', 'aria-label': limit.label + ' ' + limit.value }, h('i', { style: `width:${used * 100}%` })),
    h('span', { class: 'acct-value', text: limit.value }),
    h('span', { class: 'acct-reset', text: limit.reset }));
}

function accountRow(account) {
  const details = [account.name !== account.email && account.name, account.plan, account.agent === 'claude' ? 'Claude Code' : 'Codex'];
  const switchBtn = !account.cli && button(t('accounts.switch'), () => confirmDialog({
    title: t('accounts.switch.title', { email: account.email, cmd: account.agent }),
    body: t('accounts.switch.body.' + account.agent),
    confirm: t('accounts.switch'),
  }, () => send({ t: 'action', name: 'accounts_switch:' + account.id })), { small: true, aria: t('accounts.switch') + ': ' + account.email });
  if (switchBtn) reg(() => { switchBtn.disabled = accountsState().busy || (account.agent === 'claude' && !accountsState().keychain); });
  const line = row(account.email, details.filter(Boolean).join(' · '), [
    account.cli && pill(() => t('accounts.badge.cli.' + account.agent), () => true),
    account.orca && pill(() => t('accounts.badge.orca'), () => true),
    switchBtn,
  ]);
  const note = account.agent === 'codex' ? 'codex' : account.note || (account.limits.length ? '' : 'none');
  line.querySelector('.row-text').append(...account.limits.map(accountLimit), note && h('div', { class: 'row-desc', text: t('accounts.note.' + note) }));
  return line;
}

function accountsRows() {
  const intro = infoRow(t('accounts.intro.title'), t('accounts.intro.body'));
  const said = h('div', { class: 'row-desc', role: 'status' });
  intro.querySelector('.row-text').append(said);
  reg(() => {
    const A = accountsState();
    const inOrca = A.list.some((a) => a.agent === 'claude' && a.cli);
    const lines = [A.cli && t(inOrca ? 'accounts.cli' : 'accounts.cli.outside', { email: A.cli }),
      A.outcome && t('accounts.done.' + A.outcome.what, { detail: A.outcome.detail })];
    said.textContent = lines.filter(Boolean).join(' ');
    said.style.display = said.textContent ? '' : 'none';
  });
  const refresh = button(t('accounts.refresh'), () => send({ t: 'action', name: 'accounts_refresh' }), { small: true });
  const status = h('div', { class: 'row-desc', role: 'status' });
  reg(() => {
    const A = accountsState();
    refresh.disabled = A.busy || !A.keychain;
    status.textContent = A.busy ? t('accounts.asking') : !A.keychain ? t('accounts.nokeychain') : A.updated ? t('accounts.updated', { when: A.updated }) : '';
    status.style.display = status.textContent ? '' : 'none';
  });
  const refreshRow = row(t('accounts.refresh.title'), t('accounts.limits.desc'), refresh);
  refreshRow.querySelector('.row-text').append(status);
  const restoreRow = (agent) => {
    const btn = button(t('accounts.restore'), () => confirmDialog({
      title: t('accounts.restore.confirm'), body: t('accounts.restore.desc'), confirm: t('accounts.restore'),
    }, () => send({ t: 'action', name: 'accounts_restore:' + agent })), { small: true, aria: t('accounts.restore.' + agent) });
    const line = row(t('accounts.restore.' + agent), t('accounts.restore.desc'), btn);
    reg(() => {
      line.style.display = accountsState().restore[agent] ? '' : 'none';
      btn.disabled = accountsState().busy;
    });
    return line;
  };
  return [intro, dynamicList(() => accountsState().list, accountRow, t('accounts.empty')), refreshRow, restoreRow('claude'), restoreRow('codex')];
}

function pageAgents() {
  const body = [
    card(toggleRow('agents.enabled', t('agents.enable.title'), t('agents.enable.desc'))),
    sectionTitle(t('sec.agents.connect')),
    card(agentLine('claude'), agentLine('codex'), agentLine('pi'), agentLine('agy')),
    sectionTitle(t('sec.agents.answer')),
    card(
      row(t('agents.wait.title'), t('agents.wait.desc'), waitSelect(), { when: () => S.agents.enabled }),
      toggleRow('agents.open_on_ask', t('agents.ask.title'), t('agents.ask.desc')),
      toggleRow('agents.sound', t('agents.sound.title'), t('agents.sound.desc')),
      toggleRow('agents.peek', t('agents.peek.title'), t('agents.peek.desc'))),
    sectionTitle(t('sec.agents.limits')),
    card(usageRow()),
    sectionTitle(t('sec.agents.accounts')),
    card(...accountsRows()),
    sectionTitle(t('sec.agents.tokens')),
    card(
      toggleRow('agents.tokens', t('agents.tokens.title'), t('agents.tokens.desc')),
      dynamicList(() => (S.agents.tokens ? S.agents.spend || [] : []), spendRow, t('agents.spend.none'))),
    sectionTitle(t('sec.agents.note')),
    card(infoRow(t('agents.note.title'), t('agents.note.body'))),
  ];
  return body;
}

// -- window snapping ------------------------------------------------

const LAYOUT_RECTS = {
  left: [0, 0, 0.5, 1], right: [0.5, 0, 0.5, 1], top: [0, 0, 1, 0.5], bottom: [0, 0.5, 1, 0.5],
  top_left: [0, 0, 0.5, 0.5], top_right: [0.5, 0, 0.5, 0.5], bottom_left: [0, 0.5, 0.5, 0.5], bottom_right: [0.5, 0.5, 0.5, 0.5],
  maximize: [0, 0, 1, 1], center: [0.2, 0.18, 0.6, 0.64], first_third: [0, 0, 1 / 3, 1], center_third: [1 / 3, 0, 1 / 3, 1], last_third: [2 / 3, 0, 1 / 3, 1],
  grid_1: [0, 0, 1 / 3, 1 / 3], grid_2: [1 / 3, 0, 1 / 3, 1 / 3], grid_3: [2 / 3, 0, 1 / 3, 1 / 3],
  grid_4: [0, 1 / 3, 1 / 3, 1 / 3], grid_5: [1 / 3, 1 / 3, 1 / 3, 1 / 3], grid_6: [2 / 3, 1 / 3, 1 / 3, 1 / 3],
  grid_7: [0, 2 / 3, 1 / 3, 1 / 3], grid_8: [1 / 3, 2 / 3, 1 / 3, 1 / 3], grid_9: [2 / 3, 2 / 3, 1 / 3, 1 / 3],
};

function layoutTile(layout) {
  const [x, y, w, hh] = LAYOUT_RECTS[layout.key];
  const rect = h('i', { style: `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${hh * 100}%` });
  const target = 'tiling:' + layout.key;
  const clear = h('button', { class: 'btn icon', 'aria-label': t('tiling.clear'), title: t('tiling.clear'), onclick: () => send({ t: 'hotkey', target, value: '' }) }, icon('close'));
  reg(() => { clear.style.visibility = shortcutOf(target) ? 'visible' : 'hidden'; });
  return h('div', { class: 'tile' },
    h('div', { class: 'tile-top' }, h('div', { class: 'mini' }, rect), h('div', { class: 'tile-name', text: t('layout.' + layout.key) })),
    h('div', { class: 'tile-actions' }, hotkeyButton(target, { allowClear: true, label: t('layout.' + layout.key) }), clear));
}

function pageTiling() {
  const grid = h('section', { class: 'card' }, h('div', { class: 'tiles' }, S.tiling.layouts.map(layoutTile)));
  reg(() => grid.classList.toggle('dim', !S.tiling.enabled));
  return [
    card(toggleRow('tiling.enabled', t('tiling.enable.title'), t('tiling.enable.desc'))),
    grid,
    h('div', {}, button(t('tiling.reset'), () => send({ t: 'action', name: 'reset_tiling' }), { icon: 'refresh', small: true })),
  ];
}

// -- about ----------------------------------------------------------

// Where the update stands, as the app reports it: checking, current, available, installing, ready, blocked or failed.
let release = { state: 'idle', version: '', detail: '' };
function onUpdate(message) {
  release = message;
  runSyncs();
}

/** The update row: a button that looks for a new version, and what is happening to the one it found. */
function updateRow() {
  const desc = h('div', { class: 'row-desc' });
  const ask = (name) => () => send({ t: 'action', name });
  const install = button(t('about.update.install'), ask('install_update'), { small: true, primary: true });
  const restart = button(t('about.update.restart'), ask('restart'), { small: true, primary: true });
  const open = button(t('about.update.open'), ask('open_release'), { small: true, primary: true });
  const check = button(t('about.update.check'), ask('check_updates'), { small: true, icon: 'refresh' });
  const el = h('div', { class: 'row' },
    h('div', { class: 'row-text' }, h('div', { class: 'row-title', text: t('about.update') }), desc),
    h('div', { class: 'row-control' }, install, restart, open, check));
  reg(() => {
    const { state, version: v, detail } = release;
    desc.textContent = state === 'checking' ? t('about.update.checking')
      : state === 'current' ? t('about.update.current', { v: S.app.version })
      : state === 'available' ? t('about.update.available', { v })
      : state === 'installing' ? t(release.stage === 'downloading' ? 'about.update.downloading' : 'about.update.installing', { v, p: release.percent || 0 })
      : state === 'ready' ? t('about.update.ready', { v })
      : state === 'blocked' ? t('about.update.blocked', { v, detail })
      : state === 'failed' ? t('about.update.failed', { detail })
      : t('about.update.desc');
    install.style.display = state === 'available' ? '' : 'none';
    restart.style.display = state === 'ready' ? '' : 'none';
    open.style.display = state === 'blocked' ? '' : 'none';
    check.disabled = state === 'checking' || state === 'installing';
  });
  return el;
}

// About: what the app is, where it comes from and under what licence it is used.
// The licence text is the one in the LICENSE file of the repository, line for line; a unit test keeps them equal.
const MIT_LICENSE = [
  'MIT License',
  '',
  'Copyright (c) 2026 Doanh Thai Bui',
  '',
  'Permission is hereby granted, free of charge, to any person obtaining a copy',
  'of this software and associated documentation files (the "Software"), to deal',
  'in the Software without restriction, including without limitation the rights',
  'to use, copy, modify, merge, publish, distribute, sublicense, and/or sell',
  'copies of the Software, and to permit persons to whom the Software is',
  'furnished to do so, subject to the following conditions:',
  '',
  'The above copyright notice and this permission notice shall be included in all',
  'copies or substantial portions of the Software.',
  '',
  'THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR',
  'IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,',
  'FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE',
  'AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER',
  'LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,',
  'OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE',
  'SOFTWARE.',
];

Object.assign(I18N.en, {
  'about.sec.what': 'About Figghost',
  'about.sec.map': 'What is where',
  'about.map.title': 'Six groups, one dock',
  'about.map.body': 'The icons of the dock at the foot of this window open six groups of pages. Press one and drag along the dock to look through them; Command-K finds any setting by name.',
  'about.what.title': 'One light app for the small tools of a Mac',
  'about.what.body': 'Smooth mouse-wheel scrolling and a pointer of your own, a clipboard history, window snapping from the notch, and a panel that grows out of the notch with tabs for files, media, your calendar, coding agents, weather, system health and more. Vietnamese typing in every app, Smart Select, Translate, screenshots, screen recordings, a colour picker, sound for each app and a window kept on top can be switched on when you want them.',
  'about.native.title': 'Native, and kept on this Mac',
  'about.native.body': 'It is written in Rust on AppKit, with no browser engine in the app itself; only this settings window is a web page. It idles light, comes in English and Vietnamese, and everything it keeps stays on this Mac.',
  'about.sec.history': 'How it came to be',
  'about.why.title': 'Why it exists',
  'about.why.body': 'Smooth scrolling, a clipboard history and a notch toolbox usually come as separate apps: Mos, Maccy and OneNotch gave it its ideas, SoundSource and CleanShot X those of the sound and capture tools, and Gõ Nhanh the engine of its Vietnamese typing, used unchanged. Figghost brings them into one app with one permission prompt, one login item and one settings window. Their ideas inspired it; its code and artwork are its own.',
  'about.versions.title': 'Versions',
  'about.v.0_2': 'Version 0.2, 15 October 2026', 'about.v.0_2.body': 'Sound for each app, screenshots, scrolling capture and screen recordings with an editor, the settings window, AI agents on the notch, Smart Select, a calendar, lyrics, snapping to the notch, a menu bar icon hider and Copy Stack.',
  'about.v.0_4': 'Versions 0.3 and 0.4, 16 October 2026', 'about.v.0_4.body': 'Every surface rebuilt on Liquid Glass, and the clipboard mascot.',
  'about.v.0_5': 'Version 0.5, 19 October 2026', 'about.v.0_5.body': 'Mole: the health, clean-up and uninstall tools of the Mac on a page of their own.',
  'about.v.0_6': 'Version 0.6, 19 October 2026', 'about.v.0_6.body': 'Updating from inside the app: a daily look for a new version, the download and the install, with what is new shown first.',
  'about.v.0_7': 'Version 0.7, 19 October 2026', 'about.v.0_7.body': 'A redesigned notch, and the Drag wheel, which converts files and runs tools on them while you drag.',
  'about.v.0_8': 'Version 0.8, 19 October 2026', 'about.v.0_8.body': 'A new app icon and a sidebar with a name beside every icon.',
  'about.v.0_9': 'Version 0.9, 19 October 2026', 'about.v.0_9.body': 'A hello from the ghost, Translate, Bluetooth in the notch, 3D sound, and window snapping in two steps with layouts of your own.',
  'about.v.1_0': 'Version 1.0, 19 October 2026', 'about.v.1_0.body': 'The first big release: a custom cursor, Always on top, a Shortcuts page, a search field for the settings, and a simpler area picker for screenshots.',
  'about.v.1_x': 'Versions 1.1 to 1.9, 19 to 22 October 2026', 'about.v.1_x.body': 'Questions answered on the notch, a music widget and more volume indicators, a permissions check, ten more notch tabs (System, Notes, Launcher, Weather, Pull requests, Pomodoro, Screen Time, Day progress, Devices and Displays), live islands, utilities, Orca accounts, pi, and Vietnamese typing.',
  'about.v.2_0': 'Version 2.0, 22 October 2026', 'about.v.2_0.body': 'A new settings window on Liquid Glass: the page groups are icons in a floating dock with a lens you can drag, and the search is in the title bar.',
  'about.v.2_0_2': 'Version 2.0.2, 22 October 2026', 'about.v.2_0_2.body': 'The language of Vietnamese typing on the notch and in the menu bar, a shortcut that can be recorded even when the system uses it, Spotlight, and a dock with clearer icons.',
  'about.sec.license': 'Licence',
  'about.license.title': 'MIT License',
  'about.license.body': 'Figghost is free, open source software under the MIT License, copyright 2026 Doanh Thai Bui. You may use it for any purpose, at home or at work, copy it, change it, share it and even sell what you make from it. The one condition is that the copyright notice and the licence text stay with every copy or substantial part of it. It comes without any warranty: the authors are not liable for what it does on your Mac.',
  'about.license.show': 'Show the licence text', 'about.license.hide': 'Hide the licence text',
  'about.others.title': 'Other licences',
  'about.others.body': 'The font Inter is bundled as the fallback of this window, under the SIL Open Font License 1.1 (copyright 2020 The Inter Project Authors); its text is in Resources/Inter-OFL.txt inside the app, and that of Figghost in Resources/LICENSE.txt. Patrick Hand, the handwriting of the titles and of the ghost, is under the same licence (Resources/PatrickHand-OFL.txt). Vietnamese typing puts its marks with the engine of Gõ Nhanh (copyright 2025 Gõ Nhanh Contributors), under the BSD 3-Clause License, whose text is in Resources/GoNhanh-LICENSE.txt. The icons of the interface are SF Symbols, which macOS draws itself and which are not part of Figghost. The Rust libraries it is built from keep their own licences.',
});
Object.assign(I18N.vi, {
  'about.sec.what': 'Giới thiệu Figghost',
  'about.sec.map': 'Có gì ở đâu',
  'about.map.title': 'Sáu nhóm, một dock',
  'about.map.body': 'Các biểu tượng trên dock ở chân cửa sổ này mở sáu nhóm trang. Nhấn một biểu tượng rồi kéo dọc dock để xem lần lượt; Command-K tìm mọi cài đặt theo tên.',
  'about.what.title': 'Một ứng dụng nhẹ cho các công cụ nhỏ của máy Mac',
  'about.what.body': 'Cuộn chuột mượt và con trỏ của riêng bạn, lịch sử clipboard, chia cửa sổ từ notch, và một bảng mở ra từ notch với các tab cho tệp, nhạc, lịch, tác tử lập trình, thời tiết, sức khỏe hệ thống và nhiều thứ khác. Gõ tiếng Việt ở mọi ứng dụng, Chọn thông minh, Dịch, chụp màn hình, quay màn hình, chọn màu, âm thanh riêng cho từng ứng dụng và giữ một cửa sổ luôn trên cùng có thể bật khi bạn cần.',
  'about.native.title': 'Bản địa, và ở lại trên máy này',
  'about.native.body': 'Ứng dụng viết bằng Rust trên AppKit, không có trình duyệt nhúng trong chính ứng dụng; chỉ cửa sổ cài đặt này là một trang web. Nó chạy nhẹ, có tiếng Anh và tiếng Việt, và mọi thứ nó lưu đều ở lại trên máy Mac này.',
  'about.sec.history': 'Vì sao có Figghost',
  'about.why.title': 'Lý do ra đời',
  'about.why.body': 'Cuộn mượt, lịch sử clipboard và hộp công cụ notch thường là các ứng dụng riêng lẻ: Mos, Maccy và OneNotch gợi ý tưởng cho nó, SoundSource và CleanShot X gợi ý cho phần âm thanh và chụp màn hình, còn Gõ Nhanh là bộ máy gõ tiếng Việt, dùng nguyên bản. Figghost gom chúng vào một ứng dụng với một lần xin quyền, một mục khởi động cùng máy và một cửa sổ cài đặt. Ý tưởng lấy cảm hứng từ các ứng dụng đó; mã nguồn và hình ảnh là của riêng Figghost.',
  'about.versions.title': 'Các phiên bản',
  'about.v.0_2': 'Phiên bản 0.2, 15/10/2026', 'about.v.0_2.body': 'Âm thanh riêng cho từng ứng dụng, chụp màn hình, chụp cuộn và quay màn hình có trình chỉnh sửa, cửa sổ cài đặt, tác tử AI trên notch, Smart Select, lịch, lời bài hát, ghim cửa sổ lên notch, ẩn biểu tượng thanh menu và Copy Stack.',
  'about.v.0_4': 'Phiên bản 0.3 và 0.4, 16/10/2026', 'about.v.0_4.body': 'Dựng lại mọi bề mặt trên Liquid Glass, và linh vật của clipboard.',
  'about.v.0_5': 'Phiên bản 0.5, 19/10/2026', 'about.v.0_5.body': 'Mole: các công cụ kiểm tra sức khỏe, dọn dẹp và gỡ ứng dụng của máy Mac trên một trang riêng.',
  'about.v.0_6': 'Phiên bản 0.6, 19/10/2026', 'about.v.0_6.body': 'Cập nhật ngay trong ứng dụng: mỗi ngày tự tìm bản mới, tải và cài, có hiện trước những gì mới.',
  'about.v.0_7': 'Phiên bản 0.7, 19/10/2026', 'about.v.0_7.body': 'Notch được thiết kế lại, và Vòng kéo thả giúp đổi định dạng tệp và chạy công cụ trên tệp ngay khi bạn đang kéo.',
  'about.v.0_8': 'Phiên bản 0.8, 19/10/2026', 'about.v.0_8.body': 'Biểu tượng ứng dụng mới và thanh bên có tên bên cạnh mỗi biểu tượng.',
  'about.v.0_9': 'Phiên bản 0.9, 19/10/2026', 'about.v.0_9.body': 'Lời chào của chú ma, Dịch, Bluetooth trên notch, âm thanh 3D, và ghim cửa sổ hai bước với bố cục của riêng bạn.',
  'about.v.1_0': 'Phiên bản 1.0, 19/10/2026', 'about.v.1_0.body': 'Bản lớn đầu tiên: con trỏ tùy chỉnh, Luôn trên cùng, trang Phím tắt, ô tìm trong cài đặt, và cách chọn vùng chụp màn hình gọn hơn.',
  'about.v.1_x': 'Phiên bản 1.1 đến 1.9, 19 đến 22/10/2026', 'about.v.1_x.body': 'Trả lời câu hỏi ngay trên notch, widget nhạc và thêm kiểu chỉ báo âm lượng, kiểm tra quyền, thêm mười tab notch (Hệ thống, Ghi chú, Mở nhanh, Thời tiết, Yêu cầu duyệt, Pomodoro, Thời gian dùng, Tiến độ ngày, Thiết bị và Màn hình), đảo trạng thái, tiện ích, tài khoản Orca, pi, và gõ tiếng Việt.',
  'about.v.2_0': 'Phiên bản 2.0, 22/10/2026', 'about.v.2_0.body': 'Cửa sổ cài đặt mới trên Liquid Glass: các nhóm trang là biểu tượng trong một dock nổi có thấu kính kéo được, và ô tìm nằm ở thanh tiêu đề.',
  'about.v.2_0_2': 'Phiên bản 2.0.2, 22/10/2026', 'about.v.2_0_2.body': 'Ngôn ngữ gõ tiếng Việt hiện trên notch và ở thanh menu, phím tắt ghi được cả khi hệ thống đang dùng, Spotlight, và dock có biểu tượng rõ hơn.',
  'about.sec.license': 'Giấy phép',
  'about.license.title': 'Giấy phép MIT',
  'about.license.body': 'Figghost là phần mềm mã nguồn mở, miễn phí, theo Giấy phép MIT, bản quyền 2026 Doanh Thai Bui. Bạn được dùng cho mọi mục đích, ở nhà hay ở nơi làm việc, sao chép, sửa đổi, chia sẻ và thậm chí bán sản phẩm bạn làm ra từ nó. Điều kiện duy nhất là thông báo bản quyền và nội dung giấy phép phải đi kèm mọi bản sao hoặc phần đáng kể của phần mềm. Phần mềm được cung cấp nguyên trạng, không bảo hành: tác giả không chịu trách nhiệm về những gì nó làm trên máy Mac của bạn.',
  'about.license.show': 'Xem nội dung giấy phép', 'about.license.hide': 'Ẩn nội dung giấy phép',
  'about.others.title': 'Các giấy phép khác',
  'about.others.body': 'Phông Inter đi kèm làm phông dự phòng cho cửa sổ này, theo SIL Open Font License 1.1 (bản quyền 2020 The Inter Project Authors); nội dung giấy phép nằm ở Resources/Inter-OFL.txt trong ứng dụng, còn của Figghost ở Resources/LICENSE.txt. Patrick Hand, nét chữ tay của các tiêu đề và của chú ma, theo cùng giấy phép (Resources/PatrickHand-OFL.txt). Gõ tiếng Việt đặt dấu bằng bộ máy của Gõ Nhanh (bản quyền 2025 Gõ Nhanh Contributors), theo giấy phép BSD 3-Clause, nội dung nằm ở Resources/GoNhanh-LICENSE.txt. Các biểu tượng của giao diện là SF Symbols do macOS tự vẽ, không thuộc về Figghost. Các thư viện Rust dùng để xây dựng ứng dụng giữ giấy phép riêng của chúng.',
});

/** A group of the dock with the pages inside it, as the dock and the tabs name them, so this list cannot drift from them. */
function mapRow(cat) {
  const tile = h('span', { 'data-cat': cat.id }, h('span', { class: 'nav-icon map-tile' }, icon(cat.icon)));
  return row(h('span', { class: 'map-title' }, tile, t('cat.' + cat.id)), cat.pages.map((id) => t('nav.' + id)).join(' · '), null);
}

/** What Figghost itself uses, as numbers that follow the state. */
function usageCard() {
  const bytes = (n) => {
    if (!(n > 0)) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
    return (n / 1024 ** i).toFixed(i > 1 ? 1 : 0) + ' ' + units[i];
  };
  const percent = (v) => (v < 10 ? v.toFixed(1) : Math.round(v)) + '%';
  const shown = {
    cpu: (u) => percent(u.cpu),
    gpu: (u) => percent(u.gpu),
    memory: (u) => bytes(u.memory),
    net: (u) => bytes(u.net) + '/s',
    disk: (u) => t('about.usage.disk.n', { r: bytes(u.disk_read), w: bytes(u.disk_write) }),
    fps: (u) => t('about.usage.fps.n', { n: Math.round(u.fps) }),
  };
  const rows = Object.entries(shown).map(([key, format]) => {
    const value = h('span', { class: 'row-title usage-value' });
    reg(() => {
      const u = S.app.usage;
      value.textContent = u && u[key === 'disk' ? 'disk_read' : key] != null ? format(u) : t('about.usage.wait');
    });
    return row(t('about.usage.' + key), null, value);
  });
  return [sectionTitle(t('about.usage.title')), card(row(t('about.usage.title'), t('about.usage.desc'), null), ...rows)];
}

function pageAbout() {
  const text = h('pre', { class: 'license', hidden: true, text: MIT_LICENSE.join('\n') });
  const toggle = h('button', { class: 'btn small', 'aria-expanded': 'false', text: t('about.license.show'), onclick: () => {
    text.hidden = !text.hidden;
    toggle.setAttribute('aria-expanded', String(!text.hidden));
    toggle.textContent = t(text.hidden ? 'about.license.show' : 'about.license.hide');
  } });
  const version = (key) => row(t('about.' + key), t('about.' + key + '.body'), null);
  return [
    h('section', { class: 'hero card' },
      appIcon('hero-mark'),
      h('div', {}, h('h2', { text: 'Figghost' }), h('p', { text: t('about.line') }))),
    card(
      row(t('about.version'), null, h('span', { class: 'row-title', text: S.app.version })),
      updateRow(),
      toggleRow('update.auto', t('update.auto.title'), t('update.auto.desc')),
      row(t('update.news.title'), t('update.news.desc'), h('div', { class: 'row-actions' }, button(t('update.news.show'), () => send({ t: 'action', name: 'show_whats_new' }), { small: true }), switchEl('update.whats_new', t('update.news.title')))),
      row(t('about.privacy'), t('about.privacy.val'), null),
      row(t('data.title'), S.app.config_dir, button(t('data.open'), () => send({ t: 'action', name: 'open_config' }), { icon: 'folder', small: true }))),
    ...usageCard(),
    sectionTitle(t('about.sec.what')),
    card(row(t('about.what.title'), t('about.what.body'), null), row(t('about.native.title'), t('about.native.body'), null)),
    sectionTitle(t('about.sec.map')),
    card(row(t('about.map.title'), t('about.map.body'), null), ...CATEGORIES.map(mapRow)),
    sectionTitle(t('about.sec.history')),
    card(row(t('about.why.title'), t('about.why.body'), null)),
    card(
      row(t('about.versions.title'), null, null),
      ...['2_0_2', '2_0', '1_x', '1_0', '0_9', '0_8', '0_7', '0_6', '0_5', '0_4', '0_2'].map((v) => version('v.' + v))),
    sectionTitle(t('about.sec.license')),
    card(
      row(t('about.license.title'), t('about.license.body'), toggle),
      text,
      row(t('about.others.title'), t('about.others.body'), null)),
    card(row(t('about.quit'), t('about.quit.desc'), button(t('about.quit'), () => send({ t: 'action', name: 'quit' }), { icon: 'power', small: true, danger: true }))),
  ];
}

const CATEGORIES = [
  { id: 'general', sec: 'overview', icon: 'gearshape.fill', pages: ['general', 'mole', 'about'] },
  { id: 'keys', sec: 'overview', icon: 'keyboard', pages: ['keys'] },
  { id: 'notch', sec: 'notch', icon: 'laptopcomputer', pages: ['notch', 'islands', 'musicwidget'] },
  { id: 'agents', sec: 'notch', icon: 'terminal.fill', pages: ['agents', 'prs'] },
  { id: 'calendar', sec: 'notch', icon: 'calendar', pages: ['calendar', 'pomodoro', 'dayprogress', 'screentime'] },
  { id: 'tabs', sec: 'notch', icon: 'square.stack.3d.up.fill', pages: ['notes', 'launcher', 'weather'] },
  { id: 'system', sec: 'notch', icon: 'cpu', pages: ['system', 'updates', 'devices', 'displays'] },
  { id: 'scroll', sec: 'mouse', icon: 'computermouse.fill', pages: ['scroll', 'cursor', 'wheel'] },
  { id: 'tiling', sec: 'mouse', icon: 'rectangle.lefthalf.inset.filled', pages: ['tiling', 'pin'] },
  { id: 'clipboard', sec: 'tools', icon: 'list.clipboard.fill', pages: ['clipboard', 'typing', 'snippets'] },
  { id: 'smart', sec: 'tools', icon: 'sparkles', pages: ['smart', 'translate', 'utilities'] },
  { id: 'capture', sec: 'tools', icon: 'camera.viewfinder', pages: ['capture', 'vision', 'download'] },
];

function categoryOf(pageId) {
  return CATEGORIES.find((c) => c.pages.includes(pageId)) || CATEGORIES[0];
}

const PAGES = [
  { id: 'general', icon: 'gearshape.fill', build: pageGeneral },
  { id: 'keys', icon: 'keyboard', build: pageKeys },
  { id: 'scroll', icon: 'computermouse.fill', build: pageScroll },
  { id: 'clipboard', icon: 'doc.on.clipboard.fill', build: pageClipboard },
  { id: 'notch', icon: 'rectangle.topthird.inset.filled', build: pageNotch },
  { id: 'agents', icon: 'terminal.fill', build: pageAgents },
  { id: 'tiling', icon: 'rectangle.split.2x1.fill', build: pageTiling },
  { id: 'about', icon: 'info.circle.fill', build: pageAbout },
];

// Pages, strings and cards that the features bring (web/features/*.js). A feature's page goes before
// "Window snapping"; its cards for an existing page are appended to that page.
const PLUGINS = window.__plugins || [];
for (const p of PLUGINS) {
  Object.assign(I18N.en, p.en || {});
  Object.assign(I18N.vi, p.vi || {});
  if (p.build) {
    if (!PAGES.some((x) => x.id === p.id)) {
      PAGES.splice(PAGES.findIndex((x) => x.id === 'tiling'), 0, { id: p.id, icon: p.icon || 'info.circle.fill', build: p.build });
    }
    if (!CATEGORIES.some((c) => c.pages.includes(p.id))) {
      CATEGORIES[CATEGORIES.length - 1].pages.push(p.id);
    }
  }
}
function slotsFor(pageId) {
  return PLUGINS.flatMap((p) => (p.slots && p.slots[pageId] ? [].concat(p.slots[pageId]()) : []));
}

// ---------------------------------------------------------------- shell

let current = 'general';
let shellSyncs = [];
let pageSyncs = [];

function accessPill() {
  const mark = ghost('mini');
  const label = h('span', { class: 'nav-label' });
  const dot = h('span', { class: 'dot' });
  const el = h('button', { class: 'status-pill', onclick: () => (S.env.accessibility ? go('general') : send({ t: 'action', name: 'open_accessibility' })) }, mark, label, dot);
  reg(() => {
    label.textContent = S.env.accessibility ? t('chip.access.ok') : t('chip.access.no');
    mark.classList.toggle('asleep', !S.env.accessibility);
    dot.classList.toggle('good', !!S.env.accessibility);
  });
  return el;
}

// ---------------------------------------------------------------- search

/** Text without case or Vietnamese marks, so "cuon" finds "Cuộn" and "hover" finds "Hover delay". */
function folded(text) {
  return String(text).normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[đĐ]/g, 'd').toLowerCase();
}

/** What each page says, folded: its name, the line under its title, and the titles and notes of its rows. Each
 *  page is built off screen for it the first time a search needs it, and what it registered is dropped at once. */
let searchIndex = null;
let searchWords = [];
function pageTexts() {
  if (searchIndex) return searchIndex;
  searchIndex = new Map();
  for (const p of PAGES) {
    let node = null;
    dropSyncs(scoped(() => { node = h('div', {}, [].concat(p.build(), slotsFor(p.id))); }));
    const parts = [t('nav.' + p.id), t('page.' + p.id + '.sub')];
    for (const el of node.querySelectorAll('.row-title, .row-desc, .section-title, .feat-name, label, .btn')) parts.push(el.textContent);
    searchIndex.set(p.id, folded(parts.join('\n')));
  }
  return searchIndex;
}

/** Lists under the search field the pages that say every word of `query`, and says so when none does. */
function applySearch(query) {
  searchWords = folded(query).split(/\s+/).filter(Boolean);
  const box = $('#search-results');
  if (!box) return;
  if (!searchWords.length) {
    box.hidden = true;
    box.replaceChildren();
    return;
  }
  const index = pageTexts();
  // A page whose own name says the words comes before one that only mentions them in a row, and one whose name
  // starts with them comes first of those. The sort is stable, so the order of the dock is kept among equals.
  const rank = (pid) => {
    const name = folded(t('nav.' + pid));
    return searchWords.every((w) => name.includes(w)) ? (name.startsWith(searchWords[0]) ? 0 : 1) : 2;
  };
  const hits = CATEGORIES.flatMap((cat) => cat.pages.map((pid) => ({ cat, pid })))
    .filter(({ pid }) => searchWords.every((w) => (index.get(pid) || '').includes(w)))
    .sort((a, b) => rank(a.pid) - rank(b.pid));
  box.replaceChildren(...(hits.length
    ? hits.map(({ cat, pid }, i) => h('button', {
      class: 'result',
      role: 'option',
      'data-page': pid,
      'data-cat': cat.id,
      'aria-selected': String(i === 0),
      // The field keeps the focus, so the list stays open until the choice is made.
      onmousedown: (e) => e.preventDefault(),
      onclick: () => openResult(pid),
    },
    h('span', { class: 'nav-icon' }, icon(cat.icon)),
    h('span', { class: 'result-text' },
      h('span', { class: 'result-name', text: t('nav.' + pid) }),
      h('span', { class: 'result-cat', text: t('cat.' + cat.id) })),
    h('kbd', { class: 'result-key', 'aria-hidden': 'true', text: '↩' })))
    : [h('div', { class: 'search-empty', text: t('side.none') })]));
  box.hidden = false;
}

/** Opens a page the search found, brings its row into view, and puts the search away. */
function openResult(pid) {
  go.rise = true;
  go(pid);
  reveal(searchWords);
  const field = $('#search');
  field.value = '';
  applySearch('');
  field.blur();
}

/** On the page just opened from a search, brings the first row that says the words into view and marks it. */
function reveal(words) {
  if (!words.length) return;
  const rows = [...document.querySelectorAll('#scroller .row, #scroller .section-title, #scroller .feat')];
  const hit = rows.find((row) => words.every((w) => folded(row.textContent).includes(w)));
  if (!hit) return;
  hit.scrollIntoView({ block: 'center', behavior: document.documentElement.classList.contains('reduce-motion') ? 'auto' : 'smooth' });
  hit.classList.add('found');
  setTimeout(() => hit.classList.remove('found'), 1600);
}

function navSearch() {
  const input = h('input', { type: 'search', id: 'search', autocomplete: 'off', spellcheck: 'false', placeholder: t('side.search'), 'aria-label': t('side.search'), 'aria-controls': 'search-results' });
  const box = h('div', { id: 'search-results', role: 'listbox', hidden: true });
  input.addEventListener('input', () => applySearch(input.value));
  input.addEventListener('focus', () => applySearch(input.value));
  input.addEventListener('blur', () => { box.hidden = true; });
  input.addEventListener('keydown', (e) => {
    const results = [...box.querySelectorAll('.result')];
    const at = results.findIndex((r) => r.getAttribute('aria-selected') === 'true');
    if (e.key === 'Escape') {
      e.preventDefault();
      if (input.value) { input.value = ''; applySearch(''); } else input.blur();
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && results.length) {
      e.preventDefault();
      const next = (at + (e.key === 'ArrowDown' ? 1 : results.length - 1)) % results.length;
      results.forEach((r, i) => r.setAttribute('aria-selected', String(i === next)));
      results[next].scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter' && results.length) {
      openResult(results[Math.max(at, 0)].dataset.page);
    }
  });
  // Command-K finds, and Command-F does the same, as in any Mac window. Bound once: the shell is rebuilt when the
  // language changes.
  if (!navSearch.bound) {
    navSearch.bound = true;
    document.addEventListener('keydown', (e) => {
      if (e.metaKey && !e.shiftKey && !e.altKey && !e.ctrlKey && (e.code === 'KeyF' || e.code === 'KeyK')) {
        e.preventDefault();
        const field = $('#search');
        if (field) { field.focus(); field.select(); }
      }
    });
  }
  return h('div', { class: 'search' },
    h('label', { class: 'top-search' }, icon('magnifyingglass'), input, h('kbd', { 'aria-hidden': 'true', text: '⌘K' })),
    box);
}

function openCategory(cat) {
  if (categoryOf(current).id === cat.id) return placeLens();
  go.rise = true;
  go(cat.lastPage || cat.pages[0]);
}

function renderNavItems() {
  const out = [];
  let lastSec = null;
  for (const cat of CATEGORIES) {
    if (cat.sec && cat.sec !== lastSec) {
      lastSec = cat.sec;
      out.push(h('div', { class: 'nav-sec', text: t('nav.sec.' + cat.sec) }));
    }
    out.push(h('button', {
      class: 'nav-item',
      'data-page': cat.id,
      'data-cat': cat.id,
      title: t('cat.' + cat.id),
      'aria-label': t('cat.' + cat.id),
      // A press of the pointer is handled by the sidebar's lens; this is the keyboard's Space and Return.
      onclick: (e) => { if (e.detail === 0) openCategory(cat); },
    },
    h('span', { class: 'nav-icon' }, icon(cat.icon)),
    h('span', { class: 'nav-label', text: t('cat.' + cat.id) })));
  }
  return out;
}

/** Puts the lens of the sidebar under the item of the open group: gliding there when a person chose it, at once
 *  otherwise, so a window that opens does not show it sliding in from the edge. */
function placeLens(glide = true) {
  const nav = $('#nav');
  const item = nav && nav.querySelector('.nav-item[aria-current="page"]');
  if (!item) return;
  nav.classList.toggle('still', !glide);
  nav.style.setProperty('--y', item.offsetTop + item.offsetHeight / 2 + 'px');
  nav.style.setProperty('--s', '1');
  if (!glide) {
    void nav.offsetWidth;
    nav.classList.remove('still');
  }
}

/** The sidebar's lens follows a press: it swells where the pointer goes down, slides with a drag (stretched by its
 *  speed, like a drop of water), lights the item under it, and on release opens that item's group. */
function dockLens(nav) {
  const items = () => [...nav.querySelectorAll('.nav-item')];
  const centre = (item) => item.offsetTop + item.offsetHeight / 2;
  const nearest = (y) => items().reduce((best, item) => (Math.abs(centre(item) - y) < Math.abs(centre(best) - y) ? item : best));
  let press = null;
  let settle = 0;
  const follow = (e) => {
    const all = items();
    const y = Math.min(Math.max(e.clientY - nav.getBoundingClientRect().top + nav.scrollTop, centre(all[0])), centre(all[all.length - 1]));
    const now = e.timeStamp;
    const speed = press.t ? Math.abs(y - press.y) / Math.max(now - press.t, 1) : 0;
    press.y = y;
    press.t = now;
    nav.style.setProperty('--y', y + 'px');
    nav.style.setProperty('--s', String(1 + Math.min(speed * 0.12, 0.25)));
    clearTimeout(settle);
    settle = setTimeout(() => nav.style.setProperty('--s', '1'), 90);
    const lit = nearest(y);
    for (const item of all) item.classList.toggle('lit', item === lit);
  };
  const end = (open) => {
    if (!press) return;
    const target = nearest(press.y);
    press = null;
    clearTimeout(settle);
    nav.classList.remove('pressing', 'dragging');
    for (const item of items()) item.classList.remove('lit');
    if (open) openCategory(CATEGORIES.find((c) => c.id === target.dataset.page));
    else placeLens();
  };
  nav.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    try { nav.setPointerCapture(e.pointerId); } catch (_) { /* a synthetic pointer has nothing to capture */ }
    press = { y: 0, t: 0, y0: e.clientY };
    nav.classList.add('pressing');
    follow(e);
  });
  nav.addEventListener('pointermove', (e) => {
    if (!press) return;
    if (Math.abs(e.clientY - press.y0) > 3) nav.classList.add('dragging');
    follow(e);
  });
  nav.addEventListener('pointerup', () => end(true));
  nav.addEventListener('pointercancel', () => end(false));
  nav.addEventListener('lostpointercapture', () => end(false));
  nav.prepend(h('div', { class: 'lens', 'aria-hidden': 'true' }));
  return nav;
}

function renderSubtabs(cat, activeId) {
  const container = $('#subtabs');
  if (!container) return;
  if (!cat || cat.pages.length <= 1) {
    container.replaceChildren();
    container.hidden = true;
    return;
  }
  container.hidden = false;
  const pills = cat.pages.map((pid) => {
    const isCur = pid === activeId;
    return h('button', {
      class: 'subtab' + (isCur ? ' active' : ''),
      'aria-pressed': String(isCur),
      'aria-label': t('nav.' + pid),
      onclick: () => { go.rise = true; go(pid); },
    }, t('nav.' + pid));
  });
  const bar = h('div', { class: 'subtabs-bar' }, ...pills);
  // A mouse wheel turns sideways here when the bar is longer than the window.
  bar.addEventListener('wheel', (e) => {
    if (bar.scrollWidth <= bar.clientWidth || Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
    e.preventDefault();
    bar.scrollLeft += e.deltaY;
  }, { passive: false });
  container.replaceChildren(bar);
  // A bar that scrolls keeps the chosen page in view.
  const chosen = bar.querySelector('.subtab.active');
  if (chosen) bar.scrollLeft = Math.max(0, chosen.offsetLeft - (bar.clientWidth - chosen.offsetWidth) / 2);
}

/** What the ghost says about a page: its first sentence, which is what the page is for. */
function guideLine(pageId) {
  const sub = t('page.' + pageId + '.sub');
  const end = sub.search(/[.!?](\s|$)/);
  return end > 0 && end < sub.length - 1 ? sub.slice(0, end + 1) : sub;
}

let quipTimer = 0;
let quipNext = 0;
/** A click on the ghost: it hops and says something else for a moment, then goes back to the page. */
function poke() {
  const bubble = $('.bubble');
  const say = $('#guide-text');
  const ghostButton = $('.guide-ghost');
  if (!bubble || !say || !ghostButton) return;
  say.textContent = t('guide.q' + ((quipNext++ % 4) + 1));
  bubble.classList.add('quip');
  ghostButton.classList.remove('hop');
  void ghostButton.offsetWidth;
  ghostButton.classList.add('hop');
  clearTimeout(quipTimer);
  quipTimer = setTimeout(() => { bubble.classList.remove('quip'); say.textContent = guideLine(current); }, 3200);
}

function buildShell() {
  document.documentElement.lang = lang;
  $('#root').replaceChildren(
    h('div', { id: 'app' },
      h('div', { class: 'dragzone', 'data-drag': '' }),
      h('aside', { id: 'sidebar' },
        navSearch(),
        dockLens(h('nav', { id: 'nav', 'aria-label': 'Figghost' }, ...renderNavItems())),
        h('div', { class: 'side-foot' }, accessPill())),
      h('main', { id: 'main' },
        h('header', { class: 'page-head' },
          h('div', { class: 'page-head-main' },
            h('div', { class: 'page-head-text' },
              h('h1', { class: 'page-title', id: 'title' }),
              h('p', { class: 'page-sub', id: 'subtitle' })),
            h('div', { id: 'subtabs', class: 'page-subtabs' }))),
        h('div', { id: 'scroller' }),
        h('div', { class: 'guide' },
          h('p', { class: 'bubble' }, h('span', { class: 'bubble-text', id: 'guide-text' })),
          h('button', { class: 'guide-ghost', 'aria-label': t('guide.label'), onclick: poke }, ghost())))),
    h('div', { id: 'toast', 'aria-live': 'polite' }),
    h('div', { id: 'modal' }),
    h('div', { class: 'defs', 'aria-hidden': 'true', html: WOBBLE_SVG }));
}

function go(id) {
  stopRecording();
  closeMenu();
  const page = PAGES.find((p) => p.id === id) || PAGES[0];
  current = page.id;
  const cat = categoryOf(current);
  cat.lastPage = current;
  dropSyncs(pageSyncs);
  graphs.clear();
  const title = $('#title');
  if (title) title.textContent = t('page.' + page.id);
  const sub = $('#subtitle');
  if (sub) sub.textContent = t('page.' + page.id + '.sub');
  renderSubtabs(cat, current);
  const say = $('#guide-text');
  if (say && !say.parentElement.classList.contains('quip')) say.textContent = guideLine(page.id);
  const guide = $('.guide-ghost');
  if (guide) guide.title = t('page.' + page.id + '.sub');
  const scroller = $('#scroller');
  // A page rises into place only when a person chose it; a page that is built for any other reason (the first one, a
  // new language, a check or a picture) is simply there, so nothing waits on an animation clock.
  const rise = go.rise === true;
  go.rise = false;
  pageSyncs = scoped(() => scroller.replaceChildren(h('div', { class: 'page' + (rise ? ' rise' : '') }, [].concat(page.build(), slotsFor(page.id)))));
  scroller.scrollTop = 0;
  for (const item of document.querySelectorAll('.nav-item')) {
    if (item.dataset.page === cat.id) item.setAttribute('aria-current', 'page'); else item.removeAttribute('aria-current');
  }
  placeLens(rise);
  send({ t: 'page', id: current });
}

function rebuild() {
  // The words of every page change with the language: the search starts over.
  searchIndex = null;
  searchWords = [];
  dropSyncs(shellSyncs);
  shellSyncs = scoped(buildShell);
  go(current);
}

function applyLanguage() {
  const next = resolveLang(S.general.language);
  if (next === lang) return;
  lang = next;
  rebuild();
}

/** The sky behind the page follows the hour. A state may name one (S.env.sky), which previews use. */
function applySky() {
  const hour = new Date().getHours();
  const sky = (S && S.env && S.env.sky) || (hour < 5 ? 'night' : hour < 9 ? 'dawn' : hour < 17 ? 'day' : hour < 20 ? 'dusk' : 'night');
  document.documentElement.dataset.sky = sky;
}
setInterval(applySky, 10 * 60 * 1000);

function applyDisplay() {
  if (!S) return;
  applySky();
  const list = document.documentElement.classList;
  list.toggle('reduce-transparency', !!S.env.reduce_transparency);
  list.toggle('reduce-motion', !!S.env.reduce_motion);
  list.toggle('increase-contrast', !!S.env.increase_contrast);
}

function onState(next) {
  const first = S === null;
  const before = lang;
  S = next;
  lang = resolveLang(S.general.language);
  applyDisplay();
  if (first) {
    shellSyncs = scoped(buildShell);
    go('general');
  } else if (lang !== before) {
    rebuild();
  } else {
    runSyncs();
  }
}

window.__fig = {
  recv(message) {
    switch (message.t) {
      case 'state': onState(message.state); break;
      case 'fault': onFault(message); break;
      case 'running': onRunning(message); break;
      case 'chord': onChord(message); break;
      case 'icons': onIcons(message.icons); break;
      case 'curve': onCurve(message); break;
      case 'update': onUpdate(message); break;
      case 'go': go(message.page); break;
    }
  },
};

// This is an app, not a web page: no browser context menu, except where the user may want to copy text.
document.addEventListener('contextmenu', (e) => { if (!e.target.closest('.code')) e.preventDefault(); });

// The title area moves and zooms the window, like a native title bar.
document.addEventListener('mousedown', (e) => { if (e.button === 0 && e.target.closest('[data-drag]')) window.fig.drag(); });
document.addEventListener('dblclick', (e) => { if (e.target.closest('[data-drag]')) window.fig.zoom(); });
window.addEventListener('blur', () => document.documentElement.classList.add('inactive'));
window.addEventListener('focus', () => document.documentElement.classList.remove('inactive'));

// ---------------------------------------------------------------- self-test (run by the headless check)

window.__selftest = function () {
  const failures = [];
  const expect = (ok, message) => { if (!ok) failures.push(message); };
  const sent = [];
  const realSend = window.fig.send;
  window.fig.send = (m) => sent.push(m);
  try {
    // Both languages cover the same keys.
    const en = Object.keys(I18N.en), vi = Object.keys(I18N.vi);
    expect(en.filter((k) => !I18N.vi[k]).length === 0, 'missing in vi: ' + en.filter((k) => !I18N.vi[k]).join(', '));
    expect(vi.filter((k) => !I18N.en[k]).length === 0, 'missing in en: ' + vi.filter((k) => !I18N.en[k]).join(', '));

    // The search in the top bar finds pages by what their settings say, with or without Vietnamese marks.
    {
      const field = $('#search');
      const search = (text) => { field.value = text; field.dispatchEvent(new Event('input')); };
      const shown = () => [...document.querySelectorAll('#search-results .result')].map((item) => item.dataset.page);
      search('hover');
      expect(shown().includes('notch') && !shown().includes('scroll'), 'search finds the notch by its hover delay: ' + shown());
      expect(folded('Cuộn chuột Đ') === 'cuon chuot d', 'search ignores marks: ' + folded('Cuộn chuột Đ'));
      field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      expect(current === 'notch' && field.value === '' && $('#search-results').hidden, 'Return opens the first page found and puts the search away: ' + current);
      search('zzqx');
      expect(shown().length === 0 && $('.search-empty') && !$('#search-results').hidden, 'a search that matches nothing says so');
      search('');
      expect($('#search-results').hidden, 'clearing the search closes the list');
      document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyK', metaKey: true, bubbles: true, cancelable: true }));
      expect(document.activeElement === field, 'Command-K puts the cursor in the search');
      field.blur();
    }

    // A press on the sidebar dragged from the first item to the fifth opens the fifth group, and the lens settles under it.
    {
      go('general');
      const nav = $('#nav');
      const items = [...nav.querySelectorAll('.nav-item')];
      const at = (item) => nav.getBoundingClientRect().top + item.offsetTop - nav.scrollTop + item.offsetHeight / 2;
      const pointer = (type, item) => nav.dispatchEvent(new PointerEvent(type, { pointerId: 9, button: 0, clientY: at(item), bubbles: true, cancelable: true }));
      pointer('pointerdown', items[0]);
      expect(nav.classList.contains('pressing') && items[0].classList.contains('lit'), 'a press swells the lens and lights the icon under it');
      pointer('pointermove', items[4]);
      expect(nav.classList.contains('dragging') && items[4].classList.contains('lit') && !items[0].classList.contains('lit'), 'a drag moves the light along');
      pointer('pointerup', items[4]);
      expect(categoryOf(current).id === CATEGORIES[4].id && !nav.classList.contains('pressing'), 'letting go opens the group under the lens: ' + current);
      expect(nav.style.getPropertyValue('--y') === items[4].offsetTop + items[4].offsetHeight / 2 + 'px', 'the lens rests under the open group');
      go('general');
    }

    // Shortcut helpers.
    expect(keyName('KeyC') === 'c' && keyName('Digit7') === '7' && keyName('F13') === 'f13' && keyName('ArrowLeft') === 'left', 'keyName basics');
    expect(keyName('Backquote') === null && keyName('F21') === null && keyName('Delete') === null, 'keyName rejects unsupported keys');
    expect(keyName('Backspace') === 'delete' && keysText('opt+shift+return') === '⌥⇧↩', 'Delete and sentence labels');
    expect(shortcutParts('cmd+shift+c').join() === 'shift,cmd,c', 'shortcutParts order: ' + shortcutParts('cmd+shift+c'));
    expect(shortcutParts('control+option+left').join() === 'ctrl,opt,left', 'shortcutParts aliases');
    expect(shortcutParts('').length === 0, 'shortcutParts empty');

    // Every page builds in both languages, with no missing text and no unlabeled buttons.
    for (const code of ['en', 'vi']) {
      setLang(code);
      for (const page of PAGES) {
        go(page.id);
        const root = $('#scroller');
        expect(root.children.length > 0 && root.textContent.trim().length > 20, `${code}/${page.id}: page is empty`);
        for (const b of root.querySelectorAll('button')) {
          expect(b.textContent.trim() || b.getAttribute('aria-label'), `${code}/${page.id}: a button has no label`);
        }
        for (const i of root.querySelectorAll('input')) expect(i.getAttribute('aria-label'), `${code}/${page.id}: an input has no label`);
      }
    }
    expect(missingKeys.size === 0, 'keys without English text: ' + [...missingKeys].join(', '));
    expect(missingSymbols.size === 0, 'icons without a symbol: ' + [...missingSymbols].join(', '));
    setLang('en');

    // Controls send what the app expects.
    go('scroll');
    sent.length = 0;
    const smooth = [...document.querySelectorAll('.switch')][0];
    const was = S.mouse.smooth;
    smooth.click();
    expect(S.mouse.smooth === !was, 'the switch flips the local state');
    expect(sent.some((m) => m.t === 'set' && m.path === 'mouse.smooth' && m.value === !was), 'the switch sends a set message: ' + JSON.stringify(sent));
    smooth.click();
    const range = $('input[type=range]');
    range.value = String(Math.min(+range.max, 60));
    range.dispatchEvent(new Event('input'));
    expect(S.mouse.step_px === +range.value, 'the slider updates the local state');
    expect(range.style.getPropertyValue('--p') !== '', 'the slider paints its fill');

    // Presets set all three values.
    const fluid = [...document.querySelectorAll('.presets .btn')].find((b) => b.textContent === t('preset.fluid'));
    fluid.click();
    expect(S.mouse.step_px === 48 && S.mouse.duration_ms === 560, 'the fluid preset applies');

    // App rules and lists draw from the state.
    S.mouse.apps = [{ id: 'com.example.game', name: 'Game', smooth: false, reverse: null }];
    runSyncs();
    expect(document.querySelectorAll('.app-row').length === 1, 'a rule row is drawn');
    sent.length = 0;
    const remove = $('.app-row .btn.icon');
    remove.click();
    expect(sent.some((m) => m.t === 'app_remove' && m.id === 'com.example.game'), 'removing a rule sends app_remove');

    // Shortcut buttons start recording and ignore bare letters.
    go('clipboard');
    sent.length = 0;
    const hot = $('.hotkey');
    hot.click();
    expect(sent.some((m) => m.t === 'recording' && m.on === true), 'recording starts');
    window.dispatchEvent(Object.assign(new KeyboardEvent('keydown', { code: 'KeyV', bubbles: true, cancelable: true })));
    expect(!sent.some((m) => m.t === 'hotkey'), 'a bare letter is not accepted as a shortcut');
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyV', metaKey: true, altKey: true, bubbles: true, cancelable: true }));
    expect(sent.some((m) => m.t === 'hotkey' && m.target === 'clipboard' && m.value === 'opt+cmd+v'), 'a combination is sent: ' + JSON.stringify(sent));
    expect(sent.some((m) => m.t === 'recording' && m.on === false), 'recording stops');
    go('tiling');
    expect(document.querySelectorAll('.tile').length === 22, 'all 22 layouts are shown');
    expect(S.tiling.layouts[13].key === 'grid_1' && S.tiling.layouts[13].shortcut === 'cmd+1', 'the first grid cell uses Command+1');

    // The keys inside the panels are recorded like any shortcut, Delete included, and the clipboard page
    // lists the ones in use.
    go('keys');
    const panelKey = (action) => [...document.querySelectorAll('.hotkey')].find((b) => b.getAttribute('aria-label').startsWith(t(action === 'pin' ? 'key.pin' : 'keys.delete.title')));
    sent.length = 0;
    panelKey('pin').click();
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyK', altKey: true, bubbles: true, cancelable: true }));
    expect(sent.some((m) => m.t === 'hotkey' && m.target === 'keys:pin' && m.value === 'opt+k'), 'a new pin key is sent: ' + JSON.stringify(sent));
    panelKey('delete').click();
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Backspace', ctrlKey: true, bubbles: true, cancelable: true }));
    expect(sent.some((m) => m.t === 'hotkey' && m.target === 'keys:delete' && m.value === 'ctrl+delete'), 'Delete can be part of a key: ' + JSON.stringify(sent));
    expect(targetName('keys:delete') === t('target.keys:delete'), 'a conflict names the panel key');
    expect(document.querySelectorAll('.hotkey').length >= 6, 'the shortcuts of the whole Mac are listed too');
    const pinKey = S.keys.shortcuts.pin;
    S.keys.shortcuts.pin = 'ctrl+j';
    go('clipboard');
    expect([...document.querySelectorAll('.cheat')].some((c) => c.textContent === t('key.pin') + '⌃J'), 'the clipboard page shows the pin key in use');
    S.keys.shortcuts.pin = pinKey;

    // The agents page: switches write their setting, the button follows what is installed.
    go('agents');
    sent.length = 0;
    const follow = [...document.querySelectorAll('.switch')][0];
    const following = S.agents.enabled;
    follow.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'agents.enabled' && m.value === !following), 'the agents switch sends its setting: ' + JSON.stringify(sent));
    follow.click();
    const lines = [...document.querySelectorAll('.card .row')].filter((r) => /Claude Code|Codex/.test(r.querySelector('.row-title').textContent));
    expect(lines.length >= 2, 'a line per agent');
    sent.length = 0;
    lines[0].querySelector('button').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'agents_remove_claude'), 'a connected agent offers to remove: ' + JSON.stringify(sent));
    sent.length = 0;
    lines[1].querySelector('button').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'agents_install_codex'), 'an agent that is not connected offers to install: ' + JSON.stringify(sent));
    S.agents.codex.found = false; S.agents.codex.installed = 0; runSyncs();
    expect(lines[1].querySelector('button').disabled, 'an agent that is not installed cannot be connected');
    S.agents.codex.found = true; runSyncs();
    const piLine = [...document.querySelectorAll('.card .row')].find((r) => r.querySelector('.row-title').textContent === t('agents.pi.title'));
    sent.length = 0;
    piLine.querySelector('button').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'agents_install_pi'), 'pi offers its extension: ' + JSON.stringify(sent));
    S.agents.pi.installed = 1; runSyncs();
    sent.length = 0;
    piLine.querySelector('button').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'agents_remove_pi'), 'an installed extension offers to remove: ' + JSON.stringify(sent));
    S.agents.pi.installed = 0; runSyncs();
    const agyLine = [...document.querySelectorAll('.card .row')].find((r) => r.querySelector('.row-title').textContent === t('agents.agy.title'));
    sent.length = 0;
    agyLine.querySelector('button').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'agents_install_agy'), 'Antigravity offers its hook: ' + JSON.stringify(sent));
    const wait = $('select.pick');
    wait.value = '60';
    wait.dispatchEvent(new Event('change'));
    expect(S.agents.wait === 60, 'the wait is a number of seconds');
    // The accounts: a switch asks first, then names the account; the one in use offers none; restore and refresh send theirs.
    const accountLine = (email) => [...document.querySelectorAll('.card .row')].find((r) => r.querySelector('.row-title').textContent === email);
    expect(!accountLine('work@example.com').querySelector('button'), 'the account in use offers no switch');
    expect(accountLine('work@example.com').querySelectorAll('.acct-limit').length === 2, 'an account shows both windows');
    sent.length = 0;
    accountLine('home@example.com').querySelector('button').click();
    expect(!sent.length && $('#modal').classList.contains('show'), 'a switch asks first');
    $('#modal .btn.primary').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'accounts_switch:a2'), 'a confirmed switch names the account: ' + JSON.stringify(sent));
    const restoreLine = accountLine(t('accounts.restore.claude'));
    expect(restoreLine && restoreLine.style.display !== 'none', 'restore shows when there is a backup');
    expect(accountLine(t('accounts.restore.codex')).style.display === 'none', 'and not without one');
    sent.length = 0;
    accountLine(t('accounts.refresh.title')).querySelector('button').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'accounts_refresh'), 'refresh asks for the limits');
    S.agents.accounts.busy = true; runSyncs();
    expect(accountLine(t('accounts.refresh.title')).querySelector('button').disabled, 'nothing is asked twice at once');
    S.agents.accounts.busy = false; runSyncs();

    // The notch page: choices send their words, the tab list sends the ids that show, in order, and never none.
    go('notch');
    sent.length = 0;
    // The material and the indicator style are pop-up buttons; the tab list is the segmented one.
    const material = [...document.querySelectorAll('select.pick')].find((s) => s.getAttribute('aria-label') === t('notch.material.title'));
    material.value = 'accent';
    material.dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.t === 'set' && m.path === 'notch.material' && m.value === 'accent'), 'the material is sent: ' + JSON.stringify(sent));
    sent.length = 0;
    document.querySelectorAll('.style-card')[2].click();
    expect(sent.some((m) => m.path === 'notch.width' && m.value === 800) && sent.some((m) => m.path === 'notch.rest_height' && m.value === 32), 'a style card sends its sizes: ' + JSON.stringify(sent));
    expect(document.querySelectorAll('.style-card')[2].getAttribute('aria-pressed') === 'true', 'and shows as chosen');
    expect($('.notch').classList.contains('mat-accent'), 'the preview follows the material');
    sent.length = 0;
    const fullScreen = S.notch.show_in_fullscreen;
    $(`.switch[aria-label="${t('notch.fullscreen.title')}"]`).click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'notch.show_in_fullscreen' && m.value === !fullScreen), 'the full-screen switch sends its setting: ' + JSON.stringify(sent));
    S.notch.volume_hud = true;
    runSyncs();
    sent.length = 0;
    [...document.querySelectorAll('.hud-cards .style-card')][3].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'notch.hud' && m.value === 'bar'), 'the indicator style is sent: ' + JSON.stringify(sent));
    S.notch.pages = [{ id: 'clipboard', title: 'Clipboard', on: true }, { id: 'media', title: 'Media', on: true }, { id: 'sound', title: 'Sound', on: false }];
    runSyncs();
    const tabRows = [...document.querySelectorAll('.card .row')].filter((r) => r.querySelector('.tab-order'));
    expect(tabRows.length === 3, 'a row per tab: ' + tabRows.length);
    sent.length = 0;
    tabRows[1].querySelector('.tab-order .btn').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'notch.pages' && JSON.stringify(m.value) === '["media","clipboard"]'), 'moving a tab sends the new order: ' + JSON.stringify(sent));
    S.notch.pages = [{ id: 'clipboard', title: 'Clipboard', on: true }, { id: 'sound', title: 'Sound', on: false }];
    runSyncs();
    const lone = [...document.querySelectorAll('.card .row')].find((r) => r.querySelector('.tab-order'));
    expect(lone.querySelector('.switch').disabled, 'the last tab that shows cannot be turned off');
    // A tab that waits for its feature does not show, so it does not count as one that does.
    S.notch.pages = [{ id: 'clipboard', title: 'Clipboard', on: true, waiting: false }, { id: 'weather', title: 'Weather', on: true, waiting: true }];
    runSyncs();
    const waits = [...document.querySelectorAll('.card .row')].filter((r) => r.querySelector('.tab-order'));
    expect(waits.length === 2 && waits[0].querySelector('.switch').disabled, 'the only tab that shows cannot go, though another is on and waiting');
    expect(!waits[1].querySelector('.switch').disabled, 'a tab that waits can be turned off');
    expect(waits[1].textContent.includes(t('tab.waiting')), 'a tab that waits says so');
    go('agents');
    expect(document.body.textContent.includes(t('agents.spend.claude')), 'the spend of Claude Code is listed');

    // The update row follows what the app reports: a button to look, then the version it found and what became of it.
    go('about');
    {
      const values = [...document.querySelectorAll('.usage-value')].map((v) => v.textContent);
      expect(values.length === 6 && values.includes('59.0 MB') && values.includes('3.4%'), 'about: what the app uses is shown: ' + values.join(' | '));
      const was = S.app.usage;
      S.app.usage = null; runSyncs();
      expect([...document.querySelectorAll('.usage-value')].every((v) => v.textContent === t('about.usage.wait')), 'about: before a reading it says so');
      S.app.usage = was; runSyncs();
    }
    // The About page says what the app is, where it comes from and under what licence it is used.
    expect(['about.sec.what', 'about.sec.history', 'about.sec.license'].every((key) => $('#scroller').textContent.includes(t(key))), 'About has its sections');
    expect($('#scroller').textContent.includes(t('about.license.title')) && $('#scroller').textContent.includes('Doanh Thai Bui'), 'and names the licence and its holder');
    expect(['0_2', '0_4', '0_5', '0_6', '0_7', '0_8', '0_9', '1_0'].every((v) => $('#scroller').textContent.includes(t('about.v.' + v + '.body'))), 'and lists the versions');
    const licence = $('.license');
    expect(licence && licence.hidden && licence.textContent.includes('Permission is hereby granted'), 'the licence text is there but hidden');
    const showLicence = [...document.querySelectorAll('.btn')].find((b) => b.textContent === t('about.license.show'));
    showLicence.click();
    expect(!licence.hidden && showLicence.textContent === t('about.license.hide') && showLicence.getAttribute('aria-expanded') === 'true', 'the button shows it');
    showLicence.click();
    expect(licence.hidden, 'and hides it again');
    const btn = (label) => [...document.querySelectorAll('.btn')].find((b) => b.textContent === label);
    const visible = (label) => { const b = btn(label); return !!b && b.style.display !== 'none'; };
    const pressed = (name) => sent.some((m) => m.t === 'action' && m.name === name);
    sent.length = 0;
    btn(t('about.update.check')).click();
    expect(pressed('check_updates'), 'the update button asks the app: ' + JSON.stringify(sent));
    expect(!visible(t('about.update.install')) && !visible(t('about.update.restart')) && !visible(t('about.update.open')), 'nothing else to press before an answer');

    window.__fig.recv({ t: 'update', state: 'available', version: '9.9.9' });
    expect($('#scroller').textContent.includes('9.9.9'), 'the row names the version that was found');
    expect(visible(t('about.update.install')), 'a version that was found can be installed');
    sent.length = 0;
    btn(t('about.update.install')).click();
    expect(pressed('install_update'), 'the update button installs: ' + JSON.stringify(sent));

    window.__fig.recv({ t: 'update', state: 'installing', version: '9.9.9', stage: 'downloading', percent: 42 });
    expect($('#scroller').textContent.includes(t('about.update.downloading', { v: '9.9.9', p: 42 })), 'the download is shown with how far it is');
    expect(btn(t('about.update.check')).disabled, 'no second check while one installs');
    window.__fig.recv({ t: 'update', state: 'installing', version: '9.9.9', stage: 'checking' });
    expect($('#scroller').textContent.includes(t('about.update.installing', { v: '9.9.9' })), 'and then the install');

    window.__fig.recv({ t: 'update', state: 'ready', version: '9.9.9' });
    expect(visible(t('about.update.restart')), 'an installed version is a restart away');
    sent.length = 0;
    btn(t('about.update.restart')).click();
    expect(pressed('restart'), 'the restart button restarts: ' + JSON.stringify(sent));

    window.__fig.recv({ t: 'update', state: 'blocked', version: '9.9.9', detail: 'no permission' });
    expect($('#scroller').textContent.includes('no permission'), 'a version that cannot be installed says why');
    sent.length = 0;
    btn(t('about.update.open')).click();
    expect(pressed('open_release'), 'and can be downloaded by hand: ' + JSON.stringify(sent));

    window.__fig.recv({ t: 'update', state: 'failed', detail: 'offline' });
    expect($('#scroller').textContent.includes('offline'), 'a failed check says why');
    window.__fig.recv({ t: 'update', state: 'idle' });

    const auto = [...document.querySelectorAll('.switch')].find((s) => s.getAttribute('aria-label') === t('update.auto.title'));
    expect(auto, 'the About page has the switch for updating by itself');
    sent.length = 0;
    auto.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'update.auto' && m.value === true), 'the switch is sent: ' + JSON.stringify(sent));

    // Each feature's own checks (web/features/*.js).
    for (const p of PLUGINS) if (p.selftest) p.selftest({ expect, go, sent, S, h, $ });
  } catch (error) {
    failures.push('exception: ' + (error && error.stack ? error.stack : error));
  } finally {
    window.fig.send = realSend;
  }
  return failures;
};

function setLang(code) {
  S.general.language = code;
  lang = code;
  rebuild();
}

send({ t: 'ready' });
