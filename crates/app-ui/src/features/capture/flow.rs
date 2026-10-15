//! The steps of a capture, from a shortcut to the clipboard, the folder, the editor and the confirmation.
//! Everything that touches the system goes through `Backend`, so the steps are tested with a fake.

use super::{Config, Mode};
use core_capture::{
    Display, Grab, Pick, Point, Rect, Shot, display_index_at,
    naming::{LocalTime, file_stem, save_unique},
};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

/// How long a picture may be on its way before a new capture is allowed to start. The system answers in a
/// fraction of a second; this only keeps a lost answer from switching the feature off for good.
const GRAB_LIMIT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickMode {
    Region,
    Window,
}

/// What the person is told when a capture is over.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    Captured(Captured),
    Text(String),
    NoText,
    TextFailed(String),
    PermissionNeeded,
    NothingToRepeat,
    Unavailable,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Captured {
    pub png: Vec<u8>,
    pub copied: bool,
    pub saved: Option<PathBuf>,
    pub save_error: Option<String>,
}

/// The system, as the steps need it. Answers that take time come back through a callback, on the main
/// thread; the fake answers at once.
pub trait Backend {
    /// Whether this system can capture at all (macOS 14 and later).
    fn available(&self) -> bool;
    fn permission(&self) -> bool;
    /// Asks the system for the permission; it shows its dialog only the first time.
    fn request_permission(&self);
    fn displays(&self) -> Vec<Display>;
    fn pointer(&self) -> Point;
    /// Shows the overlay. A call while an earlier overlay is still up replaces it, and the earlier one
    /// never answers.
    fn pick(&self, mode: PickMode, done: Box<dyn FnOnce(Pick)>);
    fn grab(&self, what: Grab, done: Box<dyn FnOnce(Result<Shot, String>)>);
    fn copy_png(&self, png: &[u8]);
    fn copy_text(&self, text: &str);
    fn recognize(&self, png: Vec<u8>, done: Box<dyn FnOnce(Result<String, String>)>);
    /// Opens the editor on the picture; `folder` is where its Save writes.
    fn edit(&self, shot: &Shot, folder: &Path, title: &str);
    fn notify(&self, note: Note);
    fn now(&self) -> LocalTime;
    /// The file name without extension, with `{date}` and `{time}` still to fill in.
    fn name_template(&self) -> String;
}

/// What a picture is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Image,
    Text,
}

pub struct Flow<B: Backend> {
    backend: Rc<B>,
    /// Counts the captures that were started; an answer for an older one is dropped.
    epoch: Cell<u64>,
    grabbing: Cell<Option<Instant>>,
    grab_limit: Cell<Duration>,
    /// The last region captured: the display it was on and the rectangle.
    last: RefCell<Option<(u32, Rect)>>,
}

impl<B: Backend + 'static> Flow<B> {
    pub fn new(backend: Rc<B>) -> Self {
        Self {
            backend,
            epoch: Cell::new(0),
            grabbing: Cell::new(None),
            grab_limit: Cell::new(GRAB_LIMIT),
            last: RefCell::new(None),
        }
    }

    pub fn run(self: &Rc<Self>, mode: Mode, config: &Config) {
        if self
            .grabbing
            .get()
            .is_some_and(|since| since.elapsed() < self.grab_limit.get())
        {
            return;
        }
        self.grabbing.set(None);
        let epoch = self.epoch.get() + 1;
        self.epoch.set(epoch);
        let backend = &self.backend;
        if !backend.available() {
            return backend.notify(Note::Unavailable);
        }
        if !backend.permission() {
            backend.request_permission();
            return backend.notify(Note::PermissionNeeded);
        }
        let config = Rc::new(config.clone());
        match mode {
            Mode::Screen => {
                let displays = backend.displays();
                match display_index_at(&displays, backend.pointer()) {
                    Some(i) => self.grab(Grab::Screen(displays[i]), Purpose::Image, epoch, &config),
                    None => backend.notify(Note::Failed("no display".into())),
                }
            }
            Mode::Region | Mode::Text => {
                let purpose = if mode == Mode::Text {
                    Purpose::Text
                } else {
                    Purpose::Image
                };
                let (flow, config) = (Rc::clone(self), config);
                backend.pick(
                    PickMode::Region,
                    Box::new(move |pick| {
                        if flow.epoch.get() != epoch {
                            return;
                        }
                        if let Pick::Region { display, rect } = pick {
                            *flow.last.borrow_mut() = Some((display.id, rect));
                            flow.grab(Grab::Region { display, rect }, purpose, epoch, &config);
                        }
                    }),
                );
            }
            Mode::Window => {
                let (flow, config) = (Rc::clone(self), config);
                backend.pick(
                    PickMode::Window,
                    Box::new(move |pick| {
                        if flow.epoch.get() != epoch {
                            return;
                        }
                        if let Pick::Window(window) = pick {
                            let what = Grab::Window {
                                id: window.id,
                                shadow: config.window_shadow,
                            };
                            flow.grab(what, Purpose::Image, epoch, &config);
                        }
                    }),
                );
            }
            Mode::Repeat => {
                let last = *self.last.borrow();
                let target = last.and_then(|(id, rect)| {
                    let display = backend.displays().into_iter().find(|d| d.id == id)?;
                    // The display may have changed size since: keep what is still on it.
                    let rect = rect.intersection(&display.frame)?;
                    Some((display, rect))
                });
                match target {
                    Some((display, rect)) => {
                        self.grab(
                            Grab::Region { display, rect },
                            Purpose::Image,
                            epoch,
                            &config,
                        );
                    }
                    None => backend.notify(Note::NothingToRepeat),
                }
            }
        }
    }

    fn grab(self: &Rc<Self>, what: Grab, purpose: Purpose, epoch: u64, config: &Rc<Config>) {
        self.grabbing.set(Some(Instant::now()));
        let (flow, config) = (Rc::clone(self), Rc::clone(config));
        self.backend.grab(
            what,
            Box::new(move |result| {
                if flow.epoch.get() != epoch {
                    return;
                }
                flow.grabbing.set(None);
                match result {
                    Err(message) => flow.backend.notify(Note::Failed(message)),
                    Ok(shot) => match purpose {
                        Purpose::Image => flow.keep(shot, &config),
                        Purpose::Text => flow.read_text(shot),
                    },
                }
            }),
        );
    }

    /// Copies, saves and opens the picture as the settings say, then tells the person.
    fn keep(&self, shot: Shot, config: &Config) {
        let backend = &self.backend;
        let template = backend.name_template();
        let stem = file_stem(&template, backend.now());
        let folder = config.save_dir();
        let (mut saved, mut save_error) = (None, None);
        if config.save {
            match save_unique(&folder, &stem, "png", &shot.png) {
                Ok(path) => saved = Some(path),
                Err(error) => {
                    eprintln!("capture: cannot save to {}: {error}", folder.display());
                    save_error = Some(error.to_string());
                }
            }
        }
        if config.copy {
            backend.copy_png(&shot.png);
        }
        if config.edit {
            backend.edit(&shot, &folder, &stem);
        }
        backend.notify(Note::Captured(Captured {
            png: shot.png,
            copied: config.copy,
            saved,
            save_error,
        }));
    }

    fn read_text(&self, shot: Shot) {
        let backend = Rc::clone(&self.backend);
        self.backend.recognize(
            shot.png,
            Box::new(move |result| match result {
                Ok(text) if text.trim().is_empty() => backend.notify(Note::NoText),
                Ok(text) => {
                    backend.copy_text(&text);
                    backend.notify(Note::Text(text));
                }
                Err(error) => backend.notify(Note::TextFailed(error)),
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_capture::WindowBox;
    use std::{collections::VecDeque, fs};

    type Callback<T> = Box<dyn FnOnce(T)>;

    /// A system made of lists: it records what it was asked and answers from what the test queued.
    #[derive(Default)]
    struct Fake {
        log: RefCell<Vec<String>>,
        notes: RefCell<Vec<Note>>,
        displays: RefCell<Vec<Display>>,
        pointer: Cell<Point>,
        denied: Cell<bool>,
        unavailable: Cell<bool>,
        picks: RefCell<VecDeque<Pick>>,
        /// Hold the answer of a pick or a grab until the test releases it.
        hold: Cell<bool>,
        held_pick: RefCell<Vec<Callback<Pick>>>,
        held_grab: RefCell<Vec<Callback<Result<Shot, String>>>>,
        shot_error: RefCell<Option<String>>,
        text: RefCell<Option<Result<String, String>>>,
        grabs: RefCell<Vec<Grab>>,
        copied_png: RefCell<Vec<Vec<u8>>>,
        copied_text: RefCell<Vec<String>>,
        edited: RefCell<Vec<(PathBuf, String)>>,
    }

    impl Fake {
        fn note(&self) -> Note {
            self.notes.borrow().last().cloned().expect("a note")
        }
    }

    impl Backend for Fake {
        fn available(&self) -> bool {
            !self.unavailable.get()
        }
        fn permission(&self) -> bool {
            !self.denied.get()
        }
        fn request_permission(&self) {
            self.log.borrow_mut().push("request".into());
        }
        fn displays(&self) -> Vec<Display> {
            self.displays.borrow().clone()
        }
        fn pointer(&self) -> Point {
            self.pointer.get()
        }
        fn pick(&self, mode: PickMode, done: Callback<Pick>) {
            self.log.borrow_mut().push(format!("pick {mode:?}"));
            if self.hold.get() {
                self.held_pick.borrow_mut().push(done);
            } else {
                let answer = self
                    .picks
                    .borrow_mut()
                    .pop_front()
                    .unwrap_or(Pick::Cancelled);
                done(answer);
            }
        }
        fn grab(&self, what: Grab, done: Callback<Result<Shot, String>>) {
            self.grabs.borrow_mut().push(what.clone());
            let result = match self.shot_error.borrow().clone() {
                Some(error) => Err(error),
                None => Ok(shot_of(&what)),
            };
            if self.hold.get() {
                self.held_grab
                    .borrow_mut()
                    .push(Box::new(move |_| done(result)));
            } else {
                done(result);
            }
        }
        fn copy_png(&self, png: &[u8]) {
            self.copied_png.borrow_mut().push(png.to_vec());
        }
        fn copy_text(&self, text: &str) {
            self.copied_text.borrow_mut().push(text.to_owned());
        }
        fn recognize(&self, png: Vec<u8>, done: Callback<Result<String, String>>) {
            self.log
                .borrow_mut()
                .push(format!("recognize {} bytes", png.len()));
            let answer = self.text.borrow().clone().unwrap_or(Ok(String::new()));
            done(answer);
        }
        fn edit(&self, _shot: &Shot, folder: &Path, title: &str) {
            self.edited
                .borrow_mut()
                .push((folder.to_path_buf(), title.to_owned()));
        }
        fn notify(&self, note: Note) {
            self.notes.borrow_mut().push(note);
        }
        fn now(&self) -> LocalTime {
            LocalTime {
                year: 2026,
                month: 10,
                day: 15,
                hour: 21,
                minute: 4,
                second: 33,
            }
        }
        fn name_template(&self) -> String {
            "Figghost {date} at {time}".into()
        }
    }

    /// The picture a display of this size would give back: its size in pixels is what the request asks for.
    fn shot_of(what: &Grab) -> Shot {
        let (w, h, scale) = match what {
            Grab::Region { display, rect } => (
                rect.w * display.scale,
                rect.h * display.scale,
                display.scale,
            ),
            Grab::Screen(d) => (d.frame.w * d.scale, d.frame.h * d.scale, d.scale),
            Grab::Window { .. } => (400.0, 300.0, 2.0),
        };
        Shot {
            png: format!("png {w}x{h}").into_bytes(),
            width: w as u32,
            height: h as u32,
            scale,
        }
    }

    fn display(id: u32, x: f64, y: f64, w: f64, h: f64, scale: f64) -> Display {
        Display {
            id,
            frame: Rect::new(x, y, w, h),
            scale,
        }
    }

    fn setup() -> (Rc<Fake>, Rc<Flow<Fake>>, PathBuf) {
        let fake = Rc::new(Fake::default());
        fake.displays.replace(vec![
            display(1, 0.0, 0.0, 1440.0, 900.0, 2.0),
            display(2, 1440.0, 300.0, 1920.0, 1080.0, 1.0),
        ]);
        fake.pointer.set(Point::new(100.0, 100.0));
        let flow = Rc::new(Flow::new(Rc::clone(&fake)));
        let dir = std::env::temp_dir().join(format!(
            "figghost-capture-flow-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        (fake, flow, dir)
    }

    fn config(dir: &Path) -> Config {
        Config {
            enabled: true,
            folder: dir.to_string_lossy().into_owned(),
            ..Config::default()
        }
    }

    fn region(display: Display, rect: Rect) -> Pick {
        Pick::Region { display, rect }
    }

    #[test]
    fn a_region_is_grabbed_copied_saved_and_remembered() {
        let (fake, flow, dir) = setup();
        let d = fake.displays()[0];
        let rect = Rect::new(100.0, 200.0, 300.0, 150.0);
        fake.picks.borrow_mut().push_back(region(d, rect));
        flow.run(Mode::Region, &config(&dir));
        assert_eq!(
            *fake.grabs.borrow(),
            vec![Grab::Region { display: d, rect }]
        );
        assert_eq!(fake.copied_png.borrow().len(), 1);
        let Note::Captured(done) = fake.note() else {
            panic!("a capture note")
        };
        let path = dir.join("Figghost 2026-10-15 at 21.04.33.png");
        assert_eq!(done.saved.as_deref(), Some(path.as_path()));
        assert!(done.copied && done.save_error.is_none());
        assert_eq!(
            fs::read(&path).unwrap(),
            done.png,
            "the file is the picture that was copied"
        );
        assert_eq!(
            done.png,
            b"png 600x300".to_vec(),
            "native pixels: points times the scale"
        );
        assert_eq!(*flow.last.borrow(), Some((1, rect)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_settings_decide_where_the_picture_goes() {
        let (fake, flow, dir) = setup();
        let d = fake.displays()[0];
        let rect = Rect::new(0.0, 0.0, 50.0, 50.0);
        let mut cfg = config(&dir);
        cfg.copy = false;
        fake.picks.borrow_mut().push_back(region(d, rect));
        flow.run(Mode::Region, &cfg);
        assert!(fake.copied_png.borrow().is_empty());
        let Note::Captured(done) = fake.note() else {
            panic!()
        };
        assert!(!done.copied && done.saved.is_some());

        cfg.copy = true;
        cfg.save = false;
        fake.picks.borrow_mut().push_back(region(d, rect));
        flow.run(Mode::Region, &cfg);
        assert_eq!(fake.copied_png.borrow().len(), 1);
        let Note::Captured(done) = fake.note() else {
            panic!()
        };
        assert!(done.copied && done.saved.is_none());
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "only the first one was saved"
        );

        cfg.copy = false;
        fake.picks.borrow_mut().push_back(region(d, rect));
        flow.run(Mode::Region, &cfg);
        let Note::Captured(done) = fake.note() else {
            panic!()
        };
        assert!(!done.copied && done.saved.is_none() && done.save_error.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_captures_in_one_second_keep_both_files() {
        let (fake, flow, dir) = setup();
        let d = fake.displays()[0];
        for _ in 0..3 {
            fake.picks
                .borrow_mut()
                .push_back(region(d, Rect::new(0.0, 0.0, 20.0, 20.0)));
            flow.run(Mode::Region, &config(&dir));
        }
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "Figghost 2026-10-15 at 21.04.33 2.png",
                "Figghost 2026-10-15 at 21.04.33 3.png",
                "Figghost 2026-10-15 at 21.04.33.png"
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_that_cannot_be_written_is_reported_and_the_copy_still_happens() {
        let (fake, flow, dir) = setup();
        fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("file");
        fs::write(&blocker, b"x").unwrap();
        let d = fake.displays()[0];
        fake.picks
            .borrow_mut()
            .push_back(region(d, Rect::new(0.0, 0.0, 20.0, 20.0)));
        flow.run(Mode::Region, &config(&blocker));
        let Note::Captured(done) = fake.note() else {
            panic!()
        };
        assert!(done.saved.is_none() && done.save_error.is_some() && done.copied);
        assert_eq!(fake.copied_png.borrow().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_editor_opens_only_when_asked_and_knows_where_to_save() {
        let (fake, flow, dir) = setup();
        let d = fake.displays()[0];
        fake.picks
            .borrow_mut()
            .push_back(region(d, Rect::new(0.0, 0.0, 20.0, 20.0)));
        flow.run(Mode::Region, &config(&dir));
        assert!(fake.edited.borrow().is_empty());
        let mut cfg = config(&dir);
        cfg.edit = true;
        fake.picks
            .borrow_mut()
            .push_back(region(d, Rect::new(0.0, 0.0, 20.0, 20.0)));
        flow.run(Mode::Region, &cfg);
        let edited = fake.edited.borrow();
        assert_eq!(edited.len(), 1);
        assert_eq!(edited[0].0, dir);
        assert_eq!(edited[0].1, "Figghost 2026-10-15 at 21.04.33");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_overlay_captures_nothing() {
        let (fake, flow, dir) = setup();
        flow.run(Mode::Region, &config(&dir));
        assert!(fake.grabs.borrow().is_empty() && fake.notes.borrow().is_empty());
        flow.run(Mode::Window, &config(&dir));
        assert!(fake.grabs.borrow().is_empty() && fake.notes.borrow().is_empty());
        assert!(!dir.exists());
    }

    #[test]
    fn a_window_is_captured_alone_with_or_without_its_shadow() {
        let (fake, flow, dir) = setup();
        let window = WindowBox {
            id: 77,
            frame: Rect::new(10.0, 10.0, 400.0, 300.0),
            owner: "Notes".into(),
            title: "Todo".into(),
        };
        fake.picks
            .borrow_mut()
            .push_back(Pick::Window(window.clone()));
        flow.run(Mode::Window, &config(&dir));
        assert_eq!(fake.log.borrow()[0], "pick Window");
        assert_eq!(
            *fake.grabs.borrow(),
            vec![Grab::Window {
                id: 77,
                shadow: true
            }]
        );
        let mut cfg = config(&dir);
        cfg.window_shadow = false;
        fake.picks.borrow_mut().push_back(Pick::Window(window));
        flow.run(Mode::Window, &cfg);
        assert_eq!(
            fake.grabs.borrow()[1],
            Grab::Window {
                id: 77,
                shadow: false
            }
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_screen_is_the_display_with_the_pointer_and_needs_no_overlay() {
        let (fake, flow, dir) = setup();
        fake.pointer.set(Point::new(2000.0, 600.0));
        flow.run(Mode::Screen, &config(&dir));
        assert_eq!(*fake.grabs.borrow(), vec![Grab::Screen(fake.displays()[1])]);
        assert!(fake.log.borrow().is_empty(), "no overlay");
        fake.pointer.set(Point::new(10.0, 10.0));
        flow.run(Mode::Screen, &config(&dir));
        assert_eq!(fake.grabs.borrow()[1], Grab::Screen(fake.displays()[0]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repeat_takes_the_last_region_again_without_an_overlay() {
        let (fake, flow, dir) = setup();
        flow.run(Mode::Repeat, &config(&dir));
        assert_eq!(fake.note(), Note::NothingToRepeat);
        assert!(fake.grabs.borrow().is_empty());

        let d = fake.displays()[1];
        let rect = Rect::new(1500.0, 400.0, 200.0, 100.0);
        fake.picks.borrow_mut().push_back(region(d, rect));
        flow.run(Mode::Region, &config(&dir));
        fake.log.borrow_mut().clear();
        flow.run(Mode::Repeat, &config(&dir));
        assert!(fake.log.borrow().is_empty(), "the overlay did not show");
        assert_eq!(fake.grabs.borrow()[1], Grab::Region { display: d, rect });
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repeat_keeps_what_is_still_on_a_display_that_got_smaller_and_gives_up_on_one_that_is_gone() {
        let (fake, flow, dir) = setup();
        let d = fake.displays()[1];
        let rect = Rect::new(2800.0, 400.0, 1000.0, 100.0);
        fake.picks.borrow_mut().push_back(region(d, rect));
        flow.run(Mode::Region, &config(&dir));
        flow.run(Mode::Repeat, &config(&dir));
        // The display is 1920 wide, so only 560 points of the 1000 are on it.
        let Grab::Region { rect: again, .. } = fake.grabs.borrow()[1].clone() else {
            panic!()
        };
        assert_eq!((again.x, again.w), (2800.0, 560.0));

        fake.displays.replace(vec![fake.displays()[0]]);
        let before = fake.grabs.borrow().len();
        flow.run(Mode::Repeat, &config(&dir));
        assert_eq!(fake.grabs.borrow().len(), before);
        assert_eq!(fake.note(), Note::NothingToRepeat);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn text_goes_to_the_clipboard_as_text_and_the_picture_is_not_kept() {
        let (fake, flow, dir) = setup();
        let d = fake.displays()[0];
        fake.picks
            .borrow_mut()
            .push_back(region(d, Rect::new(0.0, 0.0, 100.0, 40.0)));
        fake.text.replace(Some(Ok("Hello\nworld".into())));
        flow.run(Mode::Text, &config(&dir));
        assert_eq!(*fake.copied_text.borrow(), vec!["Hello\nworld".to_owned()]);
        assert_eq!(fake.note(), Note::Text("Hello\nworld".into()));
        assert!(fake.copied_png.borrow().is_empty());
        assert!(!dir.exists(), "no file is saved for a text capture");
        assert_eq!(
            *flow.last.borrow(),
            Some((1, Rect::new(0.0, 0.0, 100.0, 40.0)))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_recognition_error_is_shown_and_the_clipboard_is_left_alone() {
        let (fake, flow, dir) = setup();
        let d = fake.displays()[0];
        fake.picks
            .borrow_mut()
            .push_back(region(d, Rect::new(0.0, 0.0, 100.0, 40.0)));
        fake.text
            .replace(Some(Err("text recognition is not available yet".into())));
        flow.run(Mode::Text, &config(&dir));
        assert_eq!(
            fake.note(),
            Note::TextFailed("text recognition is not available yet".into())
        );
        assert!(fake.copied_text.borrow().is_empty());
        fake.picks
            .borrow_mut()
            .push_back(region(d, Rect::new(0.0, 0.0, 100.0, 40.0)));
        fake.text.replace(Some(Ok("  \n ".into())));
        flow.run(Mode::Text, &config(&dir));
        assert_eq!(fake.note(), Note::NoText);
        assert!(
            fake.copied_text.borrow().is_empty(),
            "blank text does not replace the clipboard"
        );
    }

    #[test]
    fn without_the_permission_the_system_is_asked_and_nothing_else_happens() {
        let (fake, flow, dir) = setup();
        fake.denied.set(true);
        for mode in Mode::ALL {
            flow.run(mode, &config(&dir));
        }
        assert_eq!(
            fake.log.borrow().iter().filter(|l| *l == "request").count(),
            5
        );
        assert!(fake.grabs.borrow().is_empty());
        assert!(
            fake.log.borrow().iter().all(|l| l == "request"),
            "no overlay without the permission"
        );
        assert_eq!(fake.note(), Note::PermissionNeeded);
    }

    #[test]
    fn an_old_system_gets_a_clear_message() {
        let (fake, flow, dir) = setup();
        fake.unavailable.set(true);
        flow.run(Mode::Screen, &config(&dir));
        assert_eq!(fake.note(), Note::Unavailable);
        assert!(fake.grabs.borrow().is_empty());
    }

    #[test]
    fn a_failed_grab_is_reported_and_nothing_is_kept() {
        let (fake, flow, dir) = setup();
        fake.shot_error
            .replace(Some("the display is no longer there".into()));
        flow.run(Mode::Screen, &config(&dir));
        assert_eq!(
            fake.note(),
            Note::Failed("the display is no longer there".into())
        );
        assert!(fake.copied_png.borrow().is_empty() && !dir.exists());
    }

    #[test]
    fn a_second_capture_waits_for_the_picture_that_is_on_its_way() {
        let (fake, flow, dir) = setup();
        fake.hold.set(true);
        flow.run(Mode::Screen, &config(&dir));
        flow.run(Mode::Screen, &config(&dir));
        assert_eq!(
            fake.grabs.borrow().len(),
            1,
            "the second shortcut press is ignored"
        );
        let release = fake.held_grab.borrow_mut().remove(0);
        release(Ok(shot_of(&fake.grabs.borrow()[0])));
        assert_eq!(fake.copied_png.borrow().len(), 1);
        fake.hold.set(false);
        flow.run(Mode::Screen, &config(&dir));
        assert_eq!(fake.grabs.borrow().len(), 2, "free again");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_lost_picture_does_not_block_captures_for_ever() {
        let (fake, flow, dir) = setup();
        flow.grab_limit.set(Duration::ZERO);
        fake.hold.set(true);
        flow.run(Mode::Screen, &config(&dir));
        flow.run(Mode::Screen, &config(&dir));
        assert_eq!(fake.grabs.borrow().len(), 2);
        // The first answer arrives late: it belongs to a capture that was given up, so it is dropped.
        let late = fake.held_grab.borrow_mut().remove(0);
        late(Ok(shot_of(&fake.grabs.borrow()[0])));
        assert!(fake.copied_png.borrow().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_overlay_replaces_the_one_that_is_still_up() {
        let (fake, flow, dir) = setup();
        fake.hold.set(true);
        flow.run(Mode::Region, &config(&dir));
        flow.run(Mode::Region, &config(&dir));
        assert_eq!(fake.held_pick.borrow().len(), 2);
        let d = fake.displays()[0];
        let rect = Rect::new(0.0, 0.0, 30.0, 30.0);
        // The first overlay was replaced, so its answer, if it ever came, is ignored.
        let first = fake.held_pick.borrow_mut().remove(0);
        first(region(d, rect));
        assert!(fake.grabs.borrow().is_empty());
        let second = fake.held_pick.borrow_mut().remove(0);
        second(region(d, rect));
        assert_eq!(fake.grabs.borrow().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
