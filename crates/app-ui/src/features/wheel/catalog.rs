//! What each wheel offers for a set of files and what each slot does. Pure: nothing here draws or touches
//! AppKit; the only thing it asks the file system is whether a path is a folder and which names are free.
//!
//! Every conversion offered here was run on a real file on macOS before it was listed, with the tools that ship
//! with the system (`sips`, `textutil`, `afconvert`, `avconvert`, `ditto`, `tar`) and the platform calls in
//! `macos_sys::file_ops`; the tests below run them again. What did not work is not offered: MP3 and WebP can
//! only be read, ICO needs a square image of a power of two, and AVIF cannot keep the orientation when its
//! metadata is stripped.

use crate::{
    i18n::Language,
    shelf_actions::{self, Job, Native, Size},
};
use macos_sys::file_ops::PageImage;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// The most slots a wheel can draw.
pub const MAX_SLOTS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Formats,
    Tools,
    Actions,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Formats, Kind::Tools, Kind::Actions];
}

#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub id: &'static str,
    pub title: String,
    pub symbol: Option<&'static str>,
}

/// What a chosen slot does.
#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    /// Commands run on a background thread, one or more per file; the new files land next to the originals.
    Jobs(Vec<Job>),
    /// Work that must run on the main thread.
    Main(MainAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainAction {
    /// Newline-separated POSIX paths on the clipboard.
    CopyPath(String),
    /// The files on the clipboard, ready to paste in Finder.
    CopyFiles(Vec<PathBuf>),
    AirDrop(Vec<PathBuf>),
    /// The folder itself, or the folder holding the file.
    Terminal(PathBuf),
    Reveal(Vec<PathBuf>),
    /// Add to the notch shelf.
    Float(Vec<PathBuf>),
    /// Open the image in the capture editor.
    Annotate(PathBuf),
    /// The shelf's "copy text from the image".
    CopyText(PathBuf),
    /// The shelf's cut-out, macOS 14 and later.
    RemoveBackground(PathBuf),
}

const IMAGE: [&str; 11] = [
    "png", "jpg", "jpeg", "heic", "heif", "tiff", "tif", "gif", "bmp", "avif", "webp",
];
const VIDEO: [&str; 3] = ["mov", "mp4", "m4v"];
const AUDIO: [&str; 8] = ["wav", "aiff", "aif", "m4a", "caf", "flac", "mp3", "aac"];
const DOCUMENT: [&str; 9] = [
    "rtf",
    "rtfd",
    "doc",
    "docx",
    "odt",
    "html",
    "htm",
    "txt",
    "webarchive",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Image,
    Video,
    Audio,
    Pdf,
    Document,
    /// Anything else, and folders.
    Other,
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn class(path: &Path) -> Class {
    let ext = extension(path);
    // A rich text package is a folder to the file system and a document to the person.
    if path.is_dir() && ext != "rtfd" {
        return Class::Other;
    }
    let ext = ext.as_str();
    if IMAGE.contains(&ext) {
        Class::Image
    } else if VIDEO.contains(&ext) {
        Class::Video
    } else if AUDIO.contains(&ext) {
        Class::Audio
    } else if ext == "pdf" {
        Class::Pdf
    } else if DOCUMENT.contains(&ext) {
        Class::Document
    } else {
        Class::Other
    }
}

/// One name for the spellings of a format, so that a `.jpeg` is not offered a conversion to JPEG.
fn same_format(ext: &str) -> &str {
    match ext {
        "jpg" => "jpeg",
        "heif" => "heic",
        "tif" => "tiff",
        "aif" => "aiff",
        "htm" => "html",
        other => other,
    }
}

/// Every id that has a translated title, for the tests.
#[cfg(test)]
pub const NAMED_IDS: [&str; 18] = [
    "compress",
    "compress_1080p",
    "compress_720p",
    "compress_540p",
    "half",
    "quarter",
    "strip_metadata",
    "remove_background",
    "copy_text",
    "annotate",
    "pdf_merge",
    "pdf_split",
    "copy_path",
    "copy",
    "airdrop",
    "terminal",
    "reveal",
    "float",
];

const IMAGE_TARGETS: [&str; 8] = [
    "to_png", "to_jpeg", "to_heic", "to_tiff", "to_avif", "to_gif", "to_bmp", "to_pdf",
];
const VIDEO_TARGETS: [&str; 4] = ["to_mp4", "to_mov", "to_gif", "to_m4a"];
const AUDIO_TARGETS: [&str; 6] = ["to_m4a", "to_wav", "to_aiff", "to_flac", "to_caf", "to_aac"];
const DOCUMENT_TARGETS: [&str; 7] = [
    "to_docx", "to_doc", "to_rtf", "to_odt", "to_html", "to_txt", "to_pdf",
];
const ARCHIVES: [&str; 3] = ["zip", "tar", "tgz"];

/// The extension a `to_*` id writes.
fn target_extension(id: &str) -> Option<&'static str> {
    Some(match id {
        "to_png" => "png",
        "to_jpeg" => "jpg",
        "to_heic" => "heic",
        "to_tiff" => "tiff",
        "to_avif" => "avif",
        "to_gif" => "gif",
        "to_bmp" => "bmp",
        "to_pdf" => "pdf",
        "to_mp4" => "mp4",
        "to_mov" => "mov",
        "to_m4a" => "m4a",
        "to_wav" => "wav",
        "to_aiff" => "aiff",
        "to_flac" => "flac",
        "to_caf" => "caf",
        "to_aac" => "aac",
        "to_docx" => "docx",
        "to_doc" => "doc",
        "to_rtf" => "rtf",
        "to_odt" => "odt",
        "to_html" => "html",
        "to_txt" => "txt",
        _ => return None,
    })
}

/// The ids of the formats one file can become, never its own, then the archives.
fn format_ids(path: &Path) -> Vec<&'static str> {
    let own = extension(path);
    let own = same_format(&own);
    let list: &[&str] = match class(path) {
        Class::Image => &IMAGE_TARGETS,
        Class::Video => &VIDEO_TARGETS,
        Class::Audio => &AUDIO_TARGETS,
        Class::Pdf => &["to_png", "to_jpeg"],
        Class::Document => &DOCUMENT_TARGETS,
        Class::Other => &[],
    };
    // Audio only needs sound: a video without an audio track would fail, so it is not offered. A file that is
    // not there is still offered, since the job will say what is wrong with it.
    let silent =
        class(path) == Class::Video && path.exists() && !macos_sys::file_ops::has_audio_track(path);
    let mut ids: Vec<&'static str> = list
        .iter()
        .copied()
        .filter(|id| !(silent && *id == "to_m4a"))
        .filter(|id| target_extension(id).is_none_or(|ext| same_format(ext) != own))
        .collect();
    ids.extend(ARCHIVES);
    ids
}

/// The ids of the tools one file supports on its own. What needs a whole set of files is added by `ids`.
fn tool_ids(path: &Path) -> Vec<&'static str> {
    let ext = extension(path);
    let ext = ext.as_str();
    match class(path) {
        Class::Image => {
            let mut ids = Vec::new();
            // Only the formats with a quality setting get smaller by being written again.
            if matches!(ext, "jpg" | "jpeg" | "heic" | "heif" | "avif") {
                ids.push("compress");
            }
            // sips cannot write WebP, so a WebP cannot be made smaller in its own format.
            if ext != "webp" {
                ids.extend(["half", "quarter"]);
            }
            if matches!(
                ext,
                "jpg" | "jpeg" | "png" | "tiff" | "tif" | "heic" | "heif"
            ) {
                ids.push("strip_metadata");
            }
            if macos_sys::vision::background_removal_available() {
                ids.push("remove_background");
            }
            ids.extend(["copy_text", "annotate"]);
            ids
        }
        Class::Video => vec![
            "compress_1080p",
            "compress_720p",
            "compress_540p",
            "strip_metadata",
        ],
        Class::Audio => vec!["compress"],
        Class::Pdf => vec!["pdf_split"],
        Class::Document | Class::Other => Vec::new(),
    }
}

/// Tools that act on one image and make no file.
const SINGLE_ONLY: [&str; 4] = ["remove_background", "copy_text", "annotate", "terminal"];

/// The ids offered for these paths, in wheel order, before the limit of slots.
fn ids(kind: Kind, paths: &[PathBuf]) -> Vec<&'static str> {
    if paths.is_empty() {
        return Vec::new();
    }
    if kind == Kind::Actions {
        let mut ids = vec!["copy_path", "copy", "airdrop"];
        if paths.len() == 1 {
            ids.push("terminal");
        }
        ids.extend(["reveal", "float"]);
        return ids;
    }
    // A batch gets only what every one of its files supports.
    let per_file = |path: &PathBuf| match kind {
        Kind::Formats => format_ids(path),
        _ => tool_ids(path),
    };
    let mut sets = paths.iter().map(per_file);
    let first = sets.next().unwrap_or_default();
    let rest: Vec<_> = sets.collect();
    let mut ids: Vec<&'static str> = first
        .into_iter()
        .filter(|id| rest.iter().all(|set| set.contains(id)))
        .collect();
    if kind == Kind::Tools {
        if paths.len() > 1 {
            ids.retain(|id| !SINGLE_ONLY.contains(id));
        }
        if paths.len() > 1 && paths.iter().all(|p| class(p) == Class::Pdf) {
            ids.insert(0, "pdf_merge");
        }
    }
    ids
}

fn symbol(id: &str) -> Option<&'static str> {
    Some(match id {
        "to_pdf" => "doc.richtext",
        "to_png" | "to_jpeg" | "to_heic" | "to_tiff" | "to_avif" | "to_gif" | "to_bmp" => "photo",
        "to_mp4" | "to_mov" => "film",
        "to_m4a" | "to_wav" | "to_aiff" | "to_flac" | "to_caf" | "to_aac" => "waveform",
        "to_docx" | "to_doc" | "to_rtf" | "to_odt" | "to_html" | "to_txt" => "doc.text",
        "zip" | "tar" | "tgz" => "doc.zipper",
        "compress" | "compress_1080p" | "compress_720p" | "compress_540p" => {
            "arrow.down.right.and.arrow.up.left"
        }
        "half" | "quarter" => "arrow.up.left.and.arrow.down.right",
        "strip_metadata" => "eye.slash",
        "remove_background" => "person.crop.rectangle",
        "copy_text" => "text.viewfinder",
        "annotate" => "pencil.tip.crop.circle",
        "pdf_merge" => "rectangle.stack",
        "pdf_split" => "scissors",
        "copy_path" => "link",
        "copy" => "doc.on.doc",
        "airdrop" => "dot.radiowaves.left.and.right",
        "terminal" => "terminal",
        "reveal" => "folder",
        "float" => "tray.and.arrow.down",
        _ => return None,
    })
}

fn title(id: &str, language: Language) -> String {
    match language.wheel_title(id) {
        Some(title) => title.to_owned(),
        None => id.strip_prefix("to_").unwrap_or(id).to_uppercase(),
    }
}

/// Every slot a wheel can ever offer, as (group, id), in the order the settings page lists them. The group is
/// the kind of file the slot is for: image, video, audio, pdf, document, or any. An id can be in several groups
/// (PDF converts both an image and a document); it is one slot, so hiding it hides it everywhere. New slots are
/// shown until the user hides them, so this is the list the settings are validated against.
pub fn all_ids(kind: Kind) -> Vec<(&'static str, &'static str)> {
    let group = |name: &'static str, ids: &[&'static str]| -> Vec<(&'static str, &'static str)> {
        ids.iter().map(|id| (name, *id)).collect()
    };
    match kind {
        Kind::Formats => [
            group("image", &IMAGE_TARGETS),
            group("video", &VIDEO_TARGETS),
            group("audio", &AUDIO_TARGETS),
            group("pdf", &["to_png", "to_jpeg"]),
            group("document", &DOCUMENT_TARGETS),
            group("any", &ARCHIVES),
        ]
        .concat(),
        Kind::Tools => [
            group(
                "image",
                &[
                    "compress",
                    "half",
                    "quarter",
                    "strip_metadata",
                    "remove_background",
                    "copy_text",
                    "annotate",
                ],
            ),
            group(
                "video",
                &[
                    "compress_1080p",
                    "compress_720p",
                    "compress_540p",
                    "strip_metadata",
                ],
            ),
            group("audio", &["compress"]),
            group("pdf", &["pdf_merge", "pdf_split"]),
        ]
        .concat(),
        Kind::Actions => group(
            "any",
            &[
                "copy_path",
                "copy",
                "airdrop",
                "terminal",
                "reveal",
                "float",
            ],
        ),
    }
}

/// The slots for these paths, at most 12, in wheel order. Several files get only what every one of them
/// supports (a batch). Empty when nothing applies: the wheel does not open.
#[allow(dead_code)]
pub fn offers(kind: Kind, paths: &[PathBuf], language: Language) -> Vec<Offer> {
    offers_with(kind, paths, language, &|_, _| true)
}

/// Like `offers`, for the slots `allowed` lets through: the settings turn a wheel off or hide some of its slots.
pub fn offers_with(
    kind: Kind,
    paths: &[PathBuf],
    language: Language,
    allowed: &dyn Fn(Kind, &str) -> bool,
) -> Vec<Offer> {
    ids(kind, paths)
        .into_iter()
        .filter(|id| allowed(kind, id))
        .take(MAX_SLOTS)
        .map(|id| Offer {
            id,
            title: title(id, language),
            symbol: symbol(id),
        })
        .collect()
}

/// What a chosen slot does. `None` for an id that `offers` would not return for these paths: the page and the
/// drop are not trusted, and a name that was never offered is never turned into a command.
#[allow(dead_code)]
pub fn plan(id: &str, paths: &[PathBuf]) -> Option<Plan> {
    plan_with(id, paths, &|_, _| true)
}

/// Like `plan`, and `None` too for an id that `allowed` does not let through: a slot that is hidden or whose
/// wheel is off can never run, even if a drop names it.
pub fn plan_with(
    id: &str,
    paths: &[PathBuf],
    allowed: &dyn Fn(Kind, &str) -> bool,
) -> Option<Plan> {
    let (kind, id) = Kind::ALL.into_iter().find_map(|kind| {
        ids(kind, paths)
            .into_iter()
            .filter(|offered| allowed(kind, offered))
            .take(MAX_SLOTS)
            .find(|offered| *offered == id)
            .map(|offered| (kind, offered))
    })?;
    if kind == Kind::Actions {
        return Some(Plan::Main(main_action(id, paths)?));
    }
    if let Some(action) = single_image_action(id, paths) {
        return Some(Plan::Main(action));
    }
    if id == "pdf_merge" {
        let first = paths.first()?;
        let dir = first.parent()?;
        let stem = first.file_stem()?.to_string_lossy();
        let output = shelf_actions::unique_output(dir, &format!("{stem} merged"), "pdf");
        return Some(Plan::Jobs(vec![Job::native(
            Native::PdfMerge {
                sources: paths.to_vec(),
            },
            output,
        )]));
    }
    let mut taken = Vec::new();
    let jobs = paths
        .iter()
        .map(|path| job(id, path, &mut taken))
        .collect::<Option<Vec<_>>>()?;
    Some(Plan::Jobs(jobs))
}

fn single_image_action(id: &str, paths: &[PathBuf]) -> Option<MainAction> {
    let path = paths.first()?.clone();
    match id {
        "remove_background" => Some(MainAction::RemoveBackground(path)),
        "copy_text" => Some(MainAction::CopyText(path)),
        "annotate" => Some(MainAction::Annotate(path)),
        _ => None,
    }
}

fn main_action(id: &str, paths: &[PathBuf]) -> Option<MainAction> {
    let first = paths.first()?;
    Some(match id {
        "copy_path" => MainAction::CopyPath(
            paths
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        "copy" => MainAction::CopyFiles(paths.to_vec()),
        "airdrop" => MainAction::AirDrop(paths.to_vec()),
        "terminal" => MainAction::Terminal(if first.is_dir() {
            first.clone()
        } else {
            first.parent()?.to_owned()
        }),
        "reveal" => MainAction::Reveal(paths.to_vec()),
        "float" => MainAction::Float(paths.to_vec()),
        _ => return None,
    })
}

fn args<const N: usize>(list: [&str; N]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

/// The job for one file. The output goes next to the original under a name that is free now and is not one a
/// job of the same batch has chosen (`taken`).
fn job(id: &str, source: &Path, taken: &mut Vec<PathBuf>) -> Option<Job> {
    let dir = source.parent()?;
    let file_name = source.file_name()?.to_string_lossy().into_owned();
    let stem = source.file_stem()?.to_string_lossy().into_owned();
    let ext = extension(source);
    let mut output = |stem: &str, ext: &str| {
        let path = shelf_actions::unique_output_avoiding(dir, stem, ext, taken);
        taken.push(path.clone());
        path
    };
    let os = |p: &Path| p.as_os_str().to_owned();
    let class = class(source);
    let to = target_extension(id);

    let work = match (id, class) {
        ("zip", _) => {
            let out = output(&file_name, "zip");
            Job::command(
                "/usr/bin/ditto",
                vec![
                    "-c".into(),
                    "-k".into(),
                    "--keepParent".into(),
                    os(source),
                    os(&out),
                ],
                out,
            )
        }
        ("tar" | "tgz", _) => {
            let out = output(&file_name, id);
            let flags = if id == "tgz" { "-czf" } else { "-cf" };
            // The name goes after `--`, so that a file called "-x" is a file and not an option.
            let mut list = args([flags]);
            list.extend([
                os(&out),
                "-C".into(),
                os(dir),
                "--".into(),
                file_name.into(),
            ]);
            Job::command("/usr/bin/tar", list, out)
        }
        ("to_png" | "to_jpeg", Class::Pdf) => {
            let out = output(&format!("{stem} pages"), "");
            Job::native(
                Native::PdfPages {
                    source: source.to_owned(),
                    stem,
                    format: if id == "to_png" {
                        PageImage::Png
                    } else {
                        PageImage::Jpeg
                    },
                },
                out,
            )
        }
        (_, Class::Image) if IMAGE_TARGETS.contains(&id) => {
            let ext = to?;
            let out = output(&stem, ext);
            let format = if ext == "jpg" { "jpeg" } else { ext };
            Job::command(
                "/usr/bin/sips",
                vec![
                    "-s".into(),
                    "format".into(),
                    format.into(),
                    os(source),
                    "--out".into(),
                    os(&out),
                ],
                out,
            )
        }
        ("to_gif", Class::Video) => {
            let out = output(&stem, "gif");
            Job::native(
                Native::VideoToGif {
                    source: source.to_owned(),
                },
                out,
            )
        }
        ("to_m4a", Class::Video) => {
            let out = output(&stem, "m4a");
            avconvert("PresetAppleM4A", source, out)
        }
        ("to_mp4" | "to_mov", Class::Video) => {
            let out = output(&stem, to?);
            avconvert("PresetHighestQuality", source, out)
        }
        (_, Class::Audio) if AUDIO_TARGETS.contains(&id) => {
            let ext = to?;
            let out = output(&stem, ext);
            let format: &[&str] = match ext {
                "m4a" => &["-f", "m4af", "-d", "aac"],
                "wav" => &["-f", "WAVE", "-d", "LEI16"],
                "aiff" => &["-f", "AIFF", "-d", "BEI16"],
                "flac" => &["-f", "flac", "-d", "flac"],
                "caf" => &["-f", "caff", "-d", "LEI16"],
                _ => &["-f", "adts", "-d", "aac"],
            };
            let mut list: Vec<OsString> = format.iter().map(OsString::from).collect();
            list.extend([os(source), os(&out)]);
            Job::command("/usr/bin/afconvert", list, out)
        }
        ("to_pdf", Class::Document) => {
            let out = output(&stem, "pdf");
            Job::native(
                Native::DocumentToPdf {
                    source: source.to_owned(),
                },
                out,
            )
        }
        (_, Class::Document) if DOCUMENT_TARGETS.contains(&id) => {
            let ext = to?;
            let out = output(&stem, ext);
            Job::command(
                "/usr/bin/textutil",
                vec![
                    "-convert".into(),
                    ext.into(),
                    os(source),
                    "-output".into(),
                    os(&out),
                ],
                out,
            )
        }
        ("compress", Class::Image) => {
            let out = output(&format!("{stem} compressed"), &ext);
            Job::command(
                "/usr/bin/sips",
                vec![
                    "-s".into(),
                    "format".into(),
                    same_format(&ext).into(),
                    "-s".into(),
                    "formatOptions".into(),
                    "60".into(),
                    os(source),
                    "--out".into(),
                    os(&out),
                ],
                out,
            )
        }
        ("compress", Class::Audio) => {
            let out = output(&format!("{stem} compressed"), "m4a");
            let mut list = args(["-f", "m4af", "-d", "aac", "-b", "64000"]);
            list.extend([os(source), os(&out)]);
            Job::command("/usr/bin/afconvert", list, out)
        }
        ("half" | "quarter", Class::Image) => {
            let size = if id == "half" {
                Size::Half
            } else {
                Size::Quarter
            };
            let out = output(&format!("{stem} {}", size.word()), &ext);
            Job::native(
                Native::Resize {
                    source: source.to_owned(),
                    size,
                },
                out,
            )
        }
        ("strip_metadata", Class::Image) => {
            let out = output(&format!("{stem} no metadata"), &ext);
            Job::native(
                Native::StripImageMetadata {
                    source: source.to_owned(),
                },
                out,
            )
        }
        ("strip_metadata", Class::Video) => {
            // avconvert drops the privacy-sensitive metadata (location, device) by default and, with the
            // passthrough preset, leaves the pictures and sound as they are.
            let out = output(&format!("{stem} no metadata"), &ext);
            avconvert("PresetPassthrough", source, out)
        }
        ("compress_1080p" | "compress_720p" | "compress_540p", Class::Video) => {
            let (preset, label) = match id {
                "compress_1080p" => ("Preset1920x1080", "1080p"),
                "compress_720p" => ("Preset1280x720", "720p"),
                _ => ("Preset960x540", "540p"),
            };
            let out = output(&format!("{stem} {label}"), &ext);
            avconvert(preset, source, out)
        }
        ("pdf_split", Class::Pdf) => {
            let out = output(&format!("{stem} split"), "");
            Job::native(
                Native::PdfSplit {
                    source: source.to_owned(),
                    stem,
                },
                out,
            )
        }
        _ => return None,
    };
    Some(work)
}

fn avconvert(preset: &str, source: &Path, out: PathBuf) -> Job {
    Job::command(
        "/usr/bin/avconvert",
        vec![
            "--preset".into(),
            preset.into(),
            "--source".into(),
            source.as_os_str().to_owned(),
            "--output".into(),
            out.as_os_str().to_owned(),
        ],
        out,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::wheel::TempDir;
    use std::fs;

    fn temp_dir() -> TempDir {
        TempDir::new("figghost-wheel-catalog")
    }

    fn ids_of(kind: Kind, paths: &[&str]) -> Vec<&'static str> {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        offers(kind, &paths, Language::English)
            .into_iter()
            .map(|o| o.id)
            .collect()
    }

    #[test]
    fn an_image_is_offered_the_other_image_formats_and_archives_but_never_its_own() {
        let png = ids_of(Kind::Formats, &["/x/a.png"]);
        assert_eq!(
            png,
            [
                "to_jpeg", "to_heic", "to_tiff", "to_avif", "to_gif", "to_bmp", "to_pdf", "zip",
                "tar", "tgz"
            ]
        );
        // The spellings of one format count as one.
        assert!(!ids_of(Kind::Formats, &["/x/a.jpeg"]).contains(&"to_jpeg"));
        assert!(!ids_of(Kind::Formats, &["/x/a.JPG"]).contains(&"to_jpeg"));
        assert!(!ids_of(Kind::Formats, &["/x/a.heif"]).contains(&"to_heic"));
        assert!(!ids_of(Kind::Formats, &["/x/a.tif"]).contains(&"to_tiff"));
        // WebP is read only, so every image format is offered for it and none writes WebP.
        let webp = ids_of(Kind::Formats, &["/x/a.webp"]);
        assert!(webp.contains(&"to_png") && webp.contains(&"to_jpeg"));
        assert!(webp.iter().all(|id| *id != "to_webp"));
        // ICO is not offered at all.
        assert!(!png.contains(&"to_ico"));
    }

    #[test]
    fn video_audio_pdf_and_documents_each_get_their_own_formats() {
        assert_eq!(
            ids_of(Kind::Formats, &["/x/a.mov"]),
            ["to_mp4", "to_gif", "to_m4a", "zip", "tar", "tgz"]
        );
        assert_eq!(
            ids_of(Kind::Formats, &["/x/a.mp4"]),
            ["to_mov", "to_gif", "to_m4a", "zip", "tar", "tgz"]
        );
        assert_eq!(
            ids_of(Kind::Formats, &["/x/a.wav"]),
            [
                "to_m4a", "to_aiff", "to_flac", "to_caf", "to_aac", "zip", "tar", "tgz"
            ]
        );
        // MP3 can be read but not written.
        let mp3 = ids_of(Kind::Formats, &["/x/a.mp3"]);
        assert!(mp3.contains(&"to_wav") && mp3.contains(&"to_m4a"));
        assert!(!mp3.contains(&"to_mp3"));
        assert_eq!(
            ids_of(Kind::Formats, &["/x/a.pdf"]),
            ["to_png", "to_jpeg", "zip", "tar", "tgz"]
        );
        assert_eq!(
            ids_of(Kind::Formats, &["/x/a.docx"]),
            [
                "to_doc", "to_rtf", "to_odt", "to_html", "to_txt", "to_pdf", "zip", "tar", "tgz"
            ]
        );
        assert!(!ids_of(Kind::Formats, &["/x/a.htm"]).contains(&"to_html"));
        assert_eq!(
            ids_of(Kind::Formats, &["/x/a.unknown"]),
            ["zip", "tar", "tgz"]
        );
    }

    #[test]
    fn a_folder_gets_archives_and_actions_only() {
        let dir = temp_dir();
        let folder = dir.join("Photos.png");
        fs::create_dir(&folder).unwrap();
        let paths = vec![folder.clone()];
        let formats = offers(Kind::Formats, &paths, Language::English);
        assert_eq!(
            formats.iter().map(|o| o.id).collect::<Vec<_>>(),
            ["zip", "tar", "tgz"]
        );
        assert!(offers(Kind::Tools, &paths, Language::English).is_empty());
        let actions: Vec<_> = offers(Kind::Actions, &paths, Language::English)
            .iter()
            .map(|o| o.id)
            .collect();
        assert_eq!(
            actions,
            [
                "copy_path",
                "copy",
                "airdrop",
                "terminal",
                "reveal",
                "float"
            ]
        );
        assert_eq!(
            plan("terminal", &paths),
            Some(Plan::Main(MainAction::Terminal(folder)))
        );
        // A rich text package is a document, not a folder.
        let package = dir.join("Note.rtfd");
        fs::create_dir(&package).unwrap();
        assert!(ids_of(Kind::Formats, &[package.to_str().unwrap()]).contains(&"to_pdf"));
    }

    #[test]
    fn tools_follow_the_kind_of_file() {
        let image = ids_of(Kind::Tools, &["/x/a.jpg"]);
        assert!(image.starts_with(&["compress", "half", "quarter", "strip_metadata"]));
        assert!(image.ends_with(&["copy_text", "annotate"]));
        // A PNG has no quality setting to lower, a GIF and a BMP carry no metadata worth removing.
        let png = ids_of(Kind::Tools, &["/x/a.png"]);
        assert!(!png.contains(&"compress") && png.contains(&"strip_metadata"));
        let gif = ids_of(Kind::Tools, &["/x/a.gif"]);
        assert!(!gif.contains(&"compress") && !gif.contains(&"strip_metadata"));
        assert!(gif.contains(&"half"));
        // WebP cannot be written, so it cannot be resized in place.
        assert!(!ids_of(Kind::Tools, &["/x/a.webp"]).contains(&"half"));
        assert_eq!(
            ids_of(Kind::Tools, &["/x/a.mov"]),
            [
                "compress_1080p",
                "compress_720p",
                "compress_540p",
                "strip_metadata"
            ]
        );
        assert_eq!(ids_of(Kind::Tools, &["/x/a.flac"]), ["compress"]);
        assert_eq!(ids_of(Kind::Tools, &["/x/a.pdf"]), ["pdf_split"]);
        assert!(ids_of(Kind::Tools, &["/x/a.docx"]).is_empty());
        assert!(ids_of(Kind::Tools, &["/x/a.zip"]).is_empty());
    }

    #[test]
    fn a_batch_gets_only_what_every_file_supports() {
        // A JPEG and a PNG: what both can become, and the tools both have.
        let formats = ids_of(Kind::Formats, &["/x/a.jpg", "/x/b.png"]);
        assert_eq!(
            formats,
            [
                "to_heic", "to_tiff", "to_avif", "to_gif", "to_bmp", "to_pdf", "zip", "tar", "tgz"
            ]
        );
        let tools = ids_of(Kind::Tools, &["/x/a.jpg", "/x/b.png"]);
        assert_eq!(tools, ["half", "quarter", "strip_metadata"]);
        // An image and a song share only the archives.
        assert_eq!(
            ids_of(Kind::Formats, &["/x/a.png", "/x/b.wav"]),
            ["zip", "tar", "tgz"]
        );
        // Tools that work on one image alone disappear from a batch.
        let single = ids_of(Kind::Tools, &["/x/a.jpg"]);
        assert!(single.contains(&"annotate") && single.contains(&"copy_text"));
        assert!(!tools.contains(&"annotate") && !tools.contains(&"copy_text"));
        assert!(!tools.contains(&"remove_background"));
        // Merging needs two PDFs or more, and only PDFs.
        assert!(!ids_of(Kind::Tools, &["/x/a.pdf"]).contains(&"pdf_merge"));
        assert_eq!(
            ids_of(Kind::Tools, &["/x/a.pdf", "/x/b.pdf"]),
            ["pdf_merge", "pdf_split"]
        );
        assert!(!ids_of(Kind::Tools, &["/x/a.pdf", "/x/b.png"]).contains(&"pdf_merge"));
        // Terminal opens at one place.
        let two = ids_of(Kind::Actions, &["/x/a.jpg", "/x/b.png"]);
        assert_eq!(two, ["copy_path", "copy", "airdrop", "reveal", "float"]);
        assert!(offers(Kind::Formats, &[], Language::English).is_empty());
    }

    #[test]
    fn no_wheel_has_more_than_twelve_slots_and_every_slot_has_a_title_and_a_symbol() {
        let every = [
            "/x/a.png",
            "/x/a.jpg",
            "/x/a.heic",
            "/x/a.webp",
            "/x/a.mov",
            "/x/a.m4v",
            "/x/a.wav",
            "/x/a.mp3",
            "/x/a.pdf",
            "/x/a.rtf",
            "/x/a.txt",
            "/x/a.zip",
        ];
        for path in every {
            for kind in Kind::ALL {
                for language in Language::ALL {
                    let paths = vec![PathBuf::from(path)];
                    let offered = offers(kind, &paths, language);
                    assert!(offered.len() <= MAX_SLOTS, "{path} {kind:?}");
                    for offer in offered {
                        assert!(!offer.title.is_empty(), "{path} {}", offer.id);
                        assert!(offer.title.chars().count() <= 12, "{:?}", offer.title);
                        assert!(offer.symbol.is_some(), "{}", offer.id);
                    }
                }
            }
        }
    }

    #[test]
    fn the_symbols_exist_on_this_system() {
        let mut seen = std::collections::BTreeSet::new();
        for path in [
            "/x/a.png", "/x/a.mov", "/x/a.wav", "/x/a.pdf", "/x/a.rtf", "/x/a.zip",
        ] {
            for kind in Kind::ALL {
                for offer in offers(kind, &[PathBuf::from(path)], Language::English) {
                    seen.extend(offer.symbol);
                }
            }
        }
        seen.insert("person.crop.rectangle");
        for name in seen {
            assert!(
                macos_sys::widgets::symbol_png(name, 16.0, 2.0).is_some(),
                "no SF Symbol called {name}"
            );
        }
    }

    #[test]
    fn titles_are_translated_for_tools_and_written_as_is_for_formats() {
        let paths = vec![PathBuf::from("/x/a.jpg")];
        let titles = |language| -> Vec<String> {
            offers(Kind::Tools, &paths, language)
                .into_iter()
                .map(|o| o.title)
                .collect()
        };
        assert_ne!(titles(Language::English), titles(Language::Vietnamese));
        let formats: Vec<String> = offers(Kind::Formats, &paths, Language::Vietnamese)
            .into_iter()
            .map(|o| o.title)
            .collect();
        assert_eq!(formats[0], "PNG");
        assert_eq!(formats.last().unwrap(), "TGZ");
    }

    #[test]
    fn an_id_that_was_not_offered_gives_nothing() {
        let image = vec![PathBuf::from("/x/a.png")];
        for bad in [
            "to_png",    // its own format
            "to_ico",    // dropped
            "to_mp3",    // cannot be written
            "to_webp",   // cannot be written
            "pdf_merge", // needs two PDFs
            "compress",  // a PNG has no quality
            "rm -rf /",
            "",
            "../../x",
            "to_png; ls",
        ] {
            assert!(plan(bad, &image).is_none(), "{bad:?}");
        }
        // A batch is not offered what one file alone would be.
        let two = vec![PathBuf::from("/x/a.png"), PathBuf::from("/x/b.png")];
        assert!(plan("annotate", &two).is_none());
        assert!(plan("terminal", &two).is_none());
        assert!(plan("zip", &[]).is_none());
    }

    #[test]
    fn jobs_build_the_expected_command_lines_and_name_outputs_next_to_the_original() {
        let dir = temp_dir();
        let png = dir.join("holiday photo.png");
        fs::write(&png, b"x").unwrap();
        let Some(Plan::Jobs(jobs)) = plan("to_jpeg", std::slice::from_ref(&png)) else {
            panic!("no jobs");
        };
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].output, dir.join("holiday photo.jpg"));
        assert_eq!(jobs[0].program(), "/usr/bin/sips");
        assert_eq!(
            jobs[0].args(),
            [
                OsString::from("-s"),
                "format".into(),
                "jpeg".into(),
                png.clone().into_os_string(),
                "--out".into(),
                dir.join("holiday photo.jpg").into_os_string()
            ]
        );

        // The name is free: one that exists is skipped.
        fs::write(dir.join("holiday photo.jpg"), b"x").unwrap();
        let Some(Plan::Jobs(jobs)) = plan("to_jpeg", std::slice::from_ref(&png)) else {
            panic!("no jobs");
        };
        assert_eq!(jobs[0].output, dir.join("holiday photo 2.jpg"));

        // Two sources that would both become "same.png" get different names.
        let (a, b) = (dir.join("same.jpg"), dir.join("same.tiff"));
        fs::write(&a, b"x").unwrap();
        fs::write(&b, b"x").unwrap();
        let Some(Plan::Jobs(jobs)) = plan("to_png", &[a, b]) else {
            panic!("no jobs");
        };
        assert_eq!(jobs[0].output, dir.join("same.png"));
        assert_eq!(jobs[1].output, dir.join("same 2.png"));

        // Archives: the folder name goes after "--" and the archive lands next to it.
        let folder = dir.join("-odd folder");
        fs::create_dir(&folder).unwrap();
        let Some(Plan::Jobs(jobs)) = plan("tgz", std::slice::from_ref(&folder)) else {
            panic!("no jobs");
        };
        assert_eq!(jobs[0].output, dir.join("-odd folder.tgz"));
        assert_eq!(jobs[0].program(), "/usr/bin/tar");
        assert_eq!(
            jobs[0].args(),
            [
                OsString::from("-czf"),
                dir.join("-odd folder.tgz").into_os_string(),
                "-C".into(),
                dir.to_path_buf().into_os_string(),
                "--".into(),
                "-odd folder".into()
            ]
        );

        // Video and audio use the system converters.
        let video = dir.join("clip.mov");
        let Some(Plan::Jobs(jobs)) = plan("compress_720p", std::slice::from_ref(&video)) else {
            panic!("no jobs");
        };
        assert_eq!(jobs[0].program(), "/usr/bin/avconvert");
        assert_eq!(jobs[0].output, dir.join("clip 720p.mov"));
        assert!(jobs[0].args().contains(&OsString::from("Preset1280x720")));
        let song = dir.join("song.wav");
        let Some(Plan::Jobs(jobs)) = plan("compress", std::slice::from_ref(&song)) else {
            panic!("no jobs");
        };
        assert_eq!(jobs[0].program(), "/usr/bin/afconvert");
        assert_eq!(jobs[0].output, dir.join("song compressed.m4a"));
        assert!(jobs[0].args().contains(&OsString::from("64000")));

        // Merging names the file after the first PDF and keeps the order of the drop.
        let (p1, p2) = (dir.join("b.pdf"), dir.join("a.pdf"));
        let Some(Plan::Jobs(jobs)) = plan("pdf_merge", &[p1.clone(), p2.clone()]) else {
            panic!("no jobs");
        };
        assert_eq!(jobs[0].output, dir.join("b merged.pdf"));
        assert_eq!(
            jobs[0].native_work(),
            Some(&Native::PdfMerge {
                sources: vec![p1, p2]
            })
        );
    }

    #[test]
    fn actions_become_main_thread_work() {
        let paths = vec![PathBuf::from("/x/a b.png"), PathBuf::from("/x/c.txt")];
        assert_eq!(
            plan("copy_path", &paths),
            Some(Plan::Main(MainAction::CopyPath(
                "/x/a b.png\n/x/c.txt".into()
            )))
        );
        assert_eq!(
            plan("copy", &paths),
            Some(Plan::Main(MainAction::CopyFiles(paths.clone())))
        );
        assert_eq!(
            plan("airdrop", &paths),
            Some(Plan::Main(MainAction::AirDrop(paths.clone())))
        );
        assert_eq!(
            plan("reveal", &paths),
            Some(Plan::Main(MainAction::Reveal(paths.clone())))
        );
        assert_eq!(
            plan("float", &paths),
            Some(Plan::Main(MainAction::Float(paths.clone())))
        );
        let one = vec![PathBuf::from("/x/a.png")];
        assert_eq!(
            plan("terminal", &one),
            Some(Plan::Main(MainAction::Terminal("/x".into())))
        );
        assert_eq!(
            plan("annotate", &one),
            Some(Plan::Main(MainAction::Annotate(one[0].clone())))
        );
        assert_eq!(
            plan("copy_text", &one),
            Some(Plan::Main(MainAction::CopyText(one[0].clone())))
        );
    }

    #[test]
    fn every_slot_that_can_be_offered_is_in_the_list_of_all_slots_once_per_group() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in Kind::ALL {
            let all = all_ids(kind);
            let groups: Vec<&str> = all.iter().map(|(g, _)| *g).collect();
            assert!(
                groups
                    .iter()
                    .all(|g| ["image", "video", "audio", "pdf", "document", "any"].contains(g))
            );
            let mut pairs = all.clone();
            pairs.sort_unstable();
            pairs.dedup();
            assert_eq!(pairs.len(), all.len(), "{kind:?} repeats a slot in a group");
            // The slots a real file is offered are all in the list.
            for path in [
                "/x/a.png", "/x/a.jpg", "/x/a.mov", "/x/a.wav", "/x/a.pdf", "/x/a.rtf", "/x/a.zip",
            ] {
                let paths = [PathBuf::from(path), PathBuf::from(path)];
                for single in [&paths[..1], &paths[..]] {
                    for id in ids(kind, single) {
                        assert!(all.iter().any(|(_, i)| *i == id), "{id} is not in all_ids");
                    }
                }
            }
            seen.extend(all.iter().map(|(_, id)| *id));
        }
        // Every title the page needs exists in both languages (formats are written as they are).
        for id in seen {
            if id.starts_with("to_") || ARCHIVES.contains(&id) {
                continue;
            }
            for language in Language::ALL {
                assert!(language.wheel_title(id).is_some(), "{id} in {language:?}");
            }
        }
    }

    #[test]
    fn hidden_slots_are_neither_offered_nor_run() {
        let image = vec![PathBuf::from("/x/a.jpg")];
        let hide_half = |kind: Kind, id: &str| !(kind == Kind::Tools && id == "half");
        let offered: Vec<_> = offers_with(Kind::Tools, &image, Language::English, &hide_half)
            .iter()
            .map(|o| o.id)
            .collect();
        assert!(!offered.contains(&"half") && offered.contains(&"quarter"));
        assert!(plan_with("half", &image, &hide_half).is_none());
        assert!(plan_with("quarter", &image, &hide_half).is_some());
        // A wheel that is off offers nothing and runs nothing.
        let formats_off = |kind: Kind, _: &str| kind != Kind::Formats;
        assert!(offers_with(Kind::Formats, &image, Language::English, &formats_off).is_empty());
        assert!(plan_with("to_png", &image, &formats_off).is_none());
        assert!(plan_with("zip", &image, &formats_off).is_none());
        assert!(plan_with("copy", &image, &formats_off).is_some());
    }

    /// B-2 of the QA report: M4A is audio only, so a video with no audio track is not offered it.
    #[test]
    fn audio_only_is_offered_for_a_video_only_when_it_has_sound() {
        let dir = temp_dir();
        let (silent, sound) = (dir.join("silent.mov"), dir.join("sound.mov"));
        fs::write(
            &silent,
            include_bytes!("../../../../macos-sys/testdata/silent.mov"),
        )
        .unwrap();
        fs::write(
            &sound,
            include_bytes!("../../../../macos-sys/testdata/sound.mov"),
        )
        .unwrap();
        let formats = |paths: &[&Path]| -> Vec<&'static str> {
            let paths: Vec<PathBuf> = paths.iter().map(|p| p.to_path_buf()).collect();
            offers(Kind::Formats, &paths, Language::English)
                .into_iter()
                .map(|o| o.id)
                .collect()
        };
        assert_eq!(
            formats(&[&sound]),
            ["to_mp4", "to_gif", "to_m4a", "zip", "tar", "tgz"]
        );
        assert_eq!(
            formats(&[&silent]),
            ["to_mp4", "to_gif", "zip", "tar", "tgz"]
        );
        // A batch gets what every file supports.
        assert!(!formats(&[&sound, &silent]).contains(&"to_m4a"));
        assert!(plan("to_m4a", std::slice::from_ref(&silent.to_path_buf())).is_none());
        assert!(plan("to_m4a", std::slice::from_ref(&sound.to_path_buf())).is_some());
    }
}
