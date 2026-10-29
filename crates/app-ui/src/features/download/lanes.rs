//! Lanes, the download manager by the same author, writes the latest of its downloads to `progress.json` in its data
//! folder for its browser extension. Figghost reads that file, and nothing else of Lanes, to put the downloads on the
//! closed notch: the share done while they run, and a short notice when one ends. Nothing is written to it.

use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const BUNDLE_ID: &str = "com.eliasbui.lanes";
pub const PAGE: &str = "https://github.com/eliasbui/lanes";

/// The states of a download in Lanes (its `engine::state`); paused is 2.
const QUEUED: u8 = 0;
const RUNNING: u8 = 1;
const DONE: u8 = 3;
const FAILED: u8 = 4;

/// One download as Lanes lists it; the rest of what it says is not needed here.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Item {
    pub id: i64,
    #[serde(default)]
    pub filename: String,
    pub state: u8,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub downloaded: u64,
}

pub fn progress_file(home: &Path) -> PathBuf {
    home.join("Library/Application Support/Lanes/progress.json")
}

/// The list in the file; `None` for a file Lanes is still writing, which waits for the next look.
pub fn parse(bytes: &[u8]) -> Option<Vec<Item>> {
    serde_json::from_slice(bytes).ok()
}

/// What runs, as the notch shows it: `None` while nothing waits or goes on, and otherwise the share done in percent over
/// the downloads whose size is known, itself `None` while no size is known yet.
pub fn running(items: &[Item]) -> Option<Option<f64>> {
    let active: Vec<&Item> = items
        .iter()
        .filter(|i| i.state == QUEUED || i.state == RUNNING)
        .collect();
    if active.is_empty() {
        return None;
    }
    let (done, total) = active
        .iter()
        .filter_map(|i| i.size.filter(|&s| s > 0).map(|s| (i.downloaded.min(s), s)))
        .fold((0u64, 0u64), |(d, t), (done, size)| (d + done, t + size));
    Some((total > 0).then(|| 100.0 * done as f64 / total as f64))
}

/// Tells the downloads that ended since the last look. The first look only takes note of what is there.
#[derive(Default)]
pub struct Watch {
    /// The state of each download at the last look. An id is never forgotten, so that an old download that comes back
    /// into the list when a newer one is removed is not announced again; it costs a few bytes a download.
    states: HashMap<i64, u8>,
    primed: bool,
}

impl Watch {
    /// The name of each download that is done or failed now and was not at the last look, or was not there at all for
    /// one that started and ended between two looks, with whether it is done.
    pub fn update(&mut self, items: &[Item]) -> Vec<(String, bool)> {
        let primed = std::mem::replace(&mut self.primed, true);
        let mut ended = Vec::new();
        for item in items {
            let before = self.states.insert(item.id, item.state);
            let over = item.state == DONE || item.state == FAILED;
            if primed && over && before != Some(item.state) {
                ended.push((item.filename.clone(), item.state == DONE));
            }
        }
        ended
    }
}

/// What was seen of the file at the last look.
#[derive(Default)]
pub struct Seen {
    /// When the file was last changed, so that a file that has not changed is not read again.
    pub stamp: Option<SystemTime>,
    pub watch: Watch,
    /// What the notch shows, so that it is told only when that changes; empty while it shows nothing.
    pub shown: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, state: u8, size: Option<u64>, downloaded: u64) -> Item {
        Item {
            id,
            filename: format!("file{id}.mp4"),
            state,
            size,
            downloaded,
        }
    }

    #[test]
    fn the_file_lanes_writes_is_read_and_half_a_file_waits() {
        let text = r#"[{"connections":1,"dir":"/Users/x/Downloads/Video","downloaded":512,"error":"","filename":"A video.mp4","id":7,"links":0,"size":1024,"sources":1,"speed":0,"state":1,"url":"https://example.com/a.mp4"},
                      {"id":8,"filename":"b.zip","state":0,"size":null,"downloaded":0}]"#;
        let items = parse(text.as_bytes()).unwrap();
        assert_eq!(
            items[0],
            Item {
                id: 7,
                filename: "A video.mp4".into(),
                state: RUNNING,
                size: Some(1024),
                downloaded: 512
            }
        );
        assert_eq!(items[1].size, None, "a size not known yet is null");
        assert_eq!(parse(&text.as_bytes()[..40]), None);
        assert_eq!(
            progress_file(Path::new("/Users/x")),
            PathBuf::from("/Users/x/Library/Application Support/Lanes/progress.json")
        );
    }

    #[test]
    fn the_share_is_over_what_runs_with_a_known_size() {
        assert_eq!(running(&[]), None);
        assert_eq!(
            running(&[
                item(1, DONE, Some(10), 10),
                item(2, FAILED, None, 0),
                item(3, 2, Some(10), 5)
            ]),
            None,
            "nothing done, failed or paused runs"
        );
        assert_eq!(
            running(&[
                item(1, RUNNING, Some(100), 25),
                item(2, QUEUED, Some(300), 0),
                item(3, DONE, Some(9), 9)
            ]),
            Some(Some(6.25)),
            "a waiting one counts, a finished one does not"
        );
        assert_eq!(
            running(&[item(1, RUNNING, None, 50), item(2, RUNNING, Some(0), 0)]),
            Some(None),
            "no size known yet"
        );
        assert_eq!(
            running(&[item(1, RUNNING, Some(100), 150)]),
            Some(Some(100.0)),
            "never more than the whole"
        );
    }

    #[test]
    fn an_end_is_told_once_and_the_first_look_tells_none() {
        let mut watch = Watch::default();
        assert!(
            watch
                .update(&[item(1, DONE, Some(1), 1), item(2, RUNNING, Some(9), 1)])
                .is_empty(),
            "what was there before is not news"
        );
        assert!(
            watch
                .update(&[item(1, DONE, Some(1), 1), item(2, RUNNING, Some(9), 5)])
                .is_empty()
        );
        assert_eq!(
            watch.update(&[item(1, DONE, Some(1), 1), item(2, DONE, Some(9), 9)]),
            vec![("file2.mp4".to_owned(), true)]
        );
        assert!(
            watch
                .update(&[item(1, DONE, Some(1), 1), item(2, DONE, Some(9), 9)])
                .is_empty(),
            "said once"
        );
        assert_eq!(
            watch.update(&[item(3, FAILED, None, 0), item(2, DONE, Some(9), 9)]),
            vec![("file3.mp4".to_owned(), false)],
            "one that started and failed between two looks"
        );
        assert!(
            watch.update(&[item(1, DONE, Some(1), 1)]).is_empty(),
            "an old one that comes back into the list is not news"
        );
        watch.update(&[item(3, RUNNING, None, 0)]);
        assert_eq!(
            watch.update(&[item(3, DONE, Some(4), 4)]),
            vec![("file3.mp4".to_owned(), true)],
            "one tried again and finished"
        );
    }
}
