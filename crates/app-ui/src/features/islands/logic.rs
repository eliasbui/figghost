//! The rules that decide when an island is shown. Plain data goes in and events come out, so that tests can drive
//! them with a pretend battery, a pretend folder and a pretend clock, and nothing here touches the system.

use std::collections::HashMap;

// -- Caps Lock ------------------------------------------------------------------------------------------------

/// Sees Caps Lock turn on or off. The first reading only takes note, so that starting the app with it on says
/// nothing.
#[derive(Debug, Default)]
pub struct CapsWatcher {
    last: Option<bool>,
}

impl CapsWatcher {
    /// `Some(state)` when the state differs from the one before.
    pub fn update(&mut self, on: bool) -> Option<bool> {
        let before = self.last.replace(on);
        before.filter(|b| *b != on).map(|_| on)
    }
}

// -- battery --------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub percent: u8,
    pub on_ac: bool,
    /// The system says the battery is full.
    pub charged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    PluggedIn(u8),
    Unplugged(u8),
    /// The charge reached the level asked for while plugged in.
    Full(u8),
    Low(u8),
}

/// How far the charge has to move back before "full" or "low" may be announced again, so that a reading that
/// shivers around the threshold says it once.
const HYSTERESIS: u8 = 5;

#[derive(Debug, Default)]
pub struct PowerWatcher {
    last: Option<Reading>,
    full_said: bool,
    low_said: bool,
}

impl PowerWatcher {
    /// Forgets what was seen, as when the settings change: the next reading is a new beginning.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, now: Reading, full_at: u8, low_at: u8) -> Vec<PowerEvent> {
        let is_full = |r: &Reading| r.on_ac && (r.charged || r.percent >= full_at);
        let is_low = |r: &Reading| !r.on_ac && r.percent <= low_at;
        let Some(before) = self.last.replace(now) else {
            // A first look only takes note: a Mac that starts full or low does not announce it.
            self.full_said = is_full(&now);
            self.low_said = is_low(&now);
            return Vec::new();
        };
        let mut events = Vec::new();
        if now.on_ac && !before.on_ac {
            events.push(PowerEvent::PluggedIn(now.percent));
            // Plugging in at a full charge is not news a second time.
            self.full_said = is_full(&now);
            self.low_said = false;
        } else if !now.on_ac && before.on_ac {
            events.push(PowerEvent::Unplugged(now.percent));
            self.full_said = false;
        }
        if is_full(&now) && !self.full_said {
            events.push(PowerEvent::Full(now.percent));
            self.full_said = true;
        } else if !now.on_ac || now.percent < full_at.saturating_sub(HYSTERESIS) {
            self.full_said = false;
        }
        if is_low(&now) && !self.low_said {
            events.push(PowerEvent::Low(now.percent));
            self.low_said = true;
        } else if now.on_ac || now.percent > low_at.saturating_add(HYSTERESIS) {
            self.low_said = false;
        }
        events
    }
}

/// The battery symbol that looks closest to the charge.
pub fn battery_symbol(percent: u8) -> &'static str {
    match percent {
        88..=100 => "battery.100",
        63..=87 => "battery.75",
        38..=62 => "battery.50",
        13..=37 => "battery.25",
        _ => "battery.0",
    }
}

// -- downloads ------------------------------------------------------------------------------------------------

/// What a browser or a downloader names a file while it is still arriving.
const PARTIAL: &[&str] = &[
    ".download",
    ".crdownload",
    ".part",
    ".partial",
    ".opdownload",
    ".tmp",
    ".aria2",
    ".filepart",
];

/// True for the name of a file that is not complete yet, or that no one chose to download.
pub fn is_partial(name: &str) -> bool {
    let lower = name.to_lowercase();
    name.starts_with('.') || PARTIAL.iter().any(|ext| lower.ends_with(ext))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub name: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seen {
    /// The size at the last look; a file is done when two looks in a row agree.
    Growing(u64),
    Settled,
}

/// Sees files arrive in a folder and says when each one has stopped growing. A browser writes `name.crdownload`
/// and renames it when it is done, so a finished download appears at once with its final size and settles at the
/// next look; a plain copy that is still being written grows from look to look and waits.
#[derive(Debug, Default)]
pub struct DownloadWatcher {
    known: HashMap<String, Seen>,
    primed: bool,
}

impl DownloadWatcher {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `files` is the (name, size) of each file in the folder now. The first look only takes note of what is
    /// already there.
    pub fn update(&mut self, files: &[(String, u64)]) -> Vec<Finished> {
        let present: Vec<&(String, u64)> =
            files.iter().filter(|(name, _)| !is_partial(name)).collect();
        if !self.primed {
            self.primed = true;
            self.known = present
                .iter()
                .map(|(n, _)| (n.clone(), Seen::Settled))
                .collect();
            return Vec::new();
        }
        // A name that is gone may come back as a new download.
        self.known
            .retain(|name, _| present.iter().any(|(n, _)| n == name));
        let mut done = Vec::new();
        for (name, size) in present {
            match self.known.get(name).copied() {
                None => {
                    self.known.insert(name.clone(), Seen::Growing(*size));
                }
                Some(Seen::Growing(before)) if before == *size => {
                    self.known.insert(name.clone(), Seen::Settled);
                    done.push(Finished {
                        name: name.clone(),
                        bytes: *size,
                    });
                }
                Some(Seen::Growing(_)) => {
                    self.known.insert(name.clone(), Seen::Growing(*size));
                }
                Some(Seen::Settled) => {}
            }
        }
        done
    }
}

/// A size as a person says it: decimal units, as Finder does.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// A name cut to `max` characters with the middle left out, so that the end, which holds the extension, stays.
pub fn shorten(name: &str, max: usize) -> String {
    // A name is shown as it is written, without what would make it read as another (a file may be called anything).
    let chars: Vec<char> = name
        .chars()
        .filter(|c| !crate::features::is_hidden_char(*c))
        .collect();
    if chars.len() <= max || max < 5 {
        return chars.into_iter().collect();
    }
    let tail = max / 3;
    let head = max - tail - 1;
    let mut out: String = chars[..head].iter().collect();
    out.push('\u{2026}');
    out.extend(&chars[chars.len() - tail..]);
    out
}

// -- drives ---------------------------------------------------------------------------------------------------

/// The drives that were announced arriving, or were there when watching began: only these are announced leaving. A disk
/// image, a backup volume or a network share that comes and goes is not news, and a drive this app has just ejected has
/// already been announced.
#[derive(Debug, Default)]
pub struct Drives {
    paths: Vec<String>,
}

impl Drives {
    /// How many are kept: a Mac has few.
    const MAX: usize = 64;

    /// Starts again from the drives that are there now.
    pub fn reset(&mut self, paths: impl IntoIterator<Item = String>) {
        self.paths.clear();
        for path in paths {
            self.arrived(path);
        }
    }

    pub fn arrived(&mut self, path: String) {
        if self.paths.contains(&path) {
            return;
        }
        if self.paths.len() >= Self::MAX {
            self.paths.remove(0);
        }
        self.paths.push(path);
    }

    /// Whether a drive that has gone was one that is announced; it is forgotten either way, so that a departure is told
    /// once, whether it came from the eject button or from the system's own notice that follows it.
    pub fn left(&mut self, path: &str) -> bool {
        let before = self.paths.len();
        self.paths.retain(|known| known != path);
        self.paths.len() != before
    }
}

// -- camera and microphone ------------------------------------------------------------------------------------

/// Sees the camera and the microphone start and stop being used.
#[derive(Debug, Default)]
pub struct PrivacyWatcher {
    last: Option<(bool, bool)>,
}

impl PrivacyWatcher {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// What was last seen: whether the camera and the microphone are in use.
    pub fn current(&self) -> (bool, bool) {
        self.last.unwrap_or((false, false))
    }

    /// `Some((camera, mic))` when either changed, including to neither, which clears the island. The first look
    /// reports what is in use if anything is, since someone should be told about a camera that is already on.
    pub fn update(&mut self, camera: bool, mic: bool) -> Option<(bool, bool)> {
        let now = (camera, mic);
        let before = self.last.replace(now);
        match before {
            None if now == (false, false) => None,
            Some(b) if b == now => None,
            _ => Some(now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(percent: u8, on_ac: bool, charged: bool) -> Reading {
        Reading {
            percent,
            on_ac,
            charged,
        }
    }

    #[test]
    fn only_a_drive_that_was_known_is_announced_leaving_and_only_once() {
        let mut drives = Drives::default();
        drives.reset(["/Volumes/Backup".to_owned()]);
        assert!(drives.left("/Volumes/Backup"));
        assert!(
            !drives.left("/Volumes/Backup"),
            "told once: an eject and the notice that follows it are one departure"
        );
        assert!(
            !drives.left("/Volumes/Disk Image"),
            "a volume that was never announced is not news"
        );
        drives.arrived("/Volumes/New".into());
        drives.arrived("/Volumes/New".into());
        assert!(drives.left("/Volumes/New"));
        assert!(!drives.left("/Volumes/New"));
        for i in 0..200 {
            drives.arrived(format!("/Volumes/{i}"));
        }
        assert!(drives.paths.len() <= Drives::MAX);
        assert!(drives.left("/Volumes/199"), "the newest are the ones kept");
    }

    #[test]
    fn the_privacy_watcher_says_what_it_last_saw() {
        let mut watcher = PrivacyWatcher::default();
        assert_eq!(watcher.current(), (false, false));
        watcher.update(true, false);
        assert_eq!(watcher.current(), (true, false));
        watcher.reset();
        assert_eq!(watcher.current(), (false, false));
    }

    #[test]
    fn caps_lock_says_nothing_until_it_changes() {
        let mut caps = CapsWatcher::default();
        assert_eq!(caps.update(true), None, "the first look only takes note");
        assert_eq!(caps.update(true), None);
        assert_eq!(caps.update(false), Some(false));
        assert_eq!(caps.update(true), Some(true));
    }

    #[test]
    fn a_mac_that_starts_full_or_low_announces_nothing() {
        let mut power = PowerWatcher::default();
        assert!(power.update(reading(100, true, true), 100, 20).is_empty());
        power.reset();
        assert!(power.update(reading(12, false, false), 100, 20).is_empty());
    }

    #[test]
    fn plugging_in_and_unplugging_are_each_announced_once() {
        let mut power = PowerWatcher::default();
        power.update(reading(60, false, false), 100, 20);
        assert_eq!(
            power.update(reading(60, true, false), 100, 20),
            vec![PowerEvent::PluggedIn(60)]
        );
        assert!(power.update(reading(61, true, false), 100, 20).is_empty());
        assert_eq!(
            power.update(reading(61, false, false), 100, 20),
            vec![PowerEvent::Unplugged(61)]
        );
    }

    #[test]
    fn full_is_announced_when_the_level_is_reached_and_again_only_after_a_real_drop() {
        let mut power = PowerWatcher::default();
        power.update(reading(95, true, false), 100, 20);
        assert_eq!(
            power.update(reading(100, true, true), 100, 20),
            vec![PowerEvent::Full(100)]
        );
        assert!(power.update(reading(100, true, true), 100, 20).is_empty());
        // A shiver of a few points around the level is not a new charge.
        assert!(power.update(reading(97, true, false), 100, 20).is_empty());
        assert!(power.update(reading(100, true, true), 100, 20).is_empty());
        // Unplugged and used, then charged again.
        power.update(reading(70, false, false), 100, 20);
        power.update(reading(70, true, false), 100, 20);
        assert_eq!(
            power.update(reading(100, true, true), 100, 20),
            vec![PowerEvent::Full(100)]
        );
    }

    #[test]
    fn a_level_chosen_below_full_is_announced_when_it_is_reached() {
        let mut power = PowerWatcher::default();
        power.update(reading(78, true, false), 80, 20);
        assert_eq!(
            power.update(reading(80, true, false), 80, 20),
            vec![PowerEvent::Full(80)]
        );
    }

    #[test]
    fn plugging_in_at_a_full_charge_is_news_once() {
        let mut power = PowerWatcher::default();
        power.update(reading(100, false, false), 100, 20);
        assert_eq!(
            power.update(reading(100, true, true), 100, 20),
            vec![PowerEvent::PluggedIn(100)]
        );
    }

    #[test]
    fn low_is_announced_on_battery_only_and_again_after_a_charge() {
        let mut power = PowerWatcher::default();
        power.update(reading(30, false, false), 100, 20);
        assert_eq!(
            power.update(reading(20, false, false), 100, 20),
            vec![PowerEvent::Low(20)]
        );
        assert!(power.update(reading(19, false, false), 100, 20).is_empty());
        // Plugged in at the same charge: not low any more, and the next time is a new time.
        assert_eq!(
            power.update(reading(19, true, false), 100, 20),
            vec![PowerEvent::PluggedIn(19)]
        );
        assert_eq!(
            power.update(reading(19, false, false), 100, 20),
            vec![PowerEvent::Unplugged(19), PowerEvent::Low(19)]
        );
    }

    #[test]
    fn the_battery_symbol_follows_the_charge() {
        assert_eq!(battery_symbol(100), "battery.100");
        assert_eq!(battery_symbol(70), "battery.75");
        assert_eq!(battery_symbol(50), "battery.50");
        assert_eq!(battery_symbol(20), "battery.25");
        assert_eq!(battery_symbol(5), "battery.0");
    }

    fn files(list: &[(&str, u64)]) -> Vec<(String, u64)> {
        list.iter().map(|(n, s)| ((*n).to_owned(), *s)).collect()
    }

    #[test]
    fn what_is_in_the_folder_at_the_start_is_not_a_download() {
        let mut watch = DownloadWatcher::default();
        assert!(watch.update(&files(&[("old.pdf", 10)])).is_empty());
        assert!(watch.update(&files(&[("old.pdf", 10)])).is_empty());
    }

    #[test]
    fn a_file_is_finished_when_two_looks_agree_on_its_size() {
        let mut watch = DownloadWatcher::default();
        watch.update(&[]);
        assert!(watch.update(&files(&[("a.zip", 500)])).is_empty());
        assert!(
            watch.update(&files(&[("a.zip", 900)])).is_empty(),
            "still growing"
        );
        let done = watch.update(&files(&[("a.zip", 900)]));
        assert_eq!(
            done,
            vec![Finished {
                name: "a.zip".into(),
                bytes: 900
            }]
        );
        assert!(
            watch.update(&files(&[("a.zip", 900)])).is_empty(),
            "said once"
        );
    }

    #[test]
    fn what_a_browser_is_still_writing_is_left_alone() {
        let mut watch = DownloadWatcher::default();
        watch.update(&[]);
        let writing = files(&[
            ("movie.mp4.crdownload", 5),
            ("page.html.download", 7),
            (".DS_Store", 1),
        ]);
        assert!(watch.update(&writing).is_empty());
        assert!(watch.update(&writing).is_empty());
        // The rename that ends the download makes the real name appear.
        assert!(watch.update(&files(&[("movie.mp4", 5000)])).is_empty());
        assert_eq!(watch.update(&files(&[("movie.mp4", 5000)])).len(), 1);
    }

    #[test]
    fn a_name_that_is_gone_and_comes_back_is_a_new_download() {
        let mut watch = DownloadWatcher::default();
        watch.update(&files(&[("a.zip", 5)]));
        watch.update(&[]);
        watch.update(&files(&[("a.zip", 5)]));
        assert_eq!(watch.update(&files(&[("a.zip", 5)])).len(), 1);
    }

    #[test]
    fn partial_names_are_recognised_whatever_their_case() {
        assert!(is_partial("Movie.MP4.CRDOWNLOAD"));
        assert!(is_partial("x.part"));
        assert!(is_partial(".hidden"));
        assert!(!is_partial("report.pdf"));
        assert!(!is_partial("download.txt"));
    }

    #[test]
    fn sizes_are_said_the_way_finder_says_them() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(999), "999 B");
        assert_eq!(human_size(1_500), "1.5 KB");
        assert_eq!(human_size(12_400_000), "12.4 MB");
        assert_eq!(human_size(250_000_000), "250 MB");
        assert_eq!(human_size(3_200_000_000), "3.2 GB");
    }

    #[test]
    fn a_long_name_loses_its_middle_and_keeps_its_end() {
        assert_eq!(shorten("short.txt", 20), "short.txt");
        let cut = shorten("a-very-long-name-for-a-file-that-keeps-going.pdf", 20);
        assert_eq!(cut.chars().count(), 20);
        assert!(cut.ends_with("ing.pdf") || cut.ends_with(".pdf"), "{cut}");
        assert!(cut.contains('\u{2026}'));
        // What would make a file read as another is not shown.
        assert_eq!(shorten("invoice\u{202E}gpj.exe", 40), "invoicegpj.exe");
    }

    #[test]
    fn the_camera_and_the_microphone_are_reported_when_they_change() {
        let mut watch = PrivacyWatcher::default();
        assert_eq!(
            watch.update(false, false),
            None,
            "nothing in use at the start"
        );
        assert_eq!(watch.update(true, false), Some((true, false)));
        assert_eq!(watch.update(true, false), None);
        assert_eq!(watch.update(true, true), Some((true, true)));
        assert_eq!(watch.update(false, false), Some((false, false)));
    }

    #[test]
    fn a_camera_that_is_already_on_at_the_start_is_reported() {
        let mut watch = PrivacyWatcher::default();
        assert_eq!(watch.update(true, false), Some((true, false)));
    }
}
