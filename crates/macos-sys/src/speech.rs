//! Speech to text with the recognizer of macOS (the Speech framework), fed from the microphone by an audio engine.
//! The models are the system's own, one for each language: English (`en-US`) and Vietnamese (`vi-VN`) are both
//! recognized on the Mac itself, so with `on_device` the audio never leaves it. Needs the Microphone and the Speech
//! Recognition access, which the system asks for once; asking without the words for it in the app's Info.plist would
//! end the process, so a copy of the program that is not in its bundle is never asked.

use crate::app;
use block2::RcBlock;
use objc2::{AnyThread, rc::Retained};
use objc2_avf_audio::{AVAudioEngine, AVAudioPCMBuffer, AVAudioTime};
use objc2_foundation::{NSBundle, NSError, NSLocale, NSOperationQueue, NSString};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognitionTask,
    SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
};
use std::{
    cell::RefCell,
    ptr::NonNull,
    rc::Rc,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/// Frames the microphone hands over at a time: about fifty times a second at 48 kHz, often enough for the meter.
const TAP_FRAMES: u32 = 1024;

/// Whether the app may recognize speech; `None` while the person has not been asked.
pub fn speech_access() -> Option<bool> {
    // SAFETY: a class method with no arguments.
    let status = unsafe { SFSpeechRecognizer::authorizationStatus() };
    if status == SFSpeechRecognizerAuthorizationStatus::NotDetermined {
        None
    } else {
        Some(status == SFSpeechRecognizerAuthorizationStatus::Authorized)
    }
}

/// Whether the app's Info.plist says why it asks for `key`. The system ends a process that asks without it.
pub fn explains(key: &str) -> bool {
    NSBundle::mainBundle()
        .objectForInfoDictionaryKey(&NSString::from_str(key))
        .is_some()
}

/// Asks for the Speech Recognition access; `done` runs on the main thread with the answer. A program that is not
/// in its bundle cannot be asked and is answered no.
pub fn speech_request(done: impl FnOnce(bool) + Send + 'static) {
    if !explains("NSSpeechRecognitionUsageDescription") {
        app::on_main(move || done(false));
        return;
    }
    let done = Mutex::new(Some(done));
    let block = RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
        let done = done.lock().ok().and_then(|mut slot| slot.take());
        if let Some(done) = done {
            let allowed = status == SFSpeechRecognizerAuthorizationStatus::Authorized;
            app::on_main(move || done(allowed));
        }
    });
    // SAFETY: the block is copied by the callee and may run on any queue; it only hops to the main thread.
    unsafe { SFSpeechRecognizer::requestAuthorization(&block) };
}

fn recognizer(locale: &str) -> Option<Retained<SFSpeechRecognizer>> {
    let locale = NSLocale::initWithLocaleIdentifier(NSLocale::alloc(), &NSString::from_str(locale));
    // SAFETY: a designated initializer with a valid locale; it returns nil for a language it does not know.
    unsafe { SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale) }
}

/// What the Mac can do for a language now: whether it recognizes it at all, and whether on the Mac itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    pub available: bool,
    pub on_device: bool,
}

/// What the Mac can do for `locale` (`en-US`, `vi-VN`). Asking needs no access.
pub fn model(locale: &str) -> Model {
    match recognizer(locale) {
        // SAFETY: plain getters on a live recognizer.
        Some(r) => unsafe {
            Model {
                available: r.isAvailable(),
                on_device: r.supportsOnDeviceRecognition(),
            }
        },
        None => Model {
            available: false,
            on_device: false,
        },
    }
}

/// What the recognizer says while it listens.
#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    /// Everything understood so far, and whether it is the last of it.
    Words { text: String, last: bool },
    /// How loud the microphone is, from 0 to 1, about fifty times a second.
    Level(f32),
    /// It stopped before the last words: why, for the log and the panel.
    Failed(String),
}

type Sink = Rc<dyn Fn(Heard)>;

thread_local! {
    /// The levels come from the audio thread and hop to the main one, which finds the listening they belong to here.
    static LEVELS: RefCell<Option<(u64, Sink)>> = const { RefCell::new(None) };
}

/// Tells one listening from the one before it, whose last levels may still be on their way.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The request, for the audio thread. The audio engine hands it the microphone's buffers, as the framework intends.
struct Feed(*const SFSpeechAudioBufferRecognitionRequest);

// SAFETY: appending a buffer to a recognition request is meant to be done from the thread of the audio tap; the
// request outlives the tap, which is removed before the request is let go (see `Listening::drop`).
unsafe impl Send for Feed {}
unsafe impl Sync for Feed {}

/// How loud a stretch of samples is, from 0 (silence, -50 dB and under) to 1 (0 dB).
pub fn loudness(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let power = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    let decibels = 10.0 * power.max(1e-10).log10();
    ((decibels + 50.0) / 50.0).clamp(0.0, 1.0)
}

fn level_of(buffer: &AVAudioPCMBuffer) -> f32 {
    // SAFETY: plain getters on the buffer the tap was handed; the first channel holds `frames` samples.
    unsafe {
        let frames = buffer.frameLength() as usize;
        let channels = buffer.floatChannelData();
        if channels.is_null() || frames == 0 {
            return 0.0;
        }
        loudness(std::slice::from_raw_parts((*channels).as_ptr(), frames))
    }
}

/// The microphone, heard by the recognizer of one language, until `finish`, `cancel` or drop.
pub struct Listening {
    engine: Retained<AVAudioEngine>,
    request: Retained<SFSpeechAudioBufferRecognitionRequest>,
    task: Retained<SFSpeechRecognitionTask>,
    generation: u64,
    _recognizer: Retained<SFSpeechRecognizer>,
}

impl Listening {
    /// Starts hearing `locale`. `on_heard` runs on the main thread. With `on_device` the audio never leaves the Mac,
    /// and a language it cannot recognize itself is refused. Fails with the reason, in English, for the log.
    pub fn start(
        locale: &str,
        on_device: bool,
        on_heard: impl Fn(Heard) + 'static,
    ) -> Result<Self, String> {
        let recognizer = recognizer(locale).ok_or("no recognizer for this language")?;
        // SAFETY: plain getters and setters on live objects, on the main thread.
        unsafe {
            if !recognizer.isAvailable() {
                return Err("the recognizer is not available now".into());
            }
            if on_device && !recognizer.supportsOnDeviceRecognition() {
                return Err("this language is not recognized on this Mac".into());
            }
            // The handler keeps state of the main thread; the default queue is not promised to be the main one.
            recognizer.setQueue(&NSOperationQueue::mainQueue());
        }
        // SAFETY: as above.
        let request = unsafe { SFSpeechAudioBufferRecognitionRequest::new() };
        unsafe {
            request.setShouldReportPartialResults(true);
            request.setRequiresOnDeviceRecognition(on_device);
            request.setAddsPunctuation(true);
        }
        // SAFETY: as above; asking for the input node opens the default microphone.
        let engine = unsafe { AVAudioEngine::new() };
        let input = unsafe { engine.inputNode() };
        let format = unsafe { input.outputFormatForBus(0) };
        // SAFETY: plain getters on the format.
        if unsafe { format.channelCount() } == 0 || unsafe { format.sampleRate() } <= 0.0 {
            return Err("there is no microphone".into());
        }
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        let feed = Feed(Retained::as_ptr(&request));
        let tap = RcBlock::new(
            move |buffer: NonNull<AVAudioPCMBuffer>, _when: NonNull<AVAudioTime>| {
                // SAFETY: the engine hands a valid buffer for the length of the call; the request is alive (see `Feed`).
                let buffer = unsafe { buffer.as_ref() };
                unsafe { (*feed.0).appendAudioPCMBuffer(buffer) };
                let level = level_of(buffer);
                app::on_main(move || {
                    let sink = LEVELS.with(|slot| {
                        slot.borrow()
                            .as_ref()
                            .filter(|(g, _)| *g == generation)
                            .map(|(_, sink)| Rc::clone(sink))
                    });
                    if let Some(sink) = sink {
                        sink(Heard::Level(level));
                    }
                });
            },
        );
        // SAFETY: the format is the node's own; the block is copied by the callee.
        unsafe {
            input.installTapOnBus_bufferSize_format_block(
                0,
                TAP_FRAMES,
                Some(&format),
                RcBlock::as_ptr(&tap),
            );
            engine.prepare();
        }
        // SAFETY: as above.
        if let Err(error) = unsafe { engine.startAndReturnError() } {
            unsafe { input.removeTapOnBus(0) };
            return Err(error.localizedDescription().to_string());
        }
        let sink: Sink = Rc::new(on_heard);
        LEVELS.with(|slot| *slot.borrow_mut() = Some((generation, Rc::clone(&sink))));
        let handler = RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                // SAFETY: the framework passes a valid result or nil, and an error or nil, on the main queue (set above).
                if let Some(result) = unsafe { result.as_ref() } {
                    let text = unsafe { result.bestTranscription().formattedString() }.to_string();
                    let last = unsafe { result.isFinal() };
                    sink(Heard::Words { text, last });
                } else if let Some(error) = unsafe { error.as_ref() } {
                    sink(Heard::Failed(error.localizedDescription().to_string()));
                }
            },
        );
        // SAFETY: a live request and a block the callee copies.
        let task =
            unsafe { recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler) };
        Ok(Self {
            engine,
            request,
            task,
            generation,
            _recognizer: recognizer,
        })
    }

    fn stop_microphone(&self) {
        // SAFETY: plain calls on the live engine; removing a tap that is gone does nothing.
        unsafe {
            self.engine.inputNode().removeTapOnBus(0);
            self.engine.stop();
        }
    }

    /// Stops the microphone; the recognizer finishes what it heard and sends the last words.
    pub fn finish(&self) {
        self.stop_microphone();
        // SAFETY: a plain call on the live request.
        unsafe { self.request.endAudio() };
    }

    /// Stops everything; no more words come.
    pub fn cancel(&self) {
        self.stop_microphone();
        // SAFETY: a plain call on the live task.
        unsafe { self.task.cancel() };
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        self.cancel();
        LEVELS.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.as_ref().is_some_and(|(g, _)| *g == self.generation) {
                *slot = None;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loudness_runs_from_silence_to_full_scale() {
        assert_eq!(loudness(&[]), 0.0);
        assert_eq!(loudness(&[0.0; 512]), 0.0);
        assert!((loudness(&[1.0, -1.0, 1.0, -1.0]) - 1.0).abs() < 1e-6);
        // A tenth of full scale is -20 dB, three fifths of the way up.
        assert!((loudness(&[0.1, -0.1]) - 0.6).abs() < 1e-3);
        assert_eq!(loudness(&[0.001; 64]), 0.0, "-60 dB is under the floor");
    }

    #[test]
    fn english_and_vietnamese_are_known_to_this_mac() {
        // Asking what the Mac can do needs no access; the answer depends on the Mac, so only the shape is checked.
        for locale in ["en-US", "vi-VN"] {
            let model = model(locale);
            assert!(model.available || !model.on_device, "{locale}: {model:?}");
        }
        assert_eq!(
            model("xx-NOPE"),
            Model {
                available: false,
                on_device: false
            }
        );
    }
}
