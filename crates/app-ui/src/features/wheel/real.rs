//! Runs every slot the wheel offers on a real file, with the real tools of this Mac, and looks at what comes
//! out. This is what keeps the catalog honest: an id is only listed if it passed here when it was added.
//!
//! The fixtures are made on the spot: a BMP written by hand, sounds that ship with macOS, text written here.
//! A video is too big to write by hand; set `FIGGHOST_TEST_VIDEO` to a short movie with sound to run the video
//! slots too.

use super::{
    TempDir,
    catalog::{Kind, MainAction, Plan, offers, plan},
};
use crate::{i18n::Language, shelf_actions};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn temp_dir() -> TempDir {
    TempDir::new("figghost-wheel-real")
}

/// A 24 bit BMP of `w` by `h` pixels with a gradient.
fn bmp(w: u32, h: u32) -> Vec<u8> {
    let row = (w * 3).div_ceil(4) * 4;
    let mut file = Vec::new();
    file.extend(b"BM");
    file.extend((54 + row * h).to_le_bytes());
    file.extend(0u32.to_le_bytes());
    file.extend(54u32.to_le_bytes());
    file.extend(40u32.to_le_bytes());
    file.extend((w as i32).to_le_bytes());
    file.extend((h as i32).to_le_bytes());
    file.extend(1u16.to_le_bytes());
    file.extend(24u16.to_le_bytes());
    file.extend(0u32.to_le_bytes());
    file.extend((row * h).to_le_bytes());
    file.extend([0u8; 16]);
    for y in 0..h {
        for x in 0..w {
            file.extend([(x * 4) as u8, (y * 5) as u8, 0x80]);
        }
        file.extend(vec![0u8; (row - w * 3) as usize]);
    }
    file
}

/// What `file -b` says about a path, which is how the type of an output is checked.
fn file_type(path: &Path) -> String {
    let out = Command::new("/usr/bin/file")
        .arg("-b")
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Part of what `file -b` prints for the output of each id.
fn expected_type(id: &str) -> &'static str {
    match id {
        "to_png" => "PNG image",
        "to_jpeg" => "JPEG image",
        "to_heic" => "HEIF",
        "to_tiff" => "TIFF image",
        "to_avif" => "AVIF",
        "to_gif" => "GIF image",
        "to_bmp" => "PC bitmap",
        "to_pdf" | "pdf_merge" => "PDF document",
        "to_mp4" => "MP4",
        "to_mov" => "QuickTime",
        "to_m4a" => "M4A",
        "to_wav" => "WAVE audio",
        "to_aiff" => "AIFF",
        "to_flac" => "FLAC",
        "to_caf" => "CoreAudio Format",
        "to_aac" => "AAC",
        "to_docx" => "Microsoft Word 2007",
        "to_doc" => "Composite Document",
        "to_rtf" => "Rich Text Format",
        "to_odt" => "OpenDocument Text",
        "to_html" => "HTML document",
        "to_txt" => "text",
        "zip" => "Zip archive",
        "tar" => "POSIX tar archive",
        "tgz" => "gzip compressed",
        "compress" | "half" | "quarter" | "strip_metadata" => "",
        _ => "",
    }
}

/// Runs every `Jobs` slot of `kinds` on `source` and returns what each made, as (id, outputs). Fails the test
/// when a job fails, makes nothing, makes an empty file, or is of the wrong type.
fn exercise(source: &Path, kinds: &[Kind]) -> Vec<(&'static str, Vec<PathBuf>)> {
    let before = fs::read(source).ok();
    let paths = vec![source.to_owned()];
    let mut made = Vec::new();
    for kind in kinds {
        for offer in offers(*kind, &paths, Language::English) {
            let Some(Plan::Jobs(jobs)) = plan(offer.id, &paths) else {
                continue;
            };
            let mut outputs = Vec::new();
            for job in &jobs {
                shelf_actions::run(job)
                    .unwrap_or_else(|e| panic!("{} on {}: {e}", offer.id, source.display()));
                let output = &job.output;
                assert!(output.exists(), "{}: no output", offer.id);
                if output.is_dir() {
                    assert!(
                        fs::read_dir(output).unwrap().count() > 0,
                        "{}: empty folder",
                        offer.id
                    );
                } else {
                    assert!(
                        fs::metadata(output).unwrap().len() > 0,
                        "{}: empty",
                        offer.id
                    );
                    let kind = file_type(output);
                    assert!(
                        kind.contains(expected_type(offer.id)),
                        "{} on {}: {kind:?}",
                        offer.id,
                        source.display()
                    );
                }
                assert_eq!(output.parent(), source.parent(), "next to the original");
                outputs.push(output.clone());
            }
            made.push((offer.id, outputs));
        }
    }
    if let Some(before) = before {
        assert_eq!(
            fs::read(source).unwrap(),
            before,
            "the original is untouched"
        );
    }
    made
}

fn ids(made: &[(&'static str, Vec<PathBuf>)]) -> Vec<&'static str> {
    made.iter().map(|(id, _)| *id).collect()
}

/// An image of every format the wheel reads, made from one BMP with the wheel's own conversions.
fn image_fixtures(dir: &Path) -> Vec<PathBuf> {
    let base = dir.join("base.bmp");
    fs::write(&base, bmp(96, 64)).unwrap();
    let mut all = vec![base.clone()];
    for id in [
        "to_png", "to_jpeg", "to_heic", "to_tiff", "to_avif", "to_gif",
    ] {
        let Some(Plan::Jobs(jobs)) = plan(id, std::slice::from_ref(&base)) else {
            panic!("{id} is not offered for a BMP");
        };
        shelf_actions::run(&jobs[0]).unwrap_or_else(|e| panic!("{id}: {e}"));
        all.push(jobs[0].output.clone());
    }
    all
}

#[test]
fn every_image_slot_works_for_every_image_format() {
    let dir = temp_dir();
    for source in image_fixtures(&dir) {
        let made = exercise(&source, &[Kind::Formats, Kind::Tools]);
        let ids = ids(&made);
        assert!(ids.contains(&"zip") && ids.contains(&"half"), "{ids:?}");
    }
    // A WebP can be read: a one pixel lossless file, written byte by byte.
    let webp = dir.join("tiny.webp");
    fs::write(
        &webp,
        [
            0x52, 0x49, 0x46, 0x46, 0x1a, 0, 0, 0, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x4c,
            0x0d, 0, 0, 0, 0x2f, 0, 0, 0, 0x10, 0x07, 0x10, 0x11, 0x11, 0x88, 0x88, 0xfe, 0x07, 0,
        ],
    )
    .unwrap();
    let made = exercise(&webp, &[Kind::Formats, Kind::Tools]);
    assert!(ids(&made).contains(&"to_png"));
}

#[test]
fn the_resized_and_compressed_images_really_are_smaller() {
    let dir = temp_dir();
    let big = dir.join("big.bmp");
    fs::write(&big, bmp(200, 120)).unwrap();
    let Some(Plan::Jobs(jobs)) = plan("to_jpeg", std::slice::from_ref(&big)) else {
        panic!();
    };
    shelf_actions::run(&jobs[0]).unwrap();
    let jpeg = jobs[0].output.clone();
    let width = |p: &Path| {
        let out = Command::new("/usr/bin/sips")
            .args(["-g", "pixelWidth"])
            .arg(p)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find_map(|l| {
                l.trim()
                    .strip_prefix("pixelWidth:")
                    .map(|v| v.trim().to_owned())
            })
            .unwrap()
    };
    for (id, expected) in [("half", "100"), ("quarter", "50")] {
        let made = exercise(&jpeg, &[Kind::Tools]);
        let (_, outputs) = made.iter().find(|(i, _)| *i == id).unwrap();
        assert_eq!(width(&outputs[0]), expected, "{id}");
    }
    let made = exercise(&jpeg, &[Kind::Tools]);
    let (_, compressed) = made.iter().find(|(i, _)| *i == "compress").unwrap();
    assert!(
        fs::metadata(&compressed[0]).unwrap().len() < fs::metadata(&jpeg).unwrap().len(),
        "compressing makes the file smaller"
    );
}

#[test]
fn every_audio_slot_works_for_every_audio_format() {
    let glass = Path::new("/System/Library/Sounds/Glass.aiff");
    if !glass.exists() {
        return;
    }
    let dir = temp_dir();
    let base = dir.join("glass.aiff");
    fs::copy(glass, &base).unwrap();
    let mut sources = vec![base.clone()];
    for id in ["to_wav", "to_m4a", "to_caf", "to_flac", "to_aac"] {
        let Some(Plan::Jobs(jobs)) = plan(id, std::slice::from_ref(&base)) else {
            panic!("{id} is not offered for an AIFF");
        };
        shelf_actions::run(&jobs[0]).unwrap_or_else(|e| panic!("{id}: {e}"));
        sources.push(jobs[0].output.clone());
    }
    for source in &sources {
        let made = exercise(source, &[Kind::Formats, Kind::Tools]);
        assert!(ids(&made).contains(&"compress"), "{}", source.display());
        // MP3 is never an output.
        assert!(!ids(&made).contains(&"to_mp3"));
    }
    // An MP3 can be read, if this Mac has one to read.
    let mp3 = Path::new(
        "/System/Library/PrivateFrameworks/PersonalAudio.framework/Versions/A/Resources/Enrollment_1.mp3",
    );
    if mp3.exists() {
        let copy = dir.join("voice.mp3");
        fs::copy(mp3, &copy).unwrap();
        let made = exercise(&copy, &[Kind::Formats, Kind::Tools]);
        assert!(ids(&made).contains(&"to_wav") && ids(&made).contains(&"compress"));
    }
}

#[test]
fn every_document_slot_works_for_every_document_format() {
    let dir = temp_dir();
    let rtf = dir.join("note.rtf");
    fs::write(
        &rtf,
        r"{\rtf1\ansi Hello {\b bold} wheel\par second line\par}",
    )
    .unwrap();
    let txt = dir.join("plain.txt");
    fs::write(&txt, "Hello wheel\nsecond line\n").unwrap();
    let html = dir.join("page.html");
    fs::write(&html, "<html><body><h1>Hi</h1><p>para</p></body></html>").unwrap();
    let mut sources = vec![rtf.clone(), txt, html];
    for id in ["to_docx", "to_doc", "to_odt"] {
        let Some(Plan::Jobs(jobs)) = plan(id, std::slice::from_ref(&rtf)) else {
            panic!("{id}");
        };
        shelf_actions::run(&jobs[0]).unwrap_or_else(|e| panic!("{id}: {e}"));
        sources.push(jobs[0].output.clone());
    }
    for source in &sources {
        let made = exercise(source, &[Kind::Formats]);
        let (_, pdf) = made
            .iter()
            .find(|(id, _)| *id == "to_pdf")
            .unwrap_or_else(|| panic!("no PDF for {}", source.display()));
        assert!(macos_sys::file_ops::pdf_page_count(&pdf[0]).unwrap() >= 1);
    }
}

#[test]
fn every_pdf_slot_works() {
    let dir = temp_dir();
    let base = dir.join("base.bmp");
    fs::write(&base, bmp(96, 64)).unwrap();
    let make_pdf = |name: &str| {
        let copy = dir.join(format!("{name}.bmp"));
        fs::copy(&base, &copy).unwrap();
        let Some(Plan::Jobs(jobs)) = plan("to_pdf", &[copy]) else {
            panic!();
        };
        shelf_actions::run(&jobs[0]).unwrap();
        jobs[0].output.clone()
    };
    let (a, b) = (make_pdf("a"), make_pdf("b"));
    let made = exercise(&a, &[Kind::Formats, Kind::Tools]);
    assert_eq!(
        ids(&made),
        ["to_png", "to_jpeg", "zip", "tar", "tgz", "pdf_split"]
    );
    let (_, pages) = made.iter().find(|(id, _)| *id == "to_png").unwrap();
    let first = fs::read_dir(&pages[0])
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(&fs::read(&first).unwrap()[..4], &[0x89, b'P', b'N', b'G']);
    // Merge: two PDFs in the order of the drop.
    let both = vec![b.clone(), a.clone()];
    let Some(Plan::Jobs(jobs)) = plan("pdf_merge", &both) else {
        panic!();
    };
    shelf_actions::run(&jobs[0]).unwrap();
    assert_eq!(
        macos_sys::file_ops::pdf_page_count(&jobs[0].output).unwrap(),
        2
    );
    assert_eq!(jobs[0].output, dir.join("b merged.pdf"));
    // A batch of two PDFs converts and splits both.
    let Some(Plan::Jobs(jobs)) = plan("pdf_split", &both) else {
        panic!();
    };
    assert_eq!(jobs.len(), 2);
    for job in &jobs {
        shelf_actions::run(job).unwrap();
        assert_eq!(fs::read_dir(&job.output).unwrap().count(), 1);
    }
}

#[test]
fn archives_hold_files_and_folders_and_odd_names() {
    let dir = temp_dir();
    let folder = dir.join("-My Folder");
    fs::create_dir_all(folder.join("sub")).unwrap();
    fs::write(folder.join("sub").join("a.txt"), "inside").unwrap();
    let file = dir.join("notes (1).dat");
    fs::write(&file, "notes").unwrap();
    for source in [&folder, &file] {
        let made = exercise(source, &[Kind::Formats]);
        assert_eq!(ids(&made), ["zip", "tar", "tgz"]);
        for (id, outputs) in &made {
            let listing = match *id {
                "zip" => Command::new("/usr/bin/unzip")
                    .arg("-l")
                    .arg(&outputs[0])
                    .output(),
                "tar" => Command::new("/usr/bin/tar")
                    .arg("-tf")
                    .arg(&outputs[0])
                    .output(),
                _ => Command::new("/usr/bin/tar")
                    .arg("-tzf")
                    .arg(&outputs[0])
                    .output(),
            }
            .unwrap();
            let text = String::from_utf8_lossy(&listing.stdout).into_owned();
            let name = source.file_name().unwrap().to_string_lossy().into_owned();
            assert!(text.contains(&name), "{id}: {text}");
        }
    }
    let inside = Command::new("/usr/bin/tar")
        .arg("-tf")
        .arg(dir.join("-My Folder.tar"))
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&inside.stdout).contains("-My Folder/sub/a.txt"));
}

#[test]
fn metadata_is_really_removed_from_an_image_and_a_picture_stays() {
    let dir = temp_dir();
    let base = dir.join("base.bmp");
    fs::write(&base, bmp(64, 48)).unwrap();
    let Some(Plan::Jobs(jobs)) = plan("to_jpeg", std::slice::from_ref(&base)) else {
        panic!();
    };
    shelf_actions::run(&jobs[0]).unwrap();
    let jpeg = jobs[0].output.clone();
    // sips can write a tag, which gives a file with something to remove.
    let tagged = Command::new("/usr/bin/sips")
        .args(["-s", "make", "SecretMaker", "-s", "model", "SecretModel"])
        .arg(&jpeg)
        .output()
        .unwrap();
    assert!(tagged.status.success());
    let properties = |p: &Path| {
        let out = Command::new("/usr/bin/sips")
            .args(["-g", "all"])
            .arg(p)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    assert!(properties(&jpeg).contains("SecretMaker"));
    let Some(Plan::Jobs(jobs)) = plan("strip_metadata", std::slice::from_ref(&jpeg)) else {
        panic!();
    };
    shelf_actions::run(&jobs[0]).unwrap();
    let after = properties(&jobs[0].output);
    assert!(
        !after.contains("SecretMaker") && !after.contains("SecretModel"),
        "{after}"
    );
    assert!(after.contains("pixelWidth: 64") && after.contains("pixelHeight: 48"));
    assert_eq!(jobs[0].output, dir.join("base no metadata.jpg"));
}

#[test]
fn a_failed_job_leaves_nothing_behind_and_a_fake_file_does_no_harm() {
    let dir = temp_dir();
    let fakes: Vec<PathBuf> = [
        "fake.png",
        "fake.jpg",
        "fake.pdf",
        "fake.rtf",
        "fake.docx",
        "fake.wav",
    ]
    .iter()
    .map(|name| {
        let path = dir.join(name);
        fs::write(&path, "this is not what its name says").unwrap();
        path
    })
    .collect();
    for source in &fakes {
        let paths = vec![source.clone()];
        for kind in [Kind::Formats, Kind::Tools] {
            for offer in offers(kind, &paths, Language::English) {
                if matches!(offer.id, "zip" | "tar" | "tgz") {
                    continue;
                }
                let Some(Plan::Jobs(jobs)) = plan(offer.id, &paths) else {
                    continue;
                };
                for job in &jobs {
                    // Some tools accept a file of the wrong kind, and then the job succeeds; what must hold is
                    // that a failure leaves no file or folder.
                    if shelf_actions::run(job).is_err() {
                        assert!(!job.output.exists(), "{} on {}", offer.id, source.display());
                    } else {
                        let _ = fs::remove_file(&job.output);
                        let _ = fs::remove_dir_all(&job.output);
                    }
                }
            }
        }
    }
    let mut left: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "fake.docx",
            "fake.jpg",
            "fake.pdf",
            "fake.png",
            "fake.rtf",
            "fake.wav"
        ]
    );
}

#[test]
fn the_video_slots_work_on_a_real_movie() {
    let Some(video) = env::var_os("FIGGHOST_TEST_VIDEO") else {
        return;
    };
    let dir = temp_dir();
    for ext in ["mov", "mp4"] {
        let copy = dir.join(format!("clip.{ext}"));
        fs::copy(&video, &copy).unwrap();
        let made = exercise(&copy, &[Kind::Formats, Kind::Tools]);
        let ids = ids(&made);
        for id in ["to_m4a", "compress_540p", "strip_metadata", "zip"] {
            assert!(ids.contains(&id), "{ext}: {id} in {ids:?}");
        }
    }
}

#[test]
fn the_main_thread_slots_are_planned_for_a_picture_and_not_run() {
    let image = vec![PathBuf::from("/x/a.png")];
    let Some(Plan::Main(MainAction::Annotate(path))) = plan("annotate", &image) else {
        panic!();
    };
    assert_eq!(path, image[0]);
}
