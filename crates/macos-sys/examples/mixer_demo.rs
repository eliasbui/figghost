//! Renders the sound page with sample apps to a PNG, for checking the layout without opening the notch.
//! `cargo run -p macos-sys --example mixer_demo -- /tmp/mixer.png`

use macos_sys::mixer_ui::{MixerCallbacks, MixerModel, MixerRow, MixerTexts, MixerView};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSBitmapImageFileType, NSBox, NSBoxType, NSColor,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::{env, fs};

fn row(key: &str, name: &str, path: &str, playing: bool, volume: f64, percent: &str) -> MixerRow {
    MixerRow {
        key: key.into(),
        name: name.into(),
        icon_path: Some(path.into()),
        playing,
        volume,
        percent: percent.into(),
        muted: false,
        output: 0,
        eq: 0,
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let out = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/mixer.png".into());
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let size = NSSize::new(500.0, 396.0);
    let texts = MixerTexts {
        tab: "Âm thanh".into(),
        hint: "Chuột phải vào tên app để chọn EQ riêng. Giữa thanh là 100%, cuối thanh 400%."
            .into(),
        enabled: "Bật bộ trộn".into(),
        device_volume: "Âm lượng phần mềm".into(),
        boost: "Nâng âm nhỏ".into(),
        night: "Chế độ đêm".into(),
        width: "Độ rộng stereo".into(),
        mute: "Tắt".into(),
        app_eq: "EQ riêng cho app này".into(),
    };
    let nothing = || Box::new(|_: f64| {});
    let view = MixerView::new(
        mtm,
        NSRect::new(NSPoint::ZERO, size),
        texts,
        MixerCallbacks {
            on_enabled: Box::new(|_| {}),
            on_device_volume: nothing(),
            on_effects_eq: Box::new(|_| {}),
            on_boost: nothing(),
            on_night: Box::new(|_| {}),
            on_width: nothing(),
            on_volume: Box::new(|_, _| {}),
            on_mute: Box::new(|_, _| {}),
            on_output: Box::new(|_, _| {}),
            on_eq: Box::new(|_, _| {}),
        },
    );
    let mut spotify = row(
        "com.spotify.client",
        "Spotify",
        "/Applications/Spotify.app",
        true,
        0.62,
        "145%",
    );
    spotify.output = 2;
    let mut teams = row(
        "com.microsoft.teams2",
        "Microsoft Teams",
        "/Applications/Microsoft Teams.app",
        false,
        0.5,
        "100%",
    );
    teams.muted = true;
    let model = MixerModel {
        enabled: true,
        notice: None,
        device_title: "Đầu ra: S24i-30 (không có âm lượng phần cứng)".into(),
        device_volume: Some((0.35, "70%".into())),
        outputs: vec![
            "Mặc định".into(),
            "Mac mini Speakers".into(),
            "S24i-30".into(),
        ],
        eq_options: vec!["Tắt EQ".into(), "Tăng bass".into(), "Rock".into()],
        app_eq_options: vec!["Tắt EQ".into(), "Tăng bass".into()],
        effects_eq: 1,
        boost: 0.4,
        night: false,
        width: 0.6,
        rows: vec![
            spotify,
            row(
                "com.google.Chrome",
                "Google Chrome",
                "/Applications/Safari.app",
                true,
                0.3,
                "60%",
            ),
            teams,
        ],
    };
    view.show(&model);

    let backdrop = NSBox::new(mtm);
    backdrop.setBoxType(NSBoxType::Custom);
    backdrop.setBorderWidth(0.0);
    backdrop.setFillColor(&NSColor::blackColor());
    backdrop.setContentViewMargins(NSSize::ZERO);
    backdrop.setFrame(NSRect::new(NSPoint::ZERO, size));
    backdrop.setContentView(Some(&view.root));
    // SAFETY: extern statics are process-lifetime constants.
    let dark = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) };
    backdrop.setAppearance(dark.as_deref());

    let bounds = backdrop.bounds();
    let rep = backdrop
        .bitmapImageRepForCachingDisplayInRect(bounds)
        .expect("bitmap rep");
    backdrop.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png: Retained<_> = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png data");
    fs::write(&out, png.to_vec()).expect("write png");
    println!("wrote {out}");
}
