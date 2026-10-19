//! Draws every page of the open notch into PNGs, with sample content, without showing anything on screen.
//! `cargo run -p macos-sys --example render_notch -- /tmp/notch [--hardware]`
//!
//! `--hardware` draws them as on a screen with a camera cutout (185 by 32, as `render_closed` uses), where the
//! header sits beside the cutout when it fits there.

use macos_sys::{
    MainThreadMarker, NSSize,
    agents_ui::{
        AgentsCallbacks, AgentsDisplay, AgentsView, ApprovalCardData, SessionData, TokensData,
        UsageData,
    },
    app,
    focus_ui::{FocusCallbacks, FocusDisplay, FocusView},
    media_ui::{MediaCallbacks, MediaView},
    notch::{Notch, NotchGeometry},
    now_playing::{Sample, Track},
    shelf_ui::{ShelfCallbacks, ShelfView},
    style::Tone,
    ui::{AppCallbacks, Row},
};
use std::{env, fs, path::PathBuf, process, rc::Rc, time::Instant};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn sample_rows() -> Vec<Row> {
    let row = |id, title: &str, subtitle: &str, pinned, mono, thumb| Row {
        id,
        title: title.into(),
        subtitle: subtitle.into(),
        pinned,
        pin_key: pinned.then_some('b'),
        mono,
        thumb,
    };
    vec![
        row(
            1,
            "Meeting notes: ship the notch shelf on Friday",
            "Notes, just now",
            true,
            false,
            None,
        ),
        row(
            2,
            "https://example.com/docs/getting-started",
            "Safari, 2 minutes ago",
            false,
            false,
            None,
        ),
        row(
            3,
            "fn main() { let total = items.iter().sum::<i32>(); }",
            "Zed, 5 minutes ago",
            false,
            true,
            None,
        ),
        row(
            4,
            "Image 512 x 512",
            "Screenshot, 1 hour ago",
            false,
            false,
            Some(PathBuf::from(format!(
                "{ROOT}/../app-ui/assets/Figghost.icns"
            ))),
        ),
        row(
            5,
            "report-final.pdf and 2 other files",
            "Finder, yesterday",
            false,
            false,
            None,
        ),
        row(
            6,
            "Nguyễn Văn Đức - 0901 234 567",
            "Contacts, 2 days ago",
            false,
            false,
            None,
        ),
    ]
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/notch".into()));
    let hardware = env::args().any(|a| a == "--hardware");
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let mut geometry = NotchGeometry::detect(mtm).expect("a screen");
        if hardware {
            geometry.notch = NSSize::new(185.0, 32.0);
            geometry.hardware = true;
        }
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let rows = sample_rows();
        let notch = Rc::new(Notch::new(mtm, geometry, noop, move |view| {
            view.set_rows(rows.clone(), "Nothing copied yet");
        }));

        let shelf = ShelfView::new(
            mtm,
            notch.page_area(660.0, 150.0),
            ShelfCallbacks {
                on_drop: Box::new(|_| {}),
                on_remove: Box::new(|_| {}),
                on_open: Box::new(|_| {}),
                actions_for: Box::new(|_| Vec::new()),
                on_action: Box::new(|_, _| {}),
            },
        );
        shelf.set_paths(
            [
                "README.md",
                "Cargo.toml",
                "../app-ui/assets/Figghost.icns",
                "src",
                "examples",
            ]
            .iter()
            .map(|p| format!("{ROOT}/{p}"))
            .collect(),
        );
        notch.add_page(shelf.page(|| {}));

        let media = MediaView::new(
            mtm,
            notch.page_frame(),
            MediaCallbacks {
                on_previous: Box::new(|| {}),
                on_play_pause: Box::new(|| {}),
                on_next: Box::new(|| {}),
                on_seek: Box::new(|_| {}),
            },
        );
        media.show(Some(&Sample {
            track: Track {
                app: "com.apple.Music".into(),
                app_name: "Music".into(),
                title: "Blue in Green".into(),
                artist: "Miles Davis".into(),
                album: "Kind of Blue".into(),
                playing: true,
                rate: 1.0,
                position_ms: 0,
                duration_ms: 0,
                artwork: None,
            },
            at: Instant::now(),
        }));
        notch.add_page(media.page(|| {}, || {}));

        let focus = FocusView::new(
            mtm,
            notch.page_frame(),
            &[15, 25, 45, 60],
            FocusCallbacks {
                on_toggle: Box::new(|| {}),
                on_reset: Box::new(|| {}),
                on_preset: Box::new(|_| {}),
                on_adjust: Box::new(|_| {}),
            },
        );
        focus.show(&FocusDisplay {
            clock: "17:42",
            progress: 0.29,
            running: true,
            paused: false,
            finished: false,
            minutes: 25,
        });
        notch.add_page(focus.page(|| {}, || {}));

        let agents = AgentsView::new(
            mtm,
            notch.page_frame(),
            AgentsCallbacks {
                on_decide: Box::new(|_, _| {}),
                on_refresh: Box::new(|| {}),
            },
        );
        notch.add_page(agents.page(|| {}, || {}));

        let sessions = vec![
            SessionData {
                tone: Tone::Orange,
                agent: "Claude Code".into(),
                project: "shop".into(),
                state: "Waiting for you".into(),
                age: "just now".into(),
            },
            SessionData {
                tone: Tone::Blue,
                agent: "Codex".into(),
                project: "figghost".into(),
                state: "Working".into(),
                age: "2 min".into(),
            },
            SessionData {
                tone: Tone::Green,
                agent: "Claude Code".into(),
                project: "api-gateway".into(),
                state: "Done".into(),
                age: "14 min".into(),
            },
        ];
        let usage = vec![
            UsageData {
                label: "Claude 5 h".into(),
                fraction: Some(0.62),
                value: "62%".into(),
                reset: "resets in 2 h 10 min".into(),
            },
            UsageData {
                label: "Claude 7 d".into(),
                fraction: Some(0.31),
                value: "31%".into(),
                reset: "resets in 4 d".into(),
            },
            UsageData {
                label: "Codex 5 h".into(),
                fraction: Some(0.94),
                value: "94%".into(),
                reset: "resets in 38 min".into(),
            },
        ];
        let approval = ApprovalCardData {
            id: 1,
            heading: "Claude Code wants to use Bash".into(),
            command:
                "rm -rf node_modules && npm ci && npm run build -- --mode production --sourcemap"
                    .into(),
            detail: "shop  -  just now".into(),
            allow: "Allow".into(),
            deny: "Deny".into(),
        };
        let display = |approval: Option<ApprovalCardData>,
                       more: &str,
                       sessions: Vec<SessionData>,
                       usage: Vec<UsageData>| {
            AgentsDisplay {
                approval,
                more_waiting: more.into(),
                sessions_heading: "Sessions".into(),
                sessions,
                usage_heading: "Usage limits".into(),
                usage,
                refresh_label: Some("Refresh usage limits (Cmd+R)".into()),
                refreshing: false,
                tokens_heading: "Tokens and cost, at API prices".into(),
                tokens: vec![
                    TokensData {
                        label: "Claude".into(),
                        today: "1.24M tokens  $3.45".into(),
                        week: "Week $21.30".into(),
                    },
                    TokensData {
                        label: "Codex".into(),
                        today: "380K tokens".into(),
                        week: "Week 2.1M tokens".into(),
                    },
                ],
                empty_title: "No agent is running".into(),
                empty_hint: "Start Claude Code or Codex in a terminal. Turn on the hooks in Settings to see it here.".into(),
            }
        };

        let shot = Rc::clone(&notch);
        let view = Rc::clone(&agents);
        app::after(0.4, move || {
            let save = |name: &str, index: usize| match shot.snapshot_png(index) {
                Some(png) => {
                    let path = dir.join(format!("{name}.png"));
                    fs::write(&path, png).expect("write png");
                    println!("wrote {}", path.display());
                }
                None => println!("no page {index}"),
            };
            save("clipboard", 0);
            save("shelf", 1);
            save("media", 2);
            save("focus", 3);
            view.show(
                mtm,
                &display(
                    Some(approval.clone()),
                    "1 more waiting",
                    sessions.clone(),
                    usage.clone(),
                ),
            );
            save("agents_approval", 4);
            view.show(mtm, &display(None, "", sessions.clone(), usage.clone()));
            save("agents_overview", 4);
            let mut busy = display(None, "", sessions.clone(), usage.clone());
            busy.refreshing = true;
            busy.usage_heading = "Usage limits  -  Codex limits unavailable".into();
            view.show(mtm, &busy);
            save("agents_refreshing", 4);
            view.show(mtm, &display(None, "", sessions.clone(), Vec::new()));
            save("agents_no_usage", 4);
            let mut empty = display(None, "", Vec::new(), Vec::new());
            empty.refresh_label = None;
            view.show(mtm, &empty);
            save("agents_empty", 4);
            process::exit(0);
        });
        (notch, shelf, media, focus, agents)
    });
}
