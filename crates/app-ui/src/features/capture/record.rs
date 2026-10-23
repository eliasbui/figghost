//! Screen recording: the overlay picks an area, a window or a display; a bar beside it shows the time
//! with Stop and Cancel; the finished MP4, or the GIF made from it, is saved, copied and shown in the
//! quick panel. Recordings of areas and displays can show the shortcuts pressed and the camera; a
//! window is recorded on its own, without them.

use super::{Feature, Kind, display_of};
use crate::{i18n, trace};
use macos_sys::{
    NSPoint, NSRect, app,
    capture_overlay::Picked,
    capture_ui::{self, AreaSession, SessionTexts},
    gif, permissions,
    record_ui::{self, CameraBubble, KeyDisplay},
    recorder::{self, RecordOptions, Recording, Target},
    screen_capture::Frozen,
};
use std::{
    cell::{Cell, RefCell},
    env, fs,
    path::PathBuf,
    rc::{Rc, Weak},
};

const FPS: u32 = 30;
const STATUS_SECONDS: f64 = 0.5;
/// A stopped recording that has not finished its file by then is given up on, so a new one can start.
const STOP_TIMEOUT_SECONDS: f64 = 15.0;

pub(super) struct Recorder {
    me: Weak<Recorder>,
    feature: Weak<Feature>,
    /// The MP4 being written: the capture itself, or a temporary file for a GIF.
    video: PathBuf,
    /// Where the GIF goes, for a GIF recording.
    gif: Option<PathBuf>,
    /// The display recorded on, for the quick panel.
    screen: NSRect,
    scale: f64,
    session: RefCell<Option<Rc<AreaSession>>>,
    recording: RefCell<Option<Recording>>,
    keys: RefCell<Option<Rc<KeyDisplay>>>,
    camera: RefCell<Option<CameraBubble>>,
    timer: RefCell<Option<app::Timer>>,
    /// Stop was asked for; a recording still starting stops as soon as it runs.
    stopping: Cell<bool>,
    cancelled: Cell<bool>,
    ended: Cell<bool>,
}

impl Recorder {
    /// Starts recording what was picked, and makes it the feature's recording.
    pub(super) fn start(feature: &Rc<Feature>, kind: Kind, frozen: &Rc<Frozen>, picked: Picked) {
        let (target, area, display) = match picked {
            Picked::Cancelled => return,
            Picked::Display(index) => {
                let Some(d) = frozen.displays.get(index) else {
                    return;
                };
                (Target::Display(index), d.frame, d)
            }
            Picked::Area { display, rect } => {
                let Some(d) = frozen.displays.get(display) else {
                    return;
                };
                let area = NSRect::new(
                    NSPoint::new(
                        d.frame.origin.x + rect.origin.x,
                        d.frame.origin.y + rect.origin.y,
                    ),
                    rect.size,
                );
                (Target::Area { display, rect }, area, d)
            }
            Picked::Window(id) => {
                let Some(frame) = frozen.windows.iter().find(|w| w.id == id).map(|w| w.frame)
                else {
                    return;
                };
                let Some(d) = display_of(frozen, frame) else {
                    return;
                };
                (Target::Window(id), frame, d)
            }
        };
        let text = i18n::current();
        let output = feature.new_path(if kind == Kind::Gif { "gif" } else { "mp4" });
        let video = match kind {
            Kind::Gif => {
                env::temp_dir().join(output.with_extension("mp4").file_name().unwrap_or_default())
            }
            _ => output.clone(),
        };
        if let Some(Err(e)) = output.parent().map(fs::create_dir_all) {
            capture_ui::toast(feature.mtm, &text.capture_write_failed(&e.to_string()));
            return;
        }
        let recorder = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            feature: Rc::downgrade(feature),
            video,
            gif: (kind == Kind::Gif).then_some(output),
            screen: display.frame,
            scale: display.shot.scale(),
            session: RefCell::new(None),
            recording: RefCell::new(None),
            keys: RefCell::new(None),
            camera: RefCell::new(None),
            timer: RefCell::new(None),
            stopping: Cell::new(false),
            cancelled: Cell::new(false),
            ended: Cell::new(false),
        });
        *feature.recording.borrow_mut() = Some(Rc::clone(&recorder));
        let overlays = !matches!(target, Target::Window(_));
        if overlays && feature.config.borrow().record.camera {
            let (me, frozen) = (recorder.me.clone(), Rc::clone(frozen));
            record_ui::camera_access(move |granted| {
                if let Some(recorder) = me.upgrade() {
                    recorder.begin(&frozen, target, area, Some(granted));
                }
            });
        } else {
            recorder.begin(frozen, target, area, None);
        }
    }

    /// Puts up the bar, the key display and the camera, then starts the stream; whatever should be in
    /// the recording has to be on screen before it starts. `camera` is whether its use was allowed,
    /// when it is wanted at all.
    fn begin(&self, frozen: &Frozen, target: Target, area: NSRect, camera: Option<bool>) {
        let Some(feature) = self.feature.upgrade() else {
            return;
        };
        if self.cancelled.get() {
            return self.end();
        }
        let (mtm, text) = (feature.mtm, i18n::current());
        let config = feature.config.borrow().record.clone();
        let mut include = Vec::new();
        match camera {
            Some(true) => match CameraBubble::show(mtm, area) {
                Some(bubble) => {
                    include.push(bubble.window_number());
                    *self.camera.borrow_mut() = Some(bubble);
                }
                None => capture_ui::toast(mtm, text.record_no_camera()),
            },
            Some(false) => capture_ui::toast(mtm, text.record_camera_denied()),
            None => {}
        }
        if config.keys && !matches!(target, Target::Window(_)) {
            match KeyDisplay::show(mtm, area) {
                Ok(keys) => {
                    include.push(keys.window_number());
                    *self.keys.borrow_mut() = Some(keys);
                }
                Err(e) => {
                    eprintln!("capture: no key display ({e:?})");
                    capture_ui::toast(mtm, text.record_keys_need_access());
                    permissions::accessibility_trusted(true);
                }
            }
        }
        let texts = SessionTexts {
            done: text.record_stop().into(),
            cancel: text.capture_scroll_cancel().into(),
        };
        let (on_stop, on_cancel) = (self.me.clone(), self.me.clone());
        let session = AreaSession::show(
            mtm,
            area,
            &texts,
            false,
            move || {
                if let Some(recorder) = on_stop.upgrade() {
                    recorder.stop();
                }
            },
            move || {
                if let Some(recorder) = on_cancel.upgrade() {
                    recorder.cancel();
                }
            },
        );
        session.set_status(text.record_starting());
        *self.session.borrow_mut() = Some(session);
        let options = RecordOptions {
            fps: FPS,
            system_audio: config.system_audio,
            microphone: config.microphone,
            clicks: config.clicks,
            cursor: true,
            include_windows: include,
        };
        let (on_started, on_finished) = (self.me.clone(), self.me.clone());
        Recording::start(
            frozen,
            target,
            &self.video,
            &options,
            move |result| {
                if let Some(recorder) = on_started.upgrade() {
                    recorder.started(result);
                }
            },
            move |result| {
                if let Some(recorder) = on_finished.upgrade() {
                    recorder.finished(result);
                }
            },
        );
    }

    fn started(&self, result: Result<Recording, String>) {
        let recording = match result {
            Ok(recording) => recording,
            Err(e) => return self.fail(&e),
        };
        trace::log(&format!("capture: recording {}", self.video.display()));
        *self.recording.borrow_mut() = Some(recording);
        if self.stopping.get() {
            return self.stop_now();
        }
        let me = self.me.clone();
        let timer = app::every(STATUS_SECONDS, move || {
            if let Some(recorder) = me.upgrade() {
                recorder.show_time();
            }
        });
        *self.timer.borrow_mut() = Some(timer);
        self.show_time();
    }

    fn show_time(&self) {
        let seconds = self
            .recording
            .borrow()
            .as_ref()
            .map_or(0.0, Recording::duration);
        if let Some(session) = self.session.borrow().as_ref() {
            session.set_status(&i18n::current().record_status(seconds.max(0.0) as u64));
        }
    }

    /// Finishes the file; a recording still starting is stopped as soon as it runs.
    pub(super) fn stop(&self) {
        if !self.stopping.replace(true) {
            self.stop_now();
        }
    }

    fn cancel(&self) {
        self.cancelled.set(true);
        self.stop();
    }

    fn stop_now(&self) {
        match self.recording.borrow().as_ref() {
            Some(recording) => recording.stop(),
            None => return,
        }
        trace::log("capture: stopping the recording");
        self.timer.borrow_mut().take();
        if let Some(session) = self.session.borrow().as_ref() {
            session.set_status(i18n::current().record_saving());
        }
        let me = self.me.clone();
        app::after(STOP_TIMEOUT_SECONDS, move || {
            if let Some(recorder) = me.upgrade()
                && !recorder.ended.get()
            {
                recorder.fail("the recording did not finish");
            }
        });
    }

    fn finished(&self, result: Result<PathBuf, String>) {
        if self.ended.get() {
            return;
        }
        let video = match result {
            Ok(video) => video,
            Err(e) => return self.fail(&e),
        };
        self.end();
        if self.cancelled.get() {
            let _ = fs::remove_file(&video);
            return;
        }
        let Some(feature) = self.feature.upgrade() else {
            return;
        };
        let poster = recorder::poster(&video, self.scale);
        let screen = self.screen;
        let Some(gif_path) = self.gif.clone() else {
            return feature.deliver_file(video, poster, screen);
        };
        capture_ui::toast(feature.mtm, i18n::current().record_making_gif());
        let (me, temporary) = (Rc::downgrade(&feature), video.clone());
        gif::video_to_gif(
            &video,
            &gif_path,
            gif::GIF_MAX_WIDTH,
            gif::GIF_FPS,
            move |result| {
                let _ = fs::remove_file(&temporary);
                let Some(feature) = me.upgrade() else {
                    return;
                };
                match result {
                    Ok(gif) => feature.deliver_file(gif, poster, screen),
                    Err(e) => capture_ui::toast(feature.mtm, &i18n::current().record_failed(&e)),
                }
            },
        );
    }

    /// Takes down the bar, the key display and the camera, and lets a new recording start.
    fn end(&self) {
        if self.ended.replace(true) {
            return;
        }
        self.timer.borrow_mut().take();
        // The recording holds the stream's delegate, so it stays until the file is finished.
        self.recording.borrow_mut().take();
        if let Some(session) = self.session.borrow_mut().take() {
            session.close();
        }
        if let Some(keys) = self.keys.borrow_mut().take() {
            keys.close();
        }
        if let Some(camera) = self.camera.borrow_mut().take() {
            camera.close();
        }
        if let Some(feature) = self.feature.upgrade() {
            feature.recording.borrow_mut().take();
        }
    }

    fn fail(&self, detail: &str) {
        eprintln!("capture: recording failed: {detail}");
        if self.ended.get() {
            return;
        }
        self.end();
        let empty = fs::metadata(&self.video).is_ok_and(|m| m.len() == 0);
        if self.gif.is_some() || empty {
            let _ = fs::remove_file(&self.video);
        }
        if let Some(feature) = self.feature.upgrade() {
            capture_ui::toast(feature.mtm, &i18n::current().record_failed(detail));
        }
    }
}
