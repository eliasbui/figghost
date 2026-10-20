# Configuration


`~/Library/Application Support/Figghost/config.json`, written by the settings window. Missing values use defaults, out-of-range values are clamped, and an unreadable file is moved to `config.json.bad` instead of being overwritten. Files written by older versions load unchanged.

    {
      "language": "auto",
      "show_settings_on_launch": true,
      "launch_effect": true,
      "panel_keys": { "pin": "opt+p", "board": "opt+b", "paste_other": "opt+shift+return", "delete": "opt+delete" },
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
      "pin": { "enabled": false, "shortcut": "ctrl+opt+cmd+p" },
      "cursor": { "enabled": false, "theme": "neon", "size": 32, "speed": 1.0, "custom_path": "", "tip": "top" },
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
      "islands": { "enabled": false, "caps_lock": true, "power": true, "drives": true, "downloads": true, "privacy": true, "full_at": 100, "low_at": 20, "seconds": 3.6 },
      "utilities": {
        "enabled": false,
        "awake_display": true,
        "awake_minutes": 60,
        "lock_seconds": 60,
        "snooze_minutes": 30,
        "awake_shortcut": "",
        "mic_shortcut": "ctrl+opt+cmd+m",
        "lock_shortcut": "ctrl+opt+cmd+k",
        "snooze_shortcut": ""
      },
      "notes": { "enabled": false, "text_size": 14, "new_on_open": false },
      "launcher": { "enabled": false, "items": [ { "kind": "app", "target": "com.apple.Notes", "name": "Notes" }, { "kind": "folder", "target": "/Users/me/Documents", "name": "Documents" }, { "kind": "shortcut", "target": "Morning routine", "name": "Morning routine" } ] },
      "prs": { "enabled": false, "github": true, "gitlab": true, "drafts": false, "minutes": 10 },
      "pomodoro": { "enabled": false, "work": 25, "short_break": 5, "long_break": 15, "rounds": 4, "auto_start": false, "sound": true, "open_on_end": true, "closed_notch": true, "shortcut": "" },
      "screentime": { "enabled": false, "idle_minutes": 2 },
      "dayprogress": { "enabled": false, "wake": 7, "bed": 23, "reminders": true },
      "weather": { "enabled": false, "units": "metric", "place": { "name": "Hanoi", "region": "", "country": "Vietnam", "latitude": 21.0245, "longitude": 105.84117 } },
      "system": { "enabled": false, "seconds": 1, "processor": true, "memory": true, "network": true, "details": true },
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

`capture.folder` empty means the folder of macOS's own screenshots, else the Desktop; it may start with `~/`. `capture.shortcuts` holds only the changed ones (`area window screen timer text scroll record gif`; an empty string removes one, a built-in one too); `capture.quick_seconds` 0 keeps the quick panel until it is closed. In `audio`, a volume is 0 to 4 (1 is unchanged), `output` is a device UID (empty follows the system's output), `eq` is a preset (`flat bass_boost bass_reducer treble_boost treble_reducer vocal loudness rock pop electronic hip_hop jazz classical acoustic spoken movie gaming`, or `custom` for `custom_eq`, 10 bands in dB within 12 either way), `boost` is 0 to 1 and `width` 0 (mono) to 2. `stack.modifier` is `option`, `command`, `control` or `shift`; `stack.sensitive` is `off`, `mask` or `never`; `stack.expire_minutes` is 1 to 1440. `snap.templates` lists by id the layouts every screen offers when a window is dragged to the notch (`full halves rows thirds quarters main_left main_right left_stack right_stack grid_six`, or the id of one in `snap.custom`), and `snap.screens` gives a screen a list of its own by the name of the screen; unknown and repeated ids are ignored and a list with none left is the default one. `snap.custom` holds the layouts made from a grid: `id` is a `c` and up to four digits, `name` is up to 24 characters, `cols` and `rows` are 1 to 6, and `cells` are `[column, row, column, row]` (the first part and the one after the last) that together cover every part of the grid once; one that does not is ignored; `pin.shortcut` pins the window in front, or lets go of the pinned one (empty for none, which leaves the switch in the menu bar panel); which window is pinned is not saved; `cursor.theme` is `neon`, `custom` or a character of the mascot (`spider frog ufo ghost wizard mario`), `cursor.size` is 16 to 96 points, `cursor.speed` is 0.25 to 3, `cursor.tip` is `top` or `centre` (where a picture of your own clicks) and `cursor.custom_path` is the file of the `custom` theme, which only the page's button writes; `menubar.auto_seconds` is 0 (never) to 3600. In `notch`, `material` is `glass`, `clear` or `black`; `motion` is `spring`, `smooth` or `none`; `hover_delay_ms` is 0 to 1000; `open_page` is `activity`, `last` or a tab id; `display` is `auto` (the built-in screen with a notch, else the main one), `main` or `pointer` (the screen whose top edge the pointer reaches); `rest` is what a screen without a notch shows, `notch` or `hidden`; `pages` lists the tabs to show in order (empty shows all; ids `clipboard shelf media focus agents calendar sound`, and `notes launcher weather prs pomodoro screentime dayprogress system`, which are in the strip only while the `enabled` of the feature of that name is on, whatever `pages` says, and come back to the place `pages` gives them when it is turned on again); `waveform` is `live` or `animated`; `shortcut` opens and closes the notch (empty for none). `theme` is `auto`, `light` or `dark`. `launch_effect` lets the ghost come out of the notch and blink each time the app is opened by hand (never at login, never with `--background`, and only while `notch.enabled` is on). `panel_keys` are the keys of the actions inside the notch and the clipboard popup, written like the other shortcuts (`delete` is the Delete key): `pin` pins or unpins, `board` files the selected item in the next board, `paste_other` pastes the other way and `delete` deletes, denies an agent or dismisses an alert. Each needs a modifier, Command alone is refused (it already means a tab, a pinned item or Delete there), and arrows, Return, Escape, Page Up and Down keep their meaning. A value of the wrong type or an unknown word in any of these falls back to the default instead of making the file unreadable.

`islands.full_at` is the charge from 80 to 100 that is announced as full while plugged in, `islands.low_at` the charge from 5 to 30 at and under which a Mac on battery is announced as low, and `islands.seconds` how long a notice stays (2 to 10; the camera and microphone notice stays as long as either is in use). Each notice has its own switch and does nothing while `islands.enabled` is off. A Mac without a battery never shows the battery notices. `utilities.awake_minutes` is one of 15, 30, 60, 120, 240, or 0 for until it is stopped (any other number is read as 0), `utilities.awake_display` keeps the screen lit as well as the Mac awake, and `utilities.lock_seconds` (15 to 300) is the longest the keyboard stays locked. `utilities.snooze_minutes` is how long a snooze of the notch lasts, one of 5, 15, 30, 60, 120, or 0 for until it is woken (any other number is read as 30); while snoozed the pointer reaching the top of the screen does not open the notch, and a click on it and anything that opens it by itself still do. The four shortcuts follow the rule below; an empty one has no shortcut, and the keep-awake and snooze ones are empty by default. `system.seconds` is how often the System tab of the notch reads the Mac while it is open, one of 1, 2 or 5 (any other number is read as 1); `system.processor`, `system.memory`, `system.network` and `system.details` (graphics, startup disk, heat and battery) choose its cards, and with all four off the tab says so. The tab does nothing while `system.enabled` is off, and nothing while it is closed. `notes.text_size` is the size of the text in the Notes tab, 12, 14 or 16 points (any other number is read as 14), and `notes.new_on_open` starts a blank note each time the tab opens. The notes themselves are the files `note-<number>.md` in the folder `notes` of the data directory, plain text, one for each note; the file's modification time is when it was last written, a file with any other name is left alone, a file over 200 kilobytes is not read, and at most 200 notes are listed (the newest). Delete or edit them from any app; the tab reads the folder again each time it opens. `weather.units` is `metric` (degrees Celsius, kilometres an hour) or `imperial` (degrees Fahrenheit, miles an hour); anything else is read as `metric`. `weather.place` is the city, chosen from a search on the settings page and not meant to be typed by hand: a name, an optional region and country, and a latitude from -90 to 90 and a longitude from -180 to 180. A place that is not of that shape is read as no place, and the file stays readable. `launcher.items` is what the Launcher tab pins, in the order of its tiles, at most twelve: `kind` is `app` (`target` is the bundle identifier, such as `com.apple.Notes`), `folder` (`target` is a full path) or `shortcut` (`target` is the name of a Shortcut, which may not begin with a hyphen); `name` is what the tile says and may be left out. A pin of any other shape, a repeated one and any beyond twelve are dropped when the file is read and the file stays readable. Nothing is launched until a tile is pressed. `screentime.idle_minutes` is how long the keyboard and pointer may be still before time stops counting, one of 1, 2 or 5 (any other number is read as 2). What is counted is in the file `screentime.json` of the data directory: for each of the last thirty days, the seconds of each app by bundle identifier, and the names the apps had; it is written about once a minute and when the tab closes, whole or not at all, and a file that is wrong in places keeps what is right. Deleting the file, or Clear it all on the settings page, forgets everything. `pomodoro.work` is one of 15, 20, 25, 30, 45, 50 or 60 minutes, `pomodoro.short_break` one of 3, 5, 10 or 15, `pomodoro.long_break` one of 10, 15, 20 or 30, and `pomodoro.rounds` (the work sessions before a long break) one of 2 to 6; a number that is not one of these is read as the usual one (25, 5, 15 and 4). `pomodoro.auto_start` starts the next stage by itself, `pomodoro.sound` plays a sound when a stage ends, `pomodoro.open_on_end` opens the notch on the tab for a few seconds, `pomodoro.closed_notch` shows the countdown in the closed notch, and `pomodoro.shortcut` (none by default, otherwise as the shortcuts below) starts, pauses and resumes. The sessions and minutes of each of the last thirty days are in the file `pomodoro.json` of the data directory, written whole when a work session ends; the Focus tab keeps no settings and no history. `prs.github` and `prs.gitlab` choose where the Pull requests tab looks, through the `gh` and `glab` tools, which are looked for in `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin` and `~/.local/bin` and run with fixed arguments and no shell: `gh search prs --review-requested=@me --state=open` and, for GitLab, the user and the open merge requests that ask for that user's review through `glab api`. `prs.drafts` lists drafts too, and `prs.minutes` (one of 5, 10, 30 or 60; any other number is read as 10) is how often the tools are asked while `prs.enabled` is on. Nothing the tools print is stored; a tool that is not installed or not signed in is simply left alone and the tab says so. The Weather tab goes online only when `weather.enabled` is on and a place is set, only while the tab is open, and asks again no sooner than ten minutes after the last answer. `dayprogress.wake` is the hour the line of the day begins at, one of 5 to 10, and `dayprogress.bed` the hour it ends at, one of 21 to 24 (24 is midnight); a pair that is not offered is read as 7 and 23. `dayprogress.reminders` marks on the line the reminders that are due today at a time, and counts every reminder that is not done and due before tomorrow, overdue ones included. The Day progress tab reads the calendars of the Calendar tab, leaving out those `calendar.excluded` names, only while it shows (when it opens and then every half minute), needs the access that the Calendar page asks for, and keeps nothing on disk.

`update.auto` installs a new version as soon as the daily check finds it, instead of asking first; either way it is used from the next launch. The check itself cannot be turned off. When it last got an answer is kept in the file `update-check` of the data folder (seconds since 1970); delete the file to have the next check come at once. A program that does not run from an app (a build folder) never checks by itself.

`version` records the layout of the file. A file without it was written before window snapping by dragging was on by default, and every such file had `"snap": {"enabled": false}` written into it whether or not the user ever chose that, so it is turned on once when such a file loads; a choice made after that stands.

A shortcut needs at least one modifier, except function keys (f1 to f20). An empty shortcut turns that layout off. Layout names: `left right top bottom top_left top_right bottom_left bottom_right maximize center first_third center_third last_third grid_1` through `grid_9`; the numbered grid cells map row-wise to a 3×3 screen and use Command+1 through Command+9 by default.

Environment variables: `FIGGHOST_HOME` changes the data directory, `FIGGHOST_DOWNLOADS` names the folder the live islands watch for finished downloads instead of `~/Downloads`, `FIGGHOST_PASTEBOARD` uses a named pasteboard instead of the real clipboard, `FIGGHOST_AGENTS_HOME` changes where `.claude` and `.codex` are looked for (with `FIGGHOST_HOME` set and no `FIGGHOST_AGENTS_HOME` they are looked for in an empty folder inside `FIGGHOST_HOME`, because the agents feature rewrites the paths of the hooks it installed each time it starts and a private instance must never reach the real files), and `FIGGHOST_NOTCH_OFFSCREEN` places the notch and the Smart Select window outside every screen, and `FIGGHOST_SMART_TEXT` with `FIGGHOST_SMART_OPEN` makes Smart Select answer for a given text as soon as the app is up (all of these are for tests), and `FIGGHOST_NOTCH_OPEN=<tab id>` opens the notch on that tab a second after the app is up and closes it six seconds later (for tests, with `FIGGHOST_TRACE=1`), and `FIGGHOST_TRACE=1` prints a timeline to stderr. Arguments: `--show-settings` always opens the window, `--background` never does, `--mcp` runs the assistant server, `--figghost-hook <claude|codex>` and `--figghost-statusline` are what the agents run (they never start the interface).
