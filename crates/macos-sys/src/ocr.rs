//! Text and code recognition with Vision: the text of a capture line by line, and the contents of any
//! QR codes and barcodes in it. Runs on a worker thread; the answer comes back on the main thread.

use crate::{reply, screen_capture::Shot};
use objc2::{AnyThread, rc::Retained, rc::autoreleasepool};
use objc2_foundation::{NSArray, NSDictionary, NSString};
use objc2_vision::{
    VNDetectBarcodesRequest, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
    VNRequestTextRecognitionLevel,
};

/// Languages tried in this order, when this system's Vision knows them.
const LANGUAGES: [&str; 2] = ["vi-VT", "en-US"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recognized {
    pub lines: Vec<String>,
    /// What QR codes and barcodes say.
    pub codes: Vec<String>,
}

impl Recognized {
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.codes.is_empty()
    }
}

/// Recognizes the text and codes in `shot`; `done` runs on the main thread.
pub fn recognize(shot: &Shot, done: impl FnOnce(Result<Recognized, String>) + 'static) {
    let ticket = reply::expect(done);
    let shot = shot.clone();
    std::thread::spawn(move || {
        let result = autoreleasepool(|_| recognize_now(&shot));
        ticket.send(result);
    });
}

fn recognize_now(shot: &Shot) -> Result<Recognized, String> {
    let text = VNRecognizeTextRequest::new();
    text.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    text.setUsesLanguageCorrection(true);
    text.setAutomaticallyDetectsLanguage(true);
    // SAFETY: a plain query on a new request.
    if let Ok(supported) = unsafe { text.supportedRecognitionLanguagesAndReturnError() } {
        let supported: Vec<String> = supported.iter().map(|s| s.to_string()).collect();
        let wanted: Vec<Retained<NSString>> = LANGUAGES
            .iter()
            .filter(|l| supported.iter().any(|s| s == *l))
            .map(|l| NSString::from_str(l))
            .collect();
        if !wanted.is_empty() {
            text.setRecognitionLanguages(&NSArray::from_retained_slice(&wanted));
        }
    }
    // SAFETY: a new request with default settings.
    let codes = unsafe { VNDetectBarcodesRequest::new() };
    let requests: Vec<Retained<VNRequest>> = vec![
        Retained::into_super(Retained::into_super(Retained::clone(&text))),
        Retained::into_super(Retained::into_super(Retained::clone(&codes))),
    ];
    // SAFETY: the image is a live CGImage; the handler only reads it.
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_options(
            VNImageRequestHandler::alloc(),
            shot.cg(),
            &NSDictionary::new(),
        )
    };
    handler
        .performRequests_error(&NSArray::from_retained_slice(&requests))
        .map_err(|e| e.localizedDescription().to_string())?;
    let lines = text
        .results()
        .map(|observations| {
            observations
                .iter()
                .filter_map(|o| {
                    o.topCandidates(1)
                        .firstObject()
                        .map(|c| c.string().to_string())
                })
                .filter(|line| !line.trim().is_empty())
                .collect()
        })
        .unwrap_or_default();
    // SAFETY: plain reads of the finished request's results.
    let codes = unsafe { codes.results() }
        .map(|observations| {
            observations
                .iter()
                .filter_map(|o| unsafe { o.payloadStringValue() }.map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    Ok(Recognized { lines, codes })
}
