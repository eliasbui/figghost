//! Draws the System tab of the open notch into PNGs with sample figures, in both languages, without showing
//! anything on screen: run it with FIGGHOST_NOTCH_OFFSCREEN=1.
//! `FIGGHOST_NOTCH_OFFSCREEN=1 cargo run -p macos-sys --example render_system -- /tmp/system`

use macos_sys::{
    MainThreadMarker, app,
    notch::{Notch, NotchGeometry},
    style::Tone,
    system_ui::{
        CpuDisplay, HISTORY, MemoryDisplay, NetworkDisplay, Note, PAGE_SIZE, Stat, SystemDisplay,
        SystemView,
    },
    ui::AppCallbacks,
};
use std::{env, fs, path::PathBuf, process, rc::Rc};

struct Words {
    cpu: &'static str,
    memory: &'static str,
    network: &'static str,
    gpu: &'static str,
    disk: &'static str,
    thermal: &'static str,
    battery: &'static str,
    efficiency: &'static str,
    performance: &'static str,
    apps: &'static str,
    wired: &'static str,
    compressed: &'static str,
    swap: &'static str,
    warning: &'static str,
    nominal: &'static str,
    free: &'static str,
    cycles: &'static str,
}

fn words(vi: bool) -> Words {
    if vi {
        Words {
            cpu: "CPU",
            memory: "Bộ nhớ",
            network: "Mạng",
            gpu: "GPU",
            disk: "Ổ khởi động",
            thermal: "Nhiệt",
            battery: "Pin",
            efficiency: "Tiết kiệm",
            performance: "Hiệu năng",
            apps: "Ứng dụng",
            wired: "Hệ thống",
            compressed: "Nén",
            swap: "Swap",
            warning: "Cảnh báo",
            nominal: "Bình thường",
            free: "còn trống trên",
            cycles: "lần sạc",
        }
    } else {
        Words {
            cpu: "CPU",
            memory: "Memory",
            network: "Network",
            gpu: "GPU",
            disk: "Startup disk",
            thermal: "Thermal",
            battery: "Battery",
            efficiency: "Efficiency",
            performance: "Performance",
            apps: "Apps",
            wired: "Wired",
            compressed: "Compressed",
            swap: "Swap",
            warning: "Warning",
            nominal: "Nominal",
            free: "free of",
            cycles: "cycles",
        }
    }
}

fn history(seed: f64, scale: f64) -> Vec<f64> {
    (0..HISTORY)
        .map(|i| {
            let t = i as f64 * 0.37 + seed;
            ((t.sin() * 0.5 + 0.5) * (0.4 + 0.6 * (t * 0.31).cos().abs()) * scale).min(1.0)
        })
        .collect()
}

fn scene(vi: bool, busy: bool, with_battery: bool) -> SystemDisplay {
    let w = words(vi);
    let cores: Vec<f64> = if busy {
        vec![0.92, 0.88, 0.95, 0.71, 0.84, 0.97, 0.99, 0.62, 0.93, 0.78]
    } else {
        vec![0.12, 0.08, 0.31, 0.05, 0.22, 0.44, 0.67, 0.18, 0.09, 0.03]
    };
    let mut stats = vec![
        Stat {
            title: w.gpu.into(),
            value: if busy { "64%".into() } else { "6%".into() },
            detail: "Apple M4".into(),
            fraction: Some(if busy { 0.64 } else { 0.06 }),
            tone: Tone::Blue,
        },
        Stat {
            title: w.disk.into(),
            value: "227 GB".into(),
            detail: format!("{} 460 GB", w.free),
            fraction: Some(0.51),
            tone: Tone::Blue,
        },
        Stat {
            title: w.thermal.into(),
            value: w.nominal.into(),
            detail: String::new(),
            fraction: None,
            tone: Tone::Green,
        },
    ];
    if with_battery {
        stats.push(Stat {
            title: w.battery.into(),
            value: "92%".into(),
            detail: format!("212 {}", w.cycles),
            fraction: Some(0.92),
            tone: Tone::Green,
        });
    }
    SystemDisplay {
        cpu: Some(CpuDisplay {
            title: w.cpu.into(),
            value: if busy { "84%".into() } else { "17%".into() },
            cores,
            slow: 6,
            slow_label: w.efficiency.into(),
            fast_label: w.performance.into(),
        }),
        memory: Some(MemoryDisplay {
            title: w.memory.into(),
            used: "12.7 GB".into(),
            of: "/ 16 GB".into(),
            shares: [0.24, 0.175, 0.38],
            legend: [
                format!("{} 3.9 GB", w.apps),
                format!("{} 2.8 GB", w.wired),
                format!("{} 6.1 GB", w.compressed),
            ],
            pressure: w.warning.into(),
            pressure_tone: Tone::Orange,
            swap_title: w.swap.to_uppercase(),
            swap: "5.3 GB".into(),
        }),
        network: Some(NetworkDisplay {
            title: w.network.into(),
            down: if busy {
                "48.2 MB/s".into()
            } else {
                "1.2 MB/s".into()
            },
            up: if busy {
                "3.9 MB/s".into()
            } else {
                "84 KB/s".into()
            },
            down_history: history(0.0, if busy { 0.9 } else { 0.35 }),
            up_history: history(2.0, if busy { 0.3 } else { 0.12 }),
        }),
        stats,
        note: None,
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/system".into()));
    fs::create_dir_all(&dir).expect("output directory");
    app::run_accessory(mtm, move || {
        let geometry = NotchGeometry::detect(mtm).expect("a screen");
        let noop = AppCallbacks {
            on_query: Box::new(|_, _| {}),
            on_activate: Box::new(|_, _, _| {}),
            on_pin: Box::new(|_, _| {}),
            on_delete: Box::new(|_, _| {}),
        };
        let notch = Rc::new(Notch::new(mtm, geometry, noop, |view| {
            view.set_rows(Vec::new(), "")
        }));
        let view = SystemView::new(mtm, notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height));
        let index = notch.page_count();
        notch.add_page(view.page("System".into(), || {}, || {}));
        let (shot, shown) = (Rc::clone(&notch), Rc::clone(&view));
        app::after(0.4, move || {
            let mut scenes: Vec<(&str, SystemDisplay)> = vec![
                ("calm_en", scene(false, false, false)),
                ("busy_en", scene(false, true, true)),
                ("calm_vi", scene(true, false, true)),
            ];
            // With some cards switched off the others share the room, and an off tab says so.
            let mut few = scene(false, true, false);
            few.memory = None;
            few.stats.truncate(2);
            scenes.push(("few_en", few));
            let mut only = scene(false, false, false);
            only.cpu = None;
            only.network = None;
            only.stats.clear();
            scenes.push(("memory_only_en", only));
            for (name, title, hint) in [
                (
                    "off_en",
                    "The System tab is off",
                    "Turn it on in Settings, under Notch tabs, System.",
                ),
                (
                    "off_vi",
                    "Tab Hệ thống đang tắt",
                    "Bật trong Cài đặt, mục Tab của notch, Hệ thống.",
                ),
            ] {
                scenes.push((
                    name,
                    SystemDisplay {
                        note: Some(Note {
                            title: title.into(),
                            hint: hint.into(),
                        }),
                        ..SystemDisplay::default()
                    },
                ));
            }
            for (name, display) in scenes {
                shown.show(display);
                let png = shot.snapshot_png(index).expect("the page");
                let path = dir.join(format!("{name}.png"));
                fs::write(&path, png).expect("write png");
                println!("wrote {}", path.display());
            }
            process::exit(0);
        });
        (notch, view)
    });
}
