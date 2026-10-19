# Configuration


`~/Library/Application Support/Figghost/config.json`, written by the settings window. Missing values use defaults, out-of-range values are clamped, and an unreadable file is moved to `config.json.bad` instead of being overwritten. Files written by older versions load unchanged.

    {
      "language": "auto",
      "show_settings_on_launch": true,
      "launch_effect": true,
      "hotkey": "cmd+shift+c",
      "auto_paste": true,
      "paste_plain": false,
      "volume_hud": false,
      "history_limit": 500,
      "ignored_apps": ["com.example.vault"],
      "mcp": { "enabled": false },
      "update": { "auto": false },
      "agents": { "enabled": false, "approval_wait_seconds": 30, "sound": true, "peek_when_done": true },
      "capture": {
        "enabled": false,
        "folder": "",
        "save": true,
        "copy": true,
        "quick_panel": true,
        "quick_seconds": 8,
        "timer_seconds": 5,
        "shortcuts": { "area": "cmd+shift+2", "text": "" },
        "record": { "system_audio": false, "microphone": false, "camera": false, "clicks": true, "keys": false }
      },
      "audio": {
        "enabled": true,
        "apps": { "com.spotify.client": { "volume": 1.5, "muted": false, "output": "", "eq": "bass_boost" } },
        "devices": { "<device UID>": { "volume": 0.5, "muted": false } },
        "effects": { "eq": "", "custom_eq": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], "boost": 0.0, "night": false, "width": 1.0 }
      },
      "stack": { "enabled": false, "modifier": "option", "sensitive": "off", "expire_minutes": 10 },
      "snap": {
        "enabled": true,
        "templates": ["full", "halves", "thirds", "quarters"],
        "screens": { "DELL U2720Q": ["halves", "c1"] },
        "custom": [{ "id": "c1", "name": "Coding", "cols": 3, "rows": 2, "cells": [[0, 0, 2, 1], [2, 0, 3, 1], [0, 1, 1, 2], [1, 1, 2, 2], [2, 1, 3, 2]] }]
      },
      "menubar": { "enabled": false, "auto_seconds": 10, "click_only": true },
      "vision": { "enabled": false, "shortcut": "ctrl+opt+cmd+8", "format": "hex" },
      "lyrics": { "enabled": false, "fetch": false },
      "calendar": { "enabled": false, "lead_minutes": 5, "sound": true, "reminders": true, "all_day": true, "excluded": [] },
      "smart": {
        "enabled": false,
        "shortcut": "ctrl+opt+s",
        "internet": false,
        "translate_to": "auto",
        "currencies": ["USD", "EUR", "VND"],
        "zones": ["UTC", "America/New_York", "Europe/London", "Asia/Tokyo"]
      },
      "notch": { "enabled": true },
      "mouse": {
        "smooth": true,
        "reverse": false,
        "step_px": 36.0,
        "speed": 1.0,
        "duration_ms": 360.0,
        "shift_horizontal": true,
        "option_boost": true,
        "boost": 5.0,
        "command_zoom": false,
        "apps": { "com.example.game": { "smooth": false } }
      },
      "tiling": {
        "enabled": true,
        "shortcuts": { "left": "ctrl+opt+left", "center": "" }
      }
    }

`capture.folder` empty means the folder of macOS's own screenshots, else the Desktop; it may start with `~/`. `capture.shortcuts` holds only the changed ones (`area window screen timer text scroll record gif`; an empty string removes one, a built-in one too); `capture.quick_seconds` 0 keeps the quick panel until it is closed. In `audio`, a volume is 0 to 4 (1 is unchanged), `output` is a device UID (empty follows the system's output), `eq` is a preset (`flat bass_boost bass_reducer treble_boost treble_reducer vocal loudness rock pop electronic hip_hop jazz classical acoustic spoken movie gaming`, or `custom` for `custom_eq`, 10 bands in dB within 12 either way), `boost` is 0 to 1 and `width` 0 (mono) to 2. `stack.modifier` is `option`, `command`, `control` or `shift`; `stack.sensitive` is `off`, `mask` or `never`; `stack.expire_minutes` is 1 to 1440. `snap.templates` lists by id the layouts every screen offers when a window is dragged to the notch (`full halves rows thirds quarters main_left main_right left_stack right_stack grid_six`, or the id of one in `snap.custom`), and `snap.screens` gives a screen a list of its own by the name of the screen; unknown and repeated ids are ignored and a list with none left is the default one. `snap.custom` holds the layouts made from a grid: `id` is a `c` and up to four digits, `name` is up to 24 characters, `cols` and `rows` are 1 to 6, and `cells` are `[column, row, column, row]` (the first part and the one after the last) that together cover every part of the grid once; one that does not is ignored; `menubar.auto_seconds` is 0 (never) to 3600. In `notch`, `material` is `glass`, `clear` or `black`; `motion` is `spring`, `smooth` or `none`; `hover_delay_ms` is 0 to 1000; `open_page` is `activity`, `last` or a tab id; `display` is `auto` (the built-in screen with a notch, else the main one), `main` or `pointer` (the screen whose top edge the pointer reaches); `rest` is what a screen without a notch shows, `notch` or `hidden`; `pages` lists the tabs to show in order (empty shows all; ids `clipboard shelf media focus agents calendar sound`); `waveform` is `live` or `animated`; `shortcut` opens and closes the notch (empty for none). `theme` is `auto`, `light` or `dark`. `launch_effect` lets the ghost come out of the notch and blink each time the app is opened by hand (never at login, never with `--background`, and only while `notch.enabled` is on). A value of the wrong type or an unknown word in any of these falls back to the default instead of making the file unreadable.

`update.auto` installs a new version as soon as the daily check finds it, instead of asking first; either way it is used from the next launch. The check itself cannot be turned off. When it last got an answer is kept in the file `update-check` of the data folder (seconds since 1970); delete the file to have the next check come at once. A program that does not run from an app (a build folder) never checks by itself.

`version` records the layout of the file. A file without it was written before window snapping by dragging was on by default, and every such file had `"snap": {"enabled": false}` written into it whether or not the user ever chose that, so it is turned on once when such a file loads; a choice made after that stands.

A shortcut needs at least one modifier, except function keys (f1 to f20). An empty shortcut turns that layout off. Layout names: `left right top bottom top_left top_right bottom_left bottom_right maximize center first_third center_third last_third`.

Environment variables: `FIGGHOST_HOME` changes the data directory, `FIGGHOST_PASTEBOARD` uses a named pasteboard instead of the real clipboard, `FIGGHOST_AGENTS_HOME` changes where `.claude` and `.codex` are looked for (with `FIGGHOST_HOME` set and no `FIGGHOST_AGENTS_HOME` they are looked for in an empty folder inside `FIGGHOST_HOME`, because the agents feature rewrites the paths of the hooks it installed each time it starts and a private instance must never reach the real files), and `FIGGHOST_NOTCH_OFFSCREEN` places the notch and the Smart Select window outside every screen, and `FIGGHOST_SMART_TEXT` with `FIGGHOST_SMART_OPEN` makes Smart Select answer for a given text as soon as the app is up (all of these are for tests), and `FIGGHOST_TRACE=1` prints a timeline to stderr. Arguments: `--show-settings` always opens the window, `--background` never does, `--mcp` runs the assistant server, `--figghost-hook <claude|codex>` and `--figghost-statusline` are what the agents run (they never start the interface).
