//! Screen recording through ScreenCaptureKit (macOS 15 and later): an area, a window or a whole display,
//! straight into an MP4 file, with the system's audio and the microphone when asked for, and clicks
//! shown. This app's windows stay out of the recording, except the ones named in the options (the
//! camera bubble, the key display).

use crate::{
    reply::{self, Ticket},
    screen_capture::{Frozen, Shot},
};
use block2::RcBlock;
use objc2::{
    AnyThread, DefinedClass, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, ProtocolObject},
};
use objc2_av_foundation::{AVAssetImageGenerator, AVURLAsset};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect};
use objc2_core_media::CMTime;
use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol, NSRect, NSString, NSURL};
use objc2_screen_capture_kit::{
    SCContentFilter, SCRecordingOutput, SCRecordingOutputConfiguration, SCRecordingOutputDelegate,
    SCRunningApplication, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamDelegate,
    SCWindow,
};
use std::{
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::{Mutex, PoisonError},
};

/// MP4 with H.264, which every player and upload form accepts.
const FILE_TYPE: &str = "public.mpeg-4";
const CODEC: &str = "avc1";

/// Whether this system can record to a file directly (macOS 15 and later).
pub fn recording_supported() -> bool {
    AnyClass::get(c"SCRecordingOutput").is_some()
}

/// What to record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// An area of a display, in its points with the origin at its bottom-left.
    Area {
        display: usize,
        rect: NSRect,
    },
    Display(usize),
    Window(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordOptions {
    pub fps: u32,
    pub system_audio: bool,
    pub microphone: bool,
    /// Rings around clicks.
    pub clicks: bool,
    pub cursor: bool,
    /// Window numbers of this app's windows that should appear in the recording.
    pub include_windows: Vec<u32>,
}

type Finished = Ticket<Result<PathBuf, String>>;
type Started = Ticket<Result<Sendable<Recording>, String>>;

pub struct DelegateIvars {
    path: PathBuf,
    finished: Mutex<Option<Finished>>,
}

impl DelegateIvars {
    fn finish(&self, result: Result<PathBuf, String>) {
        if let Some(ticket) = self
            .finished
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            ticket.send(result);
        }
    }
}

fn error_text(error: &NSError) -> String {
    error.localizedDescription().to_string()
}

define_class!(
    // SAFETY: NSObject allows subclassing. ScreenCaptureKit calls the delegate on its own queues; the
    // ivars are a path and a mutex, both safe to use from any thread.
    #[unsafe(super(NSObject))]
    #[name = "FigRecorderDelegate"]
    #[ivars = DelegateIvars]
    struct RecorderDelegate;

    unsafe impl NSObjectProtocol for RecorderDelegate {}

    unsafe impl SCStreamDelegate for RecorderDelegate {
        #[unsafe(method(stream:didStopWithError:))]
        fn stream_did_stop(&self, _stream: &SCStream, error: &NSError) {
            self.ivars().finish(Err(error_text(error)));
        }
    }

    unsafe impl SCRecordingOutputDelegate for RecorderDelegate {
        #[unsafe(method(recordingOutput:didFailWithError:))]
        fn recording_failed(&self, _output: &SCRecordingOutput, error: &NSError) {
            self.ivars().finish(Err(error_text(error)));
        }

        #[unsafe(method(recordingOutputDidFinishRecording:))]
        fn recording_finished(&self, _output: &SCRecordingOutput) {
            let path = self.ivars().path.clone();
            self.ivars().finish(Ok(path));
        }
    }
);

/// ScreenCaptureKit objects created on one queue and used from the main thread; they are not tied to a
/// thread.
struct Sendable<T>(T);

// SAFETY: see above; only SCStream and SCRecordingOutput are wrapped, and only moved, never shared.
unsafe impl<T> Send for Sendable<T> {}

/// A recording in progress; stop it to finish the file.
pub struct Recording {
    stream: Retained<SCStream>,
    output: Retained<SCRecordingOutput>,
    /// The stream keeps its delegate weakly.
    delegate: Retained<RecorderDelegate>,
}

/// Even sizes: the H.264 encoder needs them.
fn even(points: f64, scale: f64) -> usize {
    (((points * scale).round() as usize) / 2 * 2).max(2)
}

impl Recording {
    /// Starts recording `target` of a frozen screen into `path`. `started` gets the recording once its
    /// frames flow, or why they do not; `finished` gets the file once `stop` has completed it, or an
    /// error. Both run on the main thread. Windows to include must be on screen by now.
    pub fn start(
        frozen: &Frozen,
        target: Target,
        path: &Path,
        options: &RecordOptions,
        started: impl FnOnce(Result<Recording, String>) + 'static,
        finished: impl FnOnce(Result<PathBuf, String>) + 'static,
    ) {
        let display_ids: Vec<u32> = frozen.displays.iter().map(|d| d.id).collect();
        let started_ticket = reply::expect(move |r: Result<Sendable<Recording>, String>| {
            started(r.map(|s| s.0));
        });
        let tickets = Mutex::new(Some((started_ticket, reply::expect(finished))));
        let (path, options) = (path.to_owned(), options.clone());
        let handler = RcBlock::new(
            move |content: *mut SCShareableContent, error: *mut NSError| {
                let Some((started, finished)) = tickets
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take()
                else {
                    return;
                };
                // SAFETY: a non-null content pointer is a live snapshot for the call; retaining keeps it.
                let content =
                    NonNull::new(content).and_then(|c| unsafe { Retained::retain(c.as_ptr()) });
                let Some(content) = content else {
                    // SAFETY: the handler passes a valid NSError or null.
                    let text = unsafe { error.as_ref() }
                        .map_or_else(|| "no shareable content".to_owned(), error_text);
                    started.send(Err(text));
                    return;
                };
                match build(&content, &display_ids, target, &path, &options, finished) {
                    Ok(recording) => recording.run(started),
                    Err(e) => started.send(Err(e)),
                }
            },
        );
        // SAFETY: the handler only touches Send data and is retained by ScreenCaptureKit.
        unsafe {
            SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
                false, true, &handler,
            );
        }
    }

    /// Starts the stream; `started` gets the recording once it runs, or the error that stopped it.
    fn run(self, started: Started) {
        let stream = self.stream.clone();
        let pending = Mutex::new(Some((started, Sendable(self))));
        let block = RcBlock::new(move |error: *mut NSError| {
            let Some((started, recording)) = pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
            else {
                return;
            };
            // SAFETY: the handler passes a valid NSError or null.
            match unsafe { error.as_ref() } {
                None => started.send(Ok(recording)),
                Some(error) => started.send(Err(error_text(error))),
            }
        });
        // SAFETY: a configured stream; the block only moves Send data.
        unsafe { stream.startCaptureWithCompletionHandler(Some(&block)) };
    }

    /// Ends the recording; the `finished` callback follows once the file is complete.
    pub fn stop(&self) {
        let delegate = self.delegate.clone();
        let block = RcBlock::new(move |error: *mut NSError| {
            // SAFETY: the handler passes a valid NSError or null.
            if let Some(error) = unsafe { error.as_ref() } {
                delegate.ivars().finish(Err(error_text(error)));
            }
        });
        // SAFETY: a live stream; the block only reaches the delegate's mutex.
        unsafe { self.stream.stopCaptureWithCompletionHandler(Some(&block)) };
    }

    /// Seconds recorded so far.
    pub fn duration(&self) -> f64 {
        // SAFETY: a plain property read.
        unsafe { self.output.recordedDuration().seconds() }
    }
}

/// The first frame of a video, for its thumbnail, at `scale` pixels per point.
pub fn poster(video: &Path, scale: f64) -> Option<Shot> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&video.to_string_lossy()));
    // SAFETY: plain AVFoundation reads of a local file with the documented argument types.
    unsafe {
        let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        generator.setAppliesPreferredTrackTransform(true);
        #[allow(deprecated)]
        let image = generator
            .copyCGImageAtTime_actualTime_error(CMTime::new(0, 600), std::ptr::null_mut())
            .ok()?;
        // The image is a CF object handed over as an Objective-C one; retaining it as CF keeps it.
        let image = CFRetained::retain(NonNull::from(&*image));
        Some(Shot::from_cg(image, scale.max(1.0)))
    }
}

/// Builds the stream and its file output. Runs on ScreenCaptureKit's queue, with a fresh content
/// snapshot that includes this app's windows made since the screen was frozen.
fn build(
    content: &SCShareableContent,
    display_ids: &[u32],
    target: Target,
    path: &Path,
    options: &RecordOptions,
    finished: Finished,
) -> Result<Recording, String> {
    let pid = std::process::id() as i32;
    // SAFETY: plain property reads on a live content snapshot; filters, configurations and outputs are
    // created and configured with the documented argument types.
    unsafe {
        let own: Vec<Retained<SCRunningApplication>> = content
            .applications()
            .iter()
            .filter(|a| a.processID() == pid)
            .collect();
        let shown: Vec<Retained<SCWindow>> = content
            .windows()
            .iter()
            .filter(|w| options.include_windows.contains(&w.windowID()))
            .collect();
        let display_for = |index: usize| {
            let id = display_ids.get(index).copied();
            content
                .displays()
                .iter()
                .find(|d| Some(d.displayID()) == id)
        };
        let config = SCStreamConfiguration::new();
        let filter = match target {
            Target::Window(id) => {
                let window = content
                    .windows()
                    .iter()
                    .find(|w| w.windowID() == id)
                    .ok_or("the window is gone")?;
                let filter = SCContentFilter::initWithDesktopIndependentWindow(
                    SCContentFilter::alloc(),
                    &window,
                );
                let scale = f64::from(filter.pointPixelScale()).max(1.0);
                let rect = filter.contentRect();
                config.setWidth(even(rect.size.width, scale));
                config.setHeight(even(rect.size.height, scale));
                filter
            }
            Target::Display(index) | Target::Area { display: index, .. } => {
                let display = display_for(index).ok_or("the display is gone")?;
                let filter =
                    SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
                        SCContentFilter::alloc(),
                        &display,
                        &NSArray::from_retained_slice(&own),
                        &NSArray::from_retained_slice(&shown),
                    );
                let scale = f64::from(filter.pointPixelScale()).max(1.0);
                match target {
                    Target::Area { rect, .. } => {
                        let height = display.height() as f64;
                        config.setSourceRect(CGRect::new(
                            CGPoint::new(rect.origin.x, height - rect.origin.y - rect.size.height),
                            rect.size,
                        ));
                        config.setWidth(even(rect.size.width, scale));
                        config.setHeight(even(rect.size.height, scale));
                    }
                    _ => {
                        config.setWidth(even(display.width() as f64, scale));
                        config.setHeight(even(display.height() as f64, scale));
                    }
                }
                filter
            }
        };
        config.setMinimumFrameInterval(CMTime::new(1, options.fps.clamp(1, 120) as i32));
        config.setShowsCursor(options.cursor);
        config.setShowMouseClicks(options.clicks);
        config.setCapturesAudio(options.system_audio);
        // The mixer plays the apps it routes through this process, so its audio has to stay in.
        config.setExcludesCurrentProcessAudio(false);
        config.setCaptureMicrophone(options.microphone);
        config.setQueueDepth(6);

        let delegate = RecorderDelegate::alloc().set_ivars(DelegateIvars {
            path: path.to_owned(),
            finished: Mutex::new(Some(finished)),
        });
        let delegate: Retained<RecorderDelegate> = msg_send![super(delegate), init];
        let stream = SCStream::initWithFilter_configuration_delegate(
            SCStream::alloc(),
            &filter,
            &config,
            Some(ProtocolObject::from_ref(&*delegate)),
        );
        let output_config = SCRecordingOutputConfiguration::new();
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        output_config.setOutputURL(&url);
        output_config.setOutputFileType(&NSString::from_str(FILE_TYPE));
        output_config.setVideoCodecType(&NSString::from_str(CODEC));
        let output = SCRecordingOutput::initWithConfiguration_delegate(
            SCRecordingOutput::alloc(),
            &output_config,
            ProtocolObject::from_ref(&*delegate),
        );
        stream
            .addRecordingOutput_error(&output)
            .map_err(|e| error_text(&e))?;
        Ok(Recording {
            stream,
            output,
            delegate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_even_for_the_encoder() {
        assert_eq!(even(101.0, 1.0), 100);
        assert_eq!(even(100.5, 2.0), 200);
        assert_eq!(even(0.2, 1.0), 2);
    }
}
