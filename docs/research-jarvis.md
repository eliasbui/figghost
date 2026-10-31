# Jarvis: an AI assistant in a Liquid Glass chat popup

Notes made before building. Request: an assistant called Jarvis that answers questions the way the AI tutor of app-card-english does, with an icon to call it, a Liquid Glass chat popup, an animated character at the top left of the popup (like the speaker in a dialogue) that moves while Jarvis listens and speaks, sound that goes with the change of state, marked beta and configurable in Settings.

Only the two repositories on this machine were read: `app-card-english` (the tutor) and `figghost` (where the panels live). Nothing was run.

## What app-card-english already solved

- Sources of AI (`src/ai.rs`, `PROVIDERS`): a small model on the machine (Qwen2.5 0.5B and 1.5B through candle), the user's ChatGPT plan, ChatGPT through the Codex client, Gemini with a free API key, DeepSeek with a key. The chosen source is asked first; when it fails the others that are ready take over and the answer says who gave it.
- Streaming: `chatgpt::chat` and `codex::chat` take an `on_text` callback, so the first words show before the answer is finished. The voice tutor reads each sentence as soon as it is complete (`src/talk.rs`).
- Voice in: Whisper small int8 through sherpa-onnx, 375 MB, understands Vietnamese, English and sentences mixing both. Voice out: Kokoro, Supertonic, Piper, or the system voice.
- A chat that answers only its subject and refuses the rest politely (`assistant.rs` splits what the app does by itself from what the model says; a small model picks the wrong tool too often, so requests are recognised by wording).
- UI (`ui/parts/chat-tutor.js`, `voice-tutor.js`, `voice-tutor.css`): bubbles, the tutor's face beside its message, a "beta" pill, notes that say what is missing with the button that fixes it.

## What figghost already has that Jarvis can stand on

- A feature of the same shape was shipped on 2026-10-29: Dictation (`features/dictation/`, `macos-sys/src/dictation_ui.rs`, `speech.rs`). It has a global shortcut, a Liquid Glass panel in the middle of the screen, a live voice meter, on-device Speech recognition in English and Vietnamese, Escape and Return handling, a page in Settings, a switch that is off by default, and a render example plus a check script. Jarvis is this feature with a conversation after the listening. Its panel code (`panel.rs`: the glass with its solid and pre-macOS 26 stand-ins) was moved out so other panels can use it.
- Voice in is therefore already there and costs no download: `speech::Listening` for Vietnamese and English, on the Mac unless the person allows Apple's servers.
- Other panels of Liquid Glass to copy: Translate (a window by the pointer that shimmers while it works, then shows the result), the launcher, the notch.
- Drawn animation: the mascot stage (`macos-sys/src/mascot.rs`, `features/mascot`) already draws a small character with a `CADisplayLink`, and `docs/mascots` holds the SVG sets. A character in the corner of the popup is the same technique at a smaller size.
- Sound: `NSSound` is used for the weather sounds (`weather_sound.rs`); agents have a `sound` setting. A short sound on start, on answer and on end needs no new dependency.
- Network: `core_smart::net::get` and `get_with_headers` exist, but only GET. A chat needs POST with streaming; that part is new. `core-agents` already reads the user's Claude Code and Codex sign-ins and asks Anthropic for the limits, so the sign-in of a plan the person already pays for is reachable.
- Settings: a page is a row of helpers in `app.js` plus `nav.<id>`, `page.<id>`, `page.<id>.sub` in both languages, a glyph, and a `config.rs` section with defaults and clamping. Features are off by default and must switch off cleanly (the user's standing rule).

## What is new

1. The AI client: POST, streaming, cancel, a timeout, and the choice of source.
2. The conversation: a thread kept in memory while the popup is open, and what is kept after it closes.
3. The popup: a Liquid Glass panel taller than Dictation's, with a log of bubbles and a line to type in.
4. The animated character and its states.
5. Speaking the answer aloud.

## Design

### Calling it

- A global shortcut (default unset or `opt+space`; a conflict check is already part of the hotkey code) and an icon. The icon sits in the menu bar menu and, optionally, in the notch. Both open the same popup; the shortcut again or Escape closes it.
- Jarvis asks "What can I do for you?" the moment it opens, spoken and written, with the character in its greeting animation. The person answers by voice (hold or toggle, as Dictation) or by typing. Voice is the default when the Speech access is granted; without it the popup says so once and falls back to the line to type in.

### The popup

- Liquid Glass on macOS 26 through the shared panel code, with the solid stand-in under Reduce transparency and on older systems. Like Dictation it takes the keyboard only while open and gives it back to the app being worked in when it closes.
- Top left: the character, a disc of glass with the animated face inside, as the speaker is drawn in a dialogue box. Beside it, the name and the state in words ("Listening", "Thinking", "Speaking") and a "Beta" pill.
- Under that: the log (the person's words on the right, Jarvis's on the left, the answer growing as it streams), then one line to type in, a microphone button and a stop button that cuts off the answer and the voice at once.
- Text is at least 4.5:1 on the glass in both themes; the glass sits behind controls and the chrome, and long answers sit on the veil, as the design rules say (`docs/design.md`).

### The character and its states

| State | Motion | Sound |
|---|---|---|
| Waking | rises and opens | soft rising tone |
| Listening | rings follow the voice level (the same meter data Dictation has) | none |
| Thinking | slow orbit | none, or a very quiet tick if the person asks |
| Speaking | mouth or waveform follows the speech | the voice itself |
| Done | settles | short falling tone |
| Error | one shake and a dimmed face | low tone |

- Drawn with Core Animation / `CADisplayLink` in `macos-sys`, as the mascot is, so it costs no web view and stops drawing when the popup is hidden. Reduce motion replaces the movement with a change of colour and a still pose (rule 7 of the design document).
- The tones are generated or are three tiny bundled files played by `NSSound`; the volume follows a setting and the system's, and there is a switch for them (`sound`, like agents).

### Answering

- One system prompt keeps Jarvis to being a helper that answers questions, briefly, in the language the person used. Whether it should refuse topics (the tutor does) is a decision, below.
- The answer streams into the log; the voice reads it sentence by sentence as app-card-english does, so speech starts before the answer ends. The system voice (`AVSpeechSynthesizer`) is the default and costs no download; a better voice pack is a later option.

### Which AI

In order of cost to the person:

1. The Claude Code or Codex sign-in already on the Mac (`core-agents` reads them): no new key, uses a plan already paid for. Whether using these sign-ins from another app is allowed is a policy question for each provider and has to be checked before it ships; app-card-english marks its Codex route "unofficial, may stop at any time".
2. An API key the person pastes (Claude, Gemini, OpenAI, DeepSeek), kept in the Keychain (`macos-sys/src/keychain.rs`).
3. A model on the machine (Ollama or a bundled Qwen). The only route where no word leaves the Mac, at the price of a download and slower, weaker answers.

When the chosen source fails, the others that are ready take over and the answer says who gave it, as in the tutor.

### Settings (a page of its own, "Jarvis", with a Beta pill)

- Switch for the feature (off by default; off removes the icon, the shortcut, the panel and every sound, and keeps nothing running).
- Shortcut; whether to show the icon in the menu bar and in the notch.
- Source of the AI, the model, the key; "Test" button.
- How to talk: voice or typing first, language (auto, Vietnamese, English), keep listening after an answer or not.
- Voice of the answer: off, system voice (choose which), speed.
- Sounds: on or off, volume.
- The character: on or off, and Reduce motion respected.
- What it may see: nothing, or the text selected in the front app (reuses `smart::read_selection` from Translate). Off by default.
- History: kept for this session only (default), or kept on disk, with "Clear".
- Privacy line in plain words: what is sent, to whom, and that turning the feature on is the consent, as in Translate.

Keys go in `config.rs` under `"jarvis": { ... }` with defaults and clamping, documented in `docs/configuration.md`.

## Risks and open points

- Privacy: the questions go to a service unless the on-device route is chosen. The settings page and the first run must say so before anything is sent.
- Sign-in reuse: see above; do not build on it before it is confirmed to be allowed.
- Microphone and Speech access: two permissions, requested when the feature is turned on, with the same flow Dictation has.
- Echo: the microphone must not listen while Jarvis speaks, or it will answer itself. The tutor ends listening during its reply and resumes after; the same here, with "interrupt" as the one exception.
- Name: "Jarvis" is the name of a character in a film and comic franchise. Fine as a working name; check before it is a product name and keep the id `assistant` in code and config so a rename is only text.
- Size: Dictation was about 2,000 lines across 25 files; Jarvis is larger because of the AI client and the character, but nothing in it needs a download.

## Status

Steps 1 to 5 below were built on 31 October 2026, off by default and marked beta: the AI client (`crates/core-jarvis`), the popup and its settings page, voice in (the Speech framework, as Dictation), the animated character and its sounds, and the voice that reads the answer. What was seen and what was not is in `docs/verification.md`. The source of AI is a key pasted into the settings page and kept in the Keychain; reusing the sign-in of Claude Code or Codex was left out until it is confirmed to be allowed.

## Proposed order of work

1. The AI client with streaming, cancel and source choice, with tests against a fake server. No UI.
2. The popup with typing only, the log, and the Settings page with the switch, off by default.
3. Voice in, reusing Dictation's listening.
4. The character and its states, with a render example like `render_dictation.rs` to look at each state.
5. Speaking the answer, then the sounds.
6. Beta pill, docs (`configuration.md`, `verification.md`, a check script), release note.

## Decisions needed

1. Which source of AI first: the sign-in already on the Mac, a pasted key, or a model on the machine.
2. Whether Jarvis answers anything or only some subjects, as the tutor does.
3. Whether history is kept on disk or only for the session.
4. Whether it may read the selected text of the front app.
